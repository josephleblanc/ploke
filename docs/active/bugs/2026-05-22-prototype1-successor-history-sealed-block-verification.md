# Prototype 1 Successor History Sealed Block Verification

- date: 2026-05-22 local / 2026-05-22 UTC
- campaign: `p1-google-live-run-20260521-4`
- status: patched in worktree; fresh live-run validation pending

## Summary

`p1-google-live-run-20260521-4` showed that the direct Google broad-harness
path can produce applied child patches, build the children, evaluate them, and
select a successor. The run then failed while spawning the selected successor:

```text
database setup failed during 'prototype1_history_store': sealed block failed verification before storage
```

This is now the next full-loop blocker. It is separate from the earlier
post-apply indexing timeout, because campaign `-4` got past applied parent
patches and child evaluation before failing during successor History storage.

## Evidence

Campaign root:

```text
/home/brasides/.ploke-eval/campaigns/p1-google-live-run-20260521-4
```

The run produced successful child patch/evaluation artifacts for at least:

```text
node-25c4c1e8c1ef605a / r4 / crates/ploke-llm/src/lib.rs
node-d38de5ba737dba04 / r5 / crates/ploke-tui/src/chat_history.rs
node-a3d172020d3b81ad / r8 / crates/ploke-egui/src/perf.rs
```

The transition journal selected `r8`, advanced the checkout, spawned a
successor, and then recorded the successor failure. The successor stderr at:

```text
/home/brasides/.ploke-eval/campaigns/p1-google-live-run-20260521-4/prototype1/nodes/node-a3d172020d3b81ad/streams/a2444c94-2031-4cbb-9da3-95957f01c0af/stderr.log
```

contains the same History setup error.

The child-to-parent channel at:

```text
/home/brasides/.ploke-eval/campaigns/p1-google-live-run-20260521-4/prototype1/nodes/node-a3d172020d3b81ad/channels/a2444c94-2031-4cbb-9da3-95957f01c0af/child-to-parent.jsonl
```

records the successor completion failure.

The active parent worktree was left clean on the selected child branch, but its
parent identity file still described the generation-0 parent:

```text
/home/brasides/.ploke-eval/worktrees/p1-google-live-run-20260521-4
```

Do not resume this campaign as-is without a doctor/repair step for the
checkout, parent identity, scheduler state, and History append boundary.

## Code Path

The failing append path is:

```text
crates/ploke-eval/src/cli/prototype1_process.rs::history_store.append(...)
crates/ploke-eval/src/cli/prototype1_state/history.rs::Block::verify_hash()
```

The immediate failure means stored selected-successor blocks were being loaded
with admitted entries and then re-hashed through the current in-memory serde
field order. That can differ from the exact entry JSON bytes committed when the
block was originally sealed, so the stored `entries_root` no longer matched the
recomputed entry hashes even though the segment line itself was intact.

## Resolution

The loader now extracts the raw `entries` JSON values from each stored segment
line, hashes those exact values with the `prototype1.history.entry.v1` domain,
and carries those seal-time entry hashes on loaded sealed blocks. Verification
therefore continues to check the stored header, `entries_root`, and block hash,
but no longer depends on current Rust struct field order when reading historical
blocks.

Validation in this worktree:

```text
cargo test -p ploke-eval fs_block_store_ -- --nocapture
cargo test -p ploke-eval history_candidates_reads_cross_generation_selection_payloads_with_proofs -- --nocapture
```

The observed campaign replay also now succeeds from persisted storage:

```text
./target/debug/ploke-eval history --campaign p1-google-live-run-20260521-4 selection-show --format json --row 0 --replay
# selection_show_replay_ok=true; output bytes=6927
```

This local smoke check is documented here rather than kept as an automated test
because it depends on a machine-local campaign path.

## Related Observations

The old `-4` treatment logs also showed cached starting database roots pointing
at shared repo-cache paths while treatment repos lived under per-node
`instance-targets`. The current source checkout has a repo-root-aware starting
DB cache key and child repo-cache override assertions; a fresh run should
confirm those logs no longer point at the wrong root.

`execution.mbe.enabled = false` in campaign `-4`, so this is not an MBE
packaging failure.

Provider/model behavior was not the stopper in this campaign. The raw
sidecars showed many valid Google/Gemini tool calls, no malformed raw JSON
tool-call records, and multiple applied parent patches before the successor
History failure.

## Fix Direction

1. Done in worktree: added focused storage regression coverage using a minimal
   block whose persisted entry JSON order differs from current in-memory serde
   output.
2. Done in worktree: stored-block verification now hashes the exact raw entry
   JSON values committed in the segment line, then verifies the `entries_root`
   and block hash from those seal-time entry hashes.
3. Add a run doctor or hard resume guard for campaigns left with a selected
   child checkout but stale generation-0 parent identity.
4. After the History fix, run a fresh short live Google loop from a clean parent
   worktree and confirm successor startup writes ready/completion evidence
   without `prototype1_history_store` failure.

## Next Run Watchpoints

- applied slots write both `.headless-tui.json` and submitted result JSON
- sparse post-apply logs include `sparse_refresh bm25_ready`
- treatment DB setup logs use the child-owned checkout root
- successor handoff does not emit `prototype1_history_store`
- branch, `parent_identity.json`, scheduler state, and History state agree
  after successor startup
