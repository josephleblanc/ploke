//! 2606.02449 — HLL human-verification benchmark.

/// HLL success predicate.
pub fn hll_success(task_solved: bool, trace_valid: bool, barrier_crossed: bool) -> bool {
    task_solved && trace_valid && barrier_crossed
}

/// Pass rate over HLL predicate results.
pub fn pass_rate(result: &[bool]) -> Option<f64> {
    if result.is_empty() {
        return None;
    }

    let hits = result.iter().filter(|item| **item).count();
    Some(hits as f64 / result.len() as f64)
}
