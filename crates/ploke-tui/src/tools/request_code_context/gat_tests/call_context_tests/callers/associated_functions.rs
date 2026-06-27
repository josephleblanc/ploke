use super::super::assertions::{assert_incoming_expansion, assert_resolved_target, path};
use super::super::*;

#[tokio::test]
async fn request_code_context_returns_associated_function_target_callers_with_call_context()
-> color_eyre::Result<()> {
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = one_uuid(
        &db,
        &method_id_by_trait_name("LocalAssocFunctionTrait", "trait_make"),
    )?;
    let owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_trait_associated_function"),
    )?;

    let result =
        execute_fixture_request(&db, "233 trait_make", 1, "associated_function_call_context")
            .await?;
    assert_result_ok(&result, "233 trait_make", 1, "fixture_call_graph");

    let caller_part = result
        .context
        .iter()
        .find(|part| part.id == owner)
        .expect("request_code_context should materialize the associated-function caller owner");
    let expected_path = path(&["LocalAssocFunctionTrait", "trait_make"]);
    let call = caller_part
        .call_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: expected_path.clone(),
                    }
                && call
                    .targets
                    .iter()
                    .any(|target_info| target_info.target_id == target)
        })
        .expect("associated-function caller should retain outgoing context to the seed target");
    assert_resolved_target(call, target, CallTargetKind::AssociatedFunction);
    assert_incoming_expansion(caller_part, call, target);

    Ok(())
}

fn method_id_by_trait_name(trait_name: &str, method: &str) -> String {
    format!(
        r#"?[method_id] :=
            *method {{ id: method_id, name: "{method}", owner_id: trait_id @ 'NOW' }},
            *trait {{ id: trait_id, name: "{trait_name}" @ 'NOW' }}"#
    )
}
