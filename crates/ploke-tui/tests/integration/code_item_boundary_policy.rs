use std::borrow::Cow;

use ploke_tui::tools::{
    Tool,
    code_item_boundary_policy::{
        BoundaryRuleParam, CodeItemBoundaryPolicy, CodeItemBoundaryPolicyParams,
        CodeItemBoundaryPolicyResult, CrateBoundaryRuleParam,
    },
    code_item_endpoint::CodeItemEndpoint,
};

use crate::call_graph_tool_support::{
    AxumFromFnBasicToolFixture, AxumRequestExtractPathToolFixture, ui_field,
};

#[tokio::test]
async fn code_item_boundary_policy_flags_real_corpus_request_extract_boundary() {
    let fixture = AxumRequestExtractPathToolFixture::new().await;
    let owner = CodeItemEndpoint {
        item_name: Cow::Borrowed("extract"),
        file_path: Cow::Owned(fixture.start_file_path.display().to_string()),
        node_kind: Cow::Borrowed("method"),
        module_path: Cow::Owned(fixture.start_module_path_arg()),
        owner_trait: None,
        owner_type: Some(Cow::Borrowed("Request")),
        parent_name: None,
        body_contains: None,
    };

    let result = CodeItemBoundaryPolicy::execute(
        CodeItemBoundaryPolicyParams {
            owner: owner.clone(),
            rules: vec![BoundaryRuleParam {
                rule_id: "ext-traits-must-not-call-extract".to_string(),
                caller_module_prefix: vec!["crate".to_string(), "ext_traits".to_string()],
                callee_module_prefix: vec!["crate".to_string(), "extract".to_string()],
            }],
            crate_rules: Vec::new(),
            max_depth: Some(2),
            max_paths: Some(16),
        },
        fixture.ctx("axum-code-item-boundary-policy"),
    )
    .await
    .expect("boundary policy tool execution");
    let payload: CodeItemBoundaryPolicyResult =
        serde_json::from_str(&result.content).expect("deserialize boundary policy result");

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Architecture review:
    //   "Which modules call across a boundary that should be one-way?"
    //
    // Source-oracle chain:
    //   axum-core/src/ext_traits/request.rs:268
    //     `RequestExt::extract` calls `self.extract_with_state(&())`.
    //   axum-core/src/ext_traits/request.rs:279
    //     `RequestExt::extract_with_state` calls `E::from_request(self, state)`.
    //   axum-core/src/extract/mod.rs:85
    //     defines the `FromRequest::from_request` trait method binding.
    assert_eq!(payload.owner_id, fixture.start);
    assert_eq!(payload.violations.len(), 1);
    let violation = &payload.violations[0];
    assert_eq!(violation.rule_id, "ext-traits-must-not-call-extract");
    assert_eq!(violation.edge.edge.caller_id, fixture.intermediate);
    assert_eq!(violation.edge.edge.callee_id, fixture.target);
    assert_eq!(
        violation.edge.caller.module_path,
        vec!["crate", "ext_traits", "request"]
    );
    assert_eq!(violation.edge.callee.module_path, vec!["crate", "extract"]);
    assert!(
        payload.source_files.iter().any(|path| path
            .as_ref()
            .ends_with("axum-core/src/ext_traits/request.rs")),
        "boundary policy result should include the owner/caller source file: {payload:#?}"
    );
    assert!(
        payload
            .source_files
            .iter()
            .any(|path| path.as_ref().ends_with("axum-core/src/extract/mod.rs")),
        "boundary policy result should include the callee source file: {payload:#?}"
    );
    let ui = result
        .ui_payload
        .as_ref()
        .expect("boundary policy ui payload");
    assert_eq!(ui_field(ui, "rules"), "1");
    assert_eq!(ui_field(ui, "violations"), "1");

    let reverse = CodeItemBoundaryPolicy::execute(
        CodeItemBoundaryPolicyParams {
            owner,
            rules: vec![BoundaryRuleParam {
                rule_id: "extract-must-not-call-ext-traits".to_string(),
                caller_module_prefix: vec!["crate".to_string(), "extract".to_string()],
                callee_module_prefix: vec!["crate".to_string(), "ext_traits".to_string()],
            }],
            crate_rules: Vec::new(),
            max_depth: Some(2),
            max_paths: Some(16),
        },
        fixture.ctx("axum-code-item-boundary-policy-reverse"),
    )
    .await
    .expect("reverse boundary policy tool execution");
    let payload: CodeItemBoundaryPolicyResult =
        serde_json::from_str(&reverse.content).expect("deserialize reverse boundary result");
    assert!(
        payload.violations.is_empty(),
        "reverse policy should not match this axum call chain: {payload:#?}"
    );
    let ui = reverse.ui_payload.as_ref().expect("reverse ui payload");
    assert_eq!(ui_field(ui, "violations"), "0");
}

