#![allow(missing_docs)]
//! Fusion utilities: score normalization, weighted RRF, and MMR.
//!
//! This module provides building blocks to combine sparse (BM25) and dense results into a single,
//! deterministic ranking. The core entry points are:
//! - [`normalize_scores`]: bring scores from different modalities onto a comparable scale.
//! - [`rrf_fuse`]: weighted reciprocal rank fusion with stable UUID tie-breaking.
//! - [`mmr_select`]: diversity-aware selection using cosine similarity on normalized vectors.
//!
//! The algorithms are intentionally small, well-documented, and pure (no I/O) to aid testing and reuse.
use std::collections::{HashMap, HashSet};
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

/// Configuration for Reciprocal Rank Fusion (RRF).
#[derive(Debug, Clone, Copy)]
pub struct RrfConfig {
    /// RRF smoothing parameter (typically ~60.0).
    pub k: f32,
    /// Weight for the BM25 modality.
    pub weight_bm25: f32,
    /// Weight for the dense modality.
    pub weight_dense: f32,
}

impl Default for RrfConfig {
    fn default() -> Self {
        Self {
            k: 60.0,
            weight_bm25: 1.0,
            weight_dense: 1.0,
        }
    }
}

/// Fuse BM25 and dense results using weighted Reciprocal Rank Fusion (RRF).
/// - Ranks are 1-based within each list.
/// - Missing ranks contribute 0 to the fused score.
/// - Stable tie-breaking by UUID ascending if fused scores are equal.
pub fn rrf_fuse(bm25: &[(Uuid, f32)], dense: &[(Uuid, f32)], cfg: &RrfConfig) -> Vec<(Uuid, f32)> {
    let mut fused: HashMap<Uuid, f32> = HashMap::new();

    for (i, (id, _)) in bm25.iter().enumerate() {
        let rank = (i as f32) + 1.0;
        let add = cfg.weight_bm25 / (cfg.k + rank);
        *fused.entry(*id).or_insert(0.0) += add;
    }
    for (i, (id, _)) in dense.iter().enumerate() {
        let rank = (i as f32) + 1.0;
        let add = cfg.weight_dense / (cfg.k + rank);
        *fused.entry(*id).or_insert(0.0) += add;
    }

    let mut out: Vec<(Uuid, f32)> = fused.into_iter().collect();
    out.sort_by(|(ida, sa), (idb, sb)| {
        match sb.partial_cmp(sa).unwrap_or(std::cmp::Ordering::Equal) {
            std::cmp::Ordering::Equal => ida.as_bytes().cmp(idb.as_bytes()),
            other => other,
        }
    });
    out
}

/// Similarity metrics for MMR.
#[derive(Debug, Clone, Copy)]
pub enum Similarity {
    /// Cosine similarity on L2-normalized vectors.
    Cosine,
}

/// Configuration for Maximal Marginal Relevance (MMR).
#[derive(Debug, Clone, Copy)]
pub struct MmrConfig {
    /// Tradeoff between relevance and diversity: score = λ * rel - (1-λ) * max_sim
    pub lambda: f32,
    /// Similarity metric used for the diversity penalty.
    pub sim_metric: Similarity,
    /// Consider only the top-N candidates by relevance when selecting.
    pub candidate_pool: usize,
}

impl Default for MmrConfig {
    fn default() -> Self {
        Self {
            lambda: 0.7,
            sim_metric: Similarity::Cosine,
            candidate_pool: 50,
        }
    }
}

fn l2_normalize(v: &[f32]) -> Option<Vec<f32>> {
    if v.is_empty() {
        return None;
    }
    let mut sumsq = 0.0f32;
    for &x in v {
        sumsq += x * x;
    }
    let norm = sumsq.sqrt();
    if norm > 0.0 {
        Some(v.iter().map(|&x| x / norm).collect())
    } else {
        None
    }
}

fn cosine_sim(a: &[f32], b: &[f32]) -> f32 {
    let len = a.len().min(b.len());
    if len == 0 {
        return 0.0;
    }
    a.iter().zip(b.iter()).take(len).map(|(x, y)| x * y).sum()
}

