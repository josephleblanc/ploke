use super::super::*;

#[test]
fn fixture_projection_stores_real_special_form_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let generic_fn = function_id_by_name(&db, "generic_identity")?;
    let generic_method =
        method_id_by_impl_self_type_name(&db, "GenericMethodTarget", "generic_instance")?;
    let unsafe_fn = function_id_by_name(&db, "unsafe_target")?;

    let cases = [
        ResolvedProofCase {
            label: "call_generic_identity_turbofish",
            owner: function_id_by_name(&db, "call_generic_identity_turbofish")?,
            rows: 1,
            calls: vec![ResolvedProofCall::path(
                &["generic_identity"],
                generic_fn,
                CallRelationKind::Function,
                CallTargetKind::Function,
            )],
        },
        ResolvedProofCase {
            label: "call_method_turbofish",
            owner: function_id_by_name(&db, "call_method_turbofish")?,
            rows: 1,
            calls: vec![ResolvedProofCall::method(
                "generic_instance",
                CallReceiver::TypedLocalBinding {
                    name: "value".to_string(),
                    type_path: path(&["GenericMethodTarget"]),
                },
                generic_method,
            )],
        },
        ResolvedProofCase {
            label: "call_unsafe_function",
            owner: function_id_by_name(&db, "call_unsafe_function")?,
            rows: 1,
            calls: vec![ResolvedProofCall::path(
                &["unsafe_target"],
                unsafe_fn,
                CallRelationKind::Function,
                CallTargetKind::Function,
            )],
        },
    ];

    assert_fixture_resolved_proofs(&db, "special form", &cases)?;

    Ok(())
}
