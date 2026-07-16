use std::borrow::Cow;

use ploke_tui::tools::{
    Tool,
    code_item_call_path::{CodeItemCallPath, CodeItemCallPathEndpoint, CodeItemCallPathParams},
};

use crate::call_graph_tool_support::{
    AxumFromRequestFreeFunctionPathToolFixture, AxumRequestExtractPathToolFixture, DynamicToolCase,
    DynamicToolFixture, assert_call_path_node, assert_two_hop_call_path, ui_field,
};

#[tokio::test]
async fn code_item_call_path_returns_real_corpus_two_hop_reachability() {
    let fixture = AxumRequestExtractPathToolFixture::new().await;
    let params = CodeItemCallPathParams {
        source: CodeItemCallPathEndpoint {
            item_name: Cow::Borrowed("extract"),
            file_path: Cow::Owned(fixture.start_file_path.display().to_string()),
            node_kind: Cow::Borrowed("method"),
            module_path: Cow::Owned(fixture.start_module_path_arg()),
            owner_trait: None,
            owner_type: Some(Cow::Borrowed("Request")),
            parent_name: None,
            body_contains: None,
        },
        target: CodeItemCallPathEndpoint {
            item_name: Cow::Borrowed("from_request"),
            file_path: Cow::Owned(fixture.target_file_path.display().to_string()),
            node_kind: Cow::Borrowed("method"),
            module_path: Cow::Owned(fixture.target_module_path_arg()),
            owner_trait: Some(Cow::Borrowed("FromRequest")),
            owner_type: None,
            parent_name: None,
            body_contains: None,
        },
        guard: None,
        max_depth: Some(2),
        max_paths: Some(16),
    };

    let result = CodeItemCallPath::execute(params, fixture.ctx("axum-code-item-call-path"))
        .await
        .expect("tool execution");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize CodeItemCallPathResult");
    assert_eq!(
        payload
            .get("reachable")
            .and_then(serde_json::Value::as_bool),
        Some(true),
        "code_item_call_path should report the real-corpus two-hop chain as reachable: {payload:#?}"
    );
    let paths = payload
        .get("paths")
        .and_then(serde_json::Value::as_array)
        .expect("paths array");
    let source_files = payload
        .get("source_files")
        .and_then(serde_json::Value::as_array)
        .expect("source_files array");

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Source-oracle chain:
    //   axum-core/src/ext_traits/request.rs:268
    //     `RequestExt::extract` calls `self.extract_with_state(&())`.
    //   axum-core/src/ext_traits/request.rs:279
    //     `RequestExt::extract_with_state` calls `E::from_request(self, state)`.
    //   axum-core/src/extract/mod.rs:85
    //     defines the `FromRequest::from_request` trait method binding.
    // This tool-call surface should answer the direct usage question:
    // "Can this known source symbol reach this known target symbol, and what
    // ordered call path connects them?"
    // It should also answer the documentation/RAG question:
    // "Which source files should be retrieved to answer a question about this
    // call chain?"
    assert_two_hop_call_path(
        paths,
        fixture.start,
        fixture.intermediate,
        fixture.target,
        "code_item_call_path paths",
    );
    let path = paths
        .iter()
        .find(|path| path.get("depth").and_then(serde_json::Value::as_u64) == Some(2))
        .unwrap_or_else(|| panic!("missing two-hop path: {paths:#?}"));
    assert_edge_spans_present(path, "code_item_call_path paths");
    assert_path_edge_proofs_present(&payload, path, "code_item_call_path proof context");
    let nodes = path
        .get("nodes")
        .and_then(serde_json::Value::as_array)
        .expect("path nodes");
    assert_call_path_node(
        nodes,
        fixture.start,
        "::extract",
        "axum-core/src/ext_traits/request.rs",
        "code_item_call_path paths",
    );
    assert_call_path_node(
        nodes,
        fixture.intermediate,
        "::extract_with_state",
        "axum-core/src/ext_traits/request.rs",
        "code_item_call_path paths",
    );
    assert_call_path_node(
        nodes,
        fixture.target,
        "::from_request",
        "axum-core/src/extract/mod.rs",
        "code_item_call_path paths",
    );
    assert_source_file(
        source_files,
        "axum-core/src/ext_traits/request.rs",
        "code_item_call_path source files",
    );
    assert_source_file(
        source_files,
        "axum-core/src/extract/mod.rs",
        "code_item_call_path source files",
    );

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(ui_field(ui, "reachable"), "true");
    assert_eq!(ui_field(ui, "paths"), paths.len().to_string());
    assert_eq!(ui_field(ui, "source_files"), source_files.len().to_string());
    assert!(
        ui_field(ui, "proof_context")
            .parse::<usize>()
            .expect("proof context count")
            >= 2,
        "code_item_call_path should surface path proof-context counts"
    );
    assert_eq!(ui_field(ui, "max_depth"), "2");
}

