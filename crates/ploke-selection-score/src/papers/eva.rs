//! 2606.01160 — Expected Value Alignment helpers.

use crate::common::ScoreError;

/// Softmax over anchor logits with a finite positive temperature.
pub fn anchor_probs(logit: &[f64], temp: f64) -> Result<Vec<f64>, ScoreError> {
    if logit.is_empty() {
        return Err(ScoreError::Empty);
    }
    if !temp.is_finite() || temp <= 0.0 || logit.iter().any(|item| !item.is_finite()) {
        return Err(ScoreError::NonFinite);
    }

    let max = logit.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let exp: Vec<f64> = logit
        .iter()
        .map(|item| ((item - max) / temp).exp())
        .collect();
    let total = exp.iter().sum::<f64>();
    if total <= 0.0 || !total.is_finite() {
        return Err(ScoreError::ZeroTotal);
    }

    Ok(exp.iter().map(|item| item / total).collect())
}

/// Expected score over one-indexed anchor probabilities.
pub fn expected_score(prob: &[f64]) -> Result<f64, ScoreError> {
    if prob.is_empty() {
        return Err(ScoreError::Empty);
    }
    if prob.iter().any(|item| !item.is_finite()) {
        return Err(ScoreError::NonFinite);
    }

    Ok(prob
        .iter()
        .enumerate()
        .map(|(idx, item)| (idx + 1) as f64 * item)
        .sum())
}

/// Mean squared EVA loss.
pub fn eva_loss(predicted: &[f64], target: &[f64]) -> Result<f64, ScoreError> {
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

    let sum = predicted
        .iter()
        .zip(target.iter())
        .map(|(pred, gold)| {
            let diff = pred - gold;
            diff * diff
        })
        .sum::<f64>();
    Ok(sum / predicted.len() as f64)
}

/// Total objective `SFT + alpha * EVA`.
pub fn total_loss(sft: f64, eva: f64, alpha: f64) -> Option<f64> {
    if !sft.is_finite() || !eva.is_finite() || !alpha.is_finite() {
        None
    } else {
        Some(sft + alpha * eva)
    }
}
