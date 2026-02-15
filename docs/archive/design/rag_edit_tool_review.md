# Code Review: Canonical-Path Rewrite Format for apply_code_edit

This document evaluates the recent refactor of the code edit tool in crates/ploke-tui/src/app_state/handlers/rag.rs from byte-range + hash arguments to a concise, LLM-friendly format:
- action: "code_edit"
- file: path relative to crate root (or absolute)
- canon: canonical path, e.g., module_one::example_module::Thing
- node_type: relation name, e.g., function, struct, enum, …
- code: full item rewrite text

It also compares alternate approaches and suggests incremental improvements.

## Evaluation Criteria (0–5)
1) Simplicity and Readability
- How easy is it to read/understand/update the handler code and the tool’s interface?

2) Correctness and Safety
- Guarding against stale edits, invalid ranges, mismatched hashes, UTF-8 boundaries, and ensuring @ 'NOW' snapshot semantics.

3) LLM Ergonomics
- Minimizes cognitive load for models (no byte offsets or hashes), consistent, self-explanatory schema.

4) Maintainability and Extensibility
- Typed request/response, clear seams for future features (file creation, body-only edits, AST-aware variants, multi-step flows).

5) Observability and Testability
- The ability to preview, diff, log, and write tests around well-defined seams.

6) Performance and Scalability
- Efficient DB lookup, single-pass staging, minimal string copies.

7) Architectural Fit
- Conforms to ploke-io content-lock invariants, makes use of ploke-db schema and @ 'NOW', works with event bus, approvals, and proposals.

## Code Review of the Current Patch

Summary of changes:
- New serde-based input types: ApplyCodeEditArgs, EditInput, Action.
- Simplified LLM format: no byte spans or hashes; internal resolution from (file, canon, node_type).
- Prototype Cozo query to resolve canonical path → EmbeddingData (span, file hash) at @ 'NOW'.
- Converts EmbeddingData into WriteSnippetData and stages a proposal with preview (diff or code blocks).
- Keeps get_file_metadata tool intact.
- Preserves IoManager’s token-based TrackingHash guard.
- Emits concise SysInfo messages and retains approval flow.

Strengths:
- Simplicity/Ergonomics: Major improvement for LLMs; no longer emits brittle mechanical fields.
- Safety: Still honors content-lock invariant via expected_file_hash from DB. UTF-8 boundaries enforced by ploke-io.
- Maintainability: Most of the previous ad-hoc JSON parsing is gone, replaced by serde. The request schema is cohesive and future-proof.
- Observability: Preview, per-file unified diff, and proposals remain intact.

Known risks / tradeoffs:
- DB lookup is implemented via a PROTOTYPE Cozo script inlined as a string with JSON literals. This may break for some reason (Cozo string/list literal mismatches, ambiguity in module path matching).
- Ambiguity handling: If multiple candidates are found, we fail and ask for disambiguation. That’s good, but we could add an optional kind (typed NodeType) to preempt this entirely.
- Path normalization: The initial version joined crate_root + file; the patch now attempts a better absolute path inference but still depends on project configuration and DB conventions (absolute file paths).
- @ 'NOW' usage: The query includes @ 'NOW' on the fields we touch, as required. The exact path/field names depend on the current schema.

Scores:
- Simplicity/Readability: 4.5
- Correctness/Safety: 4.0 (would reach 4.5–5.0 once query is parameterized/centralized)
- LLM Ergonomics: 5.0
- Maintainability/Extensibility: 4.0 (would improve with typed NodeType and DB helper)
- Observability/Testability: 4.0
- Performance/Scalability: 4.0
- Architectural Fit: 4.5
Total: 30.0 / 35

## Alternate Solutions and Comparison

We consider five alternatives and compare them against the current solution.

A1) Legacy Byte-Range + Hash (pre-refactor)
- Description: LLM supplies file_path, expected_file_hash, start_byte, end_byte, replacement.

Pros:
- Direct, minimal DB involvement; mechanical application.
- Simple to validate overlapping edits client-side.

Cons:
- Very poor LLM ergonomics; brittle and error-prone; high failure rate in practice.
- Requires the LLM to obtain and propagate hashes and exact byte offsets.

Scores:
- Simplicity: 2.5
- Correctness: 4.5
- Ergonomics: 1.5
- Maintainability: 3.0
- Observability/Testability: 3.5
- Performance: 4.5
- Architectural Fit: 4.5
Total: 24.5 / 35

A2) Canonical Path + Node Type (string) + File + Code (Current)
- Description: LLM supplies "action","file","canon","node_type","code"; server resolves to span/hash via DB.

Pros:
- Big ergonomic win; hash/byte-range hidden.
- Minimal changes to ploke-io and approval flow.
- Compatible with @ 'NOW' and schema.