fn assert_source_file(files: &[serde_json::Value], suffix: &str, label: &str) {
    assert!(
        files
            .iter()
            .any(|file| { file.as_str().is_some_and(|path| path.ends_with(suffix)) }),
        "{label} should include source file ending with {suffix:?}: {files:#?}"
    );
}

#[tokio::test]
async fn code_item_call_path_classifies_required_guard_paths() {
    let fixture = AxumFromRequestFreeFunctionPathToolFixture::new().await;

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Security analysis / Architecture review:
    //   "Are authorization checks always called before protected state
    //   mutations?"
    //   "Do any call chains bypass the intended abstraction layer?"
    //
    // Source-oracle chain:
    //   axum-macros/src/from_request/mod.rs:145
    //     `from_request::expand` calls `impl_struct_by_extracting_each_field(...)`.
    //   axum-macros/src/from_request/mod.rs:342
    //     `impl_struct_by_extracting_each_field` calls `extract_fields(...)`.
    //   axum-macros/src/from_request/mod.rs:412
    //     defines `extract_fields`.
    let base_source = CodeItemCallPathEndpoint {
        item_name: Cow::Borrowed("expand"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("function"),
        module_path: Cow::Owned(fixture.module_path_arg()),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
        body_contains: None,
    };
    let base_target = CodeItemCallPathEndpoint {
        item_name: Cow::Borrowed("extract_fields"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("function"),
        module_path: Cow::Owned(fixture.module_path_arg()),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
        body_contains: None,
    };
    let guarded_params = CodeItemCallPathParams {
        source: base_source.clone(),
        target: base_target.clone(),
        guard: Some(CodeItemCallPathEndpoint {
            item_name: Cow::Borrowed("impl_struct_by_extracting_each_field"),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed("function"),
            module_path: Cow::Owned(fixture.module_path_arg()),
            owner_trait: None,
            owner_type: None,
            parent_name: None,
            body_contains: None,
        }),
        max_depth: Some(2),
        max_paths: Some(128),
    };

    let result = CodeItemCallPath::execute(
        guarded_params,
        fixture.ctx("axum-free-function-guarded-call-path"),
    )
    .await
    .expect("guarded tool execution");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize guarded result");
    assert_eq!(
        payload.get("guard_id").and_then(serde_json::Value::as_str),
        Some(fixture.intermediate.to_string().as_str())
    );
    assert_eq!(
        payload.get("guarded").and_then(serde_json::Value::as_bool),
        Some(true),
        "known intermediate should guard every returned path: {payload:#?}"
    );
    let paths = payload
        .get("paths")
        .and_then(serde_json::Value::as_array)
        .expect("guarded paths array");
    assert_two_hop_call_path(
        paths,
        fixture.start,
        fixture.intermediate,
        fixture.target,
        "guarded code_item_call_path paths",
    );
    assert!(
        payload
            .get("violations")
            .and_then(serde_json::Value::as_array)
            .is_some_and(Vec::is_empty),
        "known intermediate should not produce violating paths: {payload:#?}"
    );
    let ui = result.ui_payload.as_ref().expect("guarded ui payload");
    assert_eq!(ui_field(ui, "guard_id"), fixture.intermediate.to_string());
    assert_eq!(ui_field(ui, "guarded"), "true");
    assert_eq!(ui_field(ui, "violations"), "0");

    // Real unrelated node for the missing-guard case:
    //   axum-macros/src/from_request/mod.rs:230 defines
    //   `parse_single_generic_type_on_struct`, which is not on the
    //   from_request::expand -> extract_fields path.
    let unguarded_params = CodeItemCallPathParams {
        source: base_source,
        target: base_target,
        guard: Some(CodeItemCallPathEndpoint {
            item_name: Cow::Borrowed("parse_single_generic_type_on_struct"),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed("function"),
            module_path: Cow::Owned(fixture.module_path_arg()),
            owner_trait: None,
            owner_type: None,
            parent_name: None,
            body_contains: None,
        }),
        max_depth: Some(2),
        max_paths: Some(128),
    };
    let result = CodeItemCallPath::execute(
        unguarded_params,
        fixture.ctx("axum-free-function-unguarded-call-path"),
    )
    .await
    .expect("unguarded tool execution");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize unguarded result");
    assert_eq!(
        payload.get("guarded").and_then(serde_json::Value::as_bool),
        Some(false),
        "unrelated guard should fail the path policy: {payload:#?}"
    );
    let paths = payload
        .get("paths")
        .and_then(serde_json::Value::as_array)
        .expect("unguarded paths array");
    let violations = payload
        .get("violations")
        .and_then(serde_json::Value::as_array)
        .expect("violations array");
    assert_eq!(
        violations.len(),
        paths.len(),
        "every returned path should be reported as a violation when guard is absent: {payload:#?}"
    );
    let ui = result.ui_payload.as_ref().expect("unguarded ui payload");
    assert_eq!(ui_field(ui, "guarded"), "false");
    assert_eq!(ui_field(ui, "violations"), violations.len().to_string());
}

#[tokio::test]
async fn code_item_call_path_returns_real_corpus_free_function_two_hop_reachability() {
    let fixture = AxumFromRequestFreeFunctionPathToolFixture::new().await;
    let params = CodeItemCallPathParams {
        source: CodeItemCallPathEndpoint {
            item_name: Cow::Borrowed("expand"),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed("function"),
            module_path: Cow::Owned(fixture.module_path_arg()),
            owner_trait: None,
            owner_type: None,
            parent_name: None,
            body_contains: None,
        },
        target: CodeItemCallPathEndpoint {
            item_name: Cow::Borrowed("extract_fields"),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed("function"),
            module_path: Cow::Owned(fixture.module_path_arg()),
            owner_trait: None,
            owner_type: None,
            parent_name: None,
            body_contains: None,
        },
        guard: None,
        max_depth: Some(2),
        max_paths: Some(128),
    };

    let result = CodeItemCallPath::execute(
        params,
        fixture.ctx("axum-free-function-code-item-call-path"),
    )
    .await
    .expect("tool execution");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize CodeItemCallPathResult");
    assert_eq!(
        payload
            .get("reachable")
            .and_then(serde_json::Value::as_bool),
        Some(true),
        "code_item_call_path should report the real-corpus free-function two-hop chain as reachable: {payload:#?}"
    );
    let paths = payload
        .get("paths")
        .and_then(serde_json::Value::as_array)
        .expect("paths array");
    let source_files = payload
        .get("source_files")
        .and_then(serde_json::Value::as_array)
        .expect("source_files array");

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Source-oracle chain:
    //   axum-macros/src/from_request/mod.rs:145
    //     `from_request::expand` calls `impl_struct_by_extracting_each_field(...)`.
    //   axum-macros/src/from_request/mod.rs:342
    //     `impl_struct_by_extracting_each_field` calls `extract_fields(...)`.
    //   axum-macros/src/from_request/mod.rs:412
    //     defines `extract_fields`.
    // This proves the direct tool-call question for regular free functions:
    // "Can this known source helper reach this known target helper, and what
    // ordered call path connects them?"
    assert_two_hop_call_path(
        paths,
        fixture.start,
        fixture.intermediate,
        fixture.target,
        "free-function code_item_call_path paths",
    );
    let path = paths
        .iter()
        .find(|path| path.get("depth").and_then(serde_json::Value::as_u64) == Some(2))
        .unwrap_or_else(|| panic!("missing free-function two-hop path: {paths:#?}"));
    assert_edge_spans_present(path, "free-function code_item_call_path paths");
    assert_path_edge_proofs_present(
        &payload,
        path,
        "free-function code_item_call_path proof context",
    );
    let nodes = path
        .get("nodes")
        .and_then(serde_json::Value::as_array)
        .expect("path nodes");
    assert_call_path_node(
        nodes,
        fixture.start,
        "::expand",
        "axum-macros/src/from_request/mod.rs",
        "free-function code_item_call_path paths",
    );
    assert_call_path_node(
        nodes,
        fixture.intermediate,
        "::impl_struct_by_extracting_each_field",
        "axum-macros/src/from_request/mod.rs",
        "free-function code_item_call_path paths",
    );
    assert_call_path_node(
        nodes,
        fixture.target,
        "::extract_fields",
        "axum-macros/src/from_request/mod.rs",
        "free-function code_item_call_path paths",
    );
    assert_source_file(
        source_files,
        "axum-macros/src/from_request/mod.rs",
        "free-function code_item_call_path source files",
    );
}

#[tokio::test]
async fn code_item_call_path_explains_unreachable_dynamic_source_frontier() {
    let case = DynamicToolCase::AXUM
        .into_iter()
        .find(|case| case.method == "accept")
        .expect("TapIo::accept dynamic callable-field case");
    let fixture = DynamicToolFixture::new(case).await;
    let params = CodeItemCallPathParams {
        source: CodeItemCallPathEndpoint {
            item_name: Cow::Borrowed(case.method),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed("method"),
            module_path: Cow::Owned(fixture.module_path_arg()),
            owner_trait: None,
            owner_type: Some(Cow::Borrowed(case.owner_type)),
            parent_name: None,
            body_contains: None,
        },
        target: CodeItemCallPathEndpoint {
            item_name: Cow::Borrowed("handle_accept_error"),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed("function"),
            module_path: Cow::Owned(fixture.module_path_arg()),
            owner_trait: None,
            owner_type: None,
            parent_name: None,
            body_contains: None,
        },
        guard: None,
        max_depth: Some(2),
        max_paths: Some(16),
    };

    let result = CodeItemCallPath::execute(
        params,
        fixture.ctx("axum-dynamic-source-unreachable-call-path"),
    )
    .await
    .expect("tool execution");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize CodeItemCallPathResult");
    assert_eq!(
        payload
            .get("reachable")
            .and_then(serde_json::Value::as_bool),
        Some(false),
        "dynamic callable-field frontier should remain fail-closed for unrelated target: {payload:#?}"
    );
    assert!(
        payload
            .get("paths")
            .and_then(serde_json::Value::as_array)
            .is_some_and(Vec::is_empty),
        "unreachable dynamic source query should not synthesize a path: {payload:#?}"
    );

    // Matrix:
    //   docs/active/agents/call-graph/
    //   2026-06-28_real-corpus-call-site-oracle-matrices.md
    //
    // Source oracle:
    //   axum/src/serve/listener.rs:236
    //     `TapIo::accept` calls `(self.tap_fn)(&mut io)`.
    //
    // This proves the user-facing call-path surface does not collapse a
    // fail-closed runtime-dispatch frontier into an unexplained "no path".
    let source_context = payload
        .get("source_context")
        .and_then(serde_json::Value::as_object)
        .expect("source_context object");
    let runtime_needs = source_context
        .get("runtime_needs")
        .and_then(serde_json::Value::as_array)
        .expect("source_context.runtime_needs array");
    assert_runtime_dispatch_need_path(
        runtime_needs,
        &["self", "tap_fn"],
        "code_item_call_path source_context.runtime_needs",
    );

    let reach = source_context
        .get("reach")
        .and_then(serde_json::Value::as_object)
        .expect("source_context.reach object");
    assert!(
        reach
            .get("unsupported_frontier_calls")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|frontier| !frontier.is_empty()),
        "source_context.reach should preserve unsupported frontier calls: {reach:#?}"
    );

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(
        ui_field(ui, "source_runtime_needs"),
        runtime_needs.len().to_string()
    );
    assert!(
        ui_field(ui, "source_unsupported")
            .parse::<usize>()
            .expect("source unsupported frontier count")
            >= 1,
        "ui payload should surface unsupported source frontier count"
    );
}

fn assert_edge_spans_present(path: &serde_json::Value, label: &str) {
    let edges = path
        .get("edges")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| panic!("{label} should include path edges: {path:#?}"));
    assert_eq!(
        edges.len(),
        2,
        "{label} should expose both call edges: {path:#?}"
    );
    for edge in edges {
        let span = edge
            .get("span")
            .and_then(serde_json::Value::as_array)
            .unwrap_or_else(|| panic!("{label} edge should include a span pair: {edge:#?}"));
        assert_eq!(span.len(), 2, "{label} edge span should have two bounds");
        assert!(
            span.iter().all(|value| value.as_u64().is_some()),
            "{label} edge span should serialize numeric byte bounds: {edge:#?}"
        );
    }
}

