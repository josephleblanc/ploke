use std::borrow::Cow;

use ploke_tui::tools::{
    Tool,
    code_item_call_path::{CodeItemCallPath, CodeItemCallPathEndpoint, CodeItemCallPathParams},
};

use crate::call_graph_tool_support::{
    AxumFromRequestFreeFunctionPathToolFixture, AxumRequestExtractPathToolFixture,
    assert_call_path_node, assert_two_hop_call_path, ui_field,
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
        },
        target: CodeItemCallPathEndpoint {
            item_name: Cow::Borrowed("from_request"),
            file_path: Cow::Owned(fixture.target_file_path.display().to_string()),
            node_kind: Cow::Borrowed("method"),
            module_path: Cow::Owned(fixture.target_module_path_arg()),
            owner_trait: Some(Cow::Borrowed("FromRequest")),
            owner_type: None,
        },
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
        },
        target: CodeItemCallPathEndpoint {
            item_name: Cow::Borrowed("extract_fields"),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed("function"),
            module_path: Cow::Owned(fixture.module_path_arg()),
            owner_trait: None,
            owner_type: None,
        },
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
