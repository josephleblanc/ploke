# TUI Approve/Deny Pipeline And Prototype 1 Admission

Status: active design reference for same-file edit/reindex blockers. Verify
line numbers against current source before changing code.

## Purpose

This document records the intended execution flow for model-authored edits in
Prototype 1. The key distinction is that `ploke-tui` and `ploke-eval` have
separate authority boundaries:

- `ploke-tui` stages proposals, applies or denies them, mutates files through
  `ploke-io`, and refreshes local index/search state after mutation.
- `ploke-eval` decides whether a settled TUI mutation is admissible Prototype 1
  candidate evidence under the current edit surface and History/Crown model.

A model-facing staged edit is not final success.

## Happy Path: One Valid Patch

1. The model calls an edit tool such as `non_semantic_patch` or
   `apply_code_edit`.
2. The tool validates arguments, resolves paths, reads or verifies the current
   file version, computes preview/hash evidence, and stores an
   `EditProposal::Pending`.
3. The tool emits model-facing staging output with `status=pending`,
   `staged>0`, and `applied=0`.
4. In `ToolLoopMode::Gated`, the LLM tool loop does not use that pending
   staging result to advance to the next provider request.
5. The Prototype 1 headless adapter inspects the staged proposal, applies edit
   surface policy, and either denies it or sends `ApproveEdits`.
6. `approve_edits` writes through `ploke-io`, which verifies the expected file
   hash before mutation.
7. If any file was mutated, TUI runs the post-apply refresh barrier before the
   final settled tool result becomes model-visible:
   - `scan_for_change`;
   - index/file-state refresh;
   - sparse/BM25 refresh when sparse-strict retrieval is active.
8. The LLM loop receives only the final settled edit result as the tool result
   for the next provider request.
9. `ploke-eval` observes the settled status, waits on its own refresh/index
   barrier, validates the workspace diff through backend edit-surface checks,
   and only then may publish candidate evidence.

## Happy Path: Sequential Same-File Edits

Same-file sequential edits are valid only across a refresh boundary.

1. First edit stages against file version `H0`.
2. First edit is approved and mutates the file to `H1`.
3. Refresh/reindex/search state reaches `H1`.
4. The model receives the settled apply result and rereads or re-resolves the
   target.
5. The second edit stages against `H1`, not against `H0`.

If the second edit still references `H0`, the correct behavior is to reject it
before staging or before write admission with a model-visible instruction to
reread or re-resolve.

Same-file edits in one batch should usually be composed into one proposal
against one base file version. Independent same-file proposals staged against
the same pre-edit version are unsafe unless an explicit ordered composition
model is added.

## Failure Shape We Are Fixing

The recurring stale-anchor failure occurs when the loop violates:

```text
apply -> refresh/reindex/search barrier -> model-visible settled result -> reread/re-resolve
```

Known variants:

- semantic edit uses DB node metadata whose file hash/spans are stale after a
  prior same-file edit;
- non-semantic patch is generated from stale context and matches only fuzzily or
  partially after a prior same-file edit;
- tool-loop evidence treats staged `ToolCallCompleted` as final success;
- a partial mutation is represented as an ordinary failure, erasing the fact
  that the workspace changed and must be refreshed/classified before further
  same-file work.

The IO rejection is correct. The broken contract is letting stale or ambiguous
same-file work proceed far enough to become noisy or misleading loop evidence.

## Partial Mutation State

Partial mutation is not the same as clean apply or ordinary failure.

Desired treatment:

- terminal for that proposal;
- model-visible;
- refresh-forced, because disk changed;
- non-admissible as Prototype 1 candidate evidence unless later backend checks
  explicitly prove a clean candidate artifact;
- classified separately from "no edits were applied" so downstream systems do
  not infer that the workspace is unchanged.

Adding a distinct TUI status such as `PartiallyApplied` may be the right
carrier, but it is a shared state-model change. It affects proposal
serialization, UI rendering, `approve`/`deny` semantics, headless adapter
polling, run evidence, and backend admission. Do not add or change this status
as a local `rag/tools.rs` helper fix.

## Required Regression Surface

Local fixed-contract tests should cover:

- pending staged edit does not unblock the LLM loop in gated mode;
- settled edit result is emitted only after apply and refresh;
- same-file stale semantic anchors fail before staging;
- fuzzy same-file `ns_patch` repair after a mutating proposal fails before
  staging;
- partial non-semantic apply is represented as mutation evidence, not ordinary
  no-op failure.

Live historical replay should also cover the real Prototype 1 failure:

- replay the historical run prefix that applied one same-file edit and then
  attempted a stale follow-up edit;
- run through the normal session/tool loop, not direct tool-result injection;
- gate the Rust test behind `live_api_tests` and `#[ignore]`;
- allow a live provider continuation only after the recorded prefix has passed
  through the current gated/refresh path;
- prove that the next provider request contains either the settled refreshed
  state or a model-visible stale-anchor rejection, not staged-only success.

Concrete historical anchor from the current bug report:

```text
campaign: p1-gemini35-flash-direct-fresh-20260524-163447
run: run-1779665713181-structured-current-policy-efa0a063
event index: 389
bug: docs/active/bugs/2026-05-25-prototype1-same-file-semantic-edit-stale-anchor.md
```

## Function Index

See [`pipeline-function-index.md`](pipeline-function-index.md) before editing
this flow. It lists the current TUI and Prototype 1 functions that carry the
pipeline.
