# Review: Point 2 — Context assembly and token budgeting

Summary
- The scope and boundaries are good: assembly independent of LLMs; tokenizer via trait injection; IO via existing Database and IoManagerHandle.
- Strong emphasis on determinism and dedup is correct for reproducibility.
- One necessary adjustment: assemble_context should be async because snippet retrieval is async (IoManagerHandle::get_snippets_batch).

Clarifications to address before implementation
- AssemblyPolicy: define an explicit policy type with defaults.
  - ordering: enum { FusedScoreThenStructure, ByFileThenScore } with default FusedScoreThenStructure.
  - include_kinds: set of ContextPartKind to include; default {Code, Doc}.
  - per_type_caps: optional HashMap<NodeType, usize> for fairness across node types.
  - allow_overlap: bool (default false) to permit overlapping ranges if strongly relevant.
  - strict_io: bool (default false) to fail on IO errors vs. skip parts.
- Modality enum: add Dense, Sparse, HybridFused (and possibly Reranked later) to tag sources in ContextPart.
- Dedup keys and canonicalization:
  - Primary: tracking_hash when available; Secondary: (file_path, normalized ranges).
  - Normalize by sorting ranges by (start, end), merging overlapping or touching ranges, clamping to file bounds, and coalescing duplicates.
  - Dedup across modalities and within a file; stability via UUID and path ordering.
- TokenCounter:
  - Keep trait here (ploke-rag) as planned; provide a simple, deterministic, approximate tokenizer for tests (e.g., chars/4 rounded up).
- Async signature for assembly:
  - Proposed: async fn assemble_context(...) -> Result<AssembledContext, RagError>
  - Rationale: IoManagerHandle::get_snippets_batch is async and already used in tests.

Proposed initial API (internal module context)
- struct TokenBudget { max_total, per_file_max, per_part_max, reserves: Option<HashMap<String, usize>> }
- enum ContextPartKind { Code, Doc, Signature, Metadata }
- enum Modality { Dense, Sparse, HybridFused }
- struct ContextPart { id: Uuid, file_path: String, ranges: Vec<(usize, usize)>, kind: ContextPartKind, text: String, score: f32, modality: Modality }
- struct ContextStats { total_tokens: usize, files: usize, parts: usize, truncated_parts: usize, dedup_removed: usize }
- struct AssembledContext { parts: Vec<ContextPart>, stats: ContextStats }
- struct AssemblyPolicy { ordering, include_kinds, per_type_caps, allow_overlap, strict_io }
- trait TokenCounter { fn count(&self, text: &str) -> usize; }
- async fn assemble_context(query: &str, hits: &[(Uuid, f32)], budget: &TokenBudget, policy: &AssemblyPolicy, tokenizer: &dyn TokenCounter, db: &Database, io: &IoManagerHandle) -> Result<AssembledContext, RagError>

Algorithm sketch (deterministic)
1) Fetch metadata: db.get_nodes_ordered(hits uuids) -> Vec<EmbeddingData>; group by file_path.
2) Dedup:
   - Use HashSet of tracking_hash; fallback to (file_path, normalized ranges).
   - Within each file, sort ranges, merge overlaps/touching, drop duplicates.
3) Stitch:
   - Combine contiguous or near-contiguous ranges into larger spans to reduce fragmentation.
   - Keep per-file list ordered by first-byte ascending.
4) Ordering:
   - Start from fused-score order; break ties by UUID asc; optionally group by file depending on policy.
5) Token budgeting:
   - Compute reserves if any; derive available budget.
   - Water-filling: iterate parts by order, admit if within per_file_max/per_part_max and remaining budget.
   - Trim parts to fit per_part_max using tokenizer; record truncated_parts.
6) Retrieval:
   - For admitted parts, fetch text via io.get_snippets_batch; map back to parts; on IO errors: skip or fail per strict_io.
7) Stats:
   - Fill ContextStats; ensure stable output ordering.

Testing plan
- Unit tests for:
  - Range normalization and stitching (overlap/touching/ordering).
  - Dedup correctness using tracking_hash and path/range fallback.
  - Budget enforcement (max_total, per_file_max, per_part_max) with deterministic approximate tokenizer.
  - Deterministic ordering and tie-breaking.
- Integration test:
  - Full assemble_context over fixture DB to ensure end-to-end correctness with IoManagerHandle.

Performance notes
- Sorting O(n log n); merging/stitching O(n); HashSet dedup O(n).
- Batch DB fetch and batch IO are already supported; avoid per-part awaits by batching.
- Keep allocations pre-sized (Vec::with_capacity) for predictable performance.

Risks and mitigations
- Async requirement vs. original signature: adopt async; if a sync facade is needed, provide a thin blocking wrapper for non-async callers.
- Tokenization variance: rely on injected TokenCounter; use a simple deterministic default for tests.
- Large files/ranges: ensure trimming occurs before IO to bound reads where possible; otherwise rely on IO layer to slice by span.

Recommended next steps
1) Add context module with types, TokenCounter trait, and approximate tokenizer; compile-only.
2) Implement range normalization, stitching, and dedup with unit tests (no IO).
3) Implement async assemble_context integrating db + io; add integration tests over fixture DB; instrument tracing fields (budget_used, files_touched, dedup_removed).
