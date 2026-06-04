//! Experimental scoring mechanisms for Ploke selection policies.
//!
//! This crate keeps the mechanisms small and pure so they can be replayed against
//! candidate-history snapshots before any selector is promoted into Ploke proper.

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

/// Frontier sampler parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrontierConfig {
    pub top_m: usize,
    pub lambda: f64,
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

/// Candidate evidence route with predicted benefit and cost.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Route {
    pub benefit: f64,
    pub cost: f64,
}

/// Errors from scoring helpers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoreError {
    Empty,
    LengthMismatch,
    NonFinite,
}

/// Returns true when every required lane has ready evidence.
pub fn evidence_gate(req: &[Lane], set: &BTreeMap<Lane, Evidence>) -> bool {
    req.iter()
        .all(|lane| set.get(lane).is_some_and(|ev| ev.is_ready()))
}

/// Converts arbitrary finite scalar scores into rank percentiles in `[0, 1]`.
///
/// Higher input scores receive higher quality. Ties receive their average rank.
pub fn rank_quality(score: &[f64]) -> Result<Vec<f64>, ScoreError> {
    if score.is_empty() {
        return Err(ScoreError::Empty);
    }
    if score.iter().any(|value| !value.is_finite()) {
        return Err(ScoreError::NonFinite);
    }
    if score.len() == 1 {
        return Ok(vec![0.5]);
    }

    let mut pair: Vec<(usize, f64)> = score.iter().copied().enumerate().collect();
    pair.sort_by(|left, right| left.1.total_cmp(&right.1));

    let denom = (score.len() - 1) as f64;
    let mut out = vec![0.0; score.len()];
    let mut start = 0;
    while start < pair.len() {
        let value = pair[start].1;
        let mut end = start + 1;
        while end < pair.len() && pair[end].1 == value {
            end += 1;
        }

        let avg = ((start + end - 1) as f64) / 2.0;
        let rank = avg / denom;
        for idx in start..end {
            out[pair[idx].0] = rank;
        }
        start = end;
    }

    Ok(out)
}

/// Computes DGM-H / Ploke-style unnormalized frontier sampling weights.
pub fn frontier_weights(
    qual: &[f64],
    child: &[usize],
    cfg: FrontierConfig,
) -> Result<Vec<f64>, ScoreError> {
    if qual.is_empty() {
        return Err(ScoreError::Empty);
    }
    if qual.len() != child.len() {
        return Err(ScoreError::LengthMismatch);
    }
    if !cfg.lambda.is_finite() || qual.iter().any(|value| !value.is_finite()) {
        return Err(ScoreError::NonFinite);
    }

    let count = cfg.top_m.clamp(1, qual.len());
    let mut top = qual.to_vec();
    top.sort_by(|left, right| right.total_cmp(left));
    let mid = top.iter().take(count).sum::<f64>() / count as f64;

    Ok(qual
        .iter()
        .zip(child.iter())
        .map(|(quality, kids)| sigmoid(cfg.lambda * (quality - mid)) / (1.0 + *kids as f64))
        .collect())
}

/// Normalizes nonnegative finite weights or returns a uniform fallback.
pub fn normalize_weights(weight: &[f64]) -> Vec<f64> {
    if weight.is_empty() {
        return Vec::new();
    }
    let bad = weight
        .iter()
        .any(|value| !value.is_finite() || *value < 0.0);
    let sum = weight.iter().sum::<f64>();
    if bad || !sum.is_finite() || sum <= 0.0 {
        let prob = 1.0 / weight.len() as f64;
        return vec![prob; weight.len()];
    }

    weight.iter().map(|value| value / sum).collect()
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

/// Chooses the route maximizing `benefit - lambda * cost`.
pub fn choose_route(route: &[Route], lambda: f64) -> Option<usize> {
    if route.is_empty() || !lambda.is_finite() {
        return None;
    }
    if route
        .iter()
        .any(|item| !item.benefit.is_finite() || !item.cost.is_finite())
    {
        return None;
    }

    route
        .iter()
        .enumerate()
        .max_by(|left, right| route_score(left.1, lambda).total_cmp(&route_score(right.1, lambda)))
        .map(|(idx, _)| idx)
}

fn route_score(route: &Route, lambda: f64) -> f64 {
    route.benefit - lambda * route.cost
}

fn sigmoid(value: f64) -> f64 {
    if value >= 0.0 {
        1.0 / (1.0 + (-value).exp())
    } else {
        let exp = value.exp();
        exp / (1.0 + exp)
    }
}
