//! 2606.02461 — AGENTCL gain metrics.

/// Plasticity gain `F_i - B_i`.
pub fn plasticity_gain(first: f64, baseline: f64) -> Option<f64> {
    finite_diff(first, baseline)
}

/// Stability gain `S_i - F_i`.
pub fn stability_gain(second: f64, first: f64) -> Option<f64> {
    finite_diff(second, first)
}

/// Generalization gain `H_j - B_j`.
pub fn generalization_gain(held: f64, baseline: f64) -> Option<f64> {
    finite_diff(held, baseline)
}

fn finite_diff(after: f64, before: f64) -> Option<f64> {
    if after.is_finite() && before.is_finite() {
        Some(after - before)
    } else {
        None
    }
}
