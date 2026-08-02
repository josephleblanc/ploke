use super::super::assertions::{assert_expansion, assert_resolved_target, path};
use super::super::*;

#[tokio::test]
async fn request_code_context_returns_raw_identifier_call_context() -> color_eyre::Result<()> {
    struct Case {
        label: &'static str,
        search_term: &'static str,
        call_id: &'static str,
        owner: Uuid,
        target: Uuid,
        call_kind: CallSiteKind,
        callee: CallCalleeInfo,
        relation: CallTargetKind,
    }

    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let raw_function_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_raw_identifier_function"),
    )?;
    let raw_method_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_raw_identifier_method"),
    )?;
    let raw_function_target = single_target_for_owner(&db, raw_function_owner)?;
    let raw_method_target = single_target_for_owner(&db, raw_method_owner)?;
    let cases = [
        Case {
            label: "raw identifier function",
            search_term: "call_raw_identifier_function",
            call_id: "raw_identifier_function_call_context",
            owner: raw_function_owner,
            target: raw_function_target,
            call_kind: CallSiteKind::Path,
            callee: CallCalleeInfo::Path {
                path: path(&["r#match"]),
            },
            relation: CallTargetKind::Function,
        },
        Case {
            label: "raw identifier method",
            search_term: "call_raw_identifier_method",
            call_id: "raw_identifier_method_call_context",
            owner: raw_method_owner,
            target: raw_method_target,
            call_kind: CallSiteKind::Method,
            callee: CallCalleeInfo::Method {
                name: "r#type".to_string(),
                receiver: Some(CallReceiverInfo::TypedLocalBinding {
                    name: "value".to_string(),
                    type_path: path(&["RawMethodTarget"]),
                }),
            },
            relation: CallTargetKind::Method,
        },
    ];

    for case in cases {
        let result = execute_fixture_request(&db, case.search_term, 1, case.call_id).await?;
        assert_result_ok(&result, case.search_term, 1, "fixture_call_graph");

        let owner_part = result
            .context
            .iter()
            .find(|part| part.id == case.owner)
            .unwrap_or_else(|| {
                panic!(
                    "request_code_context should materialize the {} owner",
                    case.label
                )
            });
        let target_part = result
            .context
            .iter()
            .find(|part| part.id == case.target)
            .unwrap_or_else(|| {
                panic!(
                    "request_code_context should materialize the {} target",
                    case.label
                )
            });
        let call = owner_part
            .call_context
            .iter()
            .find(|call| {
                call.kind == case.call_kind
                    && call.callee == case.callee
                    && call
                        .targets
                        .iter()
                        .any(|target_info| target_info.target_id == case.target)
            })
            .unwrap_or_else(|| {
                panic!(
                    "{} owner should retain outgoing raw identifier call context",
                    case.label
                )
            });
        assert_resolved_target(call, case.target, case.relation);
        assert_expansion(
            target_part,
            case.owner,
            case.target,
            call.site_id,
            CallExpansionKind::OutgoingTarget,
        );
    }

    Ok(())
}

fn single_target_for_owner(db: &Database, owner: Uuid) -> color_eyre::Result<Uuid> {
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "raw identifier owner should have one DB context row: {context:#?}"
    );
    assert_eq!(
        context[0].targets.len(),
        1,
        "raw identifier owner should have one DB target: {context:#?}"
    );
    Ok(context[0].targets[0].target_id)
}
