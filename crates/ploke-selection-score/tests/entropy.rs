use ploke_selection_score::papers::entropy;

#[test]
fn scheduler_entropy_requires_probability_distribution() {
    let entropy =
        entropy::scheduler_entropy(&[0.5, 0.25, 0.25]).expect("valid distribution should score");

    assert!((entropy - 1.5).abs() < 1e-12);
    assert!(entropy::scheduler_entropy(&[0.5, 0.25]).is_none());
    assert!(entropy::scheduler_entropy(&[0.5, -0.5, 1.0]).is_none());
}

#[test]
fn entropy_rate_sums_task_pressure_and_context_drift() {
    assert_eq!(entropy::entropy_rate(-0.2, 0.5), Some(0.3));
    assert!(entropy::entropy_rate(f64::NAN, 0.5).is_none());
}

#[test]
fn context_drift_uses_beta_over_time_plus_one() {
    assert_eq!(entropy::context_drift(6.0, 2.0), Some(2.0));
    assert!(entropy::context_drift(6.0, -1.0).is_none());
}

#[test]
fn entropy_path_matches_closed_form_terms() {
    let path = entropy::entropy_path(0.0, 2.0, 0.5, 3.0, std::f64::consts::FRAC_PI_2, 7.0, 11.0)
        .expect("finite nonnegative time should score");

    assert!((path - 13.0).abs() < 1e-12);
    assert!(entropy::entropy_path(-1.0, 2.0, 0.5, 3.0, 0.0, 7.0, 11.0).is_none());
}
