//! HistoryScoreChildProp frontier sampling helpers.

use crate::common::{ScoreError, numeric::sigmoid};

/// Frontier sampler parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrontierConfig {
    pub top_m: usize,
    pub lambda: f64,
}

/// Computes DGM-H / Ploke-style unnormalized frontier sampling weights.
pub fn frontier_weights(
    qual: &[f64],
    child: &[usize],
    cfg: FrontierConfig,
) -> Result<Vec<f64>, ScoreError> {
    if qual.is_empty() {
        return Err(ScoreError::Empty);
    }
    if qual.len() != child.len() {
        return Err(ScoreError::LengthMismatch);
    }
    if !cfg.lambda.is_finite() || qual.iter().any(|value| !value.is_finite()) {
        return Err(ScoreError::NonFinite);
    }

    let count = cfg.top_m.clamp(1, qual.len());
    let mut top = qual.to_vec();
    top.sort_by(|left, right| right.total_cmp(left));
    let mid = top.iter().take(count).sum::<f64>() / count as f64;

    Ok(qual
        .iter()
        .zip(child.iter())
        .map(|(quality, kids)| sigmoid(cfg.lambda * (quality - mid)) / (1.0 + *kids as f64))
        .collect())
}
