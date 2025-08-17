use std::{collections::HashMap, time::Duration, sync::Arc};

use tracing::instrument;

use crate::{
    chat_history::{Message, MessageKind},
    llm::RequestMessage,
};

use super::*;

use ploke_rag::{RagService, TokenBudget, RetrievalStrategy, RrfConfig, AssembledContext};

// TODO: Get a real prompt.
// - Probably there is tons of research online, check it out.
static PROMPT_HEADER: &str = r#"
<-- SYSTEM PROMPT -->
You are a highly skilled software engineer, specializing in the Rust programming language.

You will be asked to provide some assistance in collaborating with the user.
"#;

static PROMPT_CODE: &str = r#"
Next, you will be provided with some of the user's code, that has been retrieved
to provide helpful context for you to answer their questions. This context will
be provided within code tags like these:

<code="path/to/file.rs" #132:486>Code goes here</code>

Where the "path/to/file.rs" is the absolute path to the file and the #132:486
are the line numbers, inclusive.

What follows is the provided code snippets for you to use as reference, and will
be shown in a header (like # Header) and with subheaders (like ## subheader).
Follow the code section will be the User's query, delineated by a header.

After the user query, there may be a response from another collaborator marked
with a header (like # Assistant or # Collaborator). These headers may alternate
and contain subheaders with the whole text of their messages so far, summaries
of the conversation, or other contextual information about the code base.

# Code

"#;
static PROMPT_USER: &str = r#"
# USER

"#;

pub struct ContextManager {
    pub rag_event_rx: mpsc::Receiver<RagEvent>,
    pub event_bus: Arc<EventBus>,
    pub code_map: HashMap<Uuid, CodeContext>,
    pub msg_map: HashMap<Uuid, Vec<Message>>,
    pub rag: Option<Arc<RagService>>,
}

#[derive(Clone, Debug)]
pub struct CodeContext {
    snippets: Vec<String>,
    id: Uuid,
}

impl From<(Uuid, Vec<String>)> for CodeContext {
    fn from(value: (Uuid, Vec<String>)) -> Self {
        Self {
            id: value.0,
            snippets: value.1,
        }
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

    #[instrument(skip_all)]
    pub async fn handle_rag_events(&mut self, rag_event: RagEvent) {
        tracing::info!("Starting handle_rag_events: {:?}", rag_event);
        use RagEvent::*;

        match rag_event {
            ContextSnippets(id, items) => {
                let items_len = items.len();
                let code_context = CodeContext::from((id, items));
                self.code_map.insert(id, code_context);

                tracing::info!(
                    "processing id {}
                    within ContextSnippets with items.len(): {}
                    self.code_map.contains_key() {} --- self.msg_map.contains_key() {}",
                    id,
                    items_len,
                    self.code_map.contains_key(&id),
                    self.msg_map.contains_key(&id),
                );

                // Check if we have everything needed to proceed
                self.try_construct_and_send_context(id).await;
            }
            UserMessages(id, msgs) => {
                let msgs_len = msgs.len();
                self.msg_map.insert(id, msgs);

                tracing::info!(
                    "processing id {}
                    within UserMessages with msgs.len(): {}
                    self.code_map.contains_key {} --- self.msg_map.contains_key {}",
                    id,
                    msgs_len,
                    self.code_map.contains_key(&id),
                    self.msg_map.contains_key(&id),
                );

                // Check if we have everything needed to proceed
                self.try_construct_and_send_context(id).await;
            }
            ConstructContext(id) => {
                tracing::info!(
                    "processing id {}
                    within ConstructContext
                    self.code_map.contains_key {} --- self.msg_map.contains_key {}",
                    id,
                    self.code_map.contains_key(&id),
                    self.msg_map.contains_key(&id),
                );

                self.try_construct_and_send_context(id).await;
            }
        }
    }

    async fn try_construct_and_send_context(&mut self, id: Uuid) {
        if self.rag.is_some() {
            // Stage 3 path: we only require user/assistant messages; RAG will retrieve code.
            if let Some((_, messages)) = self.msg_map.remove_entry(&id) {
                tracing::debug!(
                    "RAG-enabled: constructing context for parent_id: {} (messages only)",
                    id
                );
                self.send_prompt_to_llm_via_rag(messages, id).await;
            } else {
                tracing::debug!(
                    "RAG-enabled: waiting for messages for parent_id: {} (code_map ignored)",
                    id
                );
            }
            return;
        }

        // Legacy path: require both snippets and messages
        if self.code_map.contains_key(&id) && self.msg_map.contains_key(&id) {
            let context = self.code_map.remove_entry(&id);
            let messages = self.msg_map.remove_entry(&id);
            tracing::debug!(
                "trying to send context. after removing entries, currents status is
                code_map contains_key: {}, msg_map contains_key: {},
                parent_id: {}",
                self.code_map.contains_key(&id),
                self.msg_map.contains_key(&id),
                id
            );
            self.send_prompt_to_llm(context.unwrap().1, messages.unwrap().1, id)
                .await;
        } else {
            // Not all components ready yet, keep them stored
            tracing::debug!(
                "Waiting for more components - context: {}, messages: {}, parent_id: {}",
                self.code_map.contains_key(&id),
                self.msg_map.contains_key(&id),
                id
            );
        }
    }

    async fn send_prompt_to_llm_via_rag(
        &mut self,
        messages: Vec<Message>,
        parent_id: Uuid,
    ) {
        // Extract the most recent non-empty user message as the query
        let query = messages
            .iter()
            .rev()
            .find(|m| m.kind == MessageKind::User && !m.content.is_empty())
            .map(|m| m.content.clone())
            .unwrap_or_default();

        // Reasonable defaults aligned with RAG_INTEGRATION_PLAN.md
        let budget = TokenBudget {
            max_total: 1024,
            per_file_max: 512,
            per_part_max: 256,
            reserves: None,
        };

        let rag = match &self.rag {
            Some(r) => Arc::clone(r),
            None => {
                tracing::warn!("RagService not configured; cannot assemble context via RAG");
                return;
            }
        };

        match rag
            .get_context(
                &query,
                12,
                &budget,
                RetrievalStrategy::Hybrid {
                    rrf: RrfConfig::default(),
                    mmr: None,
                },
            )
            .await
        {
            Ok(ctx) => {
                let prompt = self.construct_context_from_rag(ctx, messages, parent_id);
                self.event_bus.send(AppEvent::Llm(prompt));
                tracing::info!("LLM context (RAG) sent successfully via event bus");
            }
            Err(e) => {
                tracing::error!("Failed to assemble context via RAG for parent_id {}: {:?}", parent_id, e);
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
        self.event_bus.send(AppEvent::Llm(prompt));
        tracing::info!("LLM context sent successfully via event bus");
    }

    fn construct_context_from_rag(
        &self,
        ctx: AssembledContext,
        messages: Vec<Message>,
        parent_id: Uuid,
    ) -> llm::Event {
        tracing::info!(
            "constructing context (RAG) with {} parts and {} messages",
            ctx.parts.len(),
            messages.len()
        );

        let mut base: Vec<(MessageKind, String)> = Vec::from([
            (MessageKind::System, String::from(PROMPT_HEADER)),
            (MessageKind::System, String::from(PROMPT_CODE)),
        ]);

        // Add assembled context parts as system messages
        let text = ctx.parts.into_iter().map(|p| ( MessageKind::System, p.text ));
        base.extend(text);

        // Add conversation messages
        let msgs = messages
            .into_iter()
            .filter(|m| m.kind == MessageKind::User || m.kind == MessageKind::Assistant)
            .inspect(|m| tracing::debug!("m.content.is_empty() = {}", m.content.is_empty()))
            .map(|msg| (msg.kind, msg.content));
        base.extend(msgs);

        llm::Event::PromptConstructed {
            parent_id,
            prompt: base,
        }
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
            .inspect(|m| tracing::debug!("m.content.is_empty() = {}", m.content.is_empty()))
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
    pub fn new(rag_event_rx: mpsc::Receiver<RagEvent>, event_bus: Arc<EventBus>) -> Self {
        Self {
            rag_event_rx,
            event_bus,
            code_map: Default::default(),
            msg_map: Default::default(),
            rag: None,
        }
    }

    pub fn new_with_rag(
        rag_event_rx: mpsc::Receiver<RagEvent>,
        event_bus: Arc<EventBus>,
        rag: Arc<RagService>,
    ) -> Self {
        Self {
            rag_event_rx,
            event_bus,
            code_map: Default::default(),
            msg_map: Default::default(),
            rag: Some(rag),
        }
    }
}
