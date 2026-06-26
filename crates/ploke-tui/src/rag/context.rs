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
    ArcStr, RetrievalScope,
    rag_types::{
        AssembledContext, CallCalleeInfo, CallContextInfo, CallExpansionInfo, CallReceiverInfo,
        CallTargetInfo, ContextPart, ProofContextInfo,
    },
};
use tokio::sync::oneshot;

use crate::{
    app_state::handlers::{chat, embedding::wait_on_oneshot},
    chat_history::{AnnotationKind, Message, MessageAnnotation, MessageKind},
    context_plan::ContextPlanSnapshot,
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
                .get_context(
                    &user_msg,
                    profile.top_k,
                    &budget,
                    &retrieval_strategy,
                    RetrievalScope::LoadedWorkspace,
                )
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
                    event_bus.send(AppEvent::ContextPlanSnapshot(ContextPlanSnapshot::new(
                        context_plan.clone(),
                        Some(rag_ctx.clone()),
                    )));
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
    let outcome = state
        .with_system_txn(|txn| {
            let loaded = txn.has_loaded_crates();
            let workspace_root = txn.loaded_workspace_root();
            let first = !txn.no_workspace_tip_shown();
            if !loaded {
                txn.mark_no_workspace_tip_shown();
            }
            (loaded, workspace_root, first)
        })
        .await;
    let (crate_loaded, workspace_root, first_tip) = outcome.result;
    // If no crate is loaded, surface a user-facing tip in chat
    if !crate_loaded && first_tip {
        add_msg("No workspace is selected. Tip: use 'index start <path>' to index a project or 'load crate <name>' to load a saved database. Proceeding without code context.").await;
    }
    let mut formatted: Vec<RequestMessage> = Vec::with_capacity(messages.len() + 1);
    let context_off =
        "Context mode is Off: Context will not automatically be attached to the user message.";
    let context_on = match ctx_mode {
        CtxMode::Off => {
            "Context mode is Off: Context will not automatically be attached to the user message."
        }
        CtxMode::Light => {
            "Context mode set to Light, truncted context will be automatically added to user message."
        }
        CtxMode::Heavy => {
            "Context mode set to Heavy, verbose context will be automatically added to user message."
        }
    };
    let fallback_note = match (ctx_mode, workspace_root.as_ref(), crate_loaded) {
        (CtxMode::Off, Some(root), _) => {
            format!(
                "{context_off}; {workspace_note}.",
                workspace_note = format_args!("Context search via request_code_context is still available. workspace loaded {}",
                    root.display())
            )
        }
        (CtxMode::Off, None, _) => {
            format!("{context_off}; Context search via request_code_context is still available.")
        }
        (_, Some(root), _) => {
            format!(
                "Workspace loaded at {};",
                root.display()
            )
        }
        (_, None, false) => {
            "No workspace context loaded; proceeding without code context. Index or load a workspace to enable RAG.".to_string()
        }
        (_, None, true) => {
            "Workspace state is loaded, but no workspace root is available; proceeding without code context.".to_string()
        }
    };
    formatted.push(RequestMessage::new_system(fallback_note.clone()));
    formatted.extend(messages.into_iter());
    let mut fallback_plan_messages = plan_messages;
    let tokenizer = ApproxCharTokenizer::default();
    fallback_plan_messages.push(ContextPlanMessage {
        message_id: None,
        kind: MessageKind::System,
        estimated_tokens: tokenizer.count(&fallback_note),
    });
    let fallback_excluded_messages = excluded_plan_messages;
    let context_plan = build_context_plan(
        Uuid::new_v4(),
        msg_id,
        &fallback_plan_messages,
        &fallback_excluded_messages,
        None,
    );
    event_bus.send(AppEvent::ContextPlanSnapshot(ContextPlanSnapshot::new(
        context_plan.clone(),
        None,
    )));
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
    let type_context = ctx_part
        .type_context
        .map(|ctx| {
            format!(
                "\ntype_context: {} from {} at distance {}",
                ctx.relation.to_static_str(),
                ctx.seed_id,
                ctx.distance
            )
        })
        .unwrap_or_default();
    let call_expansion = ctx_part
        .call_expansion
        .map(|ctx| format!("\n{}", format_call_expansion(&ctx)))
        .unwrap_or_default();
    let call_context = if ctx_part.call_context.is_empty() {
        String::new()
    } else {
        format!(
            "\n{}",
            format_call_context_block(&ctx_part.call_context, "  ", 8)
        )
    };
    let proof_context = if ctx_part.proof_context.is_empty() {
        String::new()
    } else {
        format!(
            "\n{}",
            format_proof_context_block(&ctx_part.proof_context, "  ", 8)
        )
    };
    format!(
        "file_path: {}\ncanon_path: {}\nkind: {}\nscore: {:.3}{}{}{}{}\ncode_snippet:\n{}",
        ctx_part.file_path.as_ref(),
        ctx_part.canon_path.as_ref(),
        ctx_part.kind.to_static_str(),
        ctx_part.score,
        type_context,
        call_expansion,
        call_context,
        proof_context,
        snippet
    )
}

