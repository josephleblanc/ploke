Executive summary
- Keep the TUI-first ephemeral overlay and sandbox plan, but strengthen determinism, scale, and agent-readiness.
- Adopt a multi-stage retrieval policy (BM25 → graph-pruned candidate set → dense rerank → diversity via MMR) with token-budget packing and transparent provenance.
- Add a lightweight evaluation harness (SWE-bench style) to pressure-test autonomy loops early.
- Make “overlay → run → diagnose → revise” a first-class state machine usable by both humans (TUI) and agents (daemon).
- Add low-friction memory (skill library + episodic memory) that doesn’t change DB schemas but integrates naturally with your graph.

Are these approaches the best way? Short assessment
- Ephemeral overlay with git-archive is solid for safety and simplicity. Add an automatic fallback to git worktree --detach for repos needing .git context (submodules, build.rs querying VCS). Keep writing to target/ploke_overlay; never touch the repo.
- Live cargo check/clippy/test in the sandbox is correct. Add stable toolchain pinning, shared registry cache, and optional shared incremental compilation cache to make this viable on large repos.
- Accept/Reject with conventional commits is right. Add commit message guards and optional auto-rollback on failing policy checks.
- Retrieval: BM25 + dense with graph constraints is the right direction. Improve with staged retrieval, MMR diversification, and explicit provenance. No schema changes needed.
- Foundations for autonomy: reuse overlay; design a deterministic agent loop with policy gates and a small skill library. Good; add an eval harness and replayable traces.

Key deltas from v1 plan
- Determinism and speed: pin rust-toolchain, isolate CARGO_TARGET_DIR per overlay but optionally reuse the host target for incremental builds behind a hash key; add cargo --frozen and --offline modes where possible.
- Ephemeral workspace: prefer git archive, but detect when build needs VCS context and fall back to git worktree --detach; validate path dependencies exist inside the workspace copy.
- Diagnostics mapping: unify overlay hunk mapping with a small SpanMap trait so both TUI and IO layers share logic; fuzz test mappings.
- Retrieval: stage candidates with BM25, cut via code-graph scope, rerank with dense (HNSW), then diversify via MMR; show provenance and scores; budget-aware packing with a simple utility-based knapsack that reserves budget for instructions and system prompts.
- Agent loop: explicit state machine (Propose → Materialize → Check → Test → Score → Decide → Apply/Refine/Abort) with timeouts and rollback; keep the same loop usable from TUI.
- Memory: add skill library (Voyager-style) and episodic memory without DB changes (files under target/ploke_memory/) and fuse into retrieval.
- Evaluation: add a tiny SWE-bench-style runner on fixture repos to compare agent policies and retrieval settings.

Milestones (updated)
1) Overlay v1.1: robust diff parsing + shared SpanMap, mode and events, scrollable hunks, local persistence in app state.
2) Sandbox v1.1: materialize via git archive with worktree fallback; cargo runners with toolchain pinning; deterministic diagnostics mapping; shared target cache option.
3) Accept/Reject v1.1: atomic apply with preflight re-diff; conventional commit guardrails; optional post-commit policy check with auto-revert.
4) Retrieval v2.0: staged search (BM25 → graph-prune → dense → MMR), budget packer, transparent provenance; background BM25/HNSW refresh.
5) Agent foundations v2.0: shared overlay-run state machine; policy gates; skill library + episodic memory; evaluation harness and traces; docs generator for transparency.

Detailed design and brief code deltas

A) TUI Overlay and line mapping
- Overlay state
  - Add Mode::EditOverlay; overlay types overlay.rs: OverlayId, OverlayPatch { target_path, unified_diff, derived_line_map, source_language }, OverlaySet { patches, created_at, author }.
  - Store Option<OverlaySet> in AppState, plus focused_patch_idx.
- Events and input
  - New actions: EnterOverlayMode, NextPatch, PrevPatch, ToggleDiff, SaveOverlayEdits, DiscardOverlay, RunCheckOnOverlay, RunTestsOnOverlay, AcceptOverlay, RejectOverlay.
  - New events in AppEvent: OverlayUpdated(OverlayId), OverlayDiagnostics(OverlayId, payload), OverlayClosed(OverlayId), OverlayAccepted(OverlayId), OverlayRejected(OverlayId).
- Line mapping
  - overlay_line_map.rs: implement a small Hunk parser and SpanMap trait:
    - translate_overlay_to_base(file, line) -> Option<usize>
    - translate_base_to_overlay(file, line) -> Option<usize>
  - Reuse semantics from ploke-db span tracker conceptually, but keep code local to TUI for now.
  - Tests for insertions/deletions at start/middle/end and multi-hunk files; add simple fuzz with random diffs to catch corner cases.
