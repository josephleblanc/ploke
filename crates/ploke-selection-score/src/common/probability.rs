//! Probability normalization helpers.

/// Normalizes nonnegative finite weights or returns a uniform fallback.
pub fn normalize_weights(weight: &[f64]) -> Vec<f64> {
    if weight.is_empty() {
        return Vec::new();
    }
    let bad = weight
        .iter()
        .any(|value| !value.is_finite() || *value < 0.0);
    let sum = weight.iter().sum::<f64>();
    if bad || !sum.is_finite() || sum <= 0.0 {
        let prob = 1.0 / weight.len() as f64;
        return vec![prob; weight.len()];
    }

    weight.iter().map(|value| value / sum).collect()
}