fn assert_runtime_dispatch_need_path(
    needs: &[serde_json::Value],
    expected_path: &[&str],
    label: &str,
) {
    let need = needs
        .iter()
        .find(|need| {
            need.get("call_site")
                .and_then(|call| call.get("path"))
                .and_then(serde_json::Value::as_array)
                .is_some_and(|path| {
                    path.iter()
                        .filter_map(serde_json::Value::as_str)
                        .eq(expected_path.iter().copied())
                })
        })
        .unwrap_or_else(|| {
            panic!(
                "{label} should include a runtime-dispatch need for {expected_path:?}: {needs:#?}"
            )
        });
    let blockers = need
        .get("blocker_reasons")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| panic!("{label} blocker_reasons array"));
    assert!(
        blockers
            .iter()
            .any(|reason| reason.as_str() == Some("dynamic_dispatch_unbounded")),
        "{label} should preserve dynamic_dispatch_unbounded: {need:#?}"
    );
}

fn assert_path_edge_proofs_present(
    payload: &serde_json::Value,
    path: &serde_json::Value,
    label: &str,
) {
    let proof_context = payload
        .get("proof_context")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| panic!("{label} should include proof_context: {payload:#?}"));
    let edges = path
        .get("edges")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| panic!("{label} should include path edges: {path:#?}"));
    for edge in edges {
        let site = edge
            .get("call_site_id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_else(|| panic!("{label} edge should include call_site_id: {edge:#?}"));
        assert!(
            proof_context.iter().any(|proof| {
                proof.get("kind").and_then(serde_json::Value::as_str) == Some("call_edge")
                    && proof
                        .get("call_site_id")
                        .and_then(serde_json::Value::as_str)
                        == Some(site)
            }),
            "{label} should include a call_edge proof row for path callsite {site}: {proof_context:#?}"
        );
    }
}
