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
    base.is_finite()
        && base >= 0.0
        && part.iter().all(|item| item.is_finite() && *item >= 0.0)
        && part.iter().sum::<f64>() <= base
}

/// Cost-partitioned heuristic sum.
pub fn cp_heuristic(value: &[f64]) -> Option<f64> {
    if value.iter().any(|item| !item.is_finite()) {
        None
    } else {
        Some(value.iter().sum())
    }
}

/// Maximum single-change formula from abstract transition deltas.
///
/// Computes `sup_<s,l,s'> h*(c,s) - h*(c,s')` for one label after the caller has
/// materialized the relevant abstract-transition differences. The source's
/// side condition `h*(c,s) < infinity` is represented here by rejecting
/// non-finite deltas.
pub fn maximum_single_change(delta: &[f64]) -> Option<f64> {
    if delta.is_empty() || delta.iter().any(|item| !item.is_finite()) {
        return None;
    }

    Some(delta.iter().copied().fold(f64::NEG_INFINITY, f64::max))
}

/// Applies one SCP remaining-cost update `rem_i = rem_{i-1} - cost_i`.
///
/// Finite remaining costs require finite, nonnegative allocations no larger
/// than the current remainder. `+infinity` remaining costs are sticky as stated
/// in the paper: if `rem_{i-1}(label) = infinity`, then `rem_i(label)` remains
/// `infinity` regardless of the finite allocation supplied for that label.
pub fn remaining_after_saturation(remaining: &[f64], allocated: &[f64]) -> Option<Vec<f64>> {
    if remaining.len() != allocated.len() || remaining.is_empty() {
        return None;
    }

    let mut next = Vec::with_capacity(remaining.len());
    for (remaining, allocated) in remaining.iter().zip(allocated.iter()) {
        if !allocated.is_finite() || *allocated < 0.0 {
            return None;
        }
        if *remaining == f64::INFINITY {
            next.push(f64::INFINITY);
        } else if remaining.is_finite() && *remaining >= 0.0 && *allocated <= *remaining {
            next.push(remaining - allocated);
        } else {
            return None;
        }
    }

    Some(next)
}

/// Checks one saturated cost allocation against remaining costs.
///
/// This is the vector form of `cost_i = saturate(h_i, rem_{i-1})` after the
/// caller has already computed the saturated costs, for example via MSCF.
pub fn valid_saturated_allocation(remaining: &[f64], allocated: &[f64]) -> bool {
    remaining_after_saturation(remaining, allocated).is_some()
}
