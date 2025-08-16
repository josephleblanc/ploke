use uuid::Uuid;

/// Score normalization strategies for making scores comparable across modalities.
#[derive(Debug, Clone)]
pub enum ScoreNorm {
    /// No normalization; passthrough.
    None,
    /// Min-max normalization to [0, 1] with optional clamping and epsilon for stability.
    /// output = (x - min) / max(max - min, epsilon)
    MinMax {
        /// Clamp the output to [0.0, 1.0]
        clamp: bool,
        /// Small value to avoid division by zero when (max - min) ≈ 0
        epsilon: f32,
    },
    /// Standard score normalization (z-score): mean 0, unit variance.
    /// output = (x - mean) / max(stddev, epsilon)
    ZScore {
        /// Small value to avoid division by zero when stddev ≈ 0
        epsilon: f32,
    },
    /// Logistic squashing to (0, 1).
    /// output = 1 / (1 + exp(-steepness * (x - midpoint)))
    Logistic {
        /// The x value that maps to ~0.5 after transformation
        midpoint: f32,
        /// Steepness parameter k; higher values produce a sharper transition
        steepness: f32,
        /// Clamp the output to [0.0, 1.0]
        clamp: bool,
    },
}

impl Default for ScoreNorm {
    fn default() -> Self {
        ScoreNorm::MinMax {
            clamp: true,
            epsilon: 1e-6,
        }
    }
}

/// Normalize a list of (Uuid, score) pairs using the selected method.
/// The order and IDs are preserved; only scores are transformed.
pub fn normalize_scores(scores: &[(Uuid, f32)], method: &ScoreNorm) -> Vec<(Uuid, f32)> {
    match method {
        ScoreNorm::None => scores.to_vec(),
        ScoreNorm::MinMax { clamp, epsilon } => min_max(scores, *clamp, *epsilon),
        ScoreNorm::ZScore { epsilon } => z_score(scores, *epsilon),
        ScoreNorm::Logistic {
            midpoint,
            steepness,
            clamp,
        } => logistic(scores, *midpoint, *steepness, *clamp),
    }
}

fn min_max(scores: &[(Uuid, f32)], clamp: bool, epsilon: f32) -> Vec<(Uuid, f32)> {
    if scores.is_empty() {
        return Vec::new();
    }
    let mut min_v = f32::INFINITY;
    let mut max_v = f32::NEG_INFINITY;
    for &(_, s) in scores {
        if s < min_v {
            min_v = s;
        }
        if s > max_v {
            max_v = s;
        }
    }
    let denom = (max_v - min_v).max(epsilon);
    scores
        .iter()
        .map(|(id, s)| {
            let mut v = (*s - min_v) / denom;
            if clamp {
                v = v.clamp(0.0, 1.0);
            }
            (*id, v)
        })
        .collect()
}

fn z_score(scores: &[(Uuid, f32)], epsilon: f32) -> Vec<(Uuid, f32)> {
    if scores.is_empty() {
        return Vec::new();
    }
    let n = scores.len() as f32;
    let mean = scores.iter().map(|(_, s)| *s).sum::<f32>() / n;
    let var = scores
        .iter()
        .map(|(_, s)| {
            let d = *s - mean;
            d * d
        })
        .sum::<f32>()
        / n;
    let stddev = var.sqrt().max(epsilon);
    scores
        .iter()
        .map(|(id, s)| (*id, (*s - mean) / stddev))
        .collect()
}

fn logistic(
    scores: &[(Uuid, f32)],
    midpoint: f32,
    steepness: f32,
    clamp: bool,
) -> Vec<(Uuid, f32)> {
    if scores.is_empty() {
        return Vec::new();
    }
    scores
        .iter()
        .map(|(id, s)| {
            let x = *s;
            // Numerically stable enough for typical score ranges.
            let mut v = 1.0 / (1.0 + (-(steepness * (x - midpoint))).exp());
            if clamp {
                v = v.clamp(0.0, 1.0);
            }
            (*id, v)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f32, b: f32, eps: f32) -> bool {
        (a - b).abs() <= eps
    }

    #[test]
    fn test_minmax_basic() {
        let s = vec![
            (Uuid::from_u128(1), 10.0),
            (Uuid::from_u128(2), 20.0),
            (Uuid::from_u128(3), 15.0),
        ];
        let out = normalize_scores(&s, &ScoreNorm::MinMax { clamp: true, epsilon: 1e-6 });
        assert_eq!(out.len(), 3);
        assert!(approx_eq(out[0].1, 0.0, 1e-6));
        assert!(approx_eq(out[1].1, 1.0, 1e-6));
        assert!(approx_eq(out[2].1, 0.5, 1e-6));
    }

    #[test]
    fn test_minmax_all_equal_uses_epsilon() {
        let s = vec![(Uuid::from_u128(1), 5.0), (Uuid::from_u128(2), 5.0)];
        let out = normalize_scores(&s, &ScoreNorm::MinMax { clamp: true, epsilon: 1e-6 });
        assert_eq!(out.len(), 2);
        // With (max - min) ~ 0, result becomes (x - min)/epsilon = 0.
        assert!(approx_eq(out[0].1, 0.0, 1e-6));
        assert!(approx_eq(out[1].1, 0.0, 1e-6));
    }

    #[test]
    fn test_zscore() {
        let s = vec![
            (Uuid::from_u128(1), 1.0),
            (Uuid::from_u128(2), 2.0),
            (Uuid::from_u128(3), 3.0),
        ];
        let out = normalize_scores(&s, &ScoreNorm::ZScore { epsilon: 1e-6 });
        assert_eq!(out.len(), 3);
        // Expected (population) z-scores for [1,2,3] are about [-1.2247, 0.0, 1.2247]
        assert!(approx_eq(out[0].1, -1.224_7449, 1e-3));
        assert!(approx_eq(out[1].1, 0.0, 1e-6));
        assert!(approx_eq(out[2].1, 1.224_7449, 1e-3));
    }

    #[test]
    fn test_logistic() {
        let s = vec![
            (Uuid::from_u128(1), 0.0),
            (Uuid::from_u128(2), 0.5),
            (Uuid::from_u128(3), 1.0),
        ];
        let out = normalize_scores(
            &s,
            &ScoreNorm::Logistic {
                midpoint: 0.5,
                steepness: 10.0,
                clamp: true,
            },
        );
        assert_eq!(out.len(), 3);
        // Logistic at (k=10, x0=0.5): ~[0.0067, 0.5, 0.9933]
        assert!(approx_eq(out[0].1, 0.006_692_85, 1e-3));
        assert!(approx_eq(out[1].1, 0.5, 1e-6));
        assert!(approx_eq(out[2].1, 0.993_307, 1e-3));
    }
}
