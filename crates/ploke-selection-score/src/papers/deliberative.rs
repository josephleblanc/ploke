//! 2606.00007 — Deliberative Curation scoring fragments.

/// Beta reputation expectation `alpha / (alpha + beta)`.
pub fn beta_reputation(alpha: f64, beta: f64) -> Option<f64> {
    let total = alpha + beta;
    if !alpha.is_finite() || !beta.is_finite() || alpha < 0.0 || beta < 0.0 || total <= 0.0 {
        None
    } else {
        Some(alpha / total)
    }
}

/// Exponential count decay used by the reputation sketch.
pub fn decayed_count(count: f64, delta: f64, elapsed: f64) -> Option<f64> {
    if !count.is_finite()
        || count < 0.0
        || !delta.is_finite()
        || delta <= 0.0
        || !elapsed.is_finite()
        || elapsed < 0.0
    {
        None
    } else {
        Some(count * (-delta * elapsed).exp())
    }
}

/// Effective voting weight `gamma * reputation + (1 - gamma) * trust`.
pub fn voting_weight(reputation: f64, trust: f64, gamma: f64) -> Option<f64> {
    if !reputation.is_finite()
        || !trust.is_finite()
        || !unit_value(reputation)
        || !unit_value(trust)
        || !gamma.is_finite()
        || !(0.0..=1.0).contains(&gamma)
    {
        None
    } else {
        Some(gamma * reputation + (1.0 - gamma) * trust)
    }
}

/// Effective voting weight after applying the protocol's lower and upper caps.
///
/// The paper states the blend as `w_i = gamma r_i + (1 - gamma) t_i`, then
/// bounds the result by `w_min <= w_i <= w_max` so one identity cannot dominate
/// curation and newcomers retain minimal influence.
pub fn bounded_voting_weight(
    reputation: f64,
    trust: f64,
    gamma: f64,
    min_weight: f64,
    max_weight: f64,
) -> Option<f64> {
    if !min_weight.is_finite()
        || min_weight <= 0.0
        || !max_weight.is_finite()
        || max_weight <= 0.0
        || min_weight > max_weight
    {
        return None;
    }

    voting_weight(reputation, trust, gamma).map(|weight| weight.clamp(min_weight, max_weight))
}

/// Tier-aware voting weight for the paper's newcomer cold-start rule.
///
/// Tier 0 agents with `interaction_count < newcomer_threshold` receive exactly
/// `w_min`. Established agents use the bounded reputation/EigenTrust blend.
/// Review and dispute privileges are exposed separately by `can_review` and
/// `can_dispute` so callers do not confuse scalar influence with role access.
pub fn tiered_voting_weight(
    interaction_count: usize,
    newcomer_threshold: usize,
    reputation: f64,
    trust: f64,
    gamma: f64,
    min_weight: f64,
    max_weight: f64,
) -> Option<f64> {
    if newcomer_threshold == 0 {
        return None;
    }
    if !min_weight.is_finite()
        || min_weight <= 0.0
        || !max_weight.is_finite()
        || max_weight <= 0.0
        || min_weight > max_weight
    {
        return None;
    }

    if interaction_count < newcomer_threshold {
        Some(min_weight)
    } else {
        bounded_voting_weight(reputation, trust, gamma, min_weight, max_weight)
    }
}

/// Tier-1 review privilege predicate.
pub fn can_review(
    interaction_count: usize,
    newcomer_threshold: usize,
    reputation: f64,
    tier1_min: f64,
) -> Option<bool> {
    if newcomer_threshold == 0 || !unit_value(reputation) || !unit_value(tier1_min) {
        return None;
    }

    Some(interaction_count >= newcomer_threshold && reputation >= tier1_min)
}

/// Tier-2 dispute privilege predicate.
pub fn can_dispute(reputation: f64, tier2_min: f64) -> Option<bool> {
    if !unit_value(reputation) || !unit_value(tier2_min) {
        return None;
    }

    Some(reputation >= tier2_min)
}

/// Row-normalizes local interaction scores into one EigenTrust row.
///
/// Implements `C_ij = max(s_ij, 0) / sum_k max(s_ik, 0)`. Negative local
/// interaction scores are clipped to zero as in the source. If a row has no
/// positive interactions, the paper defaults the row to the uniform prior
/// `1 / |A|`; this helper does the same instead of rejecting the row.
pub fn normalize_local_trust_row(score: &[f64]) -> Option<Vec<f64>> {
    if score.is_empty() || score.iter().any(|value| !value.is_finite()) {
        return None;
    }

    let positive: Vec<f64> = score.iter().map(|value| value.max(0.0)).collect();
    let total = positive.iter().sum::<f64>();
    if !total.is_finite() {
        return None;
    }
    if total > 0.0 {
        Some(positive.iter().map(|value| value / total).collect())
    } else {
        Some(vec![1.0 / score.len() as f64; score.len()])
    }
}

/// Convergence report for damped EigenTrust power iteration.
#[derive(Debug, Clone, PartialEq)]
pub struct EigenTrustConvergence {
    pub trust: Vec<f64>,
    pub iterations: usize,
    pub residual: f64,
    pub converged: bool,
}

