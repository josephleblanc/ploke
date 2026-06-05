//! 2606.02536 — Behavioral trait-vector diff scoring.

use crate::common::{ScoreError, ranking, vectors};

/// Normalized before/after embedding difference.
pub fn normalized_diff(after: &[f64], before: &[f64]) -> Result<Vec<f64>, ScoreError> {
    if after.len() != before.len() {
        return Err(ScoreError::LengthMismatch);
    }
    if after.is_empty() {
        return Err(ScoreError::Empty);
    }

    let diff: Vec<f64> = after
        .iter()
        .zip(before.iter())
        .map(|(a, b)| a - b)
        .collect();
    vectors::normalize(&diff)
}

/// Linear trait score `d_hat dot w + b`.
pub fn trait_score(diff: &[f64], weight: &[f64], bias: f64) -> Result<f64, ScoreError> {
    if !bias.is_finite() {
        return Err(ScoreError::NonFinite);
    }

    Ok(vectors::dot(diff, weight)? + bias)
}

/// Directional sign accuracy.
pub fn sign_accuracy(predicted: &[f64], target: &[f64]) -> Result<f64, ScoreError> {
    if predicted.len() != target.len() {
        return Err(ScoreError::LengthMismatch);
    }
    if predicted.is_empty() {
        return Err(ScoreError::Empty);
    }
    if predicted
        .iter()
        .chain(target.iter())
        .any(|item| !item.is_finite())
    {
        return Err(ScoreError::NonFinite);
    }

    let hits = predicted
        .iter()
        .zip(target.iter())
        .filter(|(pred, gold)| pred.signum() == gold.signum())
        .count();
    Ok(hits as f64 / predicted.len() as f64)
}

/// Spearman rank correlation `rho` between predicted and target trait scores.
///
/// The paper reports Spearman correlation as a validation metric for the linear
/// trait-vector score. This helper computes Pearson correlation over average
/// ranks, so ties receive the same midpoint policy as `rank_quality`.
pub fn spearman_rho(predicted: &[f64], target: &[f64]) -> Result<f64, ScoreError> {
    if predicted.len() != target.len() {
        return Err(ScoreError::LengthMismatch);
    }
    if predicted.is_empty() {
        return Err(ScoreError::Empty);
    }

    let predicted_rank = ranking::rank_quality(predicted)?;
    let target_rank = ranking::rank_quality(target)?;
    pearson(&predicted_rank, &target_rank)
}

fn pearson(left: &[f64], right: &[f64]) -> Result<f64, ScoreError> {
    let left_mean = left.iter().sum::<f64>() / left.len() as f64;
    let right_mean = right.iter().sum::<f64>() / right.len() as f64;

    let mut covariance = 0.0;
    let mut left_var = 0.0;
    let mut right_var = 0.0;
    for (left, right) in left.iter().zip(right.iter()) {
        let left_diff = left - left_mean;
        let right_diff = right - right_mean;
        covariance += left_diff * right_diff;
        left_var += left_diff * left_diff;
        right_var += right_diff * right_diff;
    }

    let denom = left_var.sqrt() * right_var.sqrt();
    if denom == 0.0 {
        return Err(ScoreError::ZeroTotal);
    }

    Ok(covariance / denom)
}
