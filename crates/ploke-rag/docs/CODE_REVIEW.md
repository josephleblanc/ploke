# Code Review: ploke-rag

This review covers design, correctness, performance, ergonomics, observability, and testing for the ploke-rag crate based on the provided sources:
- src/lib.rs
- src/core/mod.rs
- src/context/mod.rs
- src/fusion/mod.rs
- src/error.rs
- Cargo.toml

It includes prioritized recommendations and low-risk improvements that maintain current behavior by default.

## High-level Architecture

Strengths:
- Clear separation of concerns:
  - fusion: pure utilities (normalize, RRF, MMR)
  - context: IO + assembly/budgeting
  - core: orchestration and strategy
- Deterministic behavior: stable sort tie-breaking by UUID is consistently applied.
- Good error taxonomy via RagError with explicit mapping to workspace error type.
- Observability: comprehensive tracing on public operations with useful fields (timeouts, retry counts, state).

Opportunities:
- Some “initial wiring” placeholders need follow-through (noted inline below).
- Config vs. hardcoded defaults are occasionally mixed (e.g., RRF usage in `hybrid_search`).
- Costly token trimming loop for large inputs can be optimized without changing behavior.

## API and Ergonomics

- RagService constructors are clear and progressive:
  - new: minimal usage (no IoManager)
  - new_with_config: explicit cfg
  - new_with_io / new_full / new_rebuilt: add IO and BM25 index lifecycle options
- get_context surfaces a strategy enum that’s expressive and future-proof (Dense/Sparse/Hybrid with params).
- Tokenization is abstracted by `TokenCounter` for deterministic tests vs. real LLM adapters later.

Recommendations:
1) Consistency for hybrid RRF defaults:
   - `core::RagService::hybrid_search` uses `RrfConfig::default()` regardless of `RagConfig::rrf_default`.
   - Align it to use `self.cfg.rrf_default` to respect user configuration uniformly.
2) Consider providing a small builder for `RagConfig` to encourage explicit overrides while keeping defaults.

## Correctness and Edge Cases

Context assembly (`assemble_context`):
- Dedup: `stable_dedup_ids_ordered` is correct and preserves order; stats track `dedup_removed`.
- Range handling: placeholder (`ranges: Vec::new()`, `allow_overlap` in policy is a no-op). This is called out by comments; fine for initial iteration.
- File grouping: `file_path` uses a placeholder `id://{uuid}`. This means `per_file_max` = `per_id_max`.
  - Suggestion: `EmbeddingData` likely contains `file_path` and related metadata (as indicated elsewhere in workspace). If available in `db.get_nodes_ordered`, prefer using the real file path for budgeting and reporting.
- Reserves: `TokenBudget.reserves` is subtracted from `max_total` but not otherwise surfaced in the output (no sections are generated).
  - Consider either (a) removing reserves until it’s used, or (b) attaching a field to `AssembledContext` so callers know how many tokens were reserved (e.g., for system prompts).

Token trimming:
- `trim_text_to_tokens` does:
  1) quick approx cut at `max_tokens * 4` chars,
  2) while-loop popping chars until it fits.
- With `ApproxCharTokenizer`, `.count()` is O(n), and repeated popping makes worst-case behavior O(n^2).
  - Suggestion: use a bounded trimming approach:
    - Binary search on char boundary to find the longest prefix with token count <= max.
    - This preserves current behavior but reduces worst-case from O(n^2) to O(n log n) (or O(n) with a two-pointer scan using a monotone predicate).
- Edge case: `max_tokens == 0` returns empty and `truncated=true` (correct and explicit).

Ordering:
- Custom sort closures implement stable tie-break by UUID asc; consistent and correct.
- `ordering_key_by_score_then_id` is defined but unused; can be removed or wired into sorting to avoid duplicated logic.

Dense search scoring:
- `core::search` converts distance to similarity via `1.0 - distance as f32`.
  - If the underlying distance is unbounded or uses different metrics (e.g., cosine distance in [0, 2] or L2 distance), this can yield negative or non-comparable scores.
  - Consider a safer mapping (e.g., `1.0 / (1.0 + distance)` or a normalization via `ScoreNorm`) if the metric type is not guaranteed to be [0, 1].

Hybrid and MMR:
- `get_context` Hybrid path fuses RRF (respecting the passed `rrf` argument) and optionally applies MMR.
- MMR is fed an empty `embed_map`, so diversity penalty is zero, making it equivalent to a simple top-k by fused relevance.
  - Consider wiring real embeddings for candidates (e.g., via `ploke-db` vectors) when available; otherwise, leave MMR off to avoid confusion.
  - Alternatively, add a clear docstring: “MMR has no effect without candidate embeddings.”

