#![allow(missing_docs)]
//! Context assembly: budgeting, deduplication, ordering, and packaging of snippets.
//!
//! Given a ranked list of node IDs (typically fused BM25+dense), this module fetches snippet text
//! via `ploke-io`, trims parts under a configurable token budget, deduplicates, and returns a
//! reproducible [`AssembledContext`] suitable for downstream prompting.
//!
//! The tokenizer is abstracted by [`TokenCounter`], enabling deterministic tests and pluggable
//! adapters for real LLM tokenizers in higher-level crates.
//!
//! Current implementation focuses on end-to-end wiring; stitching of ranges and richer metadata
//! are documented in `plans.md` for subsequent iterations.
use std::collections::{HashMap, HashSet};

use itertools::Itertools;
use ploke_core::{
    EmbeddingData,
    rag_types::{
        AssembledContext, CanonPath, ContextPart, ContextPartKind, ContextStats, Modality,
        NodeFilepath,
    },
};
use ploke_db::{Database, NodeType, get_by_id::NodePaths};
use ploke_io::IoManagerHandle;
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument};
use uuid::Uuid;

use crate::error::RagError;

/// Token budget parameters for context assembly.
#[derive(Debug, Clone)]
pub struct TokenBudget {
    /// Maximum total tokens across all included parts.
    pub max_total: usize,
    /// Maximum tokens allowed per file (grouped by file_path).
    pub per_file_max: usize,
    /// Maximum tokens allowed per part; parts exceeding this will be trimmed.
    pub per_part_max: usize,
    // Removed to make this Copy, do reserves elsewhere
    // Optional reserved tokens for named sections (e.g., system prompts).
    // pub reserves: Option<HashMap<String, usize>>,
}

