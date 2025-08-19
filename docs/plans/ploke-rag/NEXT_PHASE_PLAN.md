# Next Phase Implementation Plan: Ephemeral Edit Overlay + Agentic RAG

This plan turns the objectives in NEXT_PHASE_OBJECTIVES.md into concrete, incremental work scoped to the current codebase. It references existing crates, modules, and APIs from project_context.txt and preserves current conventions.

Goals
- Deliver a TUI-first ephemeral edit overlay that never writes to the repo unless accepted.
- Provide live lint/compile/test on the overlay with deterministic diagnostics mapping back to message blocks.
- Add an accept/reject pipeline with conventional commits for accepts.
- Introduce a pragmatic context assembly (BM25 + dense) constrained by code-graph adjacency.
- Prepare foundations for autonomy and memory without changing existing DB schemas.

Non-goals
- No full IDE; no unbounded FS writes in autonomous mode; no new DB schemas in this phase.

Milestones (incremental)
1) Overlay scaffolding in TUI (UI state, actions, eventing) and local diff storage.
2) Ephemeral workspace + cargo check/clippy/test runners, diagnostics mapping.
3) Accept/Reject flows with preflight checks and commit template.
4) Context assembly: BM25 + dense + graph constraints; budget-aware packing.
5) Autonomous mode foundations (policy-gated run loop reusing overlay), memory scaffolding, docs generator, and incremental type-resolution tasks.

----------------------------------------------------------------------------------------------------

Architecture and Work Breakdown

A) Ephemeral Edit Overlay in TUI

A1. Data model and state
- New overlay model (ploke-tui)
  - Add Mode::EditOverlay to crates/ploke-tui/src/app/types.rs.
  - Extend MessageKind or introduce an OverlayMessage kind in the same module so overlay conversations render with existing RenderMsg.
  - New overlay types in ploke-tui/src/app/overlay.rs (new file):
    - OverlayId (Uuid)
    - OverlayPatch: target_path (PathBuf), unified_diff (String), source_language (String), derived_line_map (see A3)
    - OverlaySet: ordered list of patches + metadata (created_at, author)
  - Extend app_state to hold an Option<OverlaySet> and a focused patch index.

A2. Input and events
- Actions (crates/ploke-tui/src/app/input/keymap.rs)
  - New actions: EnterOverlayMode, NextPatch, PrevPatch, ToggleDiff, SaveOverlayEdits, DiscardOverlay, RunCheckOnOverlay, RunTestsOnOverlay, AcceptOverlay, RejectOverlay.
  - Map to keys only in EditOverlay mode; reuse to_action with CommandStyle.
- Event bus (crates/ploke-tui/src/app/events.rs)
  - Add AppEvent variants: OverlayUpdated(OverlayId), OverlayDiagnostics(OverlayId), OverlayClosed(OverlayId), OverlayAccepted(OverlayId), OverlayRejected(OverlayId).
  - The existing EventBusCaps plus MessageUpdatedEvent will remain; new events should be emitted via the current send() APIs (app_state/events.rs).

A3. Span/line mapping from overlay to repo
- Create a small line-map utility in TUI (ploke-tui/src/app/overlay_line_map.rs, new file) that:
  - Parses unified diffs in OverlayPatch into hunks.
  - Maintains bidirectional mapping between (file, overlay_line) and (file, base_line).
  - Exposes translate_overlay_to_base(file, line) -> Option<usize>.
- Optionally reuse concepts from ploke-db/src/span/tracker.rs semantics; for now keep it local to TUI to avoid DB coupling.
- Keep it allocation-light; follow scripts/no_gratuitous_collect.sh guard (avoid collect+extend antipatterns).

A4. LLM integration (streamed edits)
- Integrate in crates/ploke-tui/src/llm/mod.rs:
  - When an assistant response contains code blocks tagged for edits, construct/append OverlayPatch records incrementally.
  - Use existing CommandSender to try_send UpdateMessage-like commands for overlay changes; if backpressure, fall back to await send (preserve current behavior).
  - Debounce rendering updates to avoid excessive UI churn.

Acceptance criteria for A
- Users can toggle EditOverlay mode, view an in-TUI diff per patch, navigate between patches, and save/discard overlay content locally (in app state).
- No repository files are written in this phase.
- Unit tests cover line-map for insertions/deletions at file start/middle/end.

B) Live lint/compile on the overlay

