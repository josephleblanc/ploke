use ploke_selection_score::{ScoreError, papers::eva};

#[test]
fn anchor_probs_softmaxes_exactly_five_anchor_logits() {
    let prob = eva::anchor_probs(&[0.0, 0.0, 0.0, 0.0, 0.0], 1.0)
        .expect("five finite anchor logits should score");

    assert_eq!(prob, vec![0.2; 5]);
    assert!(eva::anchor_probs(&[0.0, 0.0], 1.0).is_err());
    assert!(eva::anchor_probs(&[0.0; 5], 0.0).is_err());
}

#[test]
fn expected_score_requires_anchor_probability_distribution() {
    let score = eva::expected_score(&[0.1, 0.2, 0.3, 0.2, 0.2]).expect("valid anchor distribution");

    assert!((score - 3.2).abs() < 1e-12);
    assert_eq!(
        eva::expected_score(&[0.5, 0.5]),
        Err(ScoreError::LengthMismatch)
    );
    assert_eq!(
        eva::expected_score(&[0.2, 0.2, 0.2, 0.2, 0.3]),
        Err(ScoreError::ZeroTotal)
    );
    assert_eq!(
        eva::expected_score(&[0.2, 0.2, 0.2, 0.2, -0.2]),
        Err(ScoreError::NonFinite)
    );
}

#[test]
fn eva_loss_is_mean_squared_error_over_expected_scores() {
    let loss = eva::eva_loss(&[3.2, 4.0], &[3.0, 5.0]).expect("finite score pairs should score");

    assert!((loss - 0.52).abs() < 1e-12);
}

#[test]
fn total_loss_rejects_negative_eva_weight() {
    assert_eq!(eva::total_loss(1.0, 0.5, 2.0), Some(2.0));
    assert!(eva::total_loss(1.0, 0.5, -1.0).is_none());
}
