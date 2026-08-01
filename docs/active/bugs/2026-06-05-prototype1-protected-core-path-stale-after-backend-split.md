# Prototype 1 Protected-Core Path Stale After Backend Split

Status: fixed in source, fresh-run validation pending.

## Broken Contract

Broad-harness request metadata and doctor prompt preflight must point at an
existing protected-core authority file in the current checkout before a live
Prototype 1 run is admitted as steppable.

## Evidence

- Fresh setup campaign:
  `p1-admissionfix-g35flash-p25flash-20260605-044421`.
- Worktree:
  `/home/brasides/.ploke-eval/worktrees/p1-admissionfix-g35flash-p25flash-20260605-044421`.
- Doctor command:
  `./target/debug/ploke-eval loop prototype1-doctor --repo-root . --format json`.
- Doctor observed:
  `phase = "blocked"`, `prompt_files = []`, and blocker
  `prompt preflight missing file for protected core file referenced by prompt:
  './crates/ploke-eval/src/cli/prototype1_state/backend.rs'`.
- Current checkout authority file:
  `crates/ploke-eval/src/cli/prototype1_state/backend/mod.rs`, with
  `EVAL_CORE_SURFACE_ROOT` and `WORKSPACE_EXCEPT_AUTHORITY_*`.

## Source Trace

`BroadHarnessRequest::prototype1_workspace` encoded the protected-core anchor as
`crates/ploke-eval/src/cli/prototype1_state/backend.rs`. `prompt_preflight`
constructs a baseline broad-harness template before any prompt files exist, then
checks that anchor as `protected core file referenced by prompt`. After the
backend module split, the anchor was stale, so doctor blocked at baseline setup.

This is distinct from the older unused-slot prompt-preflight blocker: no
published prompt files existed yet in this campaign.

## Current Repro Coverage

Existing prompt-preflight unit tests cover missing and present protected-core
references, but their helper wrote the same stale `backend.rs` path, so they did
not catch drift after the module split.

## Fix Direction

Update the source-of-truth broad-harness protected-core anchor, the prompt
preflight test helper, and the policy-repair prompt text to refer to
`crates/ploke-eval/src/cli/prototype1_state/backend/mod.rs`.

Do not weaken doctor by skipping protected-core file checks; the check is the
right setup guard, and the request writer was stale.
