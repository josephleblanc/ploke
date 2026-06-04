//! 2606.01351 — Entropy dynamics orchestration helpers.

/// Shannon entropy over a scheduler distribution using log base 2.
pub fn scheduler_entropy(prob: &[f64]) -> Option<f64> {
    if prob.is_empty()
        || prob
            .iter()
            .any(|item| !item.is_finite() || *item < 0.0 || *item > 1.0)
    {
        return None;
    }

    Some(
        prob.iter()
            .filter(|item| **item > 0.0)
            .map(|item| -item * item.log2())
            .sum(),
    )
}

/// Context drift term `beta / (t + 1)`.
pub fn context_drift(beta: f64, time: f64) -> Option<f64> {
    if !beta.is_finite() || !time.is_finite() || time <= -1.0 {
        None
    } else {
        Some(beta / (time + 1.0))
    }
}

/// Closed-form entropy trajectory from the formal note.
pub fn entropy_path(
    time: f64,
    amp: f64,
    gamma: f64,
    omega: f64,
    phase: f64,
    beta: f64,
    init: f64,
) -> Option<f64> {
    if [time, amp, gamma, omega, phase, beta, init]
        .iter()
        .any(|item| !item.is_finite())
        || time <= -1.0
    {
        return None;
    }

    Some(
        amp * (-gamma * time).exp() * (omega * time + phase).sin()
            + beta * (time + 1.0).ln()
            + init,
    )
}
