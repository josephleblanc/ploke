use ploke_selection_score::{
    ScoreError,
    catalog::{Exactness, MechanismKind, registry},
    papers::hyperagents::{
        EvolvedSelectorComponents, ParentSelectionConfig, ParentStats, StagedDomainScore,
        adaptive_exploration_weight, archive_admission, at_least_one_gate, cross_domain_average,
        evolved_selector_score, parent_probabilities, parent_weights, polyglot_coding_gate,
        softmax_probabilities, ucb_score,
    },
};

#[test]
fn dgmh_parent_weights_match_frontier_midpoint_and_child_dampener() {
    let archive = [
        ParentStats {
            performance: 0.9,
            compiled_children: 0,
        },
        ParentStats {
            performance: 0.8,
            compiled_children: 3,
        },
        ParentStats {
            performance: 0.1,
            compiled_children: 0,
        },
    ];
    let cfg = ParentSelectionConfig {
        top_m: 2,
        lambda: 10.0,
    };

    let weights = parent_weights(&archive, cfg).expect("finite archive should score");
    let midpoint = (0.9 + 0.8) / 2.0;
    let expected_first = 1.0 / (1.0 + (-10.0_f64 * (0.9 - midpoint)).exp());
    let expected_second = (1.0 / (1.0 + (-10.0_f64 * (0.8 - midpoint)).exp())) / 4.0;

    assert!((weights[0] - expected_first).abs() < 1e-12);
    assert!((weights[1] - expected_second).abs() < 1e-12);
    assert!(weights[0] > weights[1]);
    assert!(weights[1] > weights[2]);
}

#[test]
fn dgmh_parent_probabilities_normalize_weights_and_reject_bad_inputs() {
    let archive = [
        ParentStats {
            performance: 0.8,
            compiled_children: 0,
        },
        ParentStats {
            performance: 0.8,
            compiled_children: 3,
        },
    ];
    let cfg = ParentSelectionConfig {
        top_m: 1,
        lambda: 10.0,
    };

    let prob = parent_probabilities(&archive, cfg).expect("finite archive should normalize");
    assert!((prob.iter().sum::<f64>() - 1.0).abs() < 1e-12);
    assert!((prob[0] / prob[1] - 4.0).abs() < 1e-12);

    let bad_archive = [ParentStats {
        performance: f64::NAN,
        compiled_children: 0,
    }];
    assert_eq!(
        parent_weights(&bad_archive, cfg),
        Err(ScoreError::NonFinite)
    );
    assert_eq!(
        parent_weights(
            &archive,
            ParentSelectionConfig {
                top_m: 0,
                lambda: 10.0
            }
        ),
        Err(ScoreError::Empty)
    );
}

#[test]
fn staged_evaluation_and_cross_domain_average_zero_failed_prechecks() {
    let domains = [
        StagedDomainScore {
            gate_passed: true,
            full_score: Some(0.6),
        },
        StagedDomainScore {
            gate_passed: false,
            full_score: None,
        },
        StagedDomainScore {
            gate_passed: true,
            full_score: Some(0.9),
        },
    ];

    let avg = cross_domain_average(&domains).expect("passed domains have finite scores");
    assert!((avg - 0.5).abs() < 1e-12);

    let missing = [StagedDomainScore {
        gate_passed: true,
        full_score: None,
    }];
    assert_eq!(cross_domain_average(&missing), Err(ScoreError::NonFinite));
    assert_eq!(cross_domain_average(&[]), Err(ScoreError::Empty));
}

#[test]
fn anchored_domain_gates_capture_reported_staged_eval_rules() {
    assert_eq!(polyglot_coding_gate(5, 10), Some(true));
    assert_eq!(polyglot_coding_gate(4, 10), Some(false));
    assert_eq!(polyglot_coding_gate(11, 10), None);

    assert_eq!(at_least_one_gate(1, 10), Some(true));
    assert_eq!(at_least_one_gate(0, 10), Some(false));
    assert_eq!(at_least_one_gate(1, 0), None);
}

#[test]
fn archive_admission_is_a_hard_validity_gate() {
    assert!(archive_admission(true));
    assert!(!archive_admission(false));
}

#[test]
fn appendix_e5_softmax_and_ucb_sketches_are_replayable() {
    let prob = softmax_probabilities(&[1.0, 2.0, 3.0], 1.0)
        .expect("finite scores and positive temperature should normalize");
    assert!((prob.iter().sum::<f64>() - 1.0).abs() < 1e-12);
    assert!(prob[2] > prob[1]);
    assert!(prob[1] > prob[0]);
    assert_eq!(
        softmax_probabilities(&[1.0], 0.0),
        Err(ScoreError::NonFinite)
    );

    let ucb = ucb_score(0.6, 8, 1, 0.5).expect("finite UCB inputs should score");
    let expected = 0.6 + 0.5 * (9.0_f64.ln() / 2.0).sqrt();
    assert!((ucb - expected).abs() < 1e-12);
    assert!(ucb_score(1.2, 8, 1, 0.5).is_none());
}

#[test]
fn appendix_e5_adaptive_and_component_scores_stay_explicit() {
    assert_eq!(
        adaptive_exploration_weight(1.0, 0.005, 0.01, 1.4),
        Some(1.4)
    );
    assert_eq!(adaptive_exploration_weight(1.0, 0.02, 0.01, 1.4), Some(1.0));

    let score = evolved_selector_score(
        EvolvedSelectorComponents {
            normalized_score: 0.5,
            exploration_bonus: 0.2,
            diversity_bonus: 0.1,
            recency_bonus: 0.05,
            elite_bonus: 1.5,
        },
        2.0,
    )
    .expect("finite component score should evaluate");
    assert!((score - 1.575).abs() < 1e-12);
}

#[test]
fn hyperagents_mechanisms_are_cataloged_against_archive_parent_note() {
    let parent = registry::by_id("2603.19461-dgmh-parent-selection")
        .expect("DGM-H parent selector should be cataloged");
    assert_eq!(parent.paper_id, Some("2603.19461"));
    assert_eq!(parent.kind, MechanismKind::Selector);
    assert_eq!(parent.exactness, Exactness::Faithful);
    assert_eq!(parent.source[0].path, registry::ARCHIVE_PARENT_NOTE);

    let learned = registry::by_id("2603.19461-dgmh-modifiable-parent-selection")
        .expect("DGM-H learned selector sketches should remain cataloged separately");
    assert_eq!(learned.kind, MechanismKind::Selector);
    assert_eq!(learned.exactness, Exactness::Interpretive);
    assert_eq!(learned.source[0].section_key, "2603.19461");
}