impl Default for TokenBudget {
    fn default() -> Self {
        Self {
            max_total: 4096,
            per_file_max: 2048,
            per_part_max: 1024,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ordering {
    /// Order by fused score (desc), tie-break by UUID asc.
    FusedScoreThenStructure,
    /// Group by file, within each group order by score (desc), tie-break by UUID asc.
    ByFileThenScore,
}

#[derive(Debug, Clone)]
pub struct AssemblyPolicy {
    pub ordering: Ordering,
    pub include_kinds: HashSet<ContextPartKind>,
    /// Optional per-node-type caps for fairness; not yet enforced in this initial version.
    pub per_type_caps: Option<HashMap<NodeType, usize>>,
    /// Allow overlapping snippet ranges; range handling is a no-op in this initial version.
    pub allow_overlap: bool,
    /// If true, IO errors during snippet retrieval are treated as fatal.
    pub strict_io: bool,
}

impl Default for AssemblyPolicy {
    fn default() -> Self {
        let mut include_kinds = HashSet::new();
        include_kinds.insert(ContextPartKind::Code);
        include_kinds.insert(ContextPartKind::Doc);
        Self {
            ordering: Ordering::FusedScoreThenStructure,
            include_kinds,
            per_type_caps: None,
            allow_overlap: false,
            strict_io: false,
        }
    }
}

/// Trait for counting tokens. Implementations can be provided by consumers.
pub trait TokenCounter: Send + Sync + std::fmt::Debug {
    fn count(&self, text: &str) -> usize;
}

/// A simple, deterministic tokenizer suitable for tests:
/// approximates tokens as ceil(chars / 4).
#[derive(Default, Debug)]
pub struct ApproxCharTokenizer;

impl TokenCounter for ApproxCharTokenizer {
    fn count(&self, text: &str) -> usize {
        text.chars().count().div_ceil(4)
    }
}

fn stable_dedup_ids_ordered(ids: &[Uuid]) -> (Vec<Uuid>, usize) {
    let mut seen: HashSet<Uuid> = HashSet::with_capacity(ids.len());
    let mut out: Vec<Uuid> = Vec::with_capacity(ids.len());
    let mut removed = 0usize;
    for id in ids {
        if seen.insert(*id) {
            out.push(*id);
        } else {
            removed += 1;
        }
    }
    (out, removed)
}

fn trim_text_to_tokens(
    text: &str,
    max_tokens: usize,
    tokenizer: &dyn TokenCounter,
) -> (String, bool) {
    let mut truncated = false;
    let mut candidate = text.to_string();
    if max_tokens == 0 {
        return (String::new(), true);
    }
    // If already within budget, return early.
    if tokenizer.count(&candidate) <= max_tokens {
        return (candidate, truncated);
    }

    // Approximate: scale down by ratio of tokens, then refine by chopping until within budget.
    // Start with a rough character limit assuming ~4 chars/token.
    let approx_chars = max_tokens.saturating_mul(4);
    candidate = candidate.chars().take(approx_chars).collect();
    truncated = true;

    // If still over (due to tokenizer behavior), shave characters until it fits or empty.
    while !candidate.is_empty() && tokenizer.count(&candidate) > max_tokens {
        candidate.pop();
    }
    (candidate, truncated)
}

fn ordering_key_by_score_then_id(score: f32, id: &Uuid) -> (std::cmp::Ordering, &[u8]) {
    // Higher scores first -> reverse ordering; UUID ascending for ties.
    (std::cmp::Ordering::Greater, id.as_bytes())
}

/// Assemble context parts from ranked hits with token budgeting and deterministic ordering.
/// Note: This initial implementation focuses on end-to-end wiring and budgeting.
/// Range normalization/stitching and rich metadata (e.g., true file paths) are placeholders
/// that will be refined in subsequent iterations.
#[instrument(
    skip(query, hits, budget, policy, tokenizer, db, io),
    fields(query_len = %query.len(), hits = hits.len())
)]
pub async fn assemble_context(
    query: &str,
    hits: &[(Uuid, f32)],
    budget: &TokenBudget,
    policy: &AssemblyPolicy,
    tokenizer: &dyn TokenCounter,
    db: &Database,
    io: &IoManagerHandle,
) -> Result<AssembledContext, RagError> {
    // Build score map and preserve incoming order.
    let mut score_map: HashMap<Uuid, f32> = HashMap::with_capacity(hits.len());
    let ordered_ids: Vec<Uuid> = hits.iter().map(|(id, _)| *id).collect();
    for (id, s) in hits {
        // If duplicate id appears, keep the first score (stable).
        score_map.entry(*id).or_insert(*s);
    }

    // Dedup by UUID while preserving order.
    let (dedup_ids, dedup_removed) = stable_dedup_ids_ordered(&ordered_ids);

    // Fetch file/span metadata in the requested order. This intentionally does not require
    // embedding-set membership because sparse retrieval can return valid node ids before dense
    // embedding rows exist.
    let context_nodes = db
        .get_snippet_context_nodes_ordered(dedup_ids.clone())
        .map_err(|e| RagError::Embed(e.to_string()))?;
    let (nodes, node_paths): (Vec<EmbeddingData>, Vec<NodePaths>) =
        context_nodes.into_iter().unzip();

    let file_paths = nodes
        .iter()
        .map(|ed| ed.file_path.to_string_lossy().to_string())
        .collect_vec();

    // Batch fetch snippets.
    let batch = io
        .get_snippets_batch(nodes)
        .await
        .map_err(|e| RagError::Search(format!("get_snippets_batch failed: {:?}", e)))?;

    // Build preliminary parts (with placeholder file path and no ranges for now).
    let mut prelim_parts: Vec<ContextPart> = Vec::with_capacity(batch.len());
    let mut skipped_io_errors = 0usize;
    for (i, (res, node_paths)) in batch.into_iter().zip(node_paths.into_iter()).enumerate() {
        let id = dedup_ids
            .get(i)
            .copied()
            .ok_or_else(|| RagError::Search(format!("mismatched batch index {}", i)))?;
        let NodePaths { file, canon } = node_paths;

        match res {
            Ok(text) => {
                // Use UUID as a stable per-"file" key until richer metadata is wired through.
                // let file_key = format!("id://{}", id);
                let part = ContextPart {
                    id,
                    file_path: NodeFilepath::new(file),
                    canon_path: CanonPath::new(canon),
                    ranges: Vec::new(),
                    kind: ContextPartKind::Code,
                    text,
                    score: *score_map.get(&id).unwrap_or(&0.0),
                    modality: Modality::HybridFused,
                };
                prelim_parts.push(part);
            }
            Err(e) => {
                if policy.strict_io {
                    return Err(RagError::Search(format!(
                        "IO error retrieving snippet for {}: {:?}",
                        id, e
                    )));
                } else {
                    skipped_io_errors += 1;
                    debug!("Skipping snippet for {} due to IO error: {:?}", id, e);
                }
            }
        }
    }

    // Filter by allowed kinds (currently we only produce Code parts).
    let prelim_parts: Vec<ContextPart> = prelim_parts
        .into_iter()
        .filter(|p| policy.include_kinds.contains(&p.kind))
        .collect();

    // Ordering
    let mut parts = prelim_parts;
    match policy.ordering {
        Ordering::FusedScoreThenStructure => {
            parts.sort_by(|a, b| {
                match b
                    .score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
                {
                    std::cmp::Ordering::Equal => a.id.as_bytes().cmp(b.id.as_bytes()),
                    other => other,
                }
            });
        }
        Ordering::ByFileThenScore => {
            parts.sort_by(|a, b| match a.file_path.cmp(&b.file_path) {
                std::cmp::Ordering::Equal => match b
                    .score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
                {
                    std::cmp::Ordering::Equal => a.id.as_bytes().cmp(b.id.as_bytes()),
                    other => other,
                },
                other => other,
            });
        }
    }

    Ok(apply_token_budget(
        parts,
        budget,
        tokenizer,
        dedup_removed,
        skipped_io_errors,
    ))
}

fn apply_token_budget(
    parts: Vec<ContextPart>,
    budget: &TokenBudget,
    tokenizer: &dyn TokenCounter,
    dedup_removed: usize,
    skipped_io_errors: usize,
) -> AssembledContext {
    // Token budgeting (water-filling).
    let mut stats = ContextStats {
        dedup_removed,
        skipped_io_errors,
        ..Default::default()
    };

    let mut per_file_used: HashMap<NodeFilepath, usize> = HashMap::new();
    let mut admitted: Vec<ContextPart> = Vec::with_capacity(parts.len());

    for mut part in parts {
        // Enforce per-part cap via trimming.
        let original_tokens = tokenizer.count(&part.text);
        let (maybe_trimmed, truncated) =
            if budget.per_part_max > 0 && original_tokens > budget.per_part_max {
                trim_text_to_tokens(&part.text, budget.per_part_max, tokenizer)
            } else {
                (part.text.clone(), false)
            };

        let part_tokens = tokenizer.count(&maybe_trimmed);
        if truncated {
            stats.truncated_parts += 1;
        }

        // Enforce per-file cap
        let used = per_file_used.get(&part.file_path).copied().unwrap_or(0);
        if budget.per_file_max > 0 && used.saturating_add(part_tokens) > budget.per_file_max {
            continue;
        }

        // Enforce total cap.
        if stats.total_tokens.saturating_add(part_tokens) > budget.max_total {
            // No more room.
            break;
        }

        // Admit the part
        part.text = maybe_trimmed;
        per_file_used
            .entry(part.file_path.clone())
            .and_modify(|t| *t += part_tokens)
            .or_insert(part_tokens);
        stats.total_tokens = stats.total_tokens.saturating_add(part_tokens);
        admitted.push(part);
    }

    stats.parts = admitted.len();
    stats.files = per_file_used.len();

    AssembledContext {
        parts: admitted,
        stats,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approx_char_tokenizer_counts() {
        let tk = ApproxCharTokenizer;
        assert_eq!(tk.count(""), 0);
        assert_eq!(tk.count("abcd"), 1);
        assert_eq!(tk.count("abcde"), 2);
        assert_eq!(tk.count("abcdefgh"), 2);
        assert_eq!(tk.count("abcdefghi"), 3);
    }

    #[test]
    fn trimming_obeys_limits() {
        let tk = ApproxCharTokenizer;
        let text = "abcdefghijklmnopqrstuvwxyz"; // 26 chars ~= 7 tokens
        let (t1, trunc1) = trim_text_to_tokens(text, 3, &tk);
        assert!(trunc1);
        assert!(tk.count(&t1) <= 3);
        let (t2, trunc2) = trim_text_to_tokens(text, 10, &tk);
        // For a budget larger than approx tokenized length (~7), truncation should be false.
        assert!(!trunc2);
        assert!(tk.count(&t2) <= 10);
    }

    #[test]
    fn budgeting_obeys_accumulated_total_limit() {
        let tk = ApproxCharTokenizer;
        let budget = TokenBudget {
            max_total: 4,
            per_file_max: 100,
            per_part_max: 2,
        };
        let parts = vec![
            test_part(1, "src/a.rs", "abcdefgh"),
            test_part(2, "src/b.rs", "ijklmnop"),
            test_part(3, "src/c.rs", "qrstuvwx"),
        ];

        let ctx = apply_token_budget(parts, &budget, &tk, 0, 0);

        assert_eq!(ctx.parts.len(), 2);
        assert_eq!(ctx.stats.parts, 2);
        assert!(ctx.stats.total_tokens <= budget.max_total);
    }

    #[test]
    fn dedup_preserves_order() {
        let ids = vec![
            Uuid::from_u128(1),
            Uuid::from_u128(2),
            Uuid::from_u128(1),
            Uuid::from_u128(3),
            Uuid::from_u128(2),
        ];
        let (out, removed) = stable_dedup_ids_ordered(&ids);
        assert_eq!(
            out,
            vec![Uuid::from_u128(1), Uuid::from_u128(2), Uuid::from_u128(3)]
        );
        assert_eq!(removed, 2);
    }

    fn test_part(id: u128, file_path: &str, text: &str) -> ContextPart {
        ContextPart {
            id: Uuid::from_u128(id),
            file_path: NodeFilepath(file_path.to_string()),
            canon_path: CanonPath(format!("test::{id}")),
            ranges: Vec::new(),
            kind: ContextPartKind::Code,
            text: text.to_string(),
            score: 1.0,
            modality: Modality::Sparse,
        }
    }
}
