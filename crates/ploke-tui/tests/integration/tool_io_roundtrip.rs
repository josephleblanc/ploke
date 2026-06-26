//! TUI tool-carrier coverage boundary:
//!
//! - Covered: serde roundtrips for `request_code_context` arguments/results
//!   and the optional `type_context` carrier on context parts.
//! - Covered elsewhere: the direct `request_code_context` production tool path
//!   is exercised in `tools::request_code_context::gat_tests` against the shared
//!   corpus-backed `TypeShapeCase` matrix. That test observes
//!   `ToolCallCompleted` and asserts structured `ConciseContext.type_context`.
//! - Not covered here: live model/tool selection. Ignored live tests should use
//!   the same matrix prompts and assert tool payloads, not final model wording.
//! - Recursive/nested type behavior is matrix-bounded by the deepest real corpus
//!   examples selected for DB and RAG coverage.

use ploke_core::rag_types::{
    ApplyCodeEditResult, AssembledMeta, CallCalleeInfo, CallContextInfo, CallExpansionInfo,
    CallExpansionKind, CallReceiverInfo, CallResolutionKind, CallSiteKind, CallStatusKind,
    CallTargetInfo, CallTargetKind, CanonPath, ConciseContext, ContextPart, ContextPartKind,
    GetFileMetadataResult, Modality, NodeFilepath, ProofContextInfo, RequestCodeContextArgs,
    RequestCodeContextResult, TypeContextInfo, TypeContextKind,
};
use uuid::Uuid;

