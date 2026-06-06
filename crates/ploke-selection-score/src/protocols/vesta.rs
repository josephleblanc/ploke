//! 2606.00384 — VESTA metric-directed model selection.

use crate::common::selection::{argmax_finite, argmin_finite};

/// VESTA metric direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Metric {
    Aic,
    Jsd,
    ElpdLoo,
}

/// Selects the model index according to the metric direction.
pub fn choose_model(metric: Metric, score: &[f64]) -> Option<usize> {
    match metric {
        Metric::Aic | Metric::Jsd => argmin_finite(score),
        Metric::ElpdLoo => argmax_finite(score),
    }
}
