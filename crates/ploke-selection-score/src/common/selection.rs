//! Generic finite argmin/argmax helpers.

/// Index of the maximum finite score. Ties keep the earliest index.
pub fn argmax_finite(score: &[f64]) -> Option<usize> {
    if score.is_empty() || score.iter().any(|value| !value.is_finite()) {
        return None;
    }

    let mut best = 0;
    for idx in 1..score.len() {
        if score[idx] > score[best] {
            best = idx;
        }
    }
    Some(best)
}

/// Index of the minimum finite score. Ties keep the earliest index.
pub fn argmin_finite(score: &[f64]) -> Option<usize> {
    if score.is_empty() || score.iter().any(|value| !value.is_finite()) {
        return None;
    }

    let mut best = 0;
    for idx in 1..score.len() {
        if score[idx] < score[best] {
            best = idx;
        }
    }
    Some(best)
}