#[test]
fn serde_roundtrip_request_code_context() {
    let args = RequestCodeContextArgs {
        token_budget_per_result: Some(512),
        token_budget_total: Some(1536),
        search_term: "SimpleStruct".to_string(),
    };
    let args_json = serde_json::to_string(&args).expect("serialize args");
    let args_back: RequestCodeContextArgs =
        serde_json::from_str(&args_json).expect("deserialize args");
    assert_eq!(args_back.token_budget_per_result, Some(512));
    assert_eq!(args_back.token_budget_total, Some(1536));
    assert_eq!(args_back.search_term, "SimpleStruct");

    let method_target = Uuid::from_u128(3);
    let dynamic_target = Uuid::from_u128(5);
    let tuple_target = Uuid::from_u128(7);
    let variant_target = Uuid::from_u128(9);
    let trait_target = Uuid::from_u128(15);
    let assoc_target = Uuid::from_u128(17);
    let imported_assoc_target = Uuid::from_u128(18);
    let call_context = vec![
        CallContextInfo {
            site_id: Uuid::from_u128(4),
            kind: CallSiteKind::Method,
            span: (11, 32),
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
            site_id: Uuid::from_u128(14),
            kind: CallSiteKind::Method,
            span: (34, 53),
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
                target_id: trait_target,
                relation: CallTargetKind::Method,
            }],
        },
        CallContextInfo {
            site_id: Uuid::from_u128(16),
            kind: CallSiteKind::Path,
            span: (54, 92),
            callee: CallCalleeInfo::Path {
                path: vec![
                    "LocalAssocFunctionTrait".to_string(),
                    "trait_make".to_string(),
                ],
            },
            status: CallStatusKind::Resolved,
            resolution: Some(CallResolutionKind::LocalExact),
            targets: vec![CallTargetInfo {
                target_id: assoc_target,
                relation: CallTargetKind::AssociatedFunction,
            }],
        },
        CallContextInfo {
            site_id: Uuid::from_u128(18),
            kind: CallSiteKind::Path,
            span: (94, 142),
            callee: CallCalleeInfo::Path {
                path: vec![
                    "VisibleAssocFunctionTrait".to_string(),
                    "imported_trait_make".to_string(),
                ],
            },
            status: CallStatusKind::Resolved,
            resolution: Some(CallResolutionKind::LocalExact),
            targets: vec![CallTargetInfo {
                target_id: imported_assoc_target,
                relation: CallTargetKind::AssociatedFunction,
            }],
        },
        CallContextInfo {
            site_id: Uuid::from_u128(6),
            kind: CallSiteKind::Dynamic,
            span: (40, 57),
            callee: CallCalleeInfo::Dynamic,
            status: CallStatusKind::Resolved,
            resolution: Some(CallResolutionKind::LocalExact),
            targets: vec![CallTargetInfo {
                target_id: dynamic_target,
                relation: CallTargetKind::DynamicFunction,
            }],
        },
        CallContextInfo {
            site_id: Uuid::from_u128(8),
            kind: CallSiteKind::Path,
            span: (60, 72),
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
            site_id: Uuid::from_u128(10),
            kind: CallSiteKind::Path,
            span: (74, 98),
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
            site_id: Uuid::from_u128(11),
            kind: CallSiteKind::Macro,
            span: (100, 124),
            callee: CallCalleeInfo::Macro {
                name: "crate::crate_scoped_macro".to_string(),
            },
            status: CallStatusKind::Unsupported,
            resolution: None,
            targets: Vec::new(),
        },
        CallContextInfo {
            site_id: Uuid::from_u128(12),
            kind: CallSiteKind::Method,
            span: (126, 140),
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
            site_id: Uuid::from_u128(13),
            kind: CallSiteKind::Method,
            span: (142, 153),
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
    let type_context = TypeContextInfo {
        seed_id: Uuid::from_u128(1),
        relation: TypeContextKind::TypeDefinitionImpact,
        distance: 1,
    };
    let call_expansion = CallExpansionInfo {
        seed_id: dynamic_target,
        relation: CallExpansionKind::IncomingCaller,
        call_site_id: Uuid::from_u128(6),
        target_id: dynamic_target,
        distance: 1,
    };
    let proof_context = vec![ProofContextInfo {
        fact_id: "call-edge:1".to_string(),
        kind: "call_edge".to_string(),
        build_domain_id: Some("bd:test".to_string()),
        call_site_id: Some("call:site".to_string()),
        call_edge_id: Some("edge:1".to_string()),
        caller_def_id: Some("def:caller".to_string()),
        callee_def_id: Some("def:callee".to_string()),
        resolution_state: Some("resolved".to_string()),
        resolved_def_id: Some("def:callee".to_string()),
        candidate_def_ids: Vec::new(),
        external_summary_id: None,
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
        start_byte: Some(1),
        end_byte: Some(10),
        line_start: Some(1),
        line_end: Some(1),
        effect_class: Some("call".to_string()),
        blocker_reason: None,
        status: Some("resolved".to_string()),
        detail: None,
    }];
    let source = ContextPart {
        id: Uuid::from_u128(2),
        file_path: NodeFilepath("id://dummy".to_string()),
        canon_path: CanonPath("some::module::dummy".to_string()),
        ranges: Vec::new(),
        kind: ContextPartKind::Code,
        text: "fn foo() {}".to_string(),
        score: 1.0,
        modality: Modality::Sparse,
        type_context: Some(type_context),
        call_expansion: Some(call_expansion),
        call_context: call_context.clone(),
        proof_context: proof_context.clone(),
    };
    let path_site = Uuid::from_u128(19);
    let path_call = CallContextInfo {
        site_id: path_site,
        kind: CallSiteKind::Path,
        span: (160, 181),
        callee: CallCalleeInfo::Path {
            path: vec!["crate".to_string(), "local_target".to_string()],
        },
        status: CallStatusKind::Resolved,
        resolution: Some(CallResolutionKind::LocalExact),
        targets: vec![CallTargetInfo {
            target_id: dynamic_target,
            relation: CallTargetKind::Function,
        }],
    };
    let path_expansion = CallExpansionInfo {
        seed_id: dynamic_target,
        relation: CallExpansionKind::IncomingCaller,
        call_site_id: path_site,
        target_id: dynamic_target,
        distance: 1,
    };
    let path_source = ContextPart {
        id: Uuid::from_u128(20),
        file_path: NodeFilepath("id://path".to_string()),
        canon_path: CanonPath("some::module::path_caller".to_string()),
        ranges: Vec::new(),
        kind: ContextPartKind::Code,
        text: "fn path_caller() { crate::local_target(); }".to_string(),
        score: 0.5,
        modality: Modality::Sparse,
        type_context: None,
        call_expansion: Some(path_expansion),
        call_context: vec![path_call.clone()],
        proof_context: Vec::new(),
    };
    let mut result = RequestCodeContextResult::from_assembled(
        vec![source, path_source],
        AssembledMeta {
            search_term: "foo".to_string(),
            top_k: 3,
            kind: ContextPartKind::Code,
        },
    );
    result.note = Some("No indexed snippets matched `foo`.".to_string());
    result.next_steps = vec![
        "Retry with an exact symbol.".to_string(),
        "Use code_item_lookup.".to_string(),
    ];
    let expected = ConciseContext {
        id: Uuid::from_u128(2),
        file_path: NodeFilepath("id://dummy".to_string()),
        canon_path: CanonPath("some::module::dummy".to_string()),
        snippet: "fn foo() {}".to_string(),
        type_context: Some(type_context),
        call_expansion: Some(call_expansion),
        call_context,
        proof_context,
    };
    let path_expected = ConciseContext {
        id: Uuid::from_u128(20),
        file_path: NodeFilepath("id://path".to_string()),
        canon_path: CanonPath("some::module::path_caller".to_string()),
        snippet: "fn path_caller() { crate::local_target(); }".to_string(),
        type_context: None,
        call_expansion: Some(path_expansion),
        call_context: vec![path_call],
        proof_context: Vec::new(),
    };
    assert_eq!(
        result.context,
        vec![expected.clone(), path_expected.clone()],
        "from_assembled must preserve typed, call, and expansion carriers"
    );

    let result = RequestCodeContextResult {
        ok: result.ok,
        search_term: "foo".to_string(),
        top_k: 3,
        note: Some("No indexed snippets matched `foo`.".to_string()),
        next_steps: vec![
            "Retry with an exact symbol.".to_string(),
            "Use code_item_lookup.".to_string(),
        ],
        context: result.context,
        kind: ContextPartKind::Code,
    };
    let res_json = serde_json::to_string(&result).expect("serialize result");
    let res_back: RequestCodeContextResult =
        serde_json::from_str(&res_json).expect("deserialize result");
    assert!(res_back.ok);
    assert_eq!(res_back.search_term, "foo");
    assert_eq!(res_back.top_k, 3);
    assert_eq!(
        res_back.note.as_deref(),
        Some("No indexed snippets matched `foo`.")
    );
    assert_eq!(res_back.next_steps.len(), 2);
    assert_eq!(res_back.context, vec![expected, path_expected]);
    assert_eq!(res_back.kind, ContextPartKind::Code);

    let missing_id_json = r#"{
        "file_path": "id://dummy",
        "canon_path": "some::module::dummy",
        "snippet": "fn foo() {}",
        "type_context": null
    }"#;
    assert!(
        serde_json::from_str::<ConciseContext>(missing_id_json).is_err(),
        "ConciseContext.id is a required tool payload identity"
    );
}

