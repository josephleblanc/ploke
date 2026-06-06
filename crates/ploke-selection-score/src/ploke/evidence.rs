//! Evidence lanes, hard gates, and evaluator calibration gates.

use std::collections::BTreeMap;

/// Evidence lanes that can be observed, gated, or made selector-driving by a profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Lane {
    Operational,
    Protocol,
    Oracle,
    Imp,
    Cost,
    Handoff,
    Trace,
    History,
}

/// Minimal comparability evidence for a lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Evidence {
    pub present: bool,
    pub matched: bool,
    pub valid: bool,
    pub replayable: bool,
}

impl Evidence {
    /// Evidence that is present, matched to the candidate, valid, and replayable.
    pub const fn ready() -> Self {
        Self {
            present: true,
            matched: true,
            valid: true,
            replayable: true,
        }
    }

    /// True when a required lane is comparable.
    pub const fn is_ready(self) -> bool {
        self.present && self.matched && self.valid && self.replayable
    }
}

/// Evaluator calibration evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvalEvidence {
    pub success: u64,
    pub failure: u64,
    pub prior_success: u64,
    pub prior_failure: u64,
    pub versioned: bool,
    pub replayable: bool,
    pub calibrated: bool,
}

/// Returns true when every required lane has ready evidence.
pub fn evidence_gate(req: &[Lane], set: &BTreeMap<Lane, Evidence>) -> bool {
    req.iter()
        .all(|lane| set.get(lane).is_some_and(|ev| ev.is_ready()))
}

/// Beta-style evaluator reliability estimate.
pub fn reliability(ev: &EvalEvidence) -> f64 {
    let good = ev.success + ev.prior_success;
    let total = good + ev.failure + ev.prior_failure;
    if total == 0 {
        0.0
    } else {
        good as f64 / total as f64
    }
}

/// True when evaluator evidence is reliable enough and has authority metadata.
pub fn eval_gate(ev: &EvalEvidence, min: f64) -> bool {
    min.is_finite() && ev.versioned && ev.replayable && ev.calibrated && reliability(ev) >= min
}