Cons:
- Stringly-typed node_type; ad-hoc Cozo script inlined; potential fragility.
- Requires crate_root path normalization to match DB.

Scores:
- Simplicity: 4.5
- Correctness: 4.0
- Ergonomics: 5.0
- Maintainability: 4.0
- Observability/Testability: 4.0
- Performance: 4.0
- Architectural Fit: 4.5
Total: 30.0 / 35

A3) Canonical Path + Typed NodeType enum + File + Code (Recommended Next)
- Description: Same as A2 but parse node_type to a typed enum (serde), mapping via relation_str; use a centralized DB helper (e.g., Database::get_pnode_from_canon) instead of ad-hoc scripts.

Pros:
- Eliminates stringly-typed errors; reduces query fragility via a single, tested helper.
- Easier to extend and test; better compile-time safety.

Cons:
- Requires adding serde to NodeType and implementing the DB helper method (not in scope with current files).
- Minor additional code to translate typed NodeType into relation logic.

Scores:
- Simplicity: 4.5
- Correctness: 4.5
- Ergonomics: 5.0
- Maintainability: 4.5
- Observability/Testability: 4.5
- Performance: 4.0
- Architectural Fit: 4.5
Total: 31.0 / 35  ← Highest

A4) Anchor-Based Edits (before/after markers) + File + Code
- Description: The LLM supplies anchors and new code; server locates span via textual anchors.

Pros:
- No DB dependency; robust when code shifts but anchors remain.
- Natural for LLM.

Cons:
- Ambiguity risk (multiple matches); sensitive to formatting changes.
- Harder to guarantee exact item replacement; weaker invariants than AST/DB-based approach.

Scores:
- Simplicity: 4.0
- Correctness: 3.5
- Ergonomics: 4.5
- Maintainability: 3.5
- Observability/Testability: 3.5
- Performance: 3.5
- Architectural Fit: 3.5
Total: 26.0 / 35

A5) Node UUID-Based Edits
- Description: LLM uses a node UUID instead of canonical path.

Pros:
- Precise, no ambiguity.

Cons:
- Unfriendly to LLMs; requires exposing internal IDs in context; brittle across re-indexing.
- Low human readability.

Scores:
- Simplicity: 3.5
- Correctness: 4.5
- Ergonomics: 2.0
- Maintainability: 3.5
- Observability/Testability: 3.0
- Performance: 4.0
- Architectural Fit: 4.0
Total: 24.5 / 35

A6) Two-Step Tooling: resolve_node → apply_code_edit
- Description: First resolve (file,canon,node_type) to a server-side handle; then apply edit by handle.

Pros:
- Reduces complexity of a single call; encourages confirmation/preview step between resolve and apply.
- Can cache resolution and avoid repeated DB lookups.

Cons:
- More round-trips; greater tool complexity; harder to prompt correctly.
- Requires additional tool and UI wiring.

Scores:
- Simplicity: 3.5
- Correctness: 4.5
- Ergonomics: 4.0
- Maintainability: 4.0
- Observability/Testability: 4.0
- Performance: 4.0
- Architectural Fit: 4.0
Total: 28.0 / 35

## Chosen Approach

- Near-term: Keep A2 (current) with small robustness improvements (path normalization fix included in patch).
- Mid-term (recommended): Move to A3 (typed NodeType + centralized DB helper). This yields the best overall score and reduces fragility in the Cozo query and node_type parsing.

## Additional Improvements

- Centralize the canonical-path resolution:
  - Implement Database::get_pnode_from_canon(canonical_path: &[&str], ty: NodeType) -> Result<EmbeddingData, Error>
  - Ensure all fields use @ 'NOW'; prefer parameterized queries instead of string interpolation.

- Adopt serde for NodeType:
  - serde rename mapping to/from relation_str strings (“function”, “struct”, …).
  - Use this in ApplyCodeEditArgs directly to remove ALLOWED_RELATIONS and string validation.

- Tests:
  - Unit tests for A2/A3 handler with small fixtures; golden tests for diff previews.
  - Integration tests for ambiguous matches, stale hash handling, and UTF-8 boundary preservation.

- Observability:
  - Add tracing spans for DB resolution and preview generation.
  - Record per-edit resolution timing and error causes for telemetry.

- Future features:
  - Body-only edits (requires AST slicing) — gate behind additional field (edit_scope: item|body).
  - File creation via a new IoManager op (atomic create + content-lock on empty-old state).

## Conclusion

The current refactor is a substantial improvement. It simplifies the tool for LLMs, maintains safety via internal hash checks, and keeps a clean UI flow with staged previews and approvals. The top follow-up is to adopt typed NodeType + centralized DB helper (A3), which would further increase correctness and maintainability and reduce the fragility of the inline Cozo query.