B1. Ephemeral workspace materialization (ploke-io)
- New module ploke-io/src/overlay/mod.rs:
  - OverlayWorkspace { root: PathBuf, head_rev: String, created_at: Instant }
  - fn materialize(head_only: bool) -> Result<OverlayWorkspace, Error>
    - Strategy: export tracked files using git archive to a temp dir under target/ploke_overlay/<uuid> to avoid touching the repo and .git.
      - Command: git -C <workspace_root> archive --format=tar HEAD | tar -xC <temp_dir>
    - Record head_rev via git rev-parse HEAD for provenance.
  - fn apply_overlay(&self, patches: &[OverlayPatch]) -> Result<(), Error>
    - Apply by writing edited files directly (compute patched contents from unified diff).
    - Validate that target_path is within repo (defense-in-depth).
- Note: do not modify workspace repo; all edits stay in the ephemeral dir.

B2. Runners for cargo check/clippy/test (ploke-io)
- New module ploke-io/src/overlay/sandbox.rs:
  - CargoRunner running in the ephemeral dir:
    - check: cargo check -q --message-format=json
    - clippy: cargo clippy -q --message-format=json -- -D warnings (configurable)
    - test: cargo test -q --message-format=json
  - Stream JSON messages; parse diagnostics minimally (package_id, target, file, line, column, message, level).
  - Map diagnostics to overlay lines with A3 mapping; then map overlay lines back to message blocks by path and range.

B3. TUI integration for diagnostics
- App events: OverlayDiagnostics(OverlayId) carries a structured diagnostics payload.
- UI rendering: new panel in conversation view to show summarized diagnostics sorted by severity, clickable to jump to patch hunk.
- Commands wired in keymap: RunCheckOnOverlay and RunTestsOnOverlay trigger async tasks; results update in-place.

Acceptance criteria for B
- Running lint/compile/test does not modify the real repo.
- Diagnostics display with correct file and line mapping for common edit cases.
- Backpressure-safe command sending is preserved (no deadlocks).

C) Accept/Reject pipeline

C1. Preflight
- Before accept:
  - Verify clean application against the current repo state: re-diff overlay vs. HEAD and check no path escapes or conflicts.
  - Optionally re-run cargo check to ensure no compile errors.

C2. Apply + commit
- Apply unified diffs to the actual repo (use in-memory patch writer, not shell git apply).
- Commit with conventional template:
  - feat(tui-overlay): <short summary> [overlay:<uuid>]
  - Body: include message IDs, files changed, and a one-line hash of the patch set.
- Add a toggle to auto-run cargo check post-commit and revert if desired policy fails (policy gate).

C3. Reject
- Drop the overlay from TUI state; leave chat history intact.

Acceptance criteria for C
- Accept applies edits atomically to the repo and creates a single conventional commit.
- Reject leaves no filesystem changes.

D) Context assembly for LLM actions (BM25 + dense + graph constraints)

D1. Dense and BM25 retrieval
- Use existing DB APIs in crates/ploke-db/src/database.rs:
  - Prefer typed helpers: get_nodes_ordered, get_common_nodes, get_nodes_by_file_with_cursor where relevant.
  - For BM25: use Bm25Indexer (crates/ploke-db/src/bm25_index/mod.rs) and existing upsert_bm25_doc_meta_batch integration points. If no corpus yet, build a transient in-memory index at TUI start or lazy-init on first query.
- Use embedding providers (crates/ingest/ploke-embed/src/providers/{openai,hugging_face}.rs) via EmbeddingService trait to score dense similarity for top-K candidates from BM25.

D2. Graph constraints
- Use syntax_edge, module, function, struct relations via Database::raw_query for scoped queries (do not require schema changes).
- Constrain context to a neighborhood around the user-selected module/file/function. Favor items sharing ModuleNode ancestry (syn_parser graph surfaces).
- Rank fusion: weighted sum of normalized BM25 and dense scores; tie-break by proximity in the code graph.

D3. Budget-aware packing
- Implement a TokenBudgetAllocator in ploke-rag/src/context/ (new module) that:
  - Measures token cost of each snippet (estimate from char length or tokenizer if already available).
  - Packs highest utility items until budget is reached; reserve budget for the user prompt and system prefixes.

Acceptance criteria for D
- For a test query, retrieved snippets come from relevant files/modules and are capped by a token budget.
- Quick unit tests verify BM25-only, dense-only, and fused behavior.

