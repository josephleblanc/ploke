//! Prompt-formatting coverage boundary for typed type context:
//!
//! - Covered: a `ContextPart` carrying `TypeContextInfo` renders stable
//!   model-facing provenance text, and context-plan summaries preserve the
//!   type-context carrier.
//! - Not covered: whether the provenance came from a where clause, whether
//!   RAG selected the right where-derived neighbor, or whether exact
//!   `TypeUseCoordinate` values survive into the prompt. Current prompt
//!   output intentionally carries relation, seed, and distance only.
//! - A future where-specific TUI test should start from the
//!   `fixture_type_resolution_v2` DB/RAG path rather than constructing
//!   `TypeContextInfo` by hand.

use super::*;
use crate::chat_history::{
    ChatHistory, ContextStatus, MessageKind, MessageStatus, RetentionClass, TurnsToLive,
};
use crate::tools::{ToolName, ToolUiPayload};
use ploke_core::rag_types::{
    CallCalleeInfo, CallContextInfo, CallExpansionInfo, CallExpansionKind, CallReceiverInfo,
    CallResolutionKind, CallSiteKind, CallStatusKind, CallTargetInfo, CallTargetKind, CanonPath,
    ContextPartKind, ContextStats, Modality, NodeFilepath, ProofContextInfo, TypeContextInfo,
    TypeContextKind,
};
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
            type_context: None,
            call_expansion: None,
            call_context: Vec::new(),
            proof_context: Vec::new(),
        }],
        stats: ContextStats {
            total_tokens: 10,
            files: 1,
            parts: 1,
            truncated_parts: 0,
            dedup_removed: 0,
            ..Default::default()
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
        type_context: Some(TypeContextInfo {
            seed_id: Uuid::from_u128(7),
            relation: TypeContextKind::TypeDefinitionImpact,
            distance: 1,
        }),
        call_expansion: None,
        call_context: Vec::new(),
        proof_context: Vec::new(),
    };

    let rendered = reformat_context_to_system(part);

    assert!(rendered.contains("kind: Doc"));
    assert!(rendered.contains("score: 0.420"));
    assert!(rendered.contains("type_context: TypeDefinitionImpact"));
    assert!(rendered.contains("line 0"));
    assert!(rendered.contains(&format!("line {}", DEFAULT_CONTEXT_PART_MAX_LINES - 1)));
    assert!(!rendered.contains(&format!("line {}", DEFAULT_CONTEXT_PART_MAX_LINES)));
    assert!(rendered.contains("... [truncated]"));
}

#[test]
fn reformat_context_to_system_includes_call_context_details() {
    let target = Uuid::from_u128(41);
    let part = ContextPart {
        id: Uuid::from_u128(40),
        file_path: NodeFilepath::new("src/main.rs".to_string()),
        canon_path: CanonPath::new("crate::main".to_string()),
        ranges: vec![],
        kind: ContextPartKind::Code,
        text: "fn main() { value.0(); }".to_string(),
        score: 0.42,
        modality: Modality::Dense,
        type_context: None,
        call_expansion: Some(CallExpansionInfo {
            seed_id: Uuid::from_u128(43),
            relation: CallExpansionKind::OutgoingTarget,
            call_site_id: Uuid::from_u128(42),
            target_id: target,
            distance: 1,
        }),
        call_context: vec![CallContextInfo {
            site_id: Uuid::from_u128(42),
            kind: CallSiteKind::Dynamic,
            span: (20, 29),
            callee: CallCalleeInfo::Dynamic,
            status: CallStatusKind::Resolved,
            resolution: Some(CallResolutionKind::LocalExact),
            targets: vec![CallTargetInfo {
                target_id: target,
                relation: CallTargetKind::DynamicFunction,
            }],
        }],
        proof_context: Vec::new(),
    };

    let rendered = reformat_context_to_system(part);

    assert!(rendered.contains("call_expansion: OutgoingTarget"));
    assert!(rendered.contains("call_context: 1 outgoing call site(s)"));
    assert!(rendered.contains("Dynamic @ 20..29: dynamic"));
    assert!(rendered.contains("Resolved(LocalExact)"));
    assert!(rendered.contains(&format!("DynamicFunction:{target}")));
}

