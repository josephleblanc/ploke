# Tool Call Failure Replay Plan

- date: 2026-04-07
- task title: Replay and diagnose the `apply_code_edit` failure path
- task description: Reproduce the recorded eval failure, probe the DB shape around the target item, and determine whether the issue is in tool definitions, canonical path mapping, or database ingestion.
- related planning files:
  - /home/brasides/code/ploke/docs/active/agents/2026-03-28_error-diagnostic-rollout-plan.md

## Scope

1. Re-run the ignored replay test in `crates/ploke-eval/src/tests/replay.rs`.
2. Inspect the exact recorded request and the observed `ToolCallFailed` payload.
3. Probe the DB for the target item by file path, canonical path, and item name.
4. Check whether the item appears as a primary node or as a method/associated item.
5. Record the likely root cause and the next change needed in tool descriptions or node-kind handling.
