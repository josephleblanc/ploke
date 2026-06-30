use std::borrow::Cow;

use ploke_tui::tools::{
    Tool,
    code_item_call_path::{CodeItemCallPath, CodeItemCallPathEndpoint, CodeItemCallPathParams},
};

use crate::call_graph_tool_support::{
    AxumRequestExtractPathToolFixture, assert_call_path_node, assert_two_hop_call_path, ui_field,
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

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(ui_field(ui, "reachable"), "true");
    assert_eq!(ui_field(ui, "paths"), paths.len().to_string());
    assert_eq!(ui_field(ui, "max_depth"), "2");
}
