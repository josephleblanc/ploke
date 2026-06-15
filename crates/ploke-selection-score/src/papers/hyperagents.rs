//! 2603.19461 — HyperAgents / DGM-H archive parent-selection helpers.
//!
//! Source authority: `/home/brasides/wiki/queries/ploke/selection-scoring/archive-parent-selection-mechanisms.md`
//! and the local extracted HyperAgents text at
//! `/home/brasides/wiki/queries/research-deep-dives/_paper_extracts/round2-trackb/2603.19461.txt`.
//!
//! The Appendix A.2 score-child-prop selector is source-visible and represented
//! as faithful formulas. Appendix E.5 learned-selector pieces are intentionally
//! exposed as small replayable sketches, not as a claim that DGM-H discovered one
//! stable production algorithm.

use crate::common::{ScoreError, normalize_weights, numeric::sigmoid};

/// Frontier count reported for the DGM-H score-child-prop selector.
pub const DEFAULT_TOP_M: usize = 3;

/// Sigmoid sharpness reported for the DGM-H score-child-prop selector.
pub const DEFAULT_LAMBDA: f64 = 10.0;

/// Archive record fields used by the DGM-H parent selector.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParentStats {
    /// Empirical performance score `alpha_i = performance(a_i)`.
    pub performance: f64,
    /// Number of children from this parent that successfully compiled.
    pub compiled_children: usize,
}

/// Parameters for DGM-H / score-child-prop parent selection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParentSelectionConfig {
    /// Number of top archive agents used for the moving frontier midpoint.
    pub top_m: usize,
    /// Sigmoid sharpness in `sigma(lambda * (alpha_i - alpha_mid))`.
    pub lambda: f64,
}

impl Default for ParentSelectionConfig {
    fn default() -> Self {
        Self {
            top_m: DEFAULT_TOP_M,
            lambda: DEFAULT_LAMBDA,
        }
    }
}

/// Staged domain score for the DGM-H multi-domain average.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StagedDomainScore {
    /// Whether the cheap preliminary gate was passed.
    pub gate_passed: bool,
    /// Full training/validation score. It is required only when the gate passed.
    pub full_score: Option<f64>,
}

/// Component form qualitatively reported for learned Appendix E.5 selectors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EvolvedSelectorComponents {
    pub normalized_score: f64,
    pub exploration_bonus: f64,
    pub diversity_bonus: f64,
    pub recency_bonus: f64,
    pub elite_bonus: f64,
}

/// Computes the DGM-H moving frontier midpoint over the top-`m` performances.
///
/// The paper reports `m = 3`; callers may vary `top_m` during replay. `top_m`
/// must be positive, and if it exceeds the archive length the full archive is
/// used. Higher performance is better.
pub fn frontier_midpoint(performance: &[f64], top_m: usize) -> Result<f64, ScoreError> {
    if performance.is_empty() || top_m == 0 {
        return Err(ScoreError::Empty);
    }
    if performance.iter().any(|score| !score.is_finite()) {
        return Err(ScoreError::NonFinite);
    }

    let count = top_m.min(performance.len());
    let mut top = performance.to_vec();
    top.sort_by(|left, right| right.total_cmp(left));
    Ok(top.iter().take(count).sum::<f64>() / count as f64)
}

/// Computes unnormalized DGM-H score-child-prop parent weights.
///
/// Implements:
///
/// `w_i = sigma(lambda * (alpha_i - alpha_mid)) * 1 / (1 + n_i)`
///
/// where `n_i` is the number of compiled children produced by archive agent
/// `a_i`.
pub fn parent_weights(
    archive: &[ParentStats],
    cfg: ParentSelectionConfig,
) -> Result<Vec<f64>, ScoreError> {
    if archive.is_empty() || cfg.top_m == 0 {
        return Err(ScoreError::Empty);
    }
    if !cfg.lambda.is_finite()
        || cfg.lambda <= 0.0
        || archive.iter().any(|item| !item.performance.is_finite())
    {
        return Err(ScoreError::NonFinite);
    }

    let performance: Vec<f64> = archive.iter().map(|item| item.performance).collect();
    let midpoint = frontier_midpoint(&performance, cfg.top_m)?;

    Ok(archive
        .iter()
        .map(|item| {
            let exploitation = sigmoid(cfg.lambda * (item.performance - midpoint));
            let exploration = 1.0 / (1.0 + item.compiled_children as f64);
            exploitation * exploration
        })
        .collect())
}

/// Normalizes DGM-H parent weights into the categorical draw distribution.
///
/// The source formula falls back to a uniform categorical distribution if total
/// weight is zero. `normalize_weights` implements that fallback for zero or
/// malformed weights, after this helper has already rejected malformed inputs.
pub fn parent_probabilities(
    archive: &[ParentStats],
    cfg: ParentSelectionConfig,
) -> Result<Vec<f64>, ScoreError> {
    let weights = parent_weights(archive, cfg)?;
    Ok(normalize_weights(&weights))
}

/// Hard archive-admission predicate from DGM-H Algorithm 1.
///
/// The child enters the archive only when it is valid/compiled. Invalid children
/// remain outside the archive rather than becoming low-scoring stepping stones.
pub fn archive_admission(valid_child: bool) -> bool {
    valid_child
}