- LLM integration
  - In llm/mod.rs, stream code-block edits into OverlayPatch; debounce UI update via ticking timer; backpressure-safe via existing EventBus try_send with fallback to send.

B) Ephemeral sandbox and diagnostics
- Materialization
  - ploke-io/src/overlay/mod.rs: OverlayWorkspace { root, head_rev, created_at }.
  - materialize(head_only: bool) -> Result<OverlayWorkspace, Error>
    - Default: git -C <root> archive HEAD to temp dir target/ploke_overlay/<uuid>.
    - Detect VCS-needed builds (submodules present, or env var overlay.require_git=1) → fallback: git worktree add --detach <tmpdir> HEAD; sanitize to avoid modifying repo.
  - apply_overlay(&self, patches: &[OverlayPatch]) -> Result<(), Error>
    - Apply unified diffs to in-memory strings and write files; normalize paths; reject escapes.
- Runners
  - ploke-io/src/overlay/sandbox.rs: CargoRunner
    - Set rust-toolchain from workspace or require explicit toolchain; set CARGO_TARGET_DIR to target/ploke_overlay/targets/<hash>; consider sharing incremental caches behind a hash of HEAD + Cargo.lock + toolchain to get fast iterations.
    - check: cargo check -q --message-format=json --frozen (fallback to unlocked if needed)
    - clippy: cargo clippy -q --message-format=json -- -D warnings (configurable)
    - test: cargo test -q --message-format=json
    - Parse JSON messages minimally; map to overlay coordinates via SpanMap; stream diagnostics to TUI through OverlayDiagnostics event.
- Determinism and performance
  - Provide offline toggle; prewarm crates io cache; configurable timeouts; propagate top-N diagnostics first for perceived latency.

C) Accept/Reject with policy gates
- Preflight
  - Rediff overlay vs HEAD in memory; ensure no path escapes and file existence has not changed in ways that break application; optionally re-run cargo check in the real repo (dry-run apply to temp copy).
- Apply + commit
  - Apply with in-memory patcher; single conventional commit:
    - feat(tui-overlay): summary [overlay:<uuid>]
    - Body: message IDs, files changed, patch-set hash, co-authored-by line.
  - Post-commit policy gate (optional): cargo check; optional test batch; auto-revert commit on failure or require user override.
- Reject
  - Drop overlay from TUI state; keep chat history as the audit.

D) Retrieval v2.0 (SWE-search, SE-Agent style)
- Staged retrieval
  - Stage 1: BM25 (lexical) → top 1–2k tokens or top N files/functions.
  - Stage 2: Graph constraints (Database::raw_query) to prune by proximity: ancestors in module tree, syntax_edge adjacency, same file, same crate; prefer items connected to user focus.
  - Stage 3: Dense similarity on pruned set using HNSW (ploke-db/index/hnsw.rs), with embeddings from ploke-embed providers; fall back if embeddings missing; schedule background embedding jobs using existing pending embeddings counters.
  - Stage 4: Diversity with MMR to avoid redundancy; keep small cross-file variety.
- Fusion and budgeting
  - Normalize BM25 and dense scores; fuse via weighted sum or RRF; MMR for diversity; then pack with TokenBudgetAllocator (new ploke-rag/src/context/budget.rs) that reserves headroom for system and user prompts.
- Provenance and transparency
  - For each snippet: show source, graph path summary, BM25 score, dense score, final rank; cache this for explainability and debugging.
- Implementation
  - ploke-rag/src/context/retrieval.rs: RetrievalEngine that orchestrates BM25, graph-prune, dense rerank, MMR; configurable via RagConfig.
  - Background services: reuse ploke-db bm25_service and HNSW index creation routines; add “refresh” on idle.

E) Agent foundations v2.0 (SE-Agent, Voyager, AutoRover lessons)
- Shared state machine
  - Define an OverlayRunSM usable from TUI and daemon:
    - Propose (generate or edit patches)
    - Materialize overlay
    - Check/clippy/test
    - Score (compile/test pass, coverage delta optional, lint clean)
    - Decide (Accept/Refine/Abort)
    - Apply and commit or iterate
  - Implement as a small actor with explicit timeouts, retries, and backoff; all transitions emit tracing spans and AppEvents.
- Policy gates
  - Hard gate: no compile errors; tests unchanged or improved (configurable); optional clippy -D warnings.
  - Scope: only files within allowlist or crate subset; patch size and file count caps.
