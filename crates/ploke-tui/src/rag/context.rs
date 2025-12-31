use crate::{
    chat_history::{ContextStatus, TurnsToLive},
    llm::{
        ChatEvt, LlmEvent,
        manager::Role,
        manager::events::{
            ContextPlan, ContextPlanExcludedMessage, ContextPlanMessage, ContextPlanRagPart,
        },
    },
};
use ploke_rag::{TokenCounter as _, context::ApproxCharTokenizer};
use std::{ops::ControlFlow, path::PathBuf};

use once_cell::sync::Lazy;
use ploke_core::{
    ArcStr,
    rag_types::{AssembledContext, ContextPart},
};
use tokio::sync::oneshot;

use crate::{
    app_state::handlers::{chat, embedding::wait_on_oneshot},
    chat_history::{AnnotationKind, Message, MessageAnnotation, MessageKind},
    error::ErrorExt as _,
    llm::manager::RequestMessage,
    user_config::CtxMode,
};

use super::*;

pub static PROMPT_HEADER: &str = r#"
<-- BEGIN SYSTEM PROMPT -->
You are a highly skilled software engineer, specializing in the Rust programming language.

You will be asked to provide some assistance in collaborating with the user.
RAG snippets are intentionally brief; request deeper context with the request_code_context tool.
<-- END SYSTEM PROMPT -->
"#;