/// Returns the staged score for one domain.
///
/// Failed preliminary gates receive zero without requiring a full score. Passed
/// gates require a finite full training/validation score.
pub fn staged_score(score: StagedDomainScore) -> Result<f64, ScoreError> {
    if !score.gate_passed {
        return Ok(0.0);
    }

    match score.full_score {
        Some(value) if value.is_finite() => Ok(value),
        _ => Err(ScoreError::NonFinite),
    }
}

/// Averages staged DGM-H performance across domains.
///
/// This represents `alpha_i = |D|^-1 sum_d \tilde q_d(a_i)`, where failed
/// preliminary gates contribute zero and passed gates use the full score.
pub fn cross_domain_average(domain: &[StagedDomainScore]) -> Result<f64, ScoreError> {
    if domain.is_empty() {
        return Err(ScoreError::Empty);
    }

    let mut total = 0.0;
    for item in domain {
        total += staged_score(*item)?;
    }
    Ok(total / domain.len() as f64)
}

/// Polyglot coding staged gate: expand from 10 initial tasks when success is above 40%.
pub fn polyglot_coding_gate(successes: usize, attempts: usize) -> Option<bool> {
    success_rate(successes, attempts).map(|rate| rate > 0.40)
}

/// Generic DGM-H staged gate used by paper-review and IMO grading: at least one success.
pub fn at_least_one_gate(successes: usize, attempts: usize) -> Option<bool> {
    if attempts == 0 || successes > attempts {
        None
    } else {
        Some(successes > 0)
    }
}

/// Robotics staged gate: at least one generated reward function yields nonzero performance.
pub fn any_nonzero_performance(score: &[f64]) -> Option<bool> {
    if score.is_empty() || score.iter().any(|value| !value.is_finite()) {
        return None;
    }

    Some(score.iter().any(|value| *value > 0.0))
}

/// Success rate for a preliminary staged-evaluation subset.
pub fn success_rate(successes: usize, attempts: usize) -> Option<f64> {
    if attempts == 0 || successes > attempts {
        None
    } else {
        Some(successes as f64 / attempts as f64)
    }
}

/// Temperature-controlled softmax sampling probabilities from Appendix E.5.
///
/// This helper implements only the source-visible `exp(scores / temperature)`
/// normalization. It does not sample or adapt the temperature.
pub fn softmax_probabilities(score: &[f64], temperature: f64) -> Result<Vec<f64>, ScoreError> {
    if score.is_empty() {
        return Err(ScoreError::Empty);
    }
    if !temperature.is_finite()
        || temperature <= 0.0
        || score.iter().any(|value| !value.is_finite())
    {
        return Err(ScoreError::NonFinite);
    }

    let max_score = score.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let exp: Vec<f64> = score
        .iter()
        .map(|value| ((value - max_score) / temperature).exp())
        .collect();
    let total = exp.iter().sum::<f64>();
    if !total.is_finite() || total <= 0.0 {
        return Err(ScoreError::ZeroTotal);
    }

    Ok(exp.iter().map(|value| value / total).collect())
}

/// UCB-style parent score reported in Appendix E.5.
///
/// Implements:
///
/// `normalized_score + exploration_weight * sqrt(log(total_children + 1) / (children + 1))`.
///
/// `normalized_score` is constrained to `[0, 1]` because the paper names it as a
/// normalized performance term.
pub fn ucb_score(
    normalized_score: f64,
    total_children: usize,
    children: usize,
    exploration_weight: f64,
) -> Option<f64> {
    if !unit_value(normalized_score) || !nonnegative(exploration_weight) {
        return None;
    }

    let numerator = (total_children as f64 + 1.0).ln();
    let denominator = children as f64 + 1.0;
    Some(normalized_score + exploration_weight * (numerator / denominator).sqrt())
}

/// Adaptive exploration-weight update reported for stagnation detection.
///
/// When score variance falls below the threshold, exploration is multiplied;
/// otherwise the existing weight is preserved.
pub fn adaptive_exploration_weight(
    current_weight: f64,
    score_variance: f64,
    variance_threshold: f64,
    multiplier: f64,
) -> Option<f64> {
    if !nonnegative(current_weight)
        || !nonnegative(score_variance)
        || !nonnegative(variance_threshold)
        || !nonnegative(multiplier)
    {
        return None;
    }

    if score_variance < variance_threshold {
        Some(current_weight * multiplier)
    } else {
        Some(current_weight)
    }
}

/// Multi-component selector score qualitatively reported in Appendix E.5.
///
/// This is an interpretive replay helper for the paper's representative learned
/// formula, not a default Ploke selector:
///
/// `(normalized_score + exploration_weight * exploration_bonus + diversity_bonus + recency_bonus) * elite_bonus`.
pub fn evolved_selector_score(
    component: EvolvedSelectorComponents,
    exploration_weight: f64,
) -> Option<f64> {
    if !unit_value(component.normalized_score)
        || !component.exploration_bonus.is_finite()
        || !component.diversity_bonus.is_finite()
        || !component.recency_bonus.is_finite()
        || !nonnegative(component.elite_bonus)
        || !nonnegative(exploration_weight)
    {
        return None;
    }

    Some(
        (component.normalized_score
            + exploration_weight * component.exploration_bonus
            + component.diversity_bonus
            + component.recency_bonus)
            * component.elite_bonus,
    )
}

fn unit_value(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

fn nonnegative(value: f64) -> bool {
    value.is_finite() && value >= 0.0
}
