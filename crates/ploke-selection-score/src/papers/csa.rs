//! 2606.00251 — Capability self-assessment scoring helpers.

/// Capability self-assessment route label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Label {
    SelfSolve,
    Delegate,
}

/// Converts an aggregation predicate result into a CSA label.
pub fn label(can_self_solve: bool) -> Label {
    if can_self_solve {
        Label::SelfSolve
    } else {
        Label::Delegate
    }
}

/// Builds a CSA label from repeated correctness probes.
///
/// The paper labels a query `SELF-SOLVE` when the aggregation function over
/// repeated probe correctness is true. Appendix D instantiates this as
/// any-correct for math and majority-correct for science. `min_correct` is that
/// aggregation threshold over an already-materialized probe group.
pub fn label_from_correctness(correct: &[bool], min_correct: usize) -> Option<Label> {
    if correct.is_empty() || min_correct == 0 || min_correct > correct.len() {
        return None;
    }

    let hits = correct.iter().filter(|item| **item).count();
    Some(label(hits >= min_correct))
}

/// Binary reward for matching the target CSA label.
pub fn binary_reward(predicted: Label, target: Label) -> i8 {
    if predicted == target { 1 } else { -1 }
}

/// True when a rollout group contains both CSA labels.
pub fn has_label_diversity(label: &[Label]) -> bool {
    label.contains(&Label::SelfSolve) && label.contains(&Label::Delegate)
}

/// Supervised negative log-likelihood objective for CSA SFT variants.
///
/// Computes `L_SFT = -sum_i log p_theta(o_i | x_i)` over caller-supplied
/// sequence log-likelihoods. Log probabilities must be finite and non-positive;
/// this helper does not tokenize outputs or estimate model probabilities.
pub fn sft_loss(log_likelihood: &[f64]) -> Option<f64> {
    if log_likelihood.is_empty()
        || log_likelihood
            .iter()
            .any(|value| !value.is_finite() || *value > 0.0)
    {
        return None;
    }

    Some(-log_likelihood.iter().sum::<f64>())
}

/// GRPO policy ratio `rho = pi_theta(o|x) / pi_theta_old(o|x)`.
pub fn policy_ratio(policy_prob: f64, old_policy_prob: f64) -> Option<f64> {
    if !unit_probability(policy_prob)
        || !unit_probability(old_policy_prob)
        || old_policy_prob == 0.0
    {
        return None;
    }

    Some(policy_prob / old_policy_prob)
}

/// Group-normalized advantages from verifiable rewards.
///
/// Computes `(R_g - mean_g R_g) / std_g R_g`. A zero-variance group returns
/// `None`, matching the paper's motivation for diversity-filtered warm-up:
/// same-label rollout groups carry no useful policy-gradient signal.
pub fn standardized_advantages(reward: &[f64]) -> Option<Vec<f64>> {
    if reward.is_empty() || reward.iter().any(|value| !value.is_finite()) {
        return None;
    }

    let mean = reward.iter().sum::<f64>() / reward.len() as f64;
    let variance = reward
        .iter()
        .map(|value| {
            let diff = value - mean;
            diff * diff
        })
        .sum::<f64>()
        / reward.len() as f64;
    let std = variance.sqrt();
    if std == 0.0 || !std.is_finite() {
        return None;
    }

    Some(reward.iter().map(|value| (value - mean) / std).collect())
}

/// One rollout's clipped GRPO surrogate term before the leading negative mean.
///
/// Implements `min(rho * A, clip(rho, 1 - epsilon, 1 + epsilon) * A)`.
pub fn clipped_surrogate(ratio: f64, advantage: f64, epsilon: f64) -> Option<f64> {
    if !ratio.is_finite()
        || ratio < 0.0
        || !advantage.is_finite()
        || !epsilon.is_finite()
        || epsilon <= 0.0
        || epsilon >= 1.0
    {
        return None;
    }

    let clipped = ratio.clamp(1.0 - epsilon, 1.0 + epsilon);
    Some((ratio * advantage).min(clipped * advantage))
}

/// Finite rollout-group version of the CSA GRPO objective.
///
/// Computes `-mean_g clipped_surrogate_g + beta * KL` for one already-sampled
/// group. The full paper objective also takes an expectation over minibatches;
/// callers can average this helper over groups when replaying a batch.
pub fn grpo_group_loss(
    ratio: &[f64],
    advantage: &[f64],
    epsilon: f64,
    kl: f64,
    beta: f64,
) -> Option<f64> {
    if ratio.len() != advantage.len()
        || ratio.is_empty()
        || !kl.is_finite()
        || kl < 0.0
        || !beta.is_finite()
        || beta < 0.0
    {
        return None;
    }

    let mut total = 0.0;
    for (ratio, advantage) in ratio.iter().zip(advantage.iter()) {
        total += clipped_surrogate(*ratio, *advantage, epsilon)?;
    }

    Some(-(total / ratio.len() as f64) + beta * kl)
}

fn unit_probability(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}
