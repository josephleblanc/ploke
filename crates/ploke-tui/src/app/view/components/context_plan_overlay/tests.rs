use super::*;
use crate::llm::manager::events::ContextPlan;
use ploke_core::rag_types::{
    CallCalleeInfo, CallContextInfo, CallExpansionInfo, CallExpansionKind, CallReceiverInfo,
    CallResolutionKind, CallSiteKind, CallStatusKind, CallTargetInfo, CallTargetKind,
    ContextPartKind, ProofContextInfo,
};

fn line_text(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
}

#[test]
fn expanded_rag_part_displays_call_context_details() {
    let part_id = Uuid::from_u128(0x701);
    let target = Uuid::from_u128(0x501);
    let method_target = Uuid::from_u128(0x502);
    let assoc_target = Uuid::from_u128(0x503);
    let dynamic_target = Uuid::from_u128(0x504);
    let tuple_target = Uuid::from_u128(0x505);
    let variant_target = Uuid::from_u128(0x506);
    let plan = ContextPlan {
        plan_id: Uuid::from_u128(0x101),
        parent_id: Uuid::from_u128(0x102),
        estimated_total_tokens: 10,
        included_messages: Vec::new(),
        excluded_messages: Vec::new(),
        included_rag_parts: vec![ContextPlanRagPart {
            part_id,
            file_path: "src/lib.rs".to_string(),
            kind: ContextPartKind::Code,
            estimated_tokens: 10,
            score: 0.75,
            type_context: None,
            call_expansion: Some(CallExpansionInfo {
                seed_id: Uuid::from_u128(0x401),
                relation: CallExpansionKind::IncomingCaller,
                call_site_id: Uuid::from_u128(0x602),
                target_id: target,
                distance: 1,
            }),
            call_context: vec![
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
                    site_id: Uuid::from_u128(0x605),
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
                    site_id: Uuid::from_u128(0x606),
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
                    site_id: Uuid::from_u128(0x607),
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
            ],
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
                candidate_def_ids: Vec::new(),
                external_summary_id: None,
                boundary_id: None,
                boundary_kind: None,
                expanded_item_id: None,
                definition_id: None,
                target_kind: None,
                target_name: None,
                target_root: None,
                profile: None,
                rustc_version: None,
                proof_policy_version: None,
                cfg_domain_id: None,
                active_cfg_hash: None,
                invocation_id: None,
                rustc_program: None,
                working_directory: None,
                argument_vector_hash: None,
                environment_hash: None,
                effect_seed_id: None,
                confidence: None,
                blocker_if_unresolved: None,
                authority_term: None,
                summary_class: None,
                artifact_hash: None,
                summary_version: None,
                review_method: None,
                scope_of_validity: None,
                allowed_effects: Vec::new(),
                required_containment: None,
                invalidation_conditions: None,
                evidence_use: Some("proof_only".to_string()),
                source_file: Some("src/lib.rs".to_string()),
                start_byte: Some(13),
                end_byte: Some(28),
                line_start: Some(1),
                line_end: Some(1),
                effect_class: Some("call".to_string()),
                blocker_reason: Some("type_resolution_missing".to_string()),
                status: Some("resolved".to_string()),
                detail: None,
            }],
        }],
        rag_stats: None,
    };
    let snapshot = ContextPlanSnapshot::new(plan, None);
    let (rows, sections) = build_rows(&snapshot, ContextPlanFilter::All);
    assert!(
        sections
            .iter()
            .any(|section| section.kind == ContextPlanSectionKind::IncludedRag),
        "rows should include a RAG section"
    );

    let expanded = HashSet::from([ContextPlanItemKey::RagPart { part_id }]);
    let items = build_display_items(
        &snapshot,
        &rows,
        &expanded,
        &HashSet::new(),
        None,
        120,
        120,
        &HashMap::new(),
        &UiTheme::default(),
    );
    let item = items
        .iter()
        .find(|item| line_text(&item.title).contains("[rag]"))
        .expect("expanded RAG part should render");
    assert!(item.expanded);
    assert!(line_text(&item.title).contains("call IncomingCaller"));
    assert!(line_text(&item.title).contains("calls 9"));
    assert!(line_text(&item.title).contains("proofs 1"));

    let details = item
        .details
        .iter()
        .map(line_text)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(details.contains("call_context: 9 outgoing call site(s)"));
    assert!(details.contains("call_expansion: IncomingCaller"));
    assert!(details.contains("proof_context: 1 proof fact(s)"));
    assert!(details.contains("call_edge"));
    assert!(details.contains("site=call:site"));
    assert!(details.contains("blocker=type_resolution_missing"));
    assert!(details.contains("domain=bd:fixture-call-graph"));
    assert!(details.contains("Path @ 10..12: path Ok => Unsupported, targets []"));
    assert!(
        details.contains(&format!(
            "Path @ 13..28: path try_local_assoc => Resolved(LocalExact), targets [Function:{target}]"
        )),
        "{details}"
    );
    assert!(
        details.contains(&format!(
            "Method @ 13..46: method instance_value on try_local_assoc()? => Resolved(LocalExact), targets [Method:{method_target}]"
        )),
        "{details}"
    );
    assert!(
        details.contains(&format!(
            "Path @ 50..62: path Self::make => Resolved(LocalExact), targets [AssociatedFunction:{assoc_target}]"
        )),
        "{details}"
    );
    assert!(
        details.contains(&format!(
            "Path @ 64..74: path NewType => Resolved(LocalExact), targets [TupleStructConstructor:{tuple_target}]"
        )),
        "{details}"
    );
    assert!(
        details.contains(&format!(
            "Path @ 75..99: path EnumWithData::Variant1 => Resolved(LocalExact), targets [EnumVariantConstructor:{variant_target}]"
        )),
        "{details}"
    );
    assert!(
        details.contains(
            "Macro @ 120..144: macro crate::crate_scoped_macro => Unsupported, targets []"
        ),
        "{details}"
    );
    assert!(
        details.contains("Method @ 145..160: method overlap on value => Ambiguous, targets []"),
        "{details}"
    );
    assert!(details.contains("- ... 1 more call site(s)"), "{details}");
    assert!(
        !details.contains(&format!("DynamicFunction:{dynamic_target}")),
        "overlay should respect the call-context row limit: {details}"
    );
}