- Skill library (Voyager-style)
  - New ploke-rag/src/memory/skills.rs: append-only JSONL store of reusable “skills”:
    - e.g., “Add feature flag with cfg”, “Create trait and impl for new id type”, “Refactor to typed ids”
    - Each skill has: name, preconditions, steps, patch templates, verification tips.
  - Agent retrieves skills by BM25 + dense + tag filters; uses as tool hints before proposing patches.
- Episodic memory
  - ploke-rag/src/memory/mod.rs: recent decisions indexed with BM25 + optional embeddings; integrated into retrieval as extra context; persisted under target/ploke_memory/.
- Evaluation and traces
  - ploke-rag/src/agent/eval.rs: small harness to replay tasks (SWE-bench-like) on fixture repos; compare policies and retrieval configs; store results and traces under target/ploke_traces/.
  - Keep deterministic replay: record overlay patches, runner outputs, and decisions.

F) Documentation generator and transparency
- ploke-rag/src/docs/generate.rs
  - Traverse crate_context, module, function, syntax_edge to emit module trees, symbol counts, and example snippets; write to docs/generated/.
  - Useful for users and for agent grounding.

SoTA alignment notes
- SWE-search: multi-stage retrieval and careful lexical-first candidate gen with later semantic rerank is incorporated. We add graph-aware pruning to reduce hallucinated context and tighten locality.
- SE-Agent: iterative propose-evaluate loop with policy gates and explicit scoring; we add MMR for diverse context and skill hints to stabilize edits.
- Voyager: skill library with reusable tool recipes; incremental learning from past successes; simple curriculum by escalating scopes after success.
- AutoRover: hierarchical planning and safety gates; we mirror this with high-level policy constraints, rollbacks, and timeouts. For later phases, consider hierarchical tasks (goal → subgoals → patch steps) with explicit verifiers.

Testing and metrics
- Overlay: golden tests for hunk mapping; fuzz with random edit sequences; performance budget for large diffs.
- Sandbox: integration tests on tiny fixture crates (via common::fixtures_crates_dir), including worktree fallback; measure cold vs warm runs.
- Retrieval: synthetic corpora for BM25-only, dense-only, fused; check precision@k and latency; ablate graph constraints impact.
- Agent loop: tiny tasks (rename, signature change, add field) with pass/fail metrics; collect compile/test pass rates and iteration counts.

Observability and performance
- Tracing: add spans per overlay session, per runner invocation, per retrieval stage; error context includes file path and snippet when safe.
- Resource limits: timeouts for cargo; process sandboxing optional; configurable parallelism.
- Avoid gratuitous allocations: follow no_gratuitous_collect.sh; prefer iterator chains and small Vec reuse in hot paths (diff mapping, retrieval loops).

Brief code change map
- ploke-tui
  - src/app/types.rs: Mode::EditOverlay; optional OverlayMessage kind.
  - src/app/overlay.rs and overlay_line_map.rs: new.
  - src/app/input/keymap.rs: new actions and mappings.
  - src/lib.rs AppEvent: Overlay* variants; expose subscribe/send helpers.
  - src/llm/mod.rs: stream to overlay; debounce updates.
  - src/app/view/components/conversation.rs: toggle diff panes; diagnostics panel.
- ploke-io
  - src/overlay/mod.rs and sandbox.rs: new OverlayWorkspace and CargoRunner.
- ploke-rag
  - src/context/budget.rs, src/context/retrieval.rs: new RetrievalEngine and TokenBudgetAllocator.
  - src/memory/{mod.rs,skills.rs}: skill and episodic memory.
  - src/docs/generate.rs: docs generator.
  - src/agent/{state_machine.rs,eval.rs}: overlay-run state machine and eval harness.
- ploke-db
  - Prefer existing APIs; add small helpers if needed for common graph-scope queries; reuse bm25_service and HNSW index functions.

Risks and mitigations
- Sandbox slowness: cache-aware CARGO_TARGET_DIR; share registry; pin toolchain; offline mode; prewarm on idle.
- Diagnostic drift: unify mapping via SpanMap; fuzz tests; never apply to repo until passing preflight.
- Retrieval quality: tie BM25 and dense via graph constraints; MMR diversity; provenance in UI; quick ablations in eval harness.
- Agent instability: strict policy gates; small steps first; skill hints; replayable traces.

Done definition for this phase (revised)
- Overlay, sandbox, and accept/reject on by default with deterministic diagnostics.
- Retrieval uses staged BM25 → graph prune → dense rerank → MMR with budget packing and provenance.
- Agent loop runs end-to-end on small tasks behind a feature flag; skills and episodic memory consulted; eval harness produces reports.
- No DB schema changes; background index refresh is stable.

This keeps your current architecture intact, adds determinism and evaluability, and moves you towards SoTA practices while remaining pragmatic for Rust developers.
