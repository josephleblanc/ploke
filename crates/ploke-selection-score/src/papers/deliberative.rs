//! 2606.00007 — Deliberative Curation scoring fragments.

/// Beta reputation expectation `alpha / (alpha + beta)`.
pub fn beta_reputation(alpha: f64, beta: f64) -> Option<f64> {
    let total = alpha + beta;
    if !alpha.is_finite() || !beta.is_finite() || total <= 0.0 {
        None
    } else {
        Some(alpha / total)
    }
}

/// Exponential count decay used by the reputation sketch.
pub fn decayed_count(count: f64, delta: f64, elapsed: f64) -> Option<f64> {
    if !count.is_finite() || !delta.is_finite() || !elapsed.is_finite() {
        None
    } else {
        Some(count * (-delta * elapsed).exp())
    }
}

/// Effective voting weight `gamma * reputation + (1 - gamma) * trust`.
pub fn voting_weight(reputation: f64, trust: f64, gamma: f64) -> Option<f64> {
    if !reputation.is_finite() || !trust.is_finite() || !gamma.is_finite() {
        None
    } else {
        Some(gamma * reputation + (1.0 - gamma) * trust)
    }
}

/// Fast-track admission result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FastTrack {
    Active,
    EscalateTier2,
}

/// Fast-track admission predicate from the formal note.
pub fn fast_track(has_tier_objection: bool) -> FastTrack {
    if has_tier_objection {
        FastTrack::EscalateTier2
    } else {
        FastTrack::Active
    }
}