const DEFAULT_CONTEXT_PART_MAX_LINES: usize = 16;

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
    let (max_leased_tokens, ctx_mode, ctx_profile, retrieval_strategy) = {
        let cfg = state.config.read().await;
        (
            cfg.context_management.max_leased_tokens,
            cfg.context_management.mode,
            cfg.context_management.mode_profile().cloned(),
            cfg.rag.strategy.to_runtime(),
        )
    };
    let (msg_id, user_msg, messages, plan_messages, excluded_plan_messages) = {
        let mut guard = state.chat.write().await;
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
        let (messages, plan_messages, excluded_plan_messages) =
            guard.current_path_as_llm_request_messages_with_plan(Some(max_leased_tokens));
        (
            msg_id,
            user_msg,
            messages,
            plan_messages,
            excluded_plan_messages,
        )
    };

    if let Some(rag) = &state.rag {
        if let Some(profile) = ctx_profile {
            let mut budget = state.budget.clone();
            budget.per_part_max = profile.per_part_max_tokens;
            match rag
                .get_context(&user_msg, profile.top_k, &budget, &retrieval_strategy)
                .await
            {
                Ok(rag_ctx) => {
                    let context_plan = build_context_plan(
                        Uuid::new_v4(),
                        msg_id,
                        &plan_messages,
                        &excluded_plan_messages,
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
                    let rag_token_est: usize = context_plan
                        .included_rag_parts
                        .iter()
                        .map(|part| part.estimated_tokens)
                        .sum();
                    let rag_parts = context_plan.included_rag_parts.len();
                    let mode_label = ctx_mode_label(ctx_mode);
                    let meter = format!(
                        "CTX: {} | parts={} | est_tokens={} | tip: open context overlay for details",
                        mode_label, rag_parts, rag_token_est
                    );
                    let annotation = MessageAnnotation {
                        audience: crate::tools::Audience::User,
                        kind: AnnotationKind::Info,
                        text: meter,
                    };
                    chat::add_message_annotation(state, event_bus, msg_id, annotation).await;
                    let augmented_prompt =
                        construct_context_from_rag(rag_ctx, messages, msg_id, context_plan);
                    event_bus.send(AppEvent::Llm(augmented_prompt));
                    return;
                }
                Err(e) => {
                    e.emit_error();
                    tracing::error!(
                        "RAG get_context failed; falling back to conversation-only prompt"
                    );
                }
            }
        }
    } else if ctx_mode != CtxMode::Off {
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
    let fallback_note = if ctx_mode == CtxMode::Off {
        "Context mode is Off; proceeding without code context."
    } else {
        "No workspace context loaded; proceeding without code context. Index or load a workspace to enable RAG."
    };
    formatted.push(RequestMessage::new_system(fallback_note.to_string()));
    formatted.extend(messages.into_iter());
    let mut fallback_plan_messages = plan_messages;
    let tokenizer = ApproxCharTokenizer::default();
    fallback_plan_messages.push(ContextPlanMessage {
        message_id: None,
        kind: MessageKind::System,
        estimated_tokens: tokenizer.count(fallback_note),
    });
    let fallback_excluded_messages = excluded_plan_messages;
    let context_plan = build_context_plan(
        Uuid::new_v4(),
        msg_id,
        &fallback_plan_messages,
        &fallback_excluded_messages,
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
    let snippet = truncate_context_text(&ctx_part.text, DEFAULT_CONTEXT_PART_MAX_LINES);
    format!(
        "file_path: {}\ncanon_path: {}\nkind: {}\nscore: {:.3}\ncode_snippet:\n{}",
        ctx_part.file_path.as_ref(),
        ctx_part.canon_path.as_ref(),
        ctx_part.kind.to_static_str(),
        ctx_part.score,
        snippet
    )
}

fn truncate_context_text(text: &str, max_lines: usize) -> String {
    let mut out_lines = Vec::new();
    let mut truncated = false;
    for (idx, line) in text.lines().enumerate() {
        if idx >= max_lines {
            truncated = true;
            break;
        }
        out_lines.push(line);
    }
    let mut out = out_lines.join("\n");
    if truncated {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str("... [truncated]");
    }
    out
}

fn ctx_mode_label(mode: CtxMode) -> &'static str {
    match mode {
        CtxMode::Off => "Off",
        CtxMode::Light => "Light",
        CtxMode::Heavy => "Heavy",
    }
}

fn build_context_plan(
    plan_id: Uuid,
    parent_id: Uuid,
    plan_messages: &[ContextPlanMessage],
    excluded_messages: &[ContextPlanExcludedMessage],
    rag_ctx: Option<&AssembledContext>,
) -> ContextPlan {
    let tokenizer = ApproxCharTokenizer::default();
    let mut rag_parts = Vec::new();
    let mut rag_tokens = 0usize;
    if let Some(ctx) = rag_ctx {
        for part in &ctx.parts {
            let truncated = truncate_context_text(&part.text, DEFAULT_CONTEXT_PART_MAX_LINES);
            let estimated_tokens = tokenizer.count(&truncated);
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
        excluded_messages: excluded_messages.to_vec(),
        included_rag_parts: rag_parts,
        rag_stats: rag_ctx.map(|ctx| ctx.stats.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat_history::{
        ChatHistory, ContextStatus, MessageKind, MessageStatus, RetentionClass, TurnsToLive,
    };
    use crate::tools::{ToolName, ToolUiPayload};
    use ploke_core::rag_types::{CanonPath, ContextPartKind, ContextStats, Modality, NodeFilepath};
    use std::collections::HashMap;

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

        let plan_a = build_context_plan(plan_id, parent_id, &plan_messages, &[], Some(&ctx));
        let plan_b = build_context_plan(plan_id, parent_id, &plan_messages, &[], Some(&ctx));

        assert_eq!(plan_a.plan_id, plan_b.plan_id);
        assert_eq!(plan_a.parent_id, plan_b.parent_id);
        assert_eq!(plan_a.estimated_total_tokens, plan_b.estimated_total_tokens);
        assert_eq!(plan_a.included_messages.len(), 2);
        assert_eq!(plan_a.included_rag_parts.len(), 1);
        assert_eq!(plan_a.included_rag_parts[0].file_path, "src/lib.rs");
        assert_eq!(plan_a.rag_stats.as_ref().unwrap().parts, 1);
    }

    #[test]
    fn reformat_context_to_system_truncates_and_includes_meta() {
        let mut text = String::new();
        let total_lines = DEFAULT_CONTEXT_PART_MAX_LINES + 2;
        for idx in 0..total_lines {
            if idx > 0 {
                text.push('\n');
            }
            text.push_str(&format!("line {idx}"));
        }
        let part = ContextPart {
            id: Uuid::from_u128(40),
            file_path: NodeFilepath::new("src/main.rs".to_string()),
            canon_path: CanonPath::new("crate::main".to_string()),
            ranges: vec![],
            kind: ContextPartKind::Doc,
            text,
            score: 0.42,
            modality: Modality::Dense,
        };

        let rendered = reformat_context_to_system(part);

        assert!(rendered.contains("kind: Doc"));
        assert!(rendered.contains("score: 0.420"));
        assert!(rendered.contains("line 0"));
        assert!(rendered.contains(&format!(
            "line {}",
            DEFAULT_CONTEXT_PART_MAX_LINES - 1
        )));
        assert!(!rendered.contains(&format!(
            "line {}",
            DEFAULT_CONTEXT_PART_MAX_LINES
        )));
        assert!(rendered.contains("... [truncated]"));
    }

    fn label_message_id(
        message_id: Option<Uuid>,
        root_id: Uuid,
        labels: &HashMap<Uuid, &'static str>,
    ) -> String {
        match message_id {
            None => "tool_call".to_string(),
            Some(id) if id == root_id => "root_system".to_string(),
            Some(id) => labels
                .get(&id)
                .copied()
                .unwrap_or("unknown")
                .to_string(),
        }
    }

    fn label_part_id(part_id: Uuid, labels: &HashMap<Uuid, &'static str>) -> String {
        labels
            .get(&part_id)
            .copied()
            .unwrap_or("unknown")
            .to_string()
    }

    fn snapshot_context_plan(
        plan: &ContextPlan,
        root_id: Uuid,
        message_labels: &HashMap<Uuid, &'static str>,
        part_labels: &HashMap<Uuid, &'static str>,
    ) -> String {
        let mut out = String::new();
        out.push_str(&format!("plan_id: {}\n", plan.plan_id));
        out.push_str(&format!(
            "parent_id: {}\n",
            label_message_id(Some(plan.parent_id), root_id, message_labels)
        ));
        out.push_str(&format!(
            "estimated_total_tokens: {}\n",
            plan.estimated_total_tokens
        ));
        out.push_str("included_messages:\n");
        for msg in &plan.included_messages {
            out.push_str(&format!(
                "- id: {} kind: {:?} tokens: {}\n",
                label_message_id(msg.message_id, root_id, message_labels),
                msg.kind,
                msg.estimated_tokens
            ));
        }
        out.push_str("excluded_messages:\n");
        for msg in &plan.excluded_messages {
            out.push_str(&format!(
                "- id: {} kind: {:?} tokens: {} reason: {:?}\n",
                label_message_id(Some(msg.message_id), root_id, message_labels),
                msg.kind,
                msg.estimated_tokens,
                msg.reason
            ));
        }
        out.push_str("included_rag_parts:\n");
        for part in &plan.included_rag_parts {
            out.push_str(&format!(
                "- id: {} path: {} kind: {:?} tokens: {} score: {:.3}\n",
                label_part_id(part.part_id, part_labels),
                part.file_path,
                part.kind,
                part.estimated_tokens,
                part.score
            ));
        }
        out.push_str("rag_stats:\n");
        match &plan.rag_stats {
            Some(stats) => {
                out.push_str(&format!(
                    "- tokens: {} files: {} parts: {} truncated: {} dedup: {}\n",
                    stats.total_tokens,
                    stats.files,
                    stats.parts,
                    stats.truncated_parts,
                    stats.dedup_removed
                ));
            }
            None => {
                out.push_str("- none\n");
            }
        }
        out
    }

    #[test]
    fn context_plan_golden_snapshot_from_chat_history() {
        let mut ch = ChatHistory::new();
        let root_id = ch.current;

        let user_id = Uuid::from_u128(10);
        ch.add_message_user(root_id, user_id, "Check status.".to_string())
            .unwrap();

        let assistant_id = Uuid::from_u128(11);
        ch.add_child(
            user_id,
            assistant_id,
            "Working on it.",
            MessageStatus::Completed,
            MessageKind::Assistant,
            None,
            None,
        )
        .unwrap();

        if let Some(msg) = ch.messages.get_mut(&assistant_id) {
            msg.context_status = ContextStatus::Pinned {
                retention: RetentionClass::Leased,
                turns_to_live: TurnsToLive::NoneRemaining,
                reason: None,
                pinned_by: None,
            };
        }

        let tool_id = Uuid::from_u128(12);
        let tool_call_id = ArcStr::from("call-1");
        let payload = ToolUiPayload::new(ToolName::NsRead, tool_call_id.clone(), "read");
        ch.add_message_tool(
            assistant_id,
            tool_id,
            MessageKind::Tool,
            "tool output".to_string(),
            Some(tool_call_id),
            Some(payload),
        )
        .unwrap();

        if let Some(msg) = ch.messages.get_mut(&user_id) {
            msg.last_included_turn = Some(2);
        }
        if let Some(msg) = ch.messages.get_mut(&tool_id) {
            msg.last_included_turn = Some(1);
        }

        ch.current = tool_id;
        ch.rebuild_path_cache();

        let (_msgs, plan_messages, excluded_messages) =
            ch.current_path_as_llm_request_messages_with_plan(Some(4));

        let rag_ctx = AssembledContext {
            parts: vec![
                ContextPart {
                    id: Uuid::from_u128(100),
                    file_path: NodeFilepath::new("src/lib.rs".to_string()),
                    canon_path: CanonPath::new("crate::lib::a".to_string()),
                    ranges: vec![],
                    kind: ContextPartKind::Code,
                    text: "fn a() {}".to_string(),
                    score: 0.2,
                    modality: Modality::Dense,
                },
                ContextPart {
                    id: Uuid::from_u128(101),
                    file_path: NodeFilepath::new("src/main.rs".to_string()),
                    canon_path: CanonPath::new("crate::main::b".to_string()),
                    ranges: vec![],
                    kind: ContextPartKind::Doc,
                    text: "struct B;".to_string(),
                    score: 0.8,
                    modality: Modality::Dense,
                },
            ],
            stats: ContextStats {
                total_tokens: 12,
                files: 2,
                parts: 2,
                truncated_parts: 0,
                dedup_removed: 0,
            },
        };

        let plan = build_context_plan(
            Uuid::from_u128(1),
            user_id,
            &plan_messages,
            &excluded_messages,
            Some(&rag_ctx),
        );

        let message_labels = HashMap::from([
            (user_id, "user"),
            (assistant_id, "assistant"),
            (tool_id, "tool"),
        ]);
        let part_labels =
            HashMap::from([(Uuid::from_u128(100), "part_a"), (Uuid::from_u128(101), "part_b")]);

        let snapshot = snapshot_context_plan(&plan, root_id, &message_labels, &part_labels);
        let expected = "\
plan_id: 00000000-0000-0000-0000-000000000001
parent_id: user
estimated_total_tokens: 91
included_messages:
- id: root_system kind: System tokens: 81
- id: user kind: User tokens: 4
excluded_messages:
- id: assistant kind: Assistant tokens: 4 reason: TtlExpired
- id: tool kind: Tool tokens: 7 reason: Budget
included_rag_parts:
- id: part_a path: src/lib.rs kind: Code tokens: 3 score: 0.200
- id: part_b path: src/main.rs kind: Doc tokens: 3 score: 0.800
rag_stats:
- tokens: 12 files: 2 parts: 2 truncated: 0 dedup: 0
";

        assert_eq!(snapshot, expected);
    }
}
