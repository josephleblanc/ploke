use super::*;

#[test]
fn proof_graph_store_accepts_effect_policy_artifacts() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut records = proof_records();
    records.push(effect_policy_record());
    db.upsert_proof_fact_values(&records)
        .expect("import proof facts");

    let rows = db
        .proof_graphrag_context("effect-policy:def-launch")
        .expect("effect policy proof context");
    assert!(
        rows.iter().any(|row| {
            row.kind == "effect_policy"
                && row.fact_id == "effect-policy:def-launch"
                && row.build_domain_id.as_deref() == Some("bd:main")
                && row.definition_id.as_deref() == Some("def:launch")
                && row.proof_policy_version.as_deref() == Some("proof-policy-test")
                && row.review_method.as_deref() == Some("manual-review")
                && row.scope_of_validity.as_deref() == Some("def:launch under bd:main")
                && row.allowed_effects == vec!["ffi_boundary".to_string()]
                && row.invalidation_conditions.as_deref()
                    == Some("definition body or proof policy changes")
                && row.status.as_deref() == Some("admitted")
        }),
        "GraphRAG proof lookup should expose stored effect policy artifacts: {rows:#?}"
    );

    let definition_rows = db
        .proof_graphrag_context("def:launch")
        .expect("definition policy proof context");
    assert!(
        definition_rows
            .iter()
            .any(|row| row.kind == "effect_policy" && row.fact_id == "effect-policy:def-launch"),
        "definition lookup should include the stored effect policy artifact: {definition_rows:#?}"
    );
}

#[test]
fn proof_graph_store_rejects_effect_policy_without_allowed_effects() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut records = proof_records();
    let mut policy = effect_policy_record();
    policy
        .as_object_mut()
        .expect("effect policy object")
        .remove("allowed_effects");
    records.push(policy);

    let error = db
        .upsert_proof_fact_values(&records)
        .expect_err("effect_policy without allowed_effects should reject the whole batch");
    assert!(error.to_string().contains("allowed_effects"));
    assert!(
        db.proof_graphrag_context("")
            .expect("query graph")
            .is_empty()
    );
}
