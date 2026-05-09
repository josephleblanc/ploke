# 2026-05-09 Records Emission Clean Sweep Handoff

Restart handoff for the `ploke-records` / `ploke-tree` / `ploke-eval` record-emission normalization thread.

## Current State

Repo was clean at the start of this handoff.

Post-compaction refresh:

- The handoff remains the active restart spine for the record-emission
  normalization thread.
- Current task-stack focus is still
  `prototype1-execution-file-surface-cleanhouse`.
- Current working tree is not clean:
  - `docs/active/agents/readme.md` includes this handoff in the active index.
  - `docs/active/agents/2026-05-09_records-emission-clean-sweep-handoff.md`
    is the uncommitted handoff.
  - `docs/workflow/evalnomicon/drafts/2026-05-08-bounded-edit-harness-adapter-plan.md`
    has unrelated bounded-edit planning additions and should be treated as
    other-thread/user work unless explicitly reassigned.
- Do not let the passive-reader success claim expand into a producer
  normalization claim. `ploke-tree` can read the current typed shared records;
  `ploke-eval` still has multiple local/direct write paths to normalize.

Recent relevant commits:

- `7ad9aec3 Split protocol records module`
- `0ddefaf2 Type protocol records for tree UI`

The current passive record layer is usable enough for `ploke-tree` to read the real long run, but producer-side normalization is not complete.

Completed cleanup after the post-compaction refresh:

- Removed `JsonRecordValue` from `ploke-records` source.
- Deleted `crates/ploke-records/src/value.rs`.
- Replaced opaque public record fields with typed mirrors:
  - invocation payloads now use scheduler/node/request and resolved-branch records;
  - channel evaluation payloads now use typed evaluation records;
  - evaluation policy is typed;
  - journal successor selection uses a typed selection decision record;
  - History entry/block payloads use typed passive mirrors for selection
    decisions, ingress imports, regime, surface commitments, opening authority,
    claims, sealed evidence, candidate artifacts, and related payloads.
- Split the largest new History payload mirrors into
  `crates/ploke-records/src/history/payload.rs` instead of keeping all of the
  new surface in `history.rs`.

First producer-normalization slice after context reset:

- Added passive record metadata in `crates/ploke-records/src/record.rs`:
  `Record`, `RecordFamily`, and `RecordFormat`.
- Tagged scheduler node/request/result/state records plus protocol and
  evaluation artifacts with record metadata.
- Added eval-owned JSON emission in
  `crates/ploke-eval/src/record_emission.rs`; this is where filesystem write
  authority lives.
- Converted the Prototype 1 scheduler node writer to emit
  `ploke_records::scheduler::NodeRecord` through that eval-owned emitter.
- Extended the scheduler test so the emitted `node.json` parses as the shared
  passive node record.

Confirmed shared passive record surfaces in `ploke-records`:

- `scheduler` / `node`
- `journal`
- `history`
- `branch`
- `channel`
- `evaluation`
- `protocol`

Confirmed reader/projection path:

- `ploke-tree` reads those typed records without depending on `ploke-eval`.
- Real run checked:
  `/home/brasides/.ploke-eval/campaigns/p1-edit-surface-history-long-20260508-1/prototype1`
- Protocol artifacts for that run were also loaded from the matching instance `protocol-artifacts` directory.

Hard rule still active:

> `ploke-records` public shared record surfaces must be typed. Use original passive types when possible. Use mirror types when original active/runtime/typestate types cannot move. Do not expose `serde_json::Value`, `JsonRecordValue`, or any equivalent untyped JSON payload as a public record field. If a persisted payload is needed by the UI, model its fields. If the original type cannot move because it carries runtime authority or private constructors, create a typed mirror. If blocked, stop and report the blocker.

## User Direction

The user wants a normalized, low-footprint way to keep readable persisted records aligned with the future tree/browser UI.

Important correction: this is not a request to keep adding "legacy" or "migration" bridges. The earlier deletion-first framing remains active.

The remembered docs were not found under an exact "slash and burn" phrase. The matching docs are:

