# Next Phase Objectives: Ephemeral Edit Overlay + Agentic RAG

This document distills the next-phase goals from the initial project brief and sets concrete, user-visible outcomes.

## Phase 1 — Near-term objectives

1) Ephemeral edit overlay in TUI
- Stage code suggestions into a virtual overlay (not applied to the filesystem).
- Enter “edit-in-message” mode; edit the suggested code within the TUI.
- Track diffs vs. repo without committing.

2) Live lint/compile on the overlay
- Run `cargo check`/`clippy` against the overlay.
- Map diagnostics back to message blocks deterministically.

3) Run tests on the overlay
- Execute tests as-if changes were applied, without touching the repo.
- Surface pass/fail, regressions, and perf deltas in the TUI.

4) Accept/Reject pipeline
- Accept → apply patch to FS and make a conventional commit (template-based message).
- Reject → discard overlay; keep an audit trail in chat history.

5) Context assembly for LLM actions
- Hybrid retrieval (BM25 + dense) constrained by code-graph neighborhood edges (e.g., Module contains X, Function has Param Y).
- Budget-aware context packing to fit model context limits.

## Phase 2 — Foundations for autonomy and learning

6) Autonomous mode (cloud/E2B)
- Reuse overlay mechanics; agent loops propose → lint → test.
- Apply changes via policy gates; scoped FS writes with rollback.

7) User/LLM memory layer
- Per-user profile (style, preferences) + LLM working memory (recent decisions).
- Stored as vectors + BM25; unify retrieval with code context.

8) Agentic workflows (SE-Agent + Voyager-style)
- Multiple candidate solutions; mutate/combine with reward signals (compiles/tests/coverage/perf).
- Persist solution traces and design notes as embeddings for reuse.

9) Documentation graph
- Auto-generate docs for pipelines, IDs, module tree, hashing invariants.
- Searchable and served as LLM context for complex tasks.

10) Incremental completion of type resolution
- Finish remaining 20% and persist resolved types/edges to DB.
- Use typed relations to sharpen RAG retrieval precision.

## Non-goals for this phase

- No full IDE; TUI-first UX.
- No model training; inference-time embeddings/memory only.
- No unbounded FS access in autonomous mode; scope and reversibility required.

## How we’ll share essential project context

- Use `scripts/gen_project_context.sh` to produce a compact `project_context.txt`.
- Paste that file into chat for planning/design iterations.
- Keep bodies large code out; prefer signatures, relation names, and invariants.

## Next steps

- Implement the overlay design and lint/test sandbox plan.
- Define TUI actions/UX for edit-in-message, run-lint, run-tests, accept/reject.
- Introduce memory schema scaffolding (user prefs + short-term LLM memory) without changing existing code schemas.
- Define policies for autonomous mode (scope, test gates, rollback triggers).
