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
    pub async fn handle_rag_events(&mut self, rag_event: RagEvent) {
        tracing::info!("Starting handle_rag_events: {:?}", rag_event);
        use RagEvent::*;
        match rag_event {
            ContextSnippets(items) => { 
                tracing::info!("within ContextSnippets: {:?}", items);
                self.code_context = Some(items.into()) ;
                tracing::info!("within ConstructContext");
                let prompt = self.construct_context(id);
                match self.llm_handle.send(prompt).await {
                    Ok(_) => tracing::info!("LLM conext sent"),
                    Err(e) => {
                        tracing::error!(
                            "Other end has already dropped? Have error: {}",
                            e.to_string()
                        );
                    }
                };
            },
            UserMessages(msgs) => { 
                tracing::info!("within UserMessages: {:?}", msgs);
                self.messages = Some(msgs) 
            },
            ConstructContext(id) => {
                tracing::info!("within ConstructContext");
                let prompt = self.construct_context(id);
                match self.llm_handle.send(prompt).await {
                    Ok(_) => tracing::info!("LLM conext sent"),
                    Err(e) => {
                        tracing::error!(
                            "Other end has already dropped? Have error: {}",
                            e.to_string()
                        );
                    }
                };
            } // _ => {}
        }
    }
    pub fn construct_context(&mut self, parent_id: Uuid) -> llm::Event {
        tracing::info!("within construct_context with parent_id {}", parent_id);
        let mut base: Vec<( MessageKind, String )> = Vec::from([
            (MessageKind::System, String::from(PROMPT_HEADER)),
            (MessageKind::System, String::from(PROMPT_CODE)),
        ]);

        if let Some(cc) = self.code_context.take() {
            base.extend(cc.snippets.into_iter().map(|c| (MessageKind::System, c)));
        }

        self.messages.take().map(|msgs| {
            let msgs = msgs
                .into_iter()
                .filter(|m| m.kind == MessageKind::User || m.kind == MessageKind::Assistant)
                .map(|msg| (msg.kind, msg.content.clone() ));
            base.extend(msgs);
        });

        llm::Event::PromptConstructed {
            parent_id,
            prompt: base,
        }
    }
}
