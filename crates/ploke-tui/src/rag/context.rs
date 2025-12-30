use crate::{
    chat_history::{ContextStatus, TurnsToLive},
    llm::{
        ChatEvt, LlmEvent,
        manager::{ApproxCharTokenizer, Role},
        manager::events::{ContextPlan, ContextPlanMessage, ContextPlanRagPart},
    },
};
use std::{ops::ControlFlow, path::PathBuf};

use once_cell::sync::Lazy;
use ploke_core::{
    ArcStr,
    rag_types::{AssembledContext, ContextPart},
};
use tokio::sync::oneshot;

use crate::{
    app_state::handlers::{chat, embedding::wait_on_oneshot},
    chat_history::{Message, MessageKind},
    error::ErrorExt as _,
    llm::manager::RequestMessage,
};

use super::*;

pub static PROMPT_HEADER: &str = r#"
<-- BEGIN SYSTEM PROMPT -->
You are a highly skilled software engineer, specializing in the Rust programming language.

You will be asked to provide some assistance in collaborating with the user.
<-- END SYSTEM PROMPT -->
"#;

/// Reads the just-submitted user message and:
/// - uses rag strategy to find similar items from the code graph
/// - adds conversation history from tail of last submitted user message
/// - forwards the complete and formatted messages to the system managing the API call
pub async fn process_with_rag(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    scan_rx: oneshot::Receiver<Option<Vec<PathBuf>>>,
    new_msg_id: Uuid,
    completion_rx: oneshot::Receiver<()>,
) {
    let add_msg = |msg: &str| {
        chat::add_msg_immediate_nofocus(
            state,
            event_bus,
            Uuid::new_v4(),
            msg.to_string(),
            MessageKind::SysInfo,
        )
    };
    if let ControlFlow::Break(_) = wait_on_oneshot(new_msg_id, completion_rx).await {
        let msg = "ScanForChange did not complete successfully";
        add_msg(msg).await;
        return;
    }

    // Snapshot chat state up front to avoid holding the lock across awaits.
    let (msg_id, user_msg, messages, plan_messages) = {
        let guard = state.chat.read().await;
        let (msg_id, user_msg) = match guard.last_user_msg().inspect_err(|e| e.emit_error()) {
            Ok(maybe_msg) => match maybe_msg {
                Some(msg) => msg,
                None => {
                    tracing::warn!("Attempting to submit empty user message");
                    return;
                }
            },
            Err(e) => {
                e.emit_error();
                return;
            }
        };
        let (messages, plan_messages) = guard.current_path_as_llm_request_messages_with_plan();
        (msg_id, user_msg, messages, plan_messages)
    };

    if let Some(rag) = &state.rag {
        let budget = &state.budget;
        let (top_k, retrieval_strategy) = {
            let cfg = state.config.read().await;
            (cfg.rag.top_k, cfg.rag.strategy.to_runtime())
        };
        match rag
            .get_context(&user_msg, top_k, budget, &retrieval_strategy)
            .await
        {
            Ok(rag_ctx) => {
                let context_plan = build_context_plan(
                    Uuid::new_v4(),
                    msg_id,
                    &plan_messages,
                    Some(&rag_ctx),
                );
                tracing::debug!(
                    plan_id = %context_plan.plan_id,
                    parent_id = %context_plan.parent_id,
                    included_messages = context_plan.included_messages.len(),
                    included_rag_parts = context_plan.included_rag_parts.len(),
                    estimated_tokens = context_plan.estimated_total_tokens,
                    "Context plan constructed"
                );
                let augmented_prompt =
                    construct_context_from_rag(rag_ctx, messages, msg_id, context_plan);
                event_bus.send(AppEvent::Llm(augmented_prompt));
                return;
            }
            Err(e) => {
                e.emit_error();
                tracing::error!("RAG get_context failed; falling back to conversation-only prompt");
            }
        }
    } else {
        add_msg("No RAG configured; using conversation-only prompt").await;
    }

    // Conversation-only fallback: prepend a short system notice then send PromptConstructed
    let (crate_loaded, first_tip): (bool, bool) = {
        let mut sys = state.system.write().await;
        let loaded = sys.focused_crate().is_some();
        let first = !sys.no_workspace_tip_shown;
        if !loaded {
            sys.no_workspace_tip_shown = true;
        }
        (loaded, first)
    };
    // If no crate is loaded, surface a user-facing tip in chat
    if !crate_loaded && first_tip {
        add_msg("No workspace is selected. Tip: use 'index start <path>' to index a project or 'load crate <name>' to load a saved database. Proceeding without code context.").await;
    }
    let mut formatted: Vec<RequestMessage> = Vec::with_capacity(messages.len() + 1);
    let fallback_note = "No workspace context loaded; proceeding without code context. Index or load a workspace to enable RAG.";
    formatted.push(RequestMessage::new_system(fallback_note.to_string()));
    formatted.extend(messages.into_iter());
    let mut fallback_plan_messages = plan_messages;
    let tokenizer = ApproxCharTokenizer::default();
    fallback_plan_messages.push(ContextPlanMessage {
        message_id: None,
        kind: MessageKind::System,
        estimated_tokens: tokenizer.count(fallback_note),
    });
    let context_plan = build_context_plan(
        Uuid::new_v4(),
        msg_id,
        &fallback_plan_messages,
        None,
    );
    tracing::debug!(
        plan_id = %context_plan.plan_id,
        parent_id = %context_plan.parent_id,
        included_messages = context_plan.included_messages.len(),
        included_rag_parts = context_plan.included_rag_parts.len(),
        estimated_tokens = context_plan.estimated_total_tokens,
        "Context plan constructed"
    );

    event_bus.send(AppEvent::Llm(LlmEvent::ChatCompletion(
        ChatEvt::PromptConstructed {
            parent_id: msg_id,
            formatted_prompt: formatted,
            context_plan,
        },
    )));
}