pub(crate) fn format_call_expansion(ctx: &CallExpansionInfo) -> String {
    format!(
        "call_expansion: {} from {} via {} to {} at distance {}",
        ctx.relation.to_static_str(),
        ctx.seed_id,
        ctx.call_site_id,
        ctx.target_id,
        ctx.distance
    )
}

pub(crate) fn format_call_context_block(
    calls: &[CallContextInfo],
    indent: &str,
    limit: usize,
) -> String {
    if calls.is_empty() {
        return String::new();
    }

    let mut out = format!("call_context: {} outgoing call site(s)", calls.len());
    let limit = limit.max(1);
    for call in calls.iter().take(limit) {
        out.push('\n');
        out.push_str(indent);
        out.push_str("- ");
        out.push_str(&format_call_context(call));
    }
    let hidden = calls.len().saturating_sub(limit);
    if hidden > 0 {
        out.push('\n');
        out.push_str(indent);
        out.push_str("- ... ");
        out.push_str(&hidden.to_string());
        out.push_str(" more call site(s)");
    }
    out
}

fn format_call_context(call: &CallContextInfo) -> String {
    format!(
        "{} @ {}..{}: {} => {}, {}",
        call.kind.to_static_str(),
        call.span.0,
        call.span.1,
        format_callee(&call.callee),
        format_status(call),
        format_targets(&call.targets)
    )
}

fn format_status(call: &CallContextInfo) -> String {
    match &call.resolution {
        Some(resolution) => format!(
            "{}({})",
            call.status.to_static_str(),
            resolution.to_static_str()
        ),
        None => call.status.to_static_str().to_string(),
    }
}

fn format_targets(targets: &[CallTargetInfo]) -> String {
    if targets.is_empty() {
        return "targets []".to_string();
    }

    let limit = 3usize;
    let mut parts = targets
        .iter()
        .take(limit)
        .map(|target| format!("{}:{}", target.relation.to_static_str(), target.target_id))
        .collect::<Vec<_>>();
    let hidden = targets.len().saturating_sub(limit);
    if hidden > 0 {
        parts.push(format!("... {hidden} more"));
    }
    format!("targets [{}]", parts.join(", "))
}

pub(crate) fn format_proof_context_block(
    rows: &[ProofContextInfo],
    indent: &str,
    limit: usize,
) -> String {
    if rows.is_empty() {
        return String::new();
    }

    let mut out = format!("proof_context: {} proof fact(s)", rows.len());
    let limit = limit.max(1);
    for row in rows.iter().take(limit) {
        out.push('\n');
        out.push_str(indent);
        out.push_str("- ");
        out.push_str(&format_proof_context(row));
    }
    let hidden = rows.len().saturating_sub(limit);
    if hidden > 0 {
        out.push('\n');
        out.push_str(indent);
        out.push_str("- ... ");
        out.push_str(&hidden.to_string());
        out.push_str(" more proof fact(s)");
    }
    out
}