#[test]
fn reformat_context_to_system_includes_proof_context_details() {
    let part = ContextPart {
        id: Uuid::from_u128(40),
        file_path: NodeFilepath::new("src/main.rs".to_string()),
        canon_path: CanonPath::new("crate::main".to_string()),
        ranges: vec![],
        kind: ContextPartKind::Code,
        text: "fn main() { local_target(); }".to_string(),
        score: 0.42,
        modality: Modality::Dense,
        type_context: None,
        call_expansion: None,
        call_context: Vec::new(),
        proof_context: vec![ProofContextInfo {
            fact_id: "call-edge:1".to_string(),
            kind: "call_edge".to_string(),
            build_domain_id: Some("bd:fixture-call-graph".to_string()),
            call_site_id: Some("call:site".to_string()),
            call_edge_id: Some("edge:1".to_string()),
            caller_def_id: Some("def:caller".to_string()),
            callee_def_id: Some("def:callee".to_string()),
            resolution_state: Some("resolved".to_string()),
            resolved_def_id: Some("def:callee".to_string()),
            candidate_def_ids: vec!["def:callee".to_string(), "def:other".to_string()],
            evidence_use: Some("proof_only".to_string()),
            source_file: Some("src/main.rs".to_string()),
            start_byte: Some(12),
            end_byte: Some(26),
            line_start: Some(1),
            line_end: Some(1),
            effect_class: Some("call".to_string()),
            blocker_reason: None,
            status: Some("resolved".to_string()),
            detail: None,
        }],
    };

    let rendered = reformat_context_to_system(part);

    assert!(rendered.contains("proof_context: 1 proof fact(s)"));
    assert!(rendered.contains("call_edge"));
    assert!(rendered.contains("site=call:site"));
    assert!(rendered.contains("edge=edge:1"));
    assert!(rendered.contains("caller=def:caller"));
    assert!(rendered.contains("callee=def:callee"));
    assert!(rendered.contains("state=resolved"));
    assert!(rendered.contains("resolved=def:callee"));
    assert!(rendered.contains("candidates=[def:callee, def:other]"));
    assert!(rendered.contains("domain=bd:fixture-call-graph"));
    assert!(rendered.contains("source=src/main.rs:12..26"));
}