/// Reformats the different kinds (in this order) os messages from:
///
/// - System Prompt
///     - system prompt (consts) -> System
/// - Retrieved code context
///     - retrieved code context -> System
/// - Message History (newest to oldest)
///     - User -> User
///     - Assistant -> Assistant
///     - SysInfo -> filtered
///     - System -> filtered
///
/// Returns an event that is sent in the caller to the system managing the API call
fn construct_context_from_rag(
    ctx: AssembledContext,
    messages: Vec<RequestMessage>,
    parent_id: Uuid,
    context_plan: ContextPlan,
) -> LlmEvent {
    use RequestMessage as ReqMsg;

    tracing::info!(
        "constructing context (RAG) with {} parts and {} messages",
        ctx.parts.len(),
        messages.len()
    );

    // Add assembled context parts as system messages
    let mut text = ctx
        .parts
        .into_iter()
        .map(reformat_context_to_system)
        .map(ReqMsg::new_system)
        .collect::<Vec<RequestMessage>>();

    // Add conversation messages
    text.extend(messages);

    LlmEvent::ChatCompletion(ChatEvt::PromptConstructed {
        parent_id,
        formatted_prompt: text,
        context_plan,
    })
}

fn reformat_context_to_system(ctx_part: ContextPart) -> String {
    format!(
        "file_path: {}\ncanon_path: {}\ncode_snippet: {}",
        ctx_part.file_path.as_ref(),
        ctx_part.canon_path.as_ref(),
        ctx_part.text
    )
}

fn build_context_plan(
    plan_id: Uuid,
    parent_id: Uuid,
    plan_messages: &[ContextPlanMessage],
    rag_ctx: Option<&AssembledContext>,
) -> ContextPlan {
    let tokenizer = ApproxCharTokenizer::default();
    let mut rag_parts = Vec::new();
    let mut rag_tokens = 0usize;
    if let Some(ctx) = rag_ctx {
        for part in &ctx.parts {
            let estimated_tokens = tokenizer.count(&part.text);
            rag_tokens = rag_tokens.saturating_add(estimated_tokens);
            rag_parts.push(ContextPlanRagPart {
                part_id: part.id,
                file_path: part.file_path.as_ref().to_string(),
                kind: part.kind,
                estimated_tokens,
                score: part.score,
            });
        }
    }
    let message_tokens: usize = plan_messages
        .iter()
        .map(|m| m.estimated_tokens)
        .sum();

    ContextPlan {
        plan_id,
        parent_id,
        estimated_total_tokens: message_tokens.saturating_add(rag_tokens),
        included_messages: plan_messages.to_vec(),
        included_rag_parts: rag_parts,
        rag_stats: rag_ctx.map(|ctx| ctx.stats.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ploke_core::rag_types::{CanonPath, ContextPartKind, ContextStats, Modality, NodeFilepath};

    #[test]
    fn context_plan_is_stable_for_fixed_inputs() {
        let plan_id = Uuid::from_u128(1);
        let parent_id = Uuid::from_u128(2);
        let plan_messages = vec![
            ContextPlanMessage {
                message_id: Some(Uuid::from_u128(10)),
                kind: MessageKind::User,
                estimated_tokens: 3,
            },
            ContextPlanMessage {
                message_id: Some(Uuid::from_u128(11)),
                kind: MessageKind::Assistant,
                estimated_tokens: 5,
            },
        ];
        let ctx = AssembledContext {
            parts: vec![ContextPart {
                id: Uuid::from_u128(30),
                file_path: NodeFilepath::new("src/lib.rs".to_string()),
                canon_path: CanonPath::new("crate::lib::foo".to_string()),
                ranges: vec![],
                kind: ContextPartKind::Code,
                text: "fn foo() {}".to_string(),
                score: 0.5,
                modality: Modality::Dense,
            }],
            stats: ContextStats {
                total_tokens: 10,
                files: 1,
                parts: 1,
                truncated_parts: 0,
                dedup_removed: 0,
            },
        };

        let plan_a = build_context_plan(plan_id, parent_id, &plan_messages, Some(&ctx));
        let plan_b = build_context_plan(plan_id, parent_id, &plan_messages, Some(&ctx));

        assert_eq!(plan_a.plan_id, plan_b.plan_id);
        assert_eq!(plan_a.parent_id, plan_b.parent_id);
        assert_eq!(plan_a.estimated_total_tokens, plan_b.estimated_total_tokens);
        assert_eq!(plan_a.included_messages.len(), 2);
        assert_eq!(plan_a.included_rag_parts.len(), 1);
        assert_eq!(plan_a.included_rag_parts[0].file_path, "src/lib.rs");
        assert_eq!(plan_a.rag_stats.as_ref().unwrap().parts, 1);
    }
}
