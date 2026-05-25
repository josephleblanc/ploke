# Sub-Goal: Edit Tool Gated Refresh Barrier

Status: active sub-goal for the Prototype 1 loop reliability track.

## Goal

Make the Prototype 1 edit-tool path enforce this boundary:

```text
stage proposal -> admit/deny -> apply -> refresh/reindex/search barrier -> settled model-visible result -> backend admission
```

The loop should not let a model take a same-file follow-up edit from stale
pre-refresh context after an earlier edit mutated that file.

## Why This Matters

The stale same-file blocker is not just a patch parser issue. It is a
coordination failure between:

- TUI tool-call staging;
- TUI approve/deny and file mutation;
- reindex/search refresh;
- `ploke-eval` broad-harness admission;
- backend edit-surface validation;
- persisted run evidence and run review.

If any layer treats staged or partial edit evidence as clean success, Prototype
1 can generate misleading candidate evidence.

## Implementation Requirements

- Preserve `ToolLoopMode::Gated` for Prototype 1 headless runs.
- Ensure staged pending edit completions do not unblock the next provider
  request.
- Ensure final settled edit completions are emitted only after apply and
  refresh/reindex/search barriers complete.
- Represent partial mutation explicitly enough that downstream systems know the
  workspace changed even though the proposal is not cleanly applied.
- Keep partial mutation non-admissible as candidate evidence unless backend
  checks explicitly prove a clean artifact.
- Preserve backend authority: TUI apply success is not Prototype 1 admission.

## Regression Requirements

Local tests:

- gated tool loop ignores pending staged edit completions;
- same-file semantic stale anchors fail before staging;
- fuzzy same-file `ns_patch` repair after a mutating proposal fails before
  staging;
- partial non-semantic apply is classified as mutation evidence, not ordinary
  no-op failure;
- headless adapter does not publish applied candidate evidence from partial or
  indeterminate settlement.

Live historical test:

- add an ignored `#[cfg(feature = "live_api_tests")]` test;
- replay the historical same-file failure through the normal session/tool loop;
- use recorded provider output for the problematic prefix and live provider
  continuation only after the gated/refresh path has settled;
- prove the next model-visible request contains settled apply evidence or a
  stale-anchor rejection, not staged-only success;
- do not inject direct tool results.

Historical anchor:

```text
campaign: p1-gemini35-flash-direct-fresh-20260524-163447
run: run-1779665713181-structured-current-policy-efa0a063
event index: 389
related bug: docs/active/bugs/2026-05-25-prototype1-same-file-semantic-edit-stale-anchor.md
reference flow: docs/workflow/evalnomicon/drafts/edit-surface/tui-approve-deny-pipeline.md
```

## Non-Goals

- Do not weaken `ploke-io` hash checks.
- Do not make invalid historical run evidence acceptable by changing readers.
- Do not treat TUI proposal status as the same thing as Prototype 1 candidate
  admission.
- Do not add a new proposal status without auditing serialization, UI,
  headless adapter polling, run evidence, and backend admission.
