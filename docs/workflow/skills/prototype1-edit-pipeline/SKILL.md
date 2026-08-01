---
name: prototype1-edit-pipeline
description: Use when changing or diagnosing Prototype 1 model-authored edit flow, ploke-tui approve/deny edit proposal state, ToolLoopMode::Gated behavior, post-apply reindex/search refresh, same-file stale-anchor failures, or ploke-eval broad-harness admission from TUI edits.
---

# Prototype 1 Edit Pipeline

Use this skill before editing the `ploke-tui` / `ploke-eval` model-authored
edit path. This area has two authority layers: TUI proposal settlement and
Prototype 1 candidate admission.

## Required First Reads

Read these before changing code:

- `docs/workflow/evalnomicon/drafts/edit-surface/tui-approve-deny-pipeline.md`
- `docs/workflow/evalnomicon/drafts/edit-surface/pipeline-function-index.md`
- `docs/active/plans/self-improvement-loop/edit-tool-gated-refresh-subgoal.md`

Then query the project-wide registry and inspect the current source around the
functions it returns:

```bash
cargo xtask pipeline show prototype1.edit_tool_gated_refresh
cargo xtask pipeline find --path <source-file>
```

Use `pipeline-function-index.md` for broader source discovery when the registry
does not yet cover the relevant function.

## Invariants

- Staging is not apply success.
- TUI apply success is not Prototype 1 candidate admission.
- In Prototype 1, edit tools should run under `ToolLoopMode::Gated`.
- Pending staged edit completions must not unblock the next provider request.
- Any disk mutation must force refresh/reindex/search barriers before the model
  acts from follow-up context.
- Same-file follow-up edits must compose before apply or reread/re-resolve after
  refresh.
- Partial mutation is not ordinary no-op failure and is not clean applied
  candidate evidence.

## Change Discipline

- Do not add or change `EditProposalStatus` variants as a local helper fix.
  Audit serialization, UI, approval commands, headless adapter polling, run
  evidence, and backend admission first.
- Do not weaken `ploke-io` expected-hash checks.
- Do not make an invalid historical run acceptable by changing readers.
- If live provider behavior is part of the contract, add an ignored
  `#[cfg(feature = "live_api_tests")]` test and run it explicitly.

## Useful Focused Tests

Run the relevant subset, not necessarily all of these every time:

```bash
cargo test -p ploke-tui execute_tools_via_event_bus_gated_waits_for_settled_edit_result -- --nocapture
cargo test -p ploke-tui apply_code_edit_rejects_stale_semantic_anchor_before_staging -- --nocapture
cargo test -p ploke-tui ns_patch_rejects_fuzzy_same_file_repair_after_applied_proposal_before_staging -- --nocapture
cargo test -p ploke-eval recorded_replay_rejects_stale_same_file_repair_after_first_apply -- --nocapture
```

For live historical verification, follow
`docs/active/plans/self-improvement-loop/edit-tool-gated-refresh-subgoal.md`.
