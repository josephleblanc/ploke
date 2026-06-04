//! 2606.00671 — AXIOM trust-first evaluation helpers.

/// Trust score `1 - wrong / attempted`.
pub fn trust_score(wrong: usize, attempted: usize) -> Option<f64> {
    if attempted == 0 {
        None
    } else {
        Some(1.0 - wrong as f64 / attempted as f64)
    }
}

/// AXIOM pipeline outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Abstain,
    Verified,
}

/// Abstain-first routing predicate.
pub fn decide(can_route: bool, translated: bool, verified: bool) -> Outcome {
    if can_route && translated && verified {
        Outcome::Verified
    } else {
        Outcome::Abstain
    }
}