#[test]
fn serde_roundtrip_get_file_metadata_result() {
    let res = GetFileMetadataResult {
        ok: true,
        file_path: "/tmp/file.rs".to_string(),
        exists: true,
        byte_len: 1234,
        modified_ms: Some(1_700_000_000_000),
        file_hash: Uuid::nil().to_string(),
        tracking_hash: Uuid::new_v4().to_string(),
    };
    let json = serde_json::to_string(&res).expect("serialize");
    let back: GetFileMetadataResult = serde_json::from_str(&json).expect("deserialize");
    assert!(back.ok);
    assert_eq!(back.file_path, "/tmp/file.rs");
    assert!(back.exists);
    assert_eq!(back.byte_len, 1234);
    assert!(back.modified_ms.is_some());
    assert_eq!(back.file_hash.len(), 36);
    assert_eq!(back.tracking_hash.len(), 36);
}

#[test]
fn serde_roundtrip_apply_code_edit_result() {
    let res = ApplyCodeEditResult {
        ok: true,
        staged: 2,
        applied: 0,
        files: vec!["src/lib.rs".to_string(), "src/main.rs".to_string()],
        preview_mode: "diff".to_string(),
        auto_confirmed: false,
    };
    let json = serde_json::to_string(&res).expect("serialize");
    let back: ApplyCodeEditResult = serde_json::from_str(&json).expect("deserialize");
    assert!(back.ok);
    assert_eq!(back.staged, 2);
    assert_eq!(back.applied, 0);
    assert_eq!(back.files.len(), 2);
    assert_eq!(back.preview_mode, "diff");
    assert!(!back.auto_confirmed);
}
