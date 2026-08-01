use ploke_selection_score::catalog::{Exactness, MechanismKind, registry};

#[test]
fn nearby_admission_candidates_are_cataloged_as_unresolved_until_primary_sources_are_audited() {
    let skilldag = registry::by_id("2606.03056-skilldag-typed-skill-graph")
        .expect("SkillDAG candidate admission should be cataloged");
    assert_eq!(skilldag.paper_id, Some("2606.03056"));
    assert_eq!(skilldag.kind, MechanismKind::Unresolved);
    assert_eq!(skilldag.exactness, Exactness::NotFormalizable);
    assert_eq!(skilldag.source[0].path, registry::NEARBY_MECHANISM_LEDGER);
    assert_eq!(skilldag.source[0].section_key, "2606.03056");

    let deltamem = registry::by_id("2606.03083-deltamem-residual-tree-write-rule")
        .expect("DELTAMEM candidate residual write rule should be cataloged");
    assert_eq!(deltamem.paper_id, Some("2606.03083"));
    assert_eq!(deltamem.kind, MechanismKind::Unresolved);
    assert_eq!(deltamem.exactness, Exactness::NotFormalizable);
    assert_eq!(deltamem.source[0].path, registry::NEARBY_MECHANISM_LEDGER);
    assert_eq!(deltamem.source[0].section_key, "2606.03083");

    let primitives = registry::by_id("2606.02994-reasoning-primitive-induction")
        .expect("reasoning primitive induction candidate should be cataloged");
    assert_eq!(primitives.paper_id, Some("2606.02994"));
    assert_eq!(primitives.kind, MechanismKind::Unresolved);
    assert_eq!(primitives.exactness, Exactness::NotFormalizable);
    assert_eq!(primitives.source[0].path, registry::NEARBY_MECHANISM_LEDGER);
    assert_eq!(primitives.source[0].section_key, "2606.02994");
}

#[test]
fn nearby_metric_and_span_candidates_remain_metadata_only_not_executable_helpers() {
    let stepfinder = registry::by_id("2606.03467-stepfinder-step-attribution")
        .expect("StepFinder span attribution candidate should be cataloged");
    assert_eq!(stepfinder.kind, MechanismKind::Unresolved);
    assert_eq!(stepfinder.exactness, Exactness::NotFormalizable);
    assert_eq!(stepfinder.source[0].path, registry::NEARBY_MECHANISM_LEDGER);

    let handoff = registry::by_id("2606.02875-handoff-debt-rediscovery-cost")
        .expect("Handoff Debt rediscovery-cost candidate should be cataloged");
    assert_eq!(handoff.kind, MechanismKind::Unresolved);
    assert_eq!(handoff.exactness, Exactness::NotFormalizable);
    assert_eq!(handoff.source[0].path, registry::NEARBY_MECHANISM_LEDGER);
}

#[test]
fn nearby_candidate_catalog_entries_keep_stable_paper_keys() {
    for id in [
        "2606.03056-skilldag-typed-skill-graph",
        "2606.03083-deltamem-residual-tree-write-rule",
        "2606.03467-stepfinder-step-attribution",
        "2606.02875-handoff-debt-rediscovery-cost",
        "2606.02994-reasoning-primitive-induction",
    ] {
        let spec = registry::by_id(id).expect("nearby candidate should be present");
        assert_eq!(spec.paper_id, Some(spec.source[0].section_key));
    }
}
