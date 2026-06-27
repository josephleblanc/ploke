use super::super::assertions::{assert_expansion, assert_resolved_target, path};
use super::super::*;

#[tokio::test]
async fn request_code_context_returns_targetless_special_form_call_context()
-> color_eyre::Result<()> {
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let extern_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_extern_c_function"),
    )?;
    let chained_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_chained_returned_function"),
    )?;
    let chained_target = one_uuid(&db, &function_in_module_query(&["crate"], "make_unary_fn"))?;

    let extern_result =
        execute_fixture_request(&db, "call_extern_c_function", 1, "extern_c_call_context").await?;
    assert_result_ok(
        &extern_result,
        "call_extern_c_function",
        1,
        "fixture_call_graph",
    );
    let extern_part = extern_result
        .context
        .iter()
        .find(|part| part.id == extern_owner)
        .expect("request_code_context should materialize the extern C owner");
    assert_eq!(
        extern_part.call_context.len(),
        1,
        "extern C call context: {extern_part:#?}"
    );
    let extern_call = &extern_part.call_context[0];
    assert_eq!(extern_call.kind, CallSiteKind::Path);
    assert_eq!(
        extern_call.callee,
        CallCalleeInfo::Path {
            path: path(&["abs"]),
        }
    );
    assert_eq!(extern_call.status, CallStatusKind::External);
    assert!(extern_call.resolution.is_none());
    assert!(
        extern_call.targets.is_empty(),
        "extern C calls must not fabricate TUI targets: {extern_call:#?}"
    );

    let chained_result = execute_fixture_request(
        &db,
        "call_chained_returned_function",
        1,
        "chained_returned_call_context",
    )
    .await?;
    assert_result_ok(
        &chained_result,
        "call_chained_returned_function",
        1,
        "fixture_call_graph",
    );
    let owner_part = chained_result
        .context
        .iter()
        .find(|part| part.id == chained_owner)
        .expect("request_code_context should materialize the chained returned-function owner");
    let target_part = chained_result
        .context
        .iter()
        .find(|part| part.id == chained_target)
        .expect("request_code_context should materialize the chained returned-function target");
    assert_eq!(
        owner_part.call_context.len(),
        2,
        "chained returned-function call context: {owner_part:#?}"
    );
    let path_call = owner_part
        .call_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: path(&["make_unary_fn"]),
                    }
        })
        .expect("inner make_unary_fn path call should stay visible");
    assert_resolved_target(path_call, chained_target, CallTargetKind::Function);
    assert_expansion(
        target_part,
        chained_owner,
        chained_target,
        path_call.site_id,
        CallExpansionKind::OutgoingTarget,
    );

    let dynamic_call = owner_part
        .call_context
        .iter()
        .find(|call| call.kind == CallSiteKind::Dynamic)
        .expect("outer chained returned-function dynamic call should stay visible");
    assert_eq!(dynamic_call.callee, CallCalleeInfo::Dynamic);
    assert_eq!(dynamic_call.status, CallStatusKind::Unsupported);
    assert!(dynamic_call.resolution.is_none());
    assert!(
        dynamic_call.targets.is_empty(),
        "outer chained returned-function dynamic calls must not fabricate TUI targets: {dynamic_call:#?}"
    );

    Ok(())
}