#[tokio::test]
async fn code_item_boundary_policy_flags_real_corpus_crate_boundary() {
    let fixture = AxumFromFnBasicToolFixture::new().await;
    let owner = CodeItemEndpoint {
        item_name: Cow::Borrowed("basic"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("function"),
        module_path: Cow::Owned(fixture.module_path_arg()),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
        body_contains: None,
    };

    let result = CodeItemBoundaryPolicy::execute(
        CodeItemBoundaryPolicyParams {
            owner: owner.clone(),
            rules: Vec::new(),
            crate_rules: vec![CrateBoundaryRuleParam {
                rule_id: "axum-must-not-call-axum-core".to_string(),
                caller_crate: "axum".to_string(),
                callee_crate: "axum-core".to_string(),
            }],
            max_depth: Some(1),
            max_paths: Some(64),
        },
        fixture.ctx("axum-code-item-crate-boundary-policy"),
    )
    .await
    .expect("crate boundary policy tool execution");
    let payload: CodeItemBoundaryPolicyResult =
        serde_json::from_str(&result.content).expect("deserialize crate boundary policy result");

    // Source oracle:
    //   axum/src/middleware/from_fn.rs:411 calls `Body::empty()`.
    //   axum-core/src/body.rs:52 defines `Body::empty`.
    assert_eq!(payload.owner_id, fixture.owner);
    assert!(payload.violations.is_empty());
    assert_eq!(payload.crate_violations.len(), 1);
    let violation = &payload.crate_violations[0];
    assert_eq!(violation.rule_id, "axum-must-not-call-axum-core");
    assert_eq!(violation.edge.edge.caller_id, fixture.owner);
    assert_eq!(violation.edge.edge.callee_id, fixture.body_empty_target);
    assert_eq!(violation.edge.caller_crate, "axum");
    assert_eq!(violation.edge.callee_crate, "axum-core");
    assert!(
        payload
            .source_files
            .iter()
            .any(|path| path.as_ref().ends_with("axum/src/middleware/from_fn.rs")),
        "crate boundary policy result should include the owner/caller source file: {payload:#?}"
    );
    assert!(
        payload
            .source_files
            .iter()
            .any(|path| path.as_ref().ends_with("axum-core/src/body.rs")),
        "crate boundary policy result should include the callee source file: {payload:#?}"
    );
    let ui = result
        .ui_payload
        .as_ref()
        .expect("crate boundary policy ui payload");
    assert_eq!(ui_field(ui, "rules"), "0");
    assert_eq!(ui_field(ui, "crate_rules"), "1");
    assert_eq!(ui_field(ui, "violations"), "0");
    assert_eq!(ui_field(ui, "crate_violations"), "1");

    let reverse = CodeItemBoundaryPolicy::execute(
        CodeItemBoundaryPolicyParams {
            owner,
            rules: Vec::new(),
            crate_rules: vec![CrateBoundaryRuleParam {
                rule_id: "axum-core-must-not-call-axum".to_string(),
                caller_crate: "axum-core".to_string(),
                callee_crate: "axum".to_string(),
            }],
            max_depth: Some(1),
            max_paths: Some(64),
        },
        fixture.ctx("axum-code-item-crate-boundary-policy-reverse"),
    )
    .await
    .expect("reverse crate boundary policy tool execution");
    let payload: CodeItemBoundaryPolicyResult =
        serde_json::from_str(&reverse.content).expect("deserialize reverse crate boundary result");
    assert!(
        payload.crate_violations.is_empty(),
        "reverse crate policy should not match this axum owner: {payload:#?}"
    );
    let ui = reverse.ui_payload.as_ref().expect("reverse ui payload");
    assert_eq!(ui_field(ui, "crate_violations"), "0");
}
