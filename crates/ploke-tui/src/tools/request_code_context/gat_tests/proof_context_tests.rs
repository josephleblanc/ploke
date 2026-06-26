use super::*;

use super::helpers::{assert_result_ok, execute_fixture_tool_request, ui_field};
use ploke_core::rag_types::RequestCodeContextResult;
use ploke_db::Database;
use ploke_test_utils::setup_db_full_multi_embedding;
use std::sync::Arc;

#[tokio::test]
async fn request_code_context_preserves_ambiguous_target_candidate_proof_context()
-> color_eyre::Result<()> {
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = one_uuid(&db, &function_in_module_query(&["crate"], "other_target"))?;
    let sibling = one_uuid(&db, &function_in_module_query(&["crate"], "local_target"))?;
    let owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_if_ambiguous_function_item"),
    )?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "ambiguous owner context: {context:#?}");
    let site_id = context[0].site.id.to_string();
    assert_eq!(
        db.project_call_proof_facts_for_target(target, "bd:fixture-call-graph")?,
        4,
        "two ambiguous dynamic callers should each project call_site and call_resolution facts"
    );

    let tool_result =
        execute_fixture_tool_request(&db, "other_target", 1, "ambiguous_projected_proof_context")
            .await?;
    let result: RequestCodeContextResult = serde_json::from_str(&tool_result.content)?;
    assert_result_ok(&result, "other_target", 1, "fixture_call_graph");
    assert!(
        result
            .note
            .as_deref()
            .is_none_or(|note| { !note.contains("Proof-context expansion is unavailable") }),
        "projected target proof facts should avoid degraded proof-context note: {result:#?}"
    );

    let site_rows = result
        .context
        .iter()
        .flat_map(|part| part.proof_context.iter())
        .filter(|row| row.call_site_id.as_deref() == Some(site_id.as_str()))
        .collect::<Vec<_>>();
    let owner_id = owner.to_string();
    assert!(
        site_rows.iter().any(|row| {
            row.kind == "call_site" && row.caller_def_id.as_deref() == Some(owner_id.as_str())
        }),
        "tool proof context should include the ambiguous caller call_site fact: {site_rows:#?}"
    );
    let resolution = site_rows
        .iter()
        .find(|row| {
            row.kind == "call_resolution" && row.resolution_state.as_deref() == Some("ambiguous")
        })
        .unwrap_or_else(|| {
            panic!(
                "tool proof context should include the ambiguous call_resolution fact: {site_rows:#?}"
            )
        });
    let mut actual = resolution.candidate_def_ids.clone();
    actual.sort();
    let mut expected = vec![target.to_string(), sibling.to_string()];
    expected.sort();
    assert_eq!(
        actual, expected,
        "request_code_context proof payload should preserve all ambiguous sibling candidates"
    );

    let payload = tool_result
        .ui_payload
        .as_ref()
        .expect("request_code_context should emit a UI payload");
    let proof_context_count = result
        .context
        .iter()
        .map(|part| part.proof_context.len())
        .sum::<usize>();
    assert_eq!(
        ui_field(payload, "proof_context"),
        proof_context_count.to_string()
    );

    Ok(())
}
