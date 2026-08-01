//! Low-level scalar helpers.

/// True when every value is finite.
pub fn all_finite(value: &[f64]) -> bool {
    value.iter().all(|item| item.is_finite())
}

/// Numerically stable sigmoid.
pub(crate) fn sigmoid(value: f64) -> f64 {
    if value >= 0.0 {
        1.0 / (1.0 + (-value).exp())
    } else {
        let exp = value.exp();
        exp / (1.0 + exp)
    }
}
