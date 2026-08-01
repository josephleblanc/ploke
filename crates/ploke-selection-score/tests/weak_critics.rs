use ploke_selection_score::papers::weak_critics;

#[test]
fn retain_example_requires_outcome_and_rubric_filters() {
    assert!(weak_critics::retain_example(true, true));
    assert!(!weak_critics::retain_example(true, false));
    assert!(!weak_critics::retain_example(false, true));
}

#[test]
fn kl_divergence_matches_token_distribution_formula() {
    let kl = weak_critics::kl_divergence(&[0.5, 0.5], &[0.25, 0.75])
        .expect("valid distributions should score");
    let expected = 0.5_f64 * (2.0_f64).ln() + 0.5_f64 * (2.0_f64 / 3.0_f64).ln();

    assert!((kl - expected).abs() < 1e-12);
}

#[test]
fn kl_divergence_ignores_zero_student_mass_but_rejects_missing_teacher_support() {
    assert_eq!(
        weak_critics::kl_divergence(&[0.0, 1.0], &[0.0, 1.0]),
        Some(0.0)
    );
    assert!(weak_critics::kl_divergence(&[0.5, 0.5], &[0.0, 1.0]).is_none());
    assert!(weak_critics::kl_divergence(&[0.5, 0.6], &[0.5, 0.5]).is_none());
}

#[test]
fn opcd_loss_sums_tokens_per_retained_example_and_divides_by_retained_set_size() {
    let loss = weak_critics::opcd_loss(&[vec![0.1, 0.2], vec![0.3]])
        .expect("valid retained examples should score");

    assert!((loss - 0.3).abs() < 1e-12);
    assert!(weak_critics::opcd_loss(&[]).is_none());
    assert!(weak_critics::opcd_loss(&[vec![]]).is_none());
    assert!(weak_critics::opcd_loss(&[vec![f64::NAN]]).is_none());
}