/// Select a diverse subset of candidates using Maximal Marginal Relevance (MMR).
/// - candidates: (Uuid, relevance_score) pairs, assumed higher is better
/// - k: number of items to select
/// - embeddings: map of Uuid -> embedding vector; missing vectors imply sim=0.0
/// - cfg: MMR configuration (λ, similarity metric, candidate pool)
///
/// Returns (Uuid, mmr_objective_score) for the selected items, in selection order.
pub fn mmr_select(
    candidates: &[(Uuid, f32)],
    k: usize,
    embeddings: &HashMap<Uuid, Vec<f32>>,
    cfg: &MmrConfig,
) -> Vec<(Uuid, f32)> {
    if k == 0 || candidates.is_empty() {
        return Vec::new();
    }

    // Normalize embeddings upfront; treat missing or zero-norm vectors as absent.
    let mut norm_map: HashMap<Uuid, Vec<f32>> = HashMap::with_capacity(embeddings.len());
    for (id, vec) in embeddings.iter() {
        if let Some(nv) = l2_normalize(vec.as_slice()) {
            norm_map.insert(*id, nv);
        }
    }

    // Build candidate pool: sort by relevance desc, tie-break by UUID asc; deduplicate IDs.
    let mut pool: Vec<(Uuid, f32)> = candidates.to_vec();
    pool.sort_by(|(ida, sa), (idb, sb)| {
        match sb.partial_cmp(sa).unwrap_or(std::cmp::Ordering::Equal) {
            std::cmp::Ordering::Equal => ida.as_bytes().cmp(idb.as_bytes()),
            other => other,
        }
    });
    let mut seen: HashSet<Uuid> = HashSet::with_capacity(pool.len());
    pool.retain(|(id, _)| seen.insert(*id));
    let limit = cfg.candidate_pool.min(pool.len());
    pool.truncate(limit);

    let mut selected: Vec<(Uuid, f32)> = Vec::with_capacity(k.min(pool.len()));
    let mut selected_ids: Vec<Uuid> = Vec::with_capacity(k.min(pool.len()));

    while selected.len() < k && !pool.is_empty() {
        let mut best_idx = 0usize;
        let mut best_obj = f32::NEG_INFINITY;
        let mut best_id = pool[0].0;

        for (i, (cid, rel)) in pool.iter().enumerate() {
            let diversity_penalty = if selected_ids.is_empty() {
                0.0
            } else {
                let mut max_sim = 0.0f32;
                for sid in &selected_ids {
                    match cfg.sim_metric {
                        Similarity::Cosine => {
                            if let (Some(cv), Some(sv)) = (norm_map.get(cid), norm_map.get(sid)) {
                                let sim = cosine_sim(cv, sv);
                                if sim > max_sim {
                                    max_sim = sim;
                                }
                            }
                        }
                    }
                }
                max_sim
            };

            let obj = cfg.lambda * (*rel) - (1.0 - cfg.lambda) * diversity_penalty;

            if obj > best_obj
                || (obj == best_obj
                    && cid.as_bytes().cmp(best_id.as_bytes()) == std::cmp::Ordering::Less)
            {
                best_obj = obj;
                best_idx = i;
                best_id = *cid;
            }
        }

        let chosen = pool.swap_remove(best_idx);
        selected_ids.push(chosen.0);
        selected.push((chosen.0, best_obj));
    }

    selected
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
        let out = normalize_scores(
            &s,
            &ScoreNorm::MinMax {
                clamp: true,
                epsilon: 1e-6,
            },
        );
        assert_eq!(out.len(), 3);
        assert!(approx_eq(out[0].1, 0.0, 1e-6));
        assert!(approx_eq(out[1].1, 1.0, 1e-6));
        assert!(approx_eq(out[2].1, 0.5, 1e-6));
    }

    #[test]
    fn test_minmax_all_equal_uses_epsilon() {
        let s = vec![(Uuid::from_u128(1), 5.0), (Uuid::from_u128(2), 5.0)];
        let out = normalize_scores(
            &s,
            &ScoreNorm::MinMax {
                clamp: true,
                epsilon: 1e-6,
            },
        );
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
        assert!(approx_eq(out[0].1, -1.224_744_9, 1e-3));
        assert!(approx_eq(out[1].1, 0.0, 1e-6));
        assert!(approx_eq(out[2].1, 1.224_744_9, 1e-3));
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
