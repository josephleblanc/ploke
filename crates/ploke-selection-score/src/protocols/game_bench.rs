//! 2606.00103 — Interactive executable-game benchmark.

/// Episode status from the formal decision logic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    FormatError,
    Success,
    Failure,
    Continue,
    Timeout,
}

/// Status transition over a single action class.
pub fn status(valid: bool, submit: bool, correct: bool, turn: usize, max_turns: usize) -> Status {
    if !valid {
        Status::FormatError
    } else if submit && correct {
        Status::Success
    } else if submit {
        Status::Failure
    } else if turn < max_turns {
        Status::Continue
    } else {
        Status::Timeout
    }
}

/// Success rate over episode statuses.
pub fn success_rate(status: &[Status]) -> Option<f64> {
    if status.is_empty() {
        return None;
    }

    let hits = status
        .iter()
        .filter(|item| **item == Status::Success)
        .count();
    Some(hits as f64 / status.len() as f64)
}

/// Average turn count over successful episodes.
///
/// Computes `1 / |S| * sum_{e in S} N_e`, where `S` is the set of successful
/// episodes. The two slices must describe the same episode set.
pub fn average_success_turns(status: &[Status], turns: &[usize]) -> Option<f64> {
    if status.len() != turns.len() || status.is_empty() {
        return None;
    }

    let mut total = 0usize;
    let mut successes = 0usize;
    for (status, turns) in status.iter().zip(turns.iter()) {
        if *status == Status::Success {
            total += *turns;
            successes += 1;
        }
    }

    if successes == 0 {
        None
    } else {
        Some(total as f64 / successes as f64)
    }
}

/// Efficiency as success rate divided by average successful turns.
pub fn efficiency(success_rate: f64, avg_turns: f64) -> Option<f64> {
    if !success_rate.is_finite() || !avg_turns.is_finite() || avg_turns <= 0.0 {
        None
    } else {
        Some(success_rate / avg_turns)
    }
}
