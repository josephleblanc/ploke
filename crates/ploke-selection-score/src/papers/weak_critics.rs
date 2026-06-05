//! 2606.00424 — Weak Critics / O-PCD filtering helpers.

/// Product filter `h_i = r_out * r_rub` as a boolean predicate.
pub fn retain_example(outcome_correct: bool, rubric_useful: bool) -> bool {
    outcome_correct && rubric_useful
}

/// KL divergence over one token distribution support.
///
/// Implements `KL(p || q) = sum_v p(v) log(p(v) / q(v))` for already-aligned
/// probability vectors over `V_KL`. Zero mass in `p` contributes zero. Positive
/// mass in `p` where `q` has zero mass is undefined here and returns `None`
/// rather than producing an infinite loss.
pub fn kl_divergence(student: &[f64], teacher: &[f64]) -> Option<f64> {
    if student.len() != teacher.len()
        || student.is_empty()
        || !is_distribution(student)
        || !is_distribution(teacher)
    {
        return None;
    }

    let mut total = 0.0;
    for (p, q) in student.iter().zip(teacher.iter()) {
        if *p == 0.0 {
            continue;
        }
        if *q == 0.0 {
            return None;
        }
        total += p * (p / q).ln();
    }
    Some(total)
}

/// Retained-set O-PCD loss from token-level KL terms.
///
/// Computes `1 / |S_e| * sum_(x,y,f in S_e) sum_t KL_t`. Each inner vector is
/// one retained triple's token trajectory. The helper takes precomputed KL
/// terms because this crate does not own model logits or tokenization.
pub fn opcd_loss(token_kl_by_example: &[Vec<f64>]) -> Option<f64> {
    if token_kl_by_example.is_empty()
        || token_kl_by_example.iter().any(|example| {
            example.is_empty()
                || example
                    .iter()
                    .any(|value| !value.is_finite() || *value < 0.0)
        })
    {
        return None;
    }

    let total = token_kl_by_example
        .iter()
        .flat_map(|example| example.iter())
        .sum::<f64>();
    Some(total / token_kl_by_example.len() as f64)
}

fn is_distribution(value: &[f64]) -> bool {
    value.iter().all(|item| item.is_finite() && *item >= 0.0)
        && (value.iter().sum::<f64>() - 1.0).abs() <= 1e-9
}
