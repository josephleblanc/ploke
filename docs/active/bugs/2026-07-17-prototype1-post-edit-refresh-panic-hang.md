# Prototype 1 Post-Edit Refresh Panic Hang

Status: source repaired at `8ae465ebf`; exact historical replay passes; fresh
strict live validation pending.

Discovered: 2026-07-17

## Broken Contract

Once an edit mutates disk, the apply path must reach one terminal outcome. If
the required workspace refresh fails, it must preserve the mutation evidence,
record the proposal as `PartiallyApplied`, and emit `ToolCallFailed`. A strict
parser invariant may reject the changed source, but its panic must not escape a
detached auto-confirm task and strand the enclosing model turn until the outer
wall-clock timeout.

The parser uniqueness check remains authoritative. Duplicate relations must
still fail; the fix must not make the parser, import, History, digest, epoch, or
handoff checks permissive.

## Preserved Incident

Campaign:

```text
p1-v20-strictkeephandoff-mbe-g35f-direct-3g1x3-p3-20260717-214004
```

The generation-zero baseline turn staged and applied:

```text
function-call-719232cf-a3bc-4957-9e6f-459ecc38ef14
```

A later exact persisted semantic request:

```text
function-call-5704a665-4e76-4f86-b03b-8a5eacb11651
```

added a second `replacement_multi_line_look_around` item to
`crates/printer/src/standard.rs`. The filesystem write succeeded. The
post-apply parser then detected duplicate `Contains` relations and panicked at
the strict `Expected unique relations` invariant. That panic escaped the
detached edit-approval task, so no terminal tool event reached the agent loop.
The baseline remained nonterminal until the configured 1,800-second outer
deadline.

Primary evidence:

```text
/home/brasides/.ploke-eval/instances/prototype1/p1-v20-strictkeephandoff-mbe-g35f-direct-3g1x3-p3-20260717-214004/BurntSushi__ripgrep-2209/runs/run-1784324895421-structured-current-policy-f98eca48/agent-turn-trace.json
```

The admitted source base was
`4dc6c73c5a9203c5a8a89ce2161feca542329812`. The controller job and session
were abandoned rather than resumed through changed source. V20 remains
preserved; no history, digest, profile, proposal, or workspace artifact was
rewritten to make it continuable.

## Source Boundary

The failure path was:

```text
apply_code_edit
-> write_snippets_batch mutates disk
-> rescan_for_changes
-> scan_paths_for_change
-> scan_for_change_target
-> run_parse_no_transform
-> strict relation validation panic
-> detached auto-confirm task exits
-> no ToolCallFailed or ToolCallCompleted
-> outer benchmark timeout
```

`run_parse_no_transform` is the safe containment boundary because it builds an
in-memory parse result before the later database transform. Catching the panic
around database mutation, weakening relation validation, or accepting the
malformed graph would hide the invariant violation.

## Repair

The repaired path:

- catches only the in-memory parse unwind and converts it to the existing
  parser-error path;
- records `last_parse_failure` with the strict invariant message;
- returns refresh errors through the existing scan boundary;
- pairs each typed write result with its actual semantic or non-semantic edit
  before deduplicating refresh targets;
- records applied and failed file lists in `ToolRetryContext`;
- keeps exact per-write paths and post-write hashes once in
  `ToolUiPayload.details`;
- persists the registered proposal as `PartiallyApplied` before emitting
  terminal `ToolCallFailed`; and
- does not emit a contradictory applied completion.

The `syn_parser/validate` feature is enabled only for
`ploke-eval/replay_tests`, so the historical test exercises the production
invariant without changing default shipping features.

## Historical Regression

`historical_v20_duplicate_edit_settles_refresh_failure` loads the real v20 run
manifest and typed `AgentTurnTraceRecord`, checks out the exact admitted base,
remaps only the recorded repository prefix into a temporary clone, and replays
both persisted tool requests through the production edit and session code.

It proves:

- the first edit recreates the pre-incident source;
- the second write reaches disk and creates the exact duplicate;
- strict validation still reports `Expected unique relations`;
- no applied `ToolCallCompleted` is emitted;
- `ToolCallFailed` arrives instead of a hang;
- typed retry context identifies the mutated file;
- UI details retain the post-write hash;
- in-memory and persisted proposal status are `PartiallyApplied`; and
- the terminal event is emitted only after normal-path proposal persistence.

Verification:

```text
cargo test -p ploke-eval --no-default-features --features replay_tests historical_v20_duplicate_edit_settles_refresh_failure -- --nocapture --test-threads=1
# 1 passed

cargo test -p ploke-tui --lib -- --test-threads=1
# 316 passed; 0 failed; 10 ignored

cargo test -p ploke-eval --lib --no-default-features -- --test-threads=1
# 1393 passed; 0 failed; 15 ignored
```

The full strict replay-feature run reached 1,395 passing tests. Its sole
failure is unrelated and fixture-local:
`test_replay_historical_fd_1121_partial_non_semantic_patch_runtime_flow`
cannot find either required call id in its three configured historical traces.
The v20 and ripgrep stale-anchor historical replays pass.

## Remaining Follow-Up

These predate this repair and must not be hidden inside the v20 fix:

- edit approval does not atomically claim a pending proposal, so concurrent
  auto-confirm and manual approval can duplicate writes;
- a mixed semantic batch still reports `Applied` when any write succeeds;
- later post-parse invariant `expect`s can still strand a different refresh
  panic; and
- proposal persistence remains best-effort under serialization or filesystem
  failure.

Fresh validation must use a newly admitted strict campaign. Do not run the
repaired binary against v20 as successor-handoff proof.

## Related Bugs

- [`2026-07-16-prototype1-post-edit-indexer-stall.md`](./2026-07-16-prototype1-post-edit-indexer-stall.md)
- [`2026-07-16-prototype1-headless-unpersisted-workspace-mutation.md`](./2026-07-16-prototype1-headless-unpersisted-workspace-mutation.md)
- [`2026-06-04-prototype1-headless-timeout-after-apply.md`](./2026-06-04-prototype1-headless-timeout-after-apply.md)
- [`2026-05-15-ns-patch-apply-missing-rescan-causes-stale-index-retries.md`](./2026-05-15-ns-patch-apply-missing-rescan-causes-stale-index-retries.md)
