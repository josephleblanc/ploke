use ploke_tui::tools::{
    Tool as _,
    code_private_uncalled::{
        CodePrivateUncalled, CodePrivateUncalledResult, PrivateUncalledParams,
    },
};

use crate::call_graph_tool_support::{AxumErrorHandlingTraitsToolFixture, ui_field};

#[tokio::test]
async fn code_private_uncalled_lists_real_corpus_private_zero_caller_target() {
    let fixture = AxumErrorHandlingTraitsToolFixture::new().await;

    let result = CodePrivateUncalled::execute(
        PrivateUncalledParams {
            max_results: Some(512),
        },
        fixture.ctx("axum-private-uncalled"),
    )
    .await
    .expect("code_private_uncalled axum fixture");
    let payload: CodePrivateUncalledResult =
        serde_json::from_str(&result.content).expect("deserialize CodePrivateUncalledResult");

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Dead-code detection:
    //   "Which private helpers have no incoming callers?"
    //
    // Source oracle:
    //   axum/src/error_handling/mod.rs:257 defines private `#[test] fn traits()`.
    //   No checked-in axum source row calls `traits(...)`; generated test harness
    //   entrypoints are outside the persisted source call graph.
    let target_id = fixture.target.to_string();
    let target = payload
        .nodes
        .iter()
        .find(|node| node.id.to_string() == target_id)
        .unwrap_or_else(|| panic!("private uncalled nodes should include traits: {payload:#?}"));
    assert_eq!(target.name, "traits");
    assert_eq!(target.kind, "Function");
    assert_eq!(target.is_public, false);
    assert!(
        target
            .file_path
            .as_ref()
            .ends_with("axum/src/error_handling/mod.rs"),
        "private uncalled target should point at the real axum source file: {target:#?}"
    );

    assert!(payload.total >= payload.returned);
    assert_eq!(payload.truncated, payload.total > payload.returned);

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(ui_field(ui, "returned"), payload.returned.to_string());
    assert_eq!(ui_field(ui, "truncated"), payload.truncated.to_string());
}
