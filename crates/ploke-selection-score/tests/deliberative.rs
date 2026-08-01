use ploke_selection_score::papers::deliberative;

#[test]
fn beta_reputation_and_decay_reject_invalid_count_domains() {
    assert_eq!(deliberative::beta_reputation(3.0, 1.0), Some(0.75));
    assert!(deliberative::beta_reputation(-1.0, 2.0).is_none());
    assert!(deliberative::beta_reputation(0.0, 0.0).is_none());

    let decayed = deliberative::decayed_count(10.0, 0.1, 2.0)
        .expect("positive count, decay, and elapsed time should decay");
    assert!((decayed - 10.0 * (-0.2_f64).exp()).abs() < 1e-12);
    assert!(deliberative::decayed_count(-1.0, 0.1, 2.0).is_none());
    assert!(deliberative::decayed_count(1.0, 0.0, 2.0).is_none());
    assert!(deliberative::decayed_count(1.0, 0.1, -2.0).is_none());
}

#[test]
fn voting_weight_rejects_gamma_outside_unit_interval() {
    let weight = deliberative::voting_weight(0.8, 0.2, 0.25).expect("valid gamma should score");
    assert!((weight - 0.35).abs() < 1e-12);
    assert!(deliberative::voting_weight(0.8, 0.2, -0.1).is_none());
    assert!(deliberative::voting_weight(0.8, 0.2, 1.1).is_none());
    assert!(deliberative::voting_weight(1.2, 0.2, 0.5).is_none());
    assert!(deliberative::voting_weight(0.8, -0.2, 0.5).is_none());
}

#[test]
fn bounded_voting_weight_applies_protocol_caps() {
    assert_eq!(
        deliberative::bounded_voting_weight(0.9, 0.9, 0.5, 0.1, 0.7),
        Some(0.7)
    );
    assert_eq!(
        deliberative::bounded_voting_weight(0.0, 0.0, 0.5, 0.1, 0.7),
        Some(0.1)
    );
    assert!(deliberative::bounded_voting_weight(0.4, 0.5, 0.5, 0.8, 0.7).is_none());
    assert!(deliberative::bounded_voting_weight(0.4, 0.5, 0.5, 0.0, 0.7).is_none());
    assert!(deliberative::bounded_voting_weight(1.4, 0.5, 0.5, 0.1, 0.7).is_none());
}

#[test]
fn tiered_voting_weight_fixes_newcomer_weight_and_gates_roles() {
    assert_eq!(
        deliberative::tiered_voting_weight(2, 3, 0.9, 0.9, 0.5, 0.1, 1.0),
        Some(0.1)
    );
    assert_eq!(
        deliberative::tiered_voting_weight(3, 3, 0.9, 0.7, 0.5, 0.1, 1.0),
        Some(0.8)
    );
    assert!(deliberative::tiered_voting_weight(3, 0, 0.9, 0.7, 0.5, 0.1, 1.0).is_none());

    assert_eq!(deliberative::can_review(2, 3, 0.9, 0.7), Some(false));
    assert_eq!(deliberative::can_review(3, 3, 0.9, 0.7), Some(true));
    assert_eq!(deliberative::can_review(3, 3, 0.6, 0.7), Some(false));
    assert_eq!(deliberative::can_dispute(0.8, 0.7), Some(true));
    assert_eq!(deliberative::can_dispute(0.6, 0.7), Some(false));
}

#[test]
fn local_trust_row_clips_negative_scores_and_normalizes_positive_mass() {
    let row = deliberative::normalize_local_trust_row(&[2.0, -5.0, 6.0])
        .expect("finite row with positive mass should normalize");

    assert_eq!(row, vec![0.25, 0.0, 0.75]);
    assert!((row.iter().sum::<f64>() - 1.0).abs() < 1e-12);
}

#[test]
fn local_trust_row_uses_uniform_prior_when_no_positive_interactions() {
    let row = deliberative::normalize_local_trust_row(&[-2.0, 0.0, -6.0])
        .expect("finite row with no positives should use source default");

    assert_eq!(row, vec![1.0 / 3.0; 3]);
}

#[test]
fn local_trust_row_rejects_finite_overflow_in_positive_mass() {
    assert!(deliberative::normalize_local_trust_row(&[f64::MAX, f64::MAX]).is_none());
}

#[test]
fn eigentrust_step_applies_transposed_propagation_and_seed_damping() {
    let matrix = vec![vec![0.0, 1.0], vec![0.25, 0.75]];
    let trust = [0.6, 0.4];
    let prior = [0.5, 0.5];

    let next = deliberative::eigentrust_step(&matrix, &trust, &prior, 0.2)
        .expect("stochastic inputs should produce one damped step");

    assert!((next[0] - 0.18).abs() < 1e-12);
    assert!((next[1] - 0.82).abs() < 1e-12);
    assert!((next.iter().sum::<f64>() - 1.0).abs() < 1e-12);
}

#[test]
fn eigentrust_iterate_starts_from_prior_and_runs_fixed_steps() {
    let matrix = vec![vec![0.0, 1.0], vec![0.25, 0.75]];
    let prior = [0.5, 0.5];

    let trust = deliberative::eigentrust_iterate(&matrix, &prior, 0.2, 2)
        .expect("stochastic inputs should iterate");

    assert!((trust[0] - 0.26).abs() < 1e-12);
    assert!((trust[1] - 0.74).abs() < 1e-12);
}

#[test]
fn eigentrust_converge_reports_residual_and_convergence_status() {
    let matrix = vec![vec![0.0, 1.0], vec![0.25, 0.75]];
    let prior = [0.5, 0.5];

    let report = deliberative::eigentrust_converge(&matrix, &prior, 0.2, 100, 1e-9)
        .expect("stochastic inputs should converge");

    assert!(report.converged);
    assert!(report.iterations <= 100);
    assert!(report.residual <= 1e-9);
    assert!((report.trust.iter().sum::<f64>() - 1.0).abs() < 1e-9);

    let exhausted = deliberative::eigentrust_converge(&matrix, &prior, 0.2, 1, 0.0)
        .expect("valid inputs should report exhaustion");
    assert!(!exhausted.converged);
    assert_eq!(exhausted.iterations, 1);
    assert!(exhausted.residual > 0.0);
}

#[test]
fn eigentrust_rejects_non_stochastic_inputs() {
    let matrix = vec![vec![0.0, 1.0], vec![0.25, 0.5]];
    let prior = [0.5, 0.5];

    assert!(deliberative::eigentrust_iterate(&matrix, &prior, 0.2, 1).is_none());
    assert!(deliberative::eigentrust_iterate(&[vec![1.0]], &[1.0], 0.0, 1).is_none());
    assert!(deliberative::eigentrust_iterate(&[vec![1.0]], &[1.0], 0.2, 0).is_none());
}

#[test]
fn simulation_metrics_match_quality_and_sanction_definitions() {
    let active = [true, true, false, true, false];
    let quality = [0.9, 0.2, 0.8, 0.7, 0.1];

    assert_eq!(
        deliberative::active_precision(&active, &quality, 0.7),
        Some(2.0 / 3.0)
    );
    assert_eq!(
        deliberative::active_recall(&active, &quality, 0.7),
        Some(2.0 / 3.0)
    );

    let honest_or_broken = [true, false, true, true, false];
    let sanction = [0.2, 1.0, 0.8, 1.2, 1.5];

    assert_eq!(
        deliberative::sanction_false_positive_rate(&honest_or_broken, &sanction, 1.0),
        Some(1.0 / 3.0)
    );
}