Reranker:
- When IO errors occur during reranking fetches, behavior depends on `assembly_policy.strict_io`.
  - If strict, error is returned; else, skip partial candidates. This is fine but should be mentioned in API docs of `Reranker`.

BM25 fallback logic:
- Lenient `search_bm25`: attempts, backoff, status-based fallback to dense. Behavior is sensible and well-instrumented.
- Strict variant returns errors when appropriate; good boundary definition.

## Performance

- Token trimming worst-case O(n^2) as noted.
- Sorting/collecting in fusion and MMR is straightforward. MMR normalizes vectors upfront once; good.
- `assemble_context` builds vectors and hash maps with capacity hints; nice touch.
- IO path is batched: `get_nodes_ordered` then `get_snippets_batch`; good.

Potential improvements:
- When using `ByFileThenScore`, if many parts from the same file exceed `per_file_max`, extra work is done to count tokens in parts that will be skipped later.
  - Consider quickly skipping once a file’s cap is exhausted to avoid counting on subsequent parts in the same file.

## Observability & Error Handling

- Tracing fields are informative and consistent. The crate disables tracer init outside tests to avoid double-registration; `ensure_tracer_initialized()` is currently commented out.
  - Choose a policy: either expose an explicit opt-in initializer or remove the unused scaffolding to reduce confusion.
- RagError -> ploke_error::Error mapping:
  - `Embed` and `Search` map to `NotImplemented` variants, which may be misleading. Consider `CompilerError` or a dedicated variant if available.
- Channel and DB errors preserve detail strings; good.

## Testing

- fusion: good coverage for normalization and logistic; consider adding MMR unit tests to cover:
  - tie-breaking behavior
  - lambda extremes (0 and 1)
  - effect of candidate pool truncation
- context: unit tests for tokenizer, trimming, and dedup. Consider:
  - assembling with small budgets (per_file and total) to exercise skipping and truncation
  - strict_io behavior (simulate IO errors and verify outcomes)
  - ordering policy variations (FusedScoreThenStructure vs. ByFileThenScore)
- core: there’s a `mod unit_tests;` include, not shown here. Ensure coverage for:
  - bm25 fallback paths (Uninitialized/Building/Empty)
  - hybrid fusion reproducibility and UUID tie-break stability
  - dense scoring transformations

## Security and Safety

- The crate primarily processes internal IDs and text snippets; no obvious unsafe code or external inputs are handled here.
- Ensure any future tokenizer adapters are careful with non-UTF8 or large inputs and avoid quadratic behavior.

## Style and Maintenance

- Remove unused items: `ordering_key_by_score_then_id` appears unused.
- Prefer consistent use of configured defaults versus hardcoded defaults (RRF in `hybrid_search`).
- Cargo.toml includes `tracing-subscriber` but the code path to initialize tracing is commented out; either gate with a feature or remove if not needed.

## Prioritized Recommendations

1) Correctness/Consistency
- In `RagService::hybrid_search`, use `self.cfg.rrf_default` instead of `RrfConfig::default()` for fusion.
- Revisit dense score mapping to be safe for the distance metric in use (e.g., `1 / (1 + d)`).

2) Performance
- Replace the pop-until-fit loop in `trim_text_to_tokens` with a binary search or monotone two-pointer on the character boundary using `tokenizer.count` as the monotone predicate.

3) UX/Clarity
- If available, set `ContextPart.file_path` from `EmbeddingData` so per-file budgeting actually means per-file, not per-UUID.
- Document that MMR is a no-op without embeddings; consider disabling it by default unless embeddings are supplied.

4) Cleanup
- Remove or use `ordering_key_by_score_then_id`.
- Decide on tracing initialization strategy; remove commented scaffolding or expose an opt-in initializer.

5) Tests
- Add MMR tests and context assembly budgeting/ordering tests.
- Add bm25 fallback unit tests covering status transitions and backoff.

## Low-risk Nits

- Replace duplicated score-tie sort closures with a shared helper to avoid divergence.
- Consider adding an overall `files_truncated` (count of files where per_file_max was hit) to stats for better telemetry.
- Use `#[derive(Default)]` where feasible for config structs with obvious defaults.

## Summary

ploke-rag is thoughtfully structured with clear separation of concerns and strong observability. Its current implementation reliably wires together BM25/dense/hybrid retrieval and context assembly under a token budget. The main improvements are consistency in configuration usage, tightening a couple of performance edges in token trimming, and clarifying placeholders (file_path, reserves, MMR embeddings). With a few targeted changes and tests, it’s well-positioned for production-ready stability.
