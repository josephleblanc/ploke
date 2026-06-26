use super::*;

#[tokio::test]
async fn request_code_context_surfaces_degraded_proof_context_note() -> color_eyre::Result<()> {
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));

    let result = execute_fixture_request(&db, "local_target", 1, "proof_context_degraded").await?;
    assert_result_ok(&result, "local_target", 1, "fixture_call_graph");

    let note = result
        .note
        .as_deref()
        .expect("call-graph fixture without proof facts should surface proof-context degradation");
    assert!(
        note.contains("Proof-context expansion is unavailable"),
        "unexpected request_code_context note: {note}"
    );
    assert!(
        result
            .next_steps
            .iter()
            .any(|step| step.contains("Project proof facts")),
        "proof-context degradation should include proof projection recovery steps: {:#?}",
        result.next_steps
    );

    Ok(())
}
