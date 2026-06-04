//! 2606.02536 — Behavioral trait-vector diff scoring.

use crate::common::{ScoreError, vectors};

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
