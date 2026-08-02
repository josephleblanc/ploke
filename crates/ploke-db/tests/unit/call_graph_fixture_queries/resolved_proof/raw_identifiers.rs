use super::super::*;

#[test]
fn fixture_projection_stores_real_raw_identifier_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let raw_function = function_id_by_exact_name(&db, "r#match")?;
    let raw_method = method_id_by_impl_self_type_exact_name(&db, "RawMethodTarget", "r#type")?;

    let cases = [
        ResolvedProofCase {
            label: "call_raw_identifier_function",
            owner: function_id_by_name(&db, "call_raw_identifier_function")?,
            rows: 1,
            calls: vec![ResolvedProofCall::path(
                &["r#match"],
                raw_function,
                CallRelationKind::Function,
                CallTargetKind::Function,
            )],
        },
        ResolvedProofCase {
            label: "call_raw_identifier_method",
            owner: function_id_by_name(&db, "call_raw_identifier_method")?,
            rows: 1,
            calls: vec![ResolvedProofCall::method(
                "r#type",
                CallReceiver::TypedLocalBinding {
                    name: "value".to_string(),
                    type_path: path(&["RawMethodTarget"]),
                },
                raw_method,
            )],
        },
    ];

    assert_fixture_resolved_proofs(&db, "raw identifier", &cases)?;

    Ok(())
}
