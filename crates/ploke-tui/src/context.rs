use std::time::Duration;

use tracing::instrument;

use crate::{
    chat_history::{Message, MessageKind},
    llm::RequestMessage,
};

use super::*;

// TODO: Get a real prompt.
// - Probably there is tons of research online, check it out.
static PROMPT_HEADER: &str = r#"
<-- SYSTEM PROMPT -->
You are a highly skilled software engineer, specializing in the Rust programming language.

You will be asked to provide some assistance in collaborating with the user.

At all times it is **VERY IMPORTANT** that you **NEVER** lie or make things up. Instead, tell the user you are uncertain and request clarification or more information.

Here are some more instructions regarding communication:

1. NEVER lie or make things up.
2. Your tone should be polite yet professional, as though speaking with a colleague.
3. ABSTAIN from complementing the user.
4. Focus on the user's requests and follow their instructions.
5. DO NOT carry out requests the user has not asked you to perform. If you are unsure, ask the user for clarificaiton.
"#;
static PROMPT_CODE: &str = r#"

Next, you will be provided with some of the user's code, that has been retrieved to provide helpful context for you to answer their questions. This context will be provided within code tags like these:

<code="path/to/file.rs" #132:486>Code goes here</code>

Where the "path/to/file.rs" is the absolute path to the file and the #132:486 are the line numbers, inclusive.