#[test]
fn expanded_rag_part_displays_external_call_context_details() {
    let part_id = Uuid::from_u128(0x801);
    let plan = ContextPlan {
        plan_id: Uuid::from_u128(0x201),
        parent_id: Uuid::from_u128(0x202),
        estimated_total_tokens: 10,
        included_messages: Vec::new(),
        excluded_messages: Vec::new(),
        included_rag_parts: vec![ContextPlanRagPart {
            part_id,
            file_path: "src/lib.rs".to_string(),
            kind: ContextPartKind::Code,
            estimated_tokens: 10,
            score: 0.75,
            type_context: None,
            call_expansion: None,
            call_context: vec![
                CallContextInfo {
                    site_id: Uuid::from_u128(0x901),
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
                    site_id: Uuid::from_u128(0x902),
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
                    site_id: Uuid::from_u128(0x903),
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
            ],
            proof_context: Vec::new(),
        }],
        rag_stats: None,
    };
    let snapshot = ContextPlanSnapshot::new(plan, None);
    let (rows, _) = build_rows(&snapshot, ContextPlanFilter::All);
    let expanded = HashSet::from([ContextPlanItemKey::RagPart { part_id }]);
    let items = build_display_items(
        &snapshot,
        &rows,
        &expanded,
        &HashSet::new(),
        None,
        120,
        120,
        &HashMap::new(),
        &UiTheme::default(),
    );
    let item = items
        .iter()
        .find(|item| line_text(&item.title).contains("[rag]"))
        .expect("expanded RAG part should render");
    assert!(item.expanded);
    assert!(line_text(&item.title).contains("calls 3"));

    let details = item
        .details
        .iter()
        .map(line_text)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(details.contains("call_context: 3 outgoing call site(s)"));
    assert!(
        details.contains("Path @ 10..23: path String::new => External, targets []"),
        "{details}"
    );
    assert!(
        details.contains("Method @ 24..45: method to_string on literal => External, targets []"),
        "{details}"
    );
    assert!(
        details.contains("Method @ 46..57: method len on value: Vec => External, targets []"),
        "{details}"
    );
}

#[test]
fn expanded_rag_part_displays_trait_dispatch_call_context_details() {
    let part_id = Uuid::from_u128(0x821);
    let seed_id = Uuid::from_u128(0x822);
    let site_id = Uuid::from_u128(0x823);
    let target = Uuid::from_u128(0x824);
    let plan = ContextPlan {
        plan_id: Uuid::from_u128(0x221),
        parent_id: Uuid::from_u128(0x222),
        estimated_total_tokens: 10,
        included_messages: Vec::new(),
        excluded_messages: Vec::new(),
        included_rag_parts: vec![ContextPlanRagPart {
            part_id,
            file_path: "src/lib.rs".to_string(),
            kind: ContextPartKind::Code,
            estimated_tokens: 10,
            score: 0.75,
            type_context: None,
            call_expansion: Some(CallExpansionInfo {
                seed_id,
                relation: CallExpansionKind::IncomingCaller,
                call_site_id: site_id,
                target_id: target,
                distance: 1,
            }),
            call_context: vec![CallContextInfo {
                site_id,
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
            }],
            proof_context: Vec::new(),
        }],
        rag_stats: None,
    };
    let snapshot = ContextPlanSnapshot::new(plan, None);
    let (rows, _) = build_rows(&snapshot, ContextPlanFilter::All);
    let expanded = HashSet::from([ContextPlanItemKey::RagPart { part_id }]);
    let items = build_display_items(
        &snapshot,
        &rows,
        &expanded,
        &HashSet::new(),
        None,
        120,
        120,
        &HashMap::new(),
        &UiTheme::default(),
    );
    let item = items
        .iter()
        .find(|item| line_text(&item.title).contains("[rag]"))
        .expect("expanded RAG part should render");
    assert!(item.expanded);
    assert!(line_text(&item.title).contains("call IncomingCaller"));
    assert!(line_text(&item.title).contains("calls 1"));

    let details = item
        .details
        .iter()
        .map(line_text)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(details.contains("call_expansion: IncomingCaller"));
    assert!(details.contains("call_context: 1 outgoing call site(s)"));
    assert!(
        details.contains(&format!(
            "Method @ 20..39: method trait_value on value = TraitDispatchTarget => Resolved(LocalExact), targets [Method:{target}]"
        )),
        "{details}"
    );
}

#[test]
fn expanded_rag_part_displays_callable_path_call_context_details() {
    let part_id = Uuid::from_u128(0x811);
    let target = Uuid::from_u128(0x812);
    let plan = ContextPlan {
        plan_id: Uuid::from_u128(0x211),
        parent_id: Uuid::from_u128(0x212),
        estimated_total_tokens: 10,
        included_messages: Vec::new(),
        excluded_messages: Vec::new(),
        included_rag_parts: vec![ContextPlanRagPart {
            part_id,
            file_path: "src/lib.rs".to_string(),
            kind: ContextPartKind::Code,
            estimated_tokens: 10,
            score: 0.75,
            type_context: None,
            call_expansion: None,
            call_context: vec![
                CallContextInfo {
                    site_id: Uuid::from_u128(0x913),
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
                    site_id: Uuid::from_u128(0x914),
                    kind: CallSiteKind::Dynamic,
                    span: (10, 21),
                    callee: CallCalleeInfo::Dynamic,
                    status: CallStatusKind::Unsupported,
                    resolution: None,
                    targets: Vec::new(),
                },
                CallContextInfo {
                    site_id: Uuid::from_u128(0x915),
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
                    site_id: Uuid::from_u128(0x916),
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
                    site_id: Uuid::from_u128(0x917),
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
                    site_id: Uuid::from_u128(0x918),
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
                    site_id: Uuid::from_u128(0x919),
                    kind: CallSiteKind::Path,
                    span: (90, 100),
                    callee: CallCalleeInfo::Path {
                        path: vec!["Vec".to_string(), "new".to_string()],
                    },
                    status: CallStatusKind::External,
                    resolution: None,
                    targets: Vec::new(),
                },
            ],
            proof_context: Vec::new(),
        }],
        rag_stats: None,
    };
    let snapshot = ContextPlanSnapshot::new(plan, None);
    let (rows, _) = build_rows(&snapshot, ContextPlanFilter::All);
    let expanded = HashSet::from([ContextPlanItemKey::RagPart { part_id }]);
    let items = build_display_items(
        &snapshot,
        &rows,
        &expanded,
        &HashSet::new(),
        None,
        120,
        120,
        &HashMap::new(),
        &UiTheme::default(),
    );
    let item = items
        .iter()
        .find(|item| line_text(&item.title).contains("[rag]"))
        .expect("expanded RAG part should render");
    assert!(item.expanded);
    assert!(line_text(&item.title).contains("calls 7"));

    let details = item
        .details
        .iter()
        .map(line_text)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(details.contains("call_context: 7 outgoing call site(s)"));
    assert!(
        details.contains(&format!(
            "Path @ 10..19: path make_fn => Resolved(LocalExact), targets [Function:{target}]"
        )),
        "{details}"
    );
    assert!(
        details.contains("Dynamic @ 10..21: dynamic => Unsupported, targets []"),
        "{details}"
    );
    assert!(
        details.contains("Path @ 30..33: path f => Unsupported, targets []"),
        "{details}"
    );
    assert!(
        details.contains("Path @ 40..51: path generic_f => Unsupported, targets []"),
        "{details}"
    );
    assert!(
        details.contains("Path @ 60..68: path boxed_fn => Unsupported, targets []"),
        "{details}"
    );
    assert!(
        details.contains("Path @ 70..85: path Box::new => External, targets []"),
        "{details}"
    );
    assert!(
        details.contains("Path @ 90..100: path Vec::new => External, targets []"),
        "{details}"
    );
}
