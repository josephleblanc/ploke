//! 2606.02470 — MCP-Persona benchmark.

/// MCP-Persona success predicate.
pub fn task_success(exec: bool, persona: bool, fidelity: bool) -> bool {
    exec && persona && fidelity
}

/// Success rate over personalized tool tasks.
pub fn success_rate(result: &[bool]) -> Option<f64> {
    if result.is_empty() {
        return None;
    }

    let hits = result.iter().filter(|item| **item).count();
    Some(hits as f64 / result.len() as f64)
}