#[test]
fn format_call_context_block_renders_fixture_derived_rows() {
    let target = Uuid::from_u128(0x501);
    let method_target = Uuid::from_u128(0x502);
    let assoc_target = Uuid::from_u128(0x503);
    let dynamic_target = Uuid::from_u128(0x504);
    let tuple_target = Uuid::from_u128(0x505);
    let variant_target = Uuid::from_u128(0x506);
    let calls = vec![
        CallContextInfo {
            site_id: Uuid::from_u128(0x601),
            kind: CallSiteKind::Path,
            span: (10, 12),
            callee: CallCalleeInfo::Path {
                path: vec!["Ok".to_string()],
            },
            status: CallStatusKind::Unsupported,
            resolution: None,
            targets: Vec::new(),
        },
        CallContextInfo {
            site_id: Uuid::from_u128(0x602),
            kind: CallSiteKind::Path,
            span: (13, 28),
            callee: CallCalleeInfo::Path {
                path: vec!["try_local_assoc".to_string()],
            },
            status: CallStatusKind::Resolved,
            resolution: Some(CallResolutionKind::LocalExact),
            targets: vec![CallTargetInfo {
                target_id: target,
                relation: CallTargetKind::Function,
            }],
        },
        CallContextInfo {
            site_id: Uuid::from_u128(0x603),
            kind: CallSiteKind::Method,
            span: (13, 46),
            callee: CallCalleeInfo::Method {
                name: "instance_value".to_string(),
                receiver: Some(CallReceiverInfo::TryPathCallResult {
                    path: vec!["try_local_assoc".to_string()],
                }),
            },
            status: CallStatusKind::Resolved,
            resolution: Some(CallResolutionKind::LocalExact),
            targets: vec![CallTargetInfo {
                target_id: method_target,
                relation: CallTargetKind::Method,
            }],
        },
        CallContextInfo {
            site_id: Uuid::from_u128(0x604),
            kind: CallSiteKind::Path,
            span: (50, 62),
            callee: CallCalleeInfo::Path {
                path: vec!["Self".to_string(), "make".to_string()],
            },
            status: CallStatusKind::Resolved,
            resolution: Some(CallResolutionKind::LocalExact),
            targets: vec![CallTargetInfo {
                target_id: assoc_target,
                relation: CallTargetKind::AssociatedFunction,
            }],
        },
        CallContextInfo {
            site_id: Uuid::from_u128(0x606),
            kind: CallSiteKind::Path,
            span: (64, 74),
            callee: CallCalleeInfo::Path {
                path: vec!["NewType".to_string()],
            },
            status: CallStatusKind::Resolved,
            resolution: Some(CallResolutionKind::LocalExact),
            targets: vec![CallTargetInfo {
                target_id: tuple_target,
                relation: CallTargetKind::TupleStructConstructor,
            }],
        },
        CallContextInfo {
            site_id: Uuid::from_u128(0x607),
            kind: CallSiteKind::Path,
            span: (75, 99),
            callee: CallCalleeInfo::Path {
                path: vec!["EnumWithData".to_string(), "Variant1".to_string()],
            },
            status: CallStatusKind::Resolved,
            resolution: Some(CallResolutionKind::LocalExact),
            targets: vec![CallTargetInfo {
                target_id: variant_target,
                relation: CallTargetKind::EnumVariantConstructor,
            }],
        },
        CallContextInfo {
            site_id: Uuid::from_u128(0x608),
            kind: CallSiteKind::Macro,
            span: (120, 144),
            callee: CallCalleeInfo::Macro {
                name: "crate::crate_scoped_macro".to_string(),
            },
            status: CallStatusKind::Unsupported,
            resolution: None,
            targets: Vec::new(),
        },
        CallContextInfo {
            site_id: Uuid::from_u128(0x609),
            kind: CallSiteKind::Method,
            span: (145, 160),
            callee: CallCalleeInfo::Method {
                name: "overlap".to_string(),
                receiver: Some(CallReceiverInfo::LocalBinding {
                    name: "value".to_string(),
                }),
            },
            status: CallStatusKind::Ambiguous,
            resolution: None,
            targets: Vec::new(),
        },
        CallContextInfo {
            site_id: Uuid::from_u128(0x605),
            kind: CallSiteKind::Dynamic,
            span: (100, 119),
            callee: CallCalleeInfo::Dynamic,
            status: CallStatusKind::Resolved,
            resolution: Some(CallResolutionKind::LocalExact),
            targets: vec![CallTargetInfo {
                target_id: dynamic_target,
                relation: CallTargetKind::DynamicFunction,
            }],
        },
    ];

    let rendered = format_call_context_block(&calls, "  ", 8);
    let expected = format!(
            "\
call_context: 9 outgoing call site(s)
  - Path @ 10..12: path Ok => Unsupported, targets []
  - Path @ 13..28: path try_local_assoc => Resolved(LocalExact), targets [Function:{target}]
  - Method @ 13..46: method instance_value on try_local_assoc()? => Resolved(LocalExact), targets [Method:{method_target}]
  - Path @ 50..62: path Self::make => Resolved(LocalExact), targets [AssociatedFunction:{assoc_target}]
  - Path @ 64..74: path NewType => Resolved(LocalExact), targets [TupleStructConstructor:{tuple_target}]
  - Path @ 75..99: path EnumWithData::Variant1 => Resolved(LocalExact), targets [EnumVariantConstructor:{variant_target}]
  - Macro @ 120..144: macro crate::crate_scoped_macro => Unsupported, targets []
  - Method @ 145..160: method overlap on value => Ambiguous, targets []
  - ... 1 more call site(s)"
        );

    assert_eq!(rendered, expected);
    assert!(
        !rendered.contains(&format!("DynamicFunction:{dynamic_target}")),
        "formatter should respect the call-context row limit"
    );
}