fn format_proof_context(row: &ProofContextInfo) -> String {
    let mut parts = vec![row.kind.clone()];
    push_opt(&mut parts, "site", row.call_site_id.as_deref());
    push_opt(&mut parts, "edge", row.call_edge_id.as_deref());
    push_opt(&mut parts, "caller", row.caller_def_id.as_deref());
    push_opt(&mut parts, "callee", row.callee_def_id.as_deref());
    push_opt(&mut parts, "state", row.resolution_state.as_deref());
    push_opt(&mut parts, "resolved", row.resolved_def_id.as_deref());
    if !row.candidate_def_ids.is_empty() {
        parts.push(format_candidates(&row.candidate_def_ids, 4));
    }
    push_opt(
        &mut parts,
        "external_summary",
        row.external_summary_id.as_deref(),
    );
    push_opt(&mut parts, "boundary", row.boundary_id.as_deref());
    push_opt(&mut parts, "boundary_kind", row.boundary_kind.as_deref());
    push_opt(&mut parts, "expanded_item", row.expanded_item_id.as_deref());
    push_opt(&mut parts, "definition", row.definition_id.as_deref());
    push_opt(&mut parts, "target_kind", row.target_kind.as_deref());
    push_opt(&mut parts, "target_name", row.target_name.as_deref());
    push_opt(&mut parts, "target_root", row.target_root.as_deref());
    push_opt(&mut parts, "profile", row.profile.as_deref());
    push_opt(&mut parts, "rustc", row.rustc_version.as_deref());
    push_opt(
        &mut parts,
        "proof_policy",
        row.proof_policy_version.as_deref(),
    );
    push_opt(&mut parts, "cfg_domain", row.cfg_domain_id.as_deref());
    push_opt(&mut parts, "active_cfg", row.active_cfg_hash.as_deref());
    push_opt(&mut parts, "invocation", row.invocation_id.as_deref());
    push_opt(&mut parts, "rustc_program", row.rustc_program.as_deref());
    push_opt(&mut parts, "working_dir", row.working_directory.as_deref());
    push_opt(
        &mut parts,
        "argument_hash",
        row.argument_vector_hash.as_deref(),
    );
    push_opt(&mut parts, "env_hash", row.environment_hash.as_deref());
    push_opt(&mut parts, "effect_seed", row.effect_seed_id.as_deref());
    push_opt(&mut parts, "confidence", row.confidence.as_deref());
    if let Some(blocker_if_unresolved) = row.blocker_if_unresolved {
        parts.push(format!("blocker_if_unresolved={blocker_if_unresolved}"));
    }
    push_opt(&mut parts, "authority", row.authority_term.as_deref());
    push_opt(&mut parts, "summary", row.summary_class.as_deref());
    push_opt(&mut parts, "artifact", row.artifact_hash.as_deref());
    push_opt(&mut parts, "version", row.summary_version.as_deref());
    push_opt(&mut parts, "review", row.review_method.as_deref());
    push_opt(&mut parts, "scope", row.scope_of_validity.as_deref());
    if !row.allowed_effects.is_empty() {
        parts.push(format_list("allowed_effects", &row.allowed_effects, 4));
    }
    push_opt(
        &mut parts,
        "containment",
        row.required_containment.as_deref(),
    );
    push_opt(
        &mut parts,
        "invalidates",
        row.invalidation_conditions.as_deref(),
    );
    push_opt(&mut parts, "status", row.status.as_deref());
    push_opt(&mut parts, "blocker", row.blocker_reason.as_deref());
    push_opt(&mut parts, "effect", row.effect_class.as_deref());
    push_opt(&mut parts, "detail", row.detail.as_deref());
    push_opt(&mut parts, "evidence", row.evidence_use.as_deref());
    push_opt(&mut parts, "domain", row.build_domain_id.as_deref());
    if let (Some(file), Some(start), Some(end)) =
        (row.source_file.as_deref(), row.start_byte, row.end_byte)
    {
        parts.push(format!("source={file}:{start}..{end}"));
    }
    parts.join(", ")
}

