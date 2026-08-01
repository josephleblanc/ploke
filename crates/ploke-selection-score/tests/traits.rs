use ploke_selection_score::{ScoreError, papers::traits};

#[test]
fn spearman_rho_is_one_for_matching_rank_order() {
    let rho = traits::spearman_rho(&[0.2, 1.0, 0.5], &[10.0, 30.0, 20.0])
        .expect("matching finite ranks should correlate");

    assert!((rho - 1.0).abs() < 1e-12);
}

#[test]
fn spearman_rho_is_negative_one_for_reversed_rank_order() {
    let rho = traits::spearman_rho(&[0.2, 1.0, 0.5], &[30.0, 10.0, 20.0])
        .expect("reversed finite ranks should correlate");

    assert!((rho - -1.0).abs() < 1e-12);
}

#[test]
fn spearman_rho_uses_midpoint_ranks_for_ties() {
    let rho = traits::spearman_rho(&[1.0, 1.0, 3.0], &[2.0, 4.0, 6.0])
        .expect("ties should receive average ranks");

    assert!((rho - 0.8660254037844387).abs() < 1e-12);
}

#[test]
fn spearman_rho_rejects_length_mismatch_and_zero_variance() {
    assert_eq!(
        traits::spearman_rho(&[1.0], &[1.0, 2.0]),
        Err(ScoreError::LengthMismatch)
    );
    assert_eq!(
        traits::spearman_rho(&[1.0, 1.0], &[1.0, 2.0]),
        Err(ScoreError::ZeroTotal)
    );
}
