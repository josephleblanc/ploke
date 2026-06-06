use ploke_selection_score::{
    catalog::{Exactness, MechanismKind, registry},
    common::{probability, ranking, selection},
    papers::{raser, sadn},
    ploke::{evidence, frontier},
};

#[test]
fn module_paths_expose_existing_scoring_helpers() {
    let rank = ranking::rank_quality(&[3.0, 1.0, 2.0]).expect("finite scores should rank");
    assert_eq!(rank, vec![1.0, 0.0, 0.5]);

    let prob = probability::normalize_weights(&[1.0, 3.0]);
    assert_eq!(prob, vec![0.25, 0.75]);

    let cfg = frontier::FrontierConfig {
        top_m: 1,
        lambda: 10.0,
    };
    let weight = frontier::frontier_weights(&[0.8, 0.8], &[0, 3], cfg)
        .expect("matching finite vectors should score");
    assert!((weight[0] / weight[1] - 4.0).abs() < 1e-12);

    let route = [
        raser::Route {
            benefit: 0.4,
            cost: 0.1,
        },
        raser::Route {
            benefit: 0.6,
            cost: 0.2,
        },
    ];
    assert_eq!(raser::choose_route(&route, 0.5), Some(1));

    let chosen = sadn::sequential_greedy(&[2], |_agent, _prefix, action| {
        [0.2, 0.7].get(action).copied()
    })
    .expect("finite SADN advantages should choose an action");
    assert_eq!(chosen, vec![1]);
}

#[test]
fn module_paths_expose_ploke_evidence_gate() {
    let req = [evidence::Lane::Operational, evidence::Lane::Oracle];
    let mut set = std::collections::BTreeMap::new();
    set.insert(evidence::Lane::Operational, evidence::Evidence::ready());

    assert!(!evidence::evidence_gate(&req, &set));

    set.insert(evidence::Lane::Oracle, evidence::Evidence::ready());

    assert!(evidence::evidence_gate(&req, &set));
}

#[test]
fn common_selection_tie_policy_keeps_earliest_index() {
    assert_eq!(selection::argmax_finite(&[1.0, 3.0, 3.0, 2.0]), Some(1));
    assert_eq!(selection::argmin_finite(&[1.0, -2.0, -2.0, 0.0]), Some(1));
}

#[test]
fn catalog_uses_stable_formal_note_keys() {
    let raser = registry::by_id("2606.02488-raser-route-argmax")
        .expect("RASER route argmax should be cataloged");
    assert_eq!(raser.paper_id, Some("2606.02488"));
    assert_eq!(raser.kind, MechanismKind::Selector);
    assert_eq!(raser.exactness, Exactness::Interpretive);
    assert_eq!(raser.source[0].path, registry::FORMAL_NOTE);
    assert_eq!(raser.source[0].section_key, "2606.02488");

    let sadn = registry::by_id("2510.23535-sadn-sequential-advantage")
        .expect("SADN sequential advantage algorithm should be cataloged");
    assert_eq!(sadn.paper_id, Some("2510.23535"));
    assert_eq!(sadn.kind, MechanismKind::ComponentSet);
    assert_eq!(sadn.exactness, Exactness::ScopeLimited);
    assert_eq!(sadn.source[0].path, registry::SADN_ALGORITHM_NOTE);
    assert_eq!(sadn.source[0].section_key, "2510.23535");

    let unresolved = registry::by_id("2606.01066-verifier-metrics-lead")
        .expect("candidate-only mechanisms should remain cataloged separately");
    assert_eq!(unresolved.kind, MechanismKind::Unresolved);
    assert_eq!(unresolved.exactness, Exactness::NotFormalizable);
    assert_eq!(unresolved.source[0].section_key, "2606.01066");

    for spec in registry::all() {
        if let Some(paper_id) = spec.paper_id {
            assert_eq!(spec.source[0].section_key, paper_id);
        }
    }
}
