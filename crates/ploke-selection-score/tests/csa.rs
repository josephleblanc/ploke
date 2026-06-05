use ploke_selection_score::papers::csa::{self, Label};

#[test]
fn correctness_probe_thresholds_cover_any_and_majority_labels() {
    assert_eq!(
        csa::label_from_correctness(&[false, false, true, false, false], 1),
        Some(Label::SelfSolve)
    );
    assert_eq!(
        csa::label_from_correctness(&[true, false, true, false, false], 3),
        Some(Label::Delegate)
    );
    assert_eq!(
        csa::label_from_correctness(&[true, false, true, true, false], 3),
        Some(Label::SelfSolve)
    );
    assert!(csa::label_from_correctness(&[], 1).is_none());
    assert!(csa::label_from_correctness(&[true], 0).is_none());
}

#[test]
fn sft_loss_sums_negative_log_likelihoods() {
    let loss = csa::sft_loss(&[-0.2, -1.3, 0.0]).expect("valid log likelihoods should score");

    assert!((loss - 1.5).abs() < 1e-12);
    assert!(csa::sft_loss(&[-0.2, 0.1]).is_none());
    assert!(csa::sft_loss(&[]).is_none());
}

#[test]
fn policy_ratio_rejects_invalid_probabilities() {
    assert_eq!(csa::policy_ratio(0.6, 0.3), Some(2.0));
    assert!(csa::policy_ratio(0.6, 0.0).is_none());
    assert!(csa::policy_ratio(1.2, 0.3).is_none());
}

#[test]
fn standardized_advantages_use_group_mean_and_std() {
    let advantage = csa::standardized_advantages(&[1.0, -1.0, 1.0, -1.0])
        .expect("mixed rewards should have variance");

    assert_eq!(advantage, vec![1.0, -1.0, 1.0, -1.0]);
    assert!(csa::standardized_advantages(&[1.0, 1.0, 1.0]).is_none());
}

#[test]
fn clipped_surrogate_matches_grpo_min_rule_for_positive_and_negative_advantage() {
    let positive =
        csa::clipped_surrogate(1.5, 2.0, 0.2).expect("valid positive-advantage term should score");
    let negative =
        csa::clipped_surrogate(1.5, -2.0, 0.2).expect("valid negative-advantage term should score");

    assert!((positive - 2.4).abs() < 1e-12);
    assert!((negative - -3.0).abs() < 1e-12);
    assert!(csa::clipped_surrogate(1.0, 1.0, 0.0).is_none());
}

#[test]
fn grpo_group_loss_averages_clipped_terms_and_adds_kl_penalty() {
    let loss = csa::grpo_group_loss(&[1.5, 0.5], &[2.0, -2.0], 0.2, 0.3, 0.1)
        .expect("valid group should score");

    assert!((loss - -0.37).abs() < 1e-12);
    assert!(csa::grpo_group_loss(&[1.0], &[1.0, -1.0], 0.2, 0.0, 0.0).is_none());
    assert!(csa::grpo_group_loss(&[1.0], &[1.0], 0.2, -0.1, 0.0).is_none());
}