/// Applies one damped EigenTrust power-iteration step.
///
/// Computes `t^(k+1) = (1 - epsilon) C^T t^(k) + epsilon p` for an already
/// normalized square trust matrix `C`, current trust distribution `t`, and
/// pre-trusted seed distribution `p`. The function validates those stochastic
/// inputs rather than normalizing them silently, because changing the mass would
/// hide malformed trust evidence.
pub fn eigentrust_step(
    matrix: &[Vec<f64>],
    trust: &[f64],
    prior: &[f64],
    epsilon: f64,
) -> Option<Vec<f64>> {
    let size = trust.len();
    if size == 0
        || matrix.len() != size
        || prior.len() != size
        || !epsilon.is_finite()
        || epsilon <= 0.0
        || epsilon >= 1.0
        || !is_distribution(trust)
        || !is_distribution(prior)
        || !matrix
            .iter()
            .all(|row| row.len() == size && is_distribution(row))
    {
        return None;
    }

    let mut next = vec![0.0; size];
    for source in 0..size {
        for target in 0..size {
            next[target] += matrix[source][target] * trust[source];
        }
    }

    Some(
        next.iter()
            .zip(prior.iter())
            .map(|(propagated, seed)| (1.0 - epsilon) * propagated + epsilon * seed)
            .collect(),
    )
}

/// Runs a fixed number of damped EigenTrust power-iteration steps.
///
/// The source describes EigenTrust as a power iteration over `C^T` with damping.
/// This helper starts from the pre-trusted seed vector and returns the final
/// iterate after `iterations` steps. It intentionally does not claim convergence
/// or pick a hidden stopping threshold for the caller.
pub fn eigentrust_iterate(
    matrix: &[Vec<f64>],
    prior: &[f64],
    epsilon: f64,
    iterations: usize,
) -> Option<Vec<f64>> {
    if iterations == 0 {
        return None;
    }

    let mut trust = prior.to_vec();
    for _ in 0..iterations {
        trust = eigentrust_step(matrix, &trust, prior, epsilon)?;
    }
    Some(trust)
}

/// Runs EigenTrust until the fixed-point residual reaches `tolerance` or the
/// iteration budget is exhausted.
///
/// Residual is the maximum absolute per-agent change between consecutive trust
/// vectors. The return value distinguishes an exhausted fixed-step result from
/// a vector that satisfied the caller's convergence tolerance.
pub fn eigentrust_converge(
    matrix: &[Vec<f64>],
    prior: &[f64],
    epsilon: f64,
    max_iterations: usize,
    tolerance: f64,
) -> Option<EigenTrustConvergence> {
    if max_iterations == 0 || !tolerance.is_finite() || tolerance < 0.0 {
        return None;
    }

    let mut trust = prior.to_vec();
    for iteration in 1..=max_iterations {
        let next = eigentrust_step(matrix, &trust, prior, epsilon)?;
        let residual = max_abs_delta(&trust, &next)?;
        let converged = residual <= tolerance;
        trust = next;
        if converged {
            return Some(EigenTrustConvergence {
                trust,
                iterations: iteration,
                residual,
                converged: true,
            });
        }
    }

    let next = eigentrust_step(matrix, &trust, prior, epsilon)?;
    let residual = max_abs_delta(&trust, &next)?;
    Some(EigenTrustConvergence {
        trust,
        iterations: max_iterations,
        residual,
        converged: false,
    })
}

/// Simulation precision for active artifacts above the quality threshold.
///
/// Computes `|{c: active(c) && q(c) >= threshold}| / |{c: active(c)}|`.
pub fn active_precision(active: &[bool], quality: &[f64], threshold: f64) -> Option<f64> {
    if active.len() != quality.len()
        || active.is_empty()
        || !threshold.is_finite()
        || quality.iter().any(|value| !value.is_finite())
    {
        return None;
    }

    let accepted = active.iter().filter(|is_active| **is_active).count();
    if accepted == 0 {
        return None;
    }

    let good_accepted = active
        .iter()
        .zip(quality.iter())
        .filter(|(is_active, score)| **is_active && **score >= threshold)
        .count();
    Some(good_accepted as f64 / accepted as f64)
}

/// Simulation recall for above-threshold artifacts that reached active state.
///
/// Computes `|{c: active(c) && q(c) >= threshold}| / |{c: q(c) >= threshold}|`.
pub fn active_recall(active: &[bool], quality: &[f64], threshold: f64) -> Option<f64> {
    if active.len() != quality.len()
        || active.is_empty()
        || !threshold.is_finite()
        || quality.iter().any(|value| !value.is_finite())
    {
        return None;
    }

    let good = quality.iter().filter(|score| **score >= threshold).count();
    if good == 0 {
        return None;
    }

    let good_accepted = active
        .iter()
        .zip(quality.iter())
        .filter(|(is_active, score)| **is_active && **score >= threshold)
        .count();
    Some(good_accepted as f64 / good as f64)
}

/// Sanction false-positive rate over honest or broken agents.
///
/// Computes `|{a in H union B: sigma(a) >= threshold}| / |H union B|`.
pub fn sanction_false_positive_rate(
    honest_or_broken: &[bool],
    sanction: &[f64],
    threshold: f64,
) -> Option<f64> {
    if honest_or_broken.len() != sanction.len()
        || honest_or_broken.is_empty()
        || !threshold.is_finite()
        || sanction.iter().any(|value| !value.is_finite())
    {
        return None;
    }

    let protected = honest_or_broken.iter().filter(|value| **value).count();
    if protected == 0 {
        return None;
    }

    let false_positive = honest_or_broken
        .iter()
        .zip(sanction.iter())
        .filter(|(protected, score)| **protected && **score >= threshold)
        .count();
    Some(false_positive as f64 / protected as f64)
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

fn is_distribution(value: &[f64]) -> bool {
    !value.is_empty()
        && value.iter().all(|item| item.is_finite() && *item >= 0.0)
        && (value.iter().sum::<f64>() - 1.0).abs() <= 1e-9
}

fn unit_value(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

fn max_abs_delta(left: &[f64], right: &[f64]) -> Option<f64> {
    if left.len() != right.len() || left.is_empty() {
        return None;
    }

    Some(
        left.iter()
            .zip(right.iter())
            .map(|(left, right)| (left - right).abs())
            .fold(0.0, f64::max),
    )
}
