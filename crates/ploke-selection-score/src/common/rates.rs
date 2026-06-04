//! Shared rate and count helpers.

/// Returns `part / total`, preserving an empty denominator as `None`.
pub fn rate(part: usize, total: usize) -> Option<f64> {
    if total == 0 {
        None
    } else {
        Some(part as f64 / total as f64)
    }
}

/// Precision for accepted positives over all accepted items.
pub fn precision(correct: usize, accepted: usize) -> Option<f64> {
    rate(correct, accepted)
}

/// Recall for accepted positives over all true positives.
pub fn recall(found: usize, relevant: usize) -> Option<f64> {
    rate(found, relevant)
}
