//! Ranking and percentile helpers.

use super::error::ScoreError;

/// Converts arbitrary finite scalar scores into rank percentiles in `[0, 1]`.
///
/// Higher input scores receive higher quality. Ties receive their average rank.
pub fn rank_quality(score: &[f64]) -> Result<Vec<f64>, ScoreError> {
    if score.is_empty() {
        return Err(ScoreError::Empty);
    }
    if score.iter().any(|value| !value.is_finite()) {
        return Err(ScoreError::NonFinite);
    }
    if score.len() == 1 {
        return Ok(vec![0.5]);
    }

    let mut pair: Vec<(usize, f64)> = score.iter().copied().enumerate().collect();
    pair.sort_by(|left, right| left.1.total_cmp(&right.1));

    let denom = (score.len() - 1) as f64;
    let mut out = vec![0.0; score.len()];
    let mut start = 0;
    while start < pair.len() {
        let value = pair[start].1;
        let mut end = start + 1;
        while end < pair.len() && pair[end].1 == value {
            end += 1;
        }

        let avg = ((start + end - 1) as f64) / 2.0;
        let rank = avg / denom;
        for idx in start..end {
            out[pair[idx].0] = rank;
        }
        start = end;
    }

    Ok(out)
}
