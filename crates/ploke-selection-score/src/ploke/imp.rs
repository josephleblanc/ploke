//! IMP@K contribution helpers.

/// Applies IMP@K points when score-bearing; otherwise preserves the non-driving default.
pub fn imp_delta(score: f64, points: f64, enabled: bool) -> Option<f64> {
    if !score.is_finite() || !points.is_finite() {
        None
    } else if enabled {
        Some(score * points)
    } else {
        Some(0.0)
    }
}
