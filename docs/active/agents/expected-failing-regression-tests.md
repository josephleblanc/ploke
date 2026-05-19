# Expected-Failing Regression Tests

This tracker records regression tests and bug-pinning reproducers that are expected to fail, or that are intentionally asserting current bad behavior until the fix lands.

Use it to avoid losing known-red tests during broad runs, interrupted work, or test-failure triage.

## Marker Convention

Every tracked regression test should have a nearby source comment:

```rust
// regr:<name>:DD-MM-YY_HH-MM
```

- `<name>` is a short one-word identifier.
- The timestamp is local time when the marker was added.
- Use `rg -n 'regr:'` from the repo root to find all marked tests.

## Current Tracker

| Marker | Status | File | Test | Expected result | Removal or update condition |
| --- | --- | --- | --- | --- | --- |
| `regr:samefile:19-05-26_06-42` | expected-fail | `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs` | `cargo test -p ploke-eval recorded_replay_rejects_stale_same_file_repair_after_first_apply -- --ignored --nocapture` | Currently fails for [`RF-05`](../bugs/2026-05-19-rf-05-edit-composition-same-file-repair.md) because a stale same-file `non_semantic_patch` replay applies as a second proposal instead of being rejected before staging/materialization. | Remove `#[ignore]` or mark resolved once replay rejects or invalidates the stale same-file repair and leaves only the first proposal applied. |

## Triage Rules

- If a marked regression test fails, check this document before treating the failure as novel.
- If a test has a `regr:` marker but is missing from this tracker, add it before broad triage continues.
- If this tracker references a removed or renamed test, update the row in the same change that moves the test.
- If the underlying bug is fixed, either remove the row or move it to a resolved section with the commit/test evidence.