E) Foundations for autonomy and learning

E1. Autonomous mode (agent loop; TUI or daemon-driven)
- Loop: propose -> materialize overlay -> check/clippy/test -> score -> accept/reject by policy.
- Policies:
  - Hard gate: no compile errors; unit tests unaffected or improved; optional clippy -D warnings clean.
  - Scope: only modify files in a configured allowlist or crate subset.
- Execution: reuse OverlayWorkspace and CargoRunner; produce trace logs.

E2. Memory layer (no DB schema changes yet)
- Introduce ploke-rag/src/memory/ (new module):
  - In-memory kmers/BM25 for recent decisions; optional JSON persistence under target/ploke_memory/.
  - Use existing EmbeddingService to embed memories transiently; fuse them into context assembly without writing to DB.
- Provide a thin abstraction so DB backed memory can be swapped in later.

E3. Documentation graph
- Generator in ploke-rag/src/docs/ (new module):
  - Query DB relations (crate_context, module, function, syntax_edge) and emit Markdown docs under ploke-rag/docs/generated/.
  - Summaries include module trees, item counts, and examples.
- Hook a simple cargo xtask or a binary in ploke-rag to produce docs on demand.

E4. Incremental type-resolution completion
- Identify remaining gaps (see syn_parser docs and TODOs). Add tests under crates/ingest/syn_parser/tests/... to cover niche generics/where-clause cases.
- No schema changes; focus on parser/visitor refinements and improved edge insertion via ploke-transform.

Acceptance criteria for E
- Sandbox agent can run end-to-end on a trivial change with policies.
- Memory can be included in retrieval without DB writes.
- Docs generator produces stable outputs from a known fixture crate.

----------------------------------------------------------------------------------------------------

Cross-cutting concerns

Telemetry and logging
- Use tracing throughout new modules (overlay, sandbox, context assembly).
- Set levels via RUST_LOG; consider a per-module span for each overlay session.

Performance and correctness
- Overlay materialization via git archive minimizes I/O and avoids .git coupling.
- Ensure path sanitation (no symlink escapes, no writes outside ephemeral dir).
- Honor scripts/no_gratuitous_collect.sh by avoiding needless allocations in iterators.

Testing strategy
- TUI: state-machine unit tests for mode transitions, overlay patch navigation, and diagnostics ingestion.
- IO/sandbox: integration tests spawning cargo check/test on a tiny fixture crate in tests/fixture_crates via common::fixtures_crates_dir().
- RAG: deterministic tests for BM25+embeddings fusion using small synthetic corpora and seeded RNG.

Rollout plan
- Land each milestone behind feature flags in ploke-tui user_config and per-crate cfg(feature = "overlay") if needed.
- Default off until milestone B is stable; enable gradually for internal testing.

Appendix: Likely touched modules (for future PRs)
- ploke-tui
  - src/app/types.rs (Mode, MessageKind)
  - src/app/input/keymap.rs (new Action variants, mapping)
  - src/app/events.rs (new events)
  - src/app_state/handlers/{indexing,db}.rs (wiring diagnostics and RAG requests)
  - src/app/view/components/conversation.rs (render overlay/diff/diagnostics)
  - src/llm/mod.rs (stream integration)
  - New: src/app/overlay.rs, src/app/overlay_line_map.rs
- ploke-io
  - New: src/overlay/mod.rs, src/overlay/sandbox.rs
- ploke-rag
  - New: src/context/budget.rs, src/context/retrieval.rs
  - New: src/memory/mod.rs
  - New: src/docs/generate.rs
- ploke-db
  - Prefer existing APIs. If needed, use Database::raw_query for graph-scoped retrieval; avoid schema changes this phase.

Conventional commit template (accept)
- Type: feat or refactor (fallback: chore)
- Scope: tui-overlay (or affected crate)
- Subject: concise summary
- Body: overlay uuid, files changed list, patch hash, “co-authored-by: assistant/<model>” optional

Risks and mitigations
- Diagnostic mapping drift: use robust hunk-based line maps; add tests for edge cases.
- Large repos: use git archive export and on-demand patching for only changed files to reduce IO.
- Backpressure or UI jank: reuse CommandSender and batch UI updates.

Done definition for the phase
- A/B/C milestones are merged and on by default for internal users.
- RAG retrieval with fused ranking is used by LLM actions, respecting token budget.
- Autonomous mode, memory, and docs generator exist and are usable but off by default.