#[test]
fn format_call_context_block_renders_external_rows() {
    let calls = vec![
        CallContextInfo {
            site_id: Uuid::from_u128(0x701),
            kind: CallSiteKind::Path,
            span: (10, 23),
            callee: CallCalleeInfo::Path {
                path: vec!["String".to_string(), "new".to_string()],
            },
            status: CallStatusKind::External,
            resolution: None,
            targets: Vec::new(),
        },
        CallContextInfo {
            site_id: Uuid::from_u128(0x702),
            kind: CallSiteKind::Method,
            span: (24, 45),
            callee: CallCalleeInfo::Method {
                name: "to_string".to_string(),
                receiver: Some(CallReceiverInfo::Literal),
            },
            status: CallStatusKind::External,
            resolution: None,
            targets: Vec::new(),
        },
        CallContextInfo {
            site_id: Uuid::from_u128(0x703),
            kind: CallSiteKind::Method,
            span: (46, 57),
            callee: CallCalleeInfo::Method {
                name: "len".to_string(),
                receiver: Some(CallReceiverInfo::TypedLocalBinding {
                    name: "value".to_string(),
                    type_path: vec!["Vec".to_string()],
                }),
            },
            status: CallStatusKind::External,
            resolution: None,
            targets: Vec::new(),
        },
    ];

    let rendered = format_call_context_block(&calls, "  ", 8);
    let expected = "\
call_context: 3 outgoing call site(s)
  - Path @ 10..23: path String::new => External, targets []
  - Method @ 24..45: method to_string on literal => External, targets []
  - Method @ 46..57: method len on value: Vec => External, targets []";

    assert_eq!(rendered, expected);
}

#[test]
fn format_call_context_block_renders_callable_path_rows() {
    let target = Uuid::from_u128(0x901);
    let calls = vec![
        CallContextInfo {
            site_id: Uuid::from_u128(0x801),
            kind: CallSiteKind::Path,
            span: (10, 19),
            callee: CallCalleeInfo::Path {
                path: vec!["make_fn".to_string()],
            },
            status: CallStatusKind::Resolved,
            resolution: Some(CallResolutionKind::LocalExact),
            targets: vec![CallTargetInfo {
                target_id: target,
                relation: CallTargetKind::Function,
            }],
        },
        CallContextInfo {
            site_id: Uuid::from_u128(0x802),
            kind: CallSiteKind::Dynamic,
            span: (10, 21),
            callee: CallCalleeInfo::Dynamic,
            status: CallStatusKind::Unsupported,
            resolution: None,
            targets: Vec::new(),
        },
        CallContextInfo {
            site_id: Uuid::from_u128(0x803),
            kind: CallSiteKind::Path,
            span: (30, 33),
            callee: CallCalleeInfo::Path {
                path: vec!["f".to_string()],
            },
            status: CallStatusKind::Unsupported,
            resolution: None,
            targets: Vec::new(),
        },
        CallContextInfo {
            site_id: Uuid::from_u128(0x804),
            kind: CallSiteKind::Path,
            span: (40, 51),
            callee: CallCalleeInfo::Path {
                path: vec!["generic_f".to_string()],
            },
            status: CallStatusKind::Unsupported,
            resolution: None,
            targets: Vec::new(),
        },
        CallContextInfo {
            site_id: Uuid::from_u128(0x805),
            kind: CallSiteKind::Path,
            span: (60, 68),
            callee: CallCalleeInfo::Path {
                path: vec!["boxed_fn".to_string()],
            },
            status: CallStatusKind::Unsupported,
            resolution: None,
            targets: Vec::new(),
        },
        CallContextInfo {
            site_id: Uuid::from_u128(0x806),
            kind: CallSiteKind::Path,
            span: (70, 85),
            callee: CallCalleeInfo::Path {
                path: vec!["Box".to_string(), "new".to_string()],
            },
            status: CallStatusKind::External,
            resolution: None,
            targets: Vec::new(),
        },
        CallContextInfo {
            site_id: Uuid::from_u128(0x807),
            kind: CallSiteKind::Path,
            span: (90, 100),
            callee: CallCalleeInfo::Path {
                path: vec!["Vec".to_string(), "new".to_string()],
            },
            status: CallStatusKind::External,
            resolution: None,
            targets: Vec::new(),
        },
    ];

    let rendered = format_call_context_block(&calls, "  ", 8);
    let expected = format!(
        "\
call_context: 7 outgoing call site(s)
  - Path @ 10..19: path make_fn => Resolved(LocalExact), targets [Function:{target}]
  - Dynamic @ 10..21: dynamic => Unsupported, targets []
  - Path @ 30..33: path f => Unsupported, targets []
  - Path @ 40..51: path generic_f => Unsupported, targets []
  - Path @ 60..68: path boxed_fn => Unsupported, targets []
  - Path @ 70..85: path Box::new => External, targets []
  - Path @ 90..100: path Vec::new => External, targets []"
    );

    assert_eq!(rendered, expected);
}

