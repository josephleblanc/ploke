//! 2510.23535 — Sequential Multi-Agent Dynamic Algorithm Configuration.
//!
//! This module keeps the executable surface close to the SADN paper: ordered
//! configuration agents, prefix-conditioned action values, shared rewards, and
//! the advantage update/decomposition rules. It does not train neural networks,
//! extract Ploke run features, or promote any Ploke-specific reward.

/// Finite approximation of the DAC policy objective's expected cost term.
///
/// The paper objective minimizes `∫ p(i)c(π,i) di` over a problem-instance
/// distribution. This helper computes the finite weighted sum for an already
/// materialized instance distribution and cost vector. Probabilities must form a
/// valid distribution; invalid weights are rejected rather than normalized.
pub fn expected_cost(probability: &[f64], cost: &[f64]) -> Option<f64> {
    if probability.is_empty() || probability.len() != cost.len() {
        return None;
    }
    if probability
        .iter()
        .any(|value| !value.is_finite() || *value < 0.0)
        || cost.iter().any(|value| !value.is_finite())
    {
        return None;
    }

    let total = probability.iter().sum::<f64>();
    if (total - 1.0).abs() > 1e-9 {
        return None;
    }

    Some(
        probability
            .iter()
            .zip(cost.iter())
            .map(|(probability, cost)| probability * cost)
            .sum(),
    )
}

/// Finite-horizon version of SADN's shared-reward value expression.
///
/// Computes `Σ_t γ^t r_t` for observed rewards. `gamma` is the discount factor.
pub fn discounted_return(reward: &[f64], gamma: f64) -> Option<f64> {
    if reward.is_empty() || !(0.0..=1.0).contains(&gamma) || !gamma.is_finite() {
        return None;
    }
    if reward.iter().any(|value| !value.is_finite()) {
        return None;
    }

    let mut discount = 1.0;
    let mut total = 0.0;
    for value in reward {
        total += discount * value;
        discount *= gamma;
    }
    Some(total)
}

/// Applies the SADN global advantage update.
///
/// Implements `A <- A + α[r + γV(s') - V(s) - A]` for already-materialized
/// scalar terms.
pub fn advantage_update(
    current: f64,
    reward: f64,
    gamma: f64,
    next_value: f64,
    value: f64,
    alpha: f64,
) -> Option<f64> {
    if [current, reward, gamma, next_value, value, alpha]
        .iter()
        .any(|item| !item.is_finite())
    {
        return None;
    }
    if !(0.0..=1.0).contains(&gamma) || alpha < 0.0 {
        return None;
    }

    Some(current + alpha * (reward + gamma * next_value - value - current))
}

/// Computes the paper's prefix-conditioned partial advantage.
///
/// Implements `A_k(s, a_{1:k-1}, a_k) = Q_{1:k} - Q_{1:k-1}`.
pub fn partial_advantage(partial_q: f64, prior_prefix_q: f64) -> Option<f64> {
    if !partial_q.is_finite() || !prior_prefix_q.is_finite() {
        return None;
    }

    Some(partial_q - prior_prefix_q)
}

/// Sums ordered-agent advantages into the global advantage.
///
/// Implements the SADN decomposition `A(s,a) = Σ_i A_i(s, a_{1:i-1}, a_i)` for
/// already-computed lane advantages.
pub fn global_advantage(advantage: &[f64]) -> Option<f64> {
    if advantage.is_empty() || advantage.iter().any(|value| !value.is_finite()) {
        return None;
    }

    Some(advantage.iter().sum())
}

/// Executes SADN's sequential IGM greedy action rule over supplied advantages.
///
/// `action_counts[j]` is the nominal size of the j-th ordered configuration
/// agent's action space. The `advantage` callback receives `(j, prefix, action)`
/// where `prefix` is the already chosen same-timestep action prefix
/// `a_{1:j-1}`. Returning `None` omits that action from the effective
/// conditional action domain for the current prefix. Ties keep the earliest
/// action index.
pub fn sequential_greedy<F>(action_counts: &[usize], mut advantage: F) -> Option<Vec<usize>>
where
    F: FnMut(usize, &[usize], usize) -> Option<f64>,
{
    if action_counts.is_empty() {
        return None;
    }

    let mut chosen = Vec::with_capacity(action_counts.len());
    for (agent, action_count) in action_counts.iter().copied().enumerate() {
        if action_count == 0 {
            return None;
        }

        let mut best: Option<(usize, f64)> = None;
        for action in 0..action_count {
            let Some(score) = advantage(agent, &chosen, action) else {
                continue;
            };
            if !score.is_finite() {
                continue;
            }
            if best.is_none_or(|(_, best_score)| score > best_score) {
                best = Some((action, score));
            }
        }

        chosen.push(best?.0);
    }

    Some(chosen)
}