- `docs/archive/reports/2026-05-06-prototype1-execution-surface-clean-sweep-handoff.md`
  - strongest deletion-first directive
  - key rule: "no legacy behavior needs to be preserved. Delete or fail closed..."
  - loop execution should read only History, Channel, or Workspace/Artifact backend surfaces
- `docs/archive/reports/2026-05-06-prototype1-file-surface-cleanhouse-map.md`
  - strongest "clean sweep" planning map
  - key rule: this is not incremental cleanup; it is a clean sweep of file reads/writes in loop execution
  - classifies scheduler, branch registry, node records, runner-result, transition journal fallback reads, and parent identity
- `docs/workflow/evalnomicon/drafts/prototype1-run-tree-browser-design.md`
  - companion UI/schema plan
  - `ploke-records` = passive schemas
  - `ploke-tree` = read-only projection
  - browser/native UI depends on those, not on `ploke-eval`

## Sub-Agent Findings

Three mini explorer passes were run and closed.

### Existing Trait-Like Seams

There is no existing unified `emit_record` or `RecordSink` API.

Existing related shapes:

- `crates/ploke-eval/src/intervention/algebra/mod.rs`
  - `RecordStore`
  - append-only journal abstraction for interventions
- `crates/ploke-eval/src/cli/prototype1_state/channel.rs`
  - `Transport`
  - parent/child channel byte transport
- `crates/ploke-eval/src/cli/prototype1_state/history.rs`
  - History block append/store methods
- `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs`
  - `EvidenceRecord`
  - read/projection classification, not live emission
- `crates/ploke-eval/src/record.rs`
  - `RunRecordBuilder`
  - older structured run replay path, not Prototype 1 loop emission

The live Prototype 1 persistence path is still mostly direct `fs` / `serde` writes plus local append traits.

### Producer/Consumer Gaps

`ploke-tree` reads typed shared records, but `ploke-eval` still writes several families through local structs or local writers:

- scheduler/node:
  - producer: `crates/ploke-eval/src/intervention/scheduler.rs`
  - still writes `Prototype1SchedulerState`, `Prototype1NodeRecord`, `Prototype1RunnerRequest`, `Prototype1RunnerResult`
- branch:
  - producer: `crates/ploke-eval/src/intervention/branch_registry.rs`
  - still writes local `Prototype1BranchRegistry`
- journal:
  - producer: `crates/ploke-eval/src/cli/prototype1_state/journal.rs`
  - still appends local `JournalEntry`
- history:
  - producer: `crates/ploke-eval/src/cli/prototype1_state/history.rs`
  - still appends local typestate `Block<Sealed>` / `StoredBlock`
  - this is expected; typestate/authority stays in `ploke-eval`
- channel:
  - producer: `crates/ploke-eval/src/cli/prototype1_state/channel.rs`
  - still serializes local `Envelope`, `ToParent`, `ToChild`
- evaluation:
  - producer: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
  - still builds/writes local `Prototype1BranchEvaluationReport`
- protocol:
  - producer: `crates/ploke-eval/src/protocol_artifacts.rs`
  - still writes local `StoredProtocolArtifact`

### Active Write Paths

The active typed path and legacy controller still use a mix of direct writes and traits:

- `cli/prototype1_state/cli_facing.rs`
  - entry/run/report writes, monitor-target cache, parent identity bootstrap, successor ack/completion, legacy trace write
- `cli/prototype1_process.rs`
  - transition journal appends, successor ready/completion, child artifact persistence, History seal/append, invocation write, runner-result write
- `cli/prototype1_state/{identity,invocation,journal,history,channel,selection}.rs`
  - parent identity, invocation/ready/completion JSON, append-only journal, sealed History blocks, channel JSONL transport, active monitor-target projection
- `intervention/scheduler.rs`
  - scheduler/node/request/result projections

Smallest re-run searches:

```bash
rg -n "run_prototype1_loop_controller|run_turn|spawn_and_handoff_prototype1_successor|record_prototype1_successor_ready|record_prototype1_successor_completion|persist_prototype1_buildable_child_artifact|write_treatment_evaluation_projection|write_runner_result_at|write_parent_identity" crates/ploke-eval/src/cli/prototype1_state crates/ploke-eval/src/cli/prototype1_process.rs crates/ploke-eval/src/intervention/scheduler.rs crates/ploke-eval/src/selection.rs
```

```bash
rg -n "fs::write|serde_json::to_vec_pretty|OpenOptions|append\\(" crates/ploke-eval/src/cli/prototype1_state/{identity,invocation,journal,history,channel}.rs crates/ploke-eval/src/intervention/scheduler.rs crates/ploke-eval/src/selection.rs crates/ploke-eval/src/cli.rs
```

## Recommended Model

Do not put authority-bearing write capability in `ploke-records`.

Recommended split:

- `ploke-records` defines passive schema identity and typed record families.
- `ploke-eval` owns emission authority and implements write/append behavior.
- `ploke-tree` reads records and builds projections only.

Possible shape:

```rust
// ploke-records
pub trait Record {
    const FAMILY: RecordFamily;
    const SCHEMA: &'static str;
    const FORMAT: RecordFormat;
}
```

```rust
// ploke-eval
pub trait EmitRecord<R: ploke_records::Record> {
    type Error;
    type Receipt;

    fn emit(&mut self, record: &R) -> Result<Self::Receipt, Self::Error>;
}
```

This preserves:

- shared schema discoverability for UI/readers
- no circular dependency
- no authority constructors in `ploke-records`
- searchable structural relationship between persisted files and record types
- ability to delete old projection-control reads instead of preserving "legacy" compatibility

The intent of `EmitRecord` is to make producer normalization explicit without
moving loop correctness into `ploke-records`: each persisted thing that should
feed the UI has a typed `ploke-records` schema, and `ploke-eval` emits that
schema from the appropriate authoritative transition or projection boundary.
The trait should replace direct ad hoc `fs` / `serde` writes where those writes
produce durable UI/debug records. It should not be a generic escape hatch for
untyped payloads.

## Next Task

Do not start with History. Start with the lowest-authority projection family and prove the pattern.

Suggested first implementation slice:

1. Done: add passive record metadata in `ploke-records`.
2. Done: implement it for already-shared scheduler node/request/result/state
   records plus protocol/evaluation artifacts.
3. Done: add a local eval-owned emission trait/store adapter generic over
   `ploke_records::record::Record`.
4. Done for first low-risk writer: scheduler `node.json` emission now writes
   the shared passive node record.
5. Done for that writer: scheduler test proves the emitted file parses as
   `ploke_records::scheduler::NodeRecord`.

Next producer-normalization slice:

1. Convert `runner-request.json` and `runner-result.json` emission to the same
   shared scheduler records.
2. Then convert `scheduler.json`, which needs a full state mirror conversion
   rather than only the single-node projection.
3. Keep local runtime structs in `ploke-eval` until their active scheduling
   behavior has an explicit typed boundary; do not move scheduling decisions
   into `ploke-records`.

Avoid:

- generic trait theater that does not replace a real write path
- keeping old and new producers side by side as indefinite "migration"
- moving History/Crown/typestate authority out of `ploke-eval`
- making `ploke-records` able to write records
- public opaque JSON in shared records, including `JsonRecordValue`

## Verification Baseline

Last known good before this handoff:

- `cargo fmt --all`
- `cargo test -p ploke-records`
- `cargo test -p ploke-tree`
- `cargo test -p ploke-records protocol::tests::all_artifacts_roundtrip -- --ignored`
- `cargo test -p ploke-records protocol::tests::typed_payloads -- --ignored`
- real `ploke-tree` campaign load with protocol artifacts
- `cargo check -p ploke-eval`

`ploke-eval` still emits many existing warnings.

## Restart Reminder

Use the clean-sweep docs as the behavioral constraint. The record-emission trait is useful only if it helps delete or replace unstructured/local/direct persistence paths and prevents loop execution from reading projection files as control or authority.