#[test]
fn format_call_context_block_renders_trait_dispatch_initialized_local_receiver() {
    let target = Uuid::from_u128(0xa01);
    let calls = vec![CallContextInfo {
        site_id: Uuid::from_u128(0xa02),
        kind: CallSiteKind::Method,
        span: (20, 39),
        callee: CallCalleeInfo::Method {
            name: "trait_value".to_string(),
            receiver: Some(CallReceiverInfo::InitializedLocalBinding {
                name: "value".to_string(),
                init_path: vec!["TraitDispatchTarget".to_string()],
            }),
        },
        status: CallStatusKind::Resolved,
        resolution: Some(CallResolutionKind::LocalExact),
        targets: vec![CallTargetInfo {
            target_id: target,
            relation: CallTargetKind::Method,
        }],
    }];

    let rendered = format_call_context_block(&calls, "  ", 8);
    let expected = format!(
            "\
call_context: 1 outgoing call site(s)
  - Method @ 20..39: method trait_value on value = TraitDispatchTarget => Resolved(LocalExact), targets [Method:{target}]"
        );

    assert_eq!(rendered, expected);
}

fn label_message_id(
    message_id: Option<Uuid>,
    root_id: Uuid,
    labels: &HashMap<Uuid, &'static str>,
) -> String {
    match message_id {
        None => "tool_call".to_string(),
        Some(id) if id == root_id => "root_system".to_string(),
        Some(id) => labels.get(&id).copied().unwrap_or("unknown").to_string(),
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
        let type_context = part
            .type_context
            .map(|ctx| {
                format!(
                    " type_context: {}:{}:{}",
                    ctx.relation.to_static_str(),
                    label_part_id(ctx.seed_id, part_labels),
                    ctx.distance
                )
            })
            .unwrap_or_default();
        let call_context = if part.call_context.is_empty() {
            String::new()
        } else {
            format!(" call_context:{}", part.call_context.len())
        };
        let call_expansion = part
            .call_expansion
            .map(|ctx| {
                format!(
                    " call_expansion: {}:{}:{}:{}",
                    ctx.relation.to_static_str(),
                    label_part_id(ctx.seed_id, part_labels),
                    label_part_id(ctx.target_id, part_labels),
                    ctx.distance
                )
            })
            .unwrap_or_default();
        out.push_str(&format!(
            "- id: {} path: {} kind: {:?} tokens: {} score: {:.3}{}{}{}\n",
            label_part_id(part.part_id, part_labels),
            part.file_path,
            part.kind,
            part.estimated_tokens,
            part.score,
            type_context,
            call_expansion,
            call_context
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
                type_context: Some(TypeContextInfo {
                    seed_id: Uuid::from_u128(101),
                    relation: TypeContextKind::UsesTypeNested,
                    distance: 2,
                }),
                call_expansion: None,
                call_context: Vec::new(),
                proof_context: Vec::new(),
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
                type_context: None,
                call_expansion: Some(CallExpansionInfo {
                    seed_id: Uuid::from_u128(100),
                    relation: CallExpansionKind::OutgoingTarget,
                    call_site_id: Uuid::from_u128(102),
                    target_id: Uuid::from_u128(101),
                    distance: 1,
                }),
                call_context: Vec::new(),
                proof_context: Vec::new(),
            },
        ],
        stats: ContextStats {
            total_tokens: 12,
            files: 2,
            parts: 2,
            truncated_parts: 0,
            dedup_removed: 0,
            ..Default::default()
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
    let part_labels = HashMap::from([
        (Uuid::from_u128(100), "part_a"),
        (Uuid::from_u128(101), "part_b"),
    ]);

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
- id: part_a path: src/lib.rs kind: Code tokens: 3 score: 0.200 type_context: UsesTypeNested:part_b:2
- id: part_b path: src/main.rs kind: Doc tokens: 3 score: 0.800 call_expansion: OutgoingTarget:part_a:part_b:1
rag_stats:
- tokens: 12 files: 2 parts: 2 truncated: 0 dedup: 0
";

    assert_eq!(snapshot, expected);
}