What follows is the provided code snippets for you to use as reference, and will be shown in a header (like # Header) and with subheaders (like ## subheader). Follow the code section will be the User's query, delineated by a header.

After the user query, there may be a response from another collaborator marked with a header (like # Assistant or # Collaborator). These headers may alternate and contain subheaders with the whole text of their messages so far, summaries of the conversation, or other contextual information about the code base.

# Code

"#;
static PROMPT_USER: &str = r#"
# USER

"#;

pub struct ContextManager {
    pub rag_event_rx: mpsc::Receiver<RagEvent>,
    pub event_bus: Arc<EventBus>,
    pub code_context: Option<CodeContext>,
    pub messages: Option<Vec<Message>>,
    pub llm_handle: mpsc::Sender<llm::Event>,
    state: ContextState, // Add state tracking
    pending_parent_id: Option< Uuid >,
}

#[derive(Clone, Debug)]
pub struct CodeContext {
    snippets: Vec<String>,
}

impl From<Vec<String>> for CodeContext {
    fn from(value: Vec<String>) -> Self {
        Self { snippets: value }
    }
}


#[derive(Debug)]
enum ContextState {
    Idle,
    WaitingForSnippets,
    HasSnippets(CodeContext),
    HasMessages(Vec<Message>),
    Ready(CodeContext, Vec<Message>, Uuid),
}

impl ContextManager {
    #[instrument(skip_all, fields(code_context))]
    pub async fn run(mut self) {
        loop {
            tokio::select! {
                Some(rag_event) = self.rag_event_rx.recv() => self.handle_rag_events(rag_event).await,
                _ = tokio::time::sleep(Duration::from_millis(50)) => {}
            }
        }
    }

    #[instrument(skip_all, fields(self.context_state))]
    pub async fn handle_rag_events(&mut self, rag_event: RagEvent) {
        tracing::info!("Starting handle_rag_events: {:?}", rag_event);
        use RagEvent::*;

        match rag_event {
            ContextSnippets(items) => {
                self.code_context = Some(items.clone().into());

                tracing::info!(
                    "within ContextSnippets with items.len(): {}
                    code_context.is_some() {} --- messages.is_some() {}
                    self.pending_parent_id: {:?}", 
                    items.len(), self.code_context.is_some(), self.messages.is_some(), self.pending_parent_id
                );
                // Check if we have both snippets and messages to process
                if let (Some(context), Some(messages), Some(parent_id)) = (
                    self.code_context.take(),
                    self.messages.take(),
                    self.pending_parent_id,
                ) {
                    self.send_prompt_to_llm(context, messages, parent_id).await;
                } else {
                    // Store snippets and wait for messages
                    self.state = ContextState::HasSnippets(items.into());
                }
            }
            UserMessages(msgs) => {
                self.messages = Some(msgs.clone());


                tracing::info!(
                    "within UserMessages with msgs.len(): {}
                    code_context.is_some() {} --- messages.is_some() {}
                    self.pending_parent_id: {:?}", 
                    msgs.len(), self.code_context.is_some(), self.messages.is_some(), self.pending_parent_id
                );
                // Check if we have both snippets and messages
                if let (Some(context), Some(messages), Some(parent_id)) = (
                    self.code_context.take(),
                    self.messages.take(),
                    self.pending_parent_id,
                ) {
                    self.send_prompt_to_llm(context, messages, parent_id).await;
                } else {
                    self.state = ContextState::HasMessages(msgs);
                }
            }
            ConstructContext(id) => {
                tracing::info!(
                    "within ConstructContext with id: {}
                    code_context.is_some() {} --- messages.is_some() {}", 
                    id, self.code_context.is_some(), self.messages.is_some()
                );

                // Check if we have all required components
                match (self.code_context.take(), self.messages.take()) {
                    (Some(context), Some(messages)) => {
                        self.send_prompt_to_llm(context, messages, id).await;
                    }
                    (Some(context), None) => {
                        // Store snippets, wait for messages
                        self.pending_parent_id = Some(id);
                        self.state = ContextState::HasSnippets(context);
                    }
                    (None, Some(messages)) => {
                        // Store messages, wait for snippets
                        self.pending_parent_id = Some(id);
                        self.state = ContextState::HasMessages(messages);
                    }
                    (None, None) => {
                        // Wait for both
                        self.pending_parent_id = Some(id);
                        self.state = ContextState::WaitingForSnippets;
                    }
                }
            }
        }
    }

    async fn send_prompt_to_llm(
        &mut self,
        context: CodeContext,
        messages: Vec<Message>,
        parent_id: Uuid,
    ) {
        let prompt = self.construct_context(context, messages, parent_id);
        match self.llm_handle.send(prompt).await {
            Ok(_) => tracing::info!("LLM context sent successfully"),
            Err(e) => {
                tracing::error!("Failed to send context to LLM: {}", e.to_string());
            }
        };
        self.state = ContextState::Idle;
        self.pending_parent_id = None;
    }

    fn construct_context(
        &self,
        context: CodeContext,
        messages: Vec<Message>,
        parent_id: Uuid,
    ) -> llm::Event {
        tracing::info!(
            "constructing context with {} snippets and {} messages",
            context.snippets.len(),
            messages.len()
        );

        let mut base: Vec<(MessageKind, String)> = Vec::from([
            (MessageKind::System, String::from(PROMPT_HEADER)),
            (MessageKind::System, String::from(PROMPT_CODE)),
        ]);

        // Add code snippets
        base.extend(
            context
                .snippets
                .into_iter()
                .map(|c| (MessageKind::System, c)),
        );

        // Add conversation messages
        let msgs = messages
            .into_iter()
            .filter(|m| m.kind == MessageKind::User || m.kind == MessageKind::Assistant)
            .map(|msg| (msg.kind, msg.content));
        base.extend(msgs);

        llm::Event::PromptConstructed {
            parent_id,
            prompt: base,
        }
    }
}

// Update ContextManager::new() to include state initialization
impl ContextManager {
    pub fn new(
        rag_event_rx: mpsc::Receiver<RagEvent>,
        event_bus: Arc<EventBus>,
        llm_handle: mpsc::Sender<llm::Event>,
    ) -> Self {
        Self {
            rag_event_rx,
            event_bus,
            code_context: None,
            messages: None,
            llm_handle,
            state: ContextState::Idle,
            pending_parent_id: None,
        }
    }
}
