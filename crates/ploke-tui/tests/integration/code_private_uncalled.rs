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
    let entrypoint = payload
        .entrypoint_summaries
        .iter()
        .find(|entrypoint| entrypoint.node_id.to_string() == target_id)
        .unwrap_or_else(|| {
            panic!("private uncalled payload should include the generated test-harness summary: {payload:#?}")
        });
    assert!(
        entrypoint.proof_context.iter().any(|row| {
            row.kind == "entrypoint_summary"
                && row.definition_id.as_deref() == Some(target_id.as_str())
                && row.target_kind.as_deref() == Some("test")
                && row.target_name.as_deref() == Some("generated-test-harness")
                && row.summary_class.as_deref() == Some("analyzed_source")
                && row.status.as_deref() == Some("admitted")
        }),
        "entrypoint summary should preserve the generated test-harness proof row: {entrypoint:#?}"
    );

    assert!(payload.total >= payload.returned);
    assert_eq!(payload.truncated, payload.total > payload.returned);

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(ui_field(ui, "returned"), payload.returned.to_string());
    assert_eq!(ui_field(ui, "truncated"), payload.truncated.to_string());
    assert_eq!(
        ui_field(ui, "entrypoint_summaries"),
        payload.entrypoint_summaries.len().to_string()
    );
}
