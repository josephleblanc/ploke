//! 2606.00611 — TRACE trajectory risk compression scoring helpers.

use crate::common::numeric::sigmoid;

/// Unsafe probability from a finite logit.
pub fn unsafe_probability(logit: f64) -> Option<f64> {
    if logit.is_finite() {
        Some(sigmoid(logit))
    } else {
        None
    }
}

/// Binary cross-entropy loss for a predicted probability and binary label.
pub fn bce_loss(prob: f64, label: bool) -> Option<f64> {
    if !prob.is_finite() || !(0.0..=1.0).contains(&prob) {
        return None;
    }

    let eps = f64::EPSILON;
    let clipped = prob.clamp(eps, 1.0 - eps);
    let y = u8::from(label) as f64;
    Some(-(y * clipped.ln() + (1.0 - y) * (1.0 - clipped).ln()))
}
