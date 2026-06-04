//! 2606.02438 — Planning cost and cost-partition helpers.

/// Plan cost as the sum of action costs.
pub fn plan_cost(cost: &[f64]) -> Option<f64> {
    if cost.iter().any(|item| !item.is_finite()) {
        None
    } else {
        Some(cost.iter().sum())
    }
}

/// Checks saturated cost partition admissibility for one label.
pub fn valid_partition(part: &[f64], base: f64) -> bool {
    base.is_finite() && part.iter().all(|item| item.is_finite()) && part.iter().sum::<f64>() <= base
}

/// Cost-partitioned heuristic sum.
pub fn cp_heuristic(value: &[f64]) -> Option<f64> {
    if value.iter().any(|item| !item.is_finite()) {
        None
    } else {
        Some(value.iter().sum())
    }
}