fn push_opt(parts: &mut Vec<String>, label: &str, value: Option<&str>) {
    if let Some(value) = value {
        parts.push(format!("{label}={value}"));
    }
}

fn format_candidates(candidates: &[String], limit: usize) -> String {
    format_list("candidates", candidates, limit)
}

fn format_list(label: &str, values: &[String], limit: usize) -> String {
    let limit = limit.max(1);
    let mut visible = values.iter().take(limit).cloned().collect::<Vec<_>>();
    let hidden = values.len().saturating_sub(limit);
    if hidden > 0 {
        visible.push(format!("... {hidden} more"));
    }
    format!("{label}=[{}]", visible.join(", "))
}

fn format_callee(callee: &CallCalleeInfo) -> String {
    match callee {
        CallCalleeInfo::Path { path } => format!("path {}", path.join("::")),
        CallCalleeInfo::Method { name, receiver } => {
            let receiver = receiver
                .as_ref()
                .map(|receiver| format!(" on {}", format_receiver(receiver)))
                .unwrap_or_default();
            format!("method {name}{receiver}")
        }
        CallCalleeInfo::Macro { name } => format!("macro {name}"),
        CallCalleeInfo::Dynamic => "dynamic".to_string(),
    }
}

fn format_receiver(receiver: &CallReceiverInfo) -> String {
    match receiver {
        CallReceiverInfo::SelfValue => "self".to_string(),
        CallReceiverInfo::SelfField { path } => format!("self.{}", path.join(".")),
        CallReceiverInfo::LocalBinding { name } => name.clone(),
        CallReceiverInfo::TypedLocalBinding { name, type_path } => {
            format!("{name}: {}", type_path.join("::"))
        }
        CallReceiverInfo::InitializedLocalBinding { name, init_path } => {
            format!("{name} = {}", init_path.join("::"))
        }
        CallReceiverInfo::BorrowedLocalBinding { name } => format!("&{name}"),
        CallReceiverInfo::BorrowedTypedLocalBinding { name, type_path } => {
            format!("&{name}: {}", type_path.join("::"))
        }
        CallReceiverInfo::DereferencedLocalBinding { name } => format!("*{name}"),
        CallReceiverInfo::DereferencedInitializedLocalBinding { name, init_path } => {
            format!("*{name} = {}", init_path.join("::"))
        }
        CallReceiverInfo::FieldLocalBinding { name, field_path } => {
            format!("{name}.{}", field_path.join("."))
        }
        CallReceiverInfo::FieldTypedLocalBinding {
            name,
            type_path,
            field_path,
        } => format!("{name}: {}.{}", type_path.join("::"), field_path.join(".")),
        CallReceiverInfo::FieldInitializedLocalBinding {
            name,
            init_path,
            field_path,
        } => format!("{name} = {}.{}", init_path.join("::"), field_path.join(".")),
        CallReceiverInfo::PathCallResult { path } => format!("{}()", path.join("::")),
        CallReceiverInfo::MethodCallResult { method_name } => format!("{method_name}()"),
        CallReceiverInfo::AwaitResult => "await".to_string(),
        CallReceiverInfo::AwaitPathCallResult { path } => {
            format!("{}().await", path.join("::"))
        }
        CallReceiverInfo::TryResult => "?".to_string(),
        CallReceiverInfo::TryPathCallResult { path } => format!("{}()?", path.join("::")),
        CallReceiverInfo::Literal => "literal".to_string(),
    }
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
                type_context: part.type_context,
                call_expansion: part.call_expansion,
                call_context: part.call_context.clone(),
                proof_context: part.proof_context.clone(),
            });
        }
    }
    let message_tokens: usize = plan_messages.iter().map(|m| m.estimated_tokens).sum();

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
mod tests;
