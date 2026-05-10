# Prototype 1 Typed Data Coverage Report

Updated: 2026-05-10

## Scope

This report evaluates whether Prototype 1 loop persisted data is covered by typed Rust deserialization, including nested payloads. The target standard is invariant:

1. every owned persisted JSON/JSONL artifact has a named Rust type;
2. production readers never parse, inspect, transform, slice, or project owned persisted JSON/JSONL through `serde_json::Value` or ad hoc field walking;
3. nested payloads deserialize into typed DTOs rather than opaque `serde_json::Value`;
4. roundtrip or real-run parse tests prove the nested shapes;
5. projection/debug/monitor readers still use named typed projection structs or enums when reading project-owned JSON/JSONL.

Current result: coverage is strong for core loop records, but not compliant with the invariant.

## Coverage Summary

| Artifact family | Current typed coverage | Status |
|---|---|---|
| `transition-journal.jsonl` | `JournalEntry` append/load is typed in `crates/ploke-eval/src/cli/prototype1_state/journal.rs`; `ploke-tree` also loads `JournalEntry`. | Covered for current variants. |
| Parent/child channel JSONL | `Envelope<M>`, `ToChild`, and `ToParent` in `crates/ploke-eval/src/cli/prototype1_state/channel.rs`; mirrored by `ploke-records` channel DTOs. | Covered. |
| Invocation and successor handoff records | `Invocation`, `SuccessorReadyRecord`, and `SuccessorCompletionRecord` in `crates/ploke-eval/src/cli/prototype1_state/invocation.rs`. | Covered. |
| Scheduler/node/request/result mirrors | Typed in `crates/ploke-eval/src/intervention/scheduler.rs` and mirrored in `ploke-records`. | Typed, but `scheduler.json` is projection for this track and must not be authority. |
| Branch registry | `Prototype1BranchRegistry` and mirrored `ploke-records` branch DTOs. | Covered. |
| Evaluation artifacts | `ploke-records::evaluation` covers nested evaluation artifacts and metrics. | Covered by family tests. |
| Protocol artifacts | `ploke-records::protocol` converts `input`, `output`, and `artifact` values into typed payload variants. | Not compliant: decode currently stages through `serde_json::Value`. |
| Sealed History blocks | `ploke-records` has typed `SealedBlockRecord`; `ploke-tree` loads those for playback; legacy `ploke-eval` stored-block loader now uses typed stored entry DTOs. | Covered on shared-record path and eval reload path. |
| History index projections | `history/index/by-hash.jsonl`, `by-lineage-height.jsonl`, and `heads.json` use typed store structures in `ploke-eval`, but are projections. | Covered by append-store projection deserialize/rebuildability test. |
| Run metadata | `RunRecord`, `LastRunRecord`, and `SnapshotStatusRecord` are typed. | Not compliant while `ToolCallRecord.arguments` remains `serde_json::Value`. |
| Browser playback model | `PlaybackBrowserModel` and nested snapshots are typed. | Serialization/projection plus deserialize roundtrip coverage. |
| Child-plan manifests | `ChildPlanFiles` and nested `ChildFiles` in `crates/ploke-eval/src/cli/prototype1_state/parent.rs` define the parent-owned message-box body. | Covered as a typed parent transition message; not a scheduler projection. |
| Streams/logs | `nodes/*/streams/*/*.log`. | Plain text logs; outside the owned JSON/JSONL typed-persistence target. |

## Confirmed Strong Areas

- Nested History selection and traversal payloads are covered in `crates/ploke-records/src/history/payload.rs`, including the prior `entries[*].core.payload.traversal.strategy` mismatch.
- Branch, channel, evaluation, protocol, and sealed block families have focused tests in `ploke-records`.
- `RunPlayback`, `RunPlaybackRef`, coarse/fine playback, and browser DTOs preserve evidence strength and playback granularity structurally.
- `ploke-tree` loads sealed History blocks as `SealedBlockRecord` for playback and loads the transition journal as `JournalEntry`.

## Blocking Gaps For 100%

1. Protocol artifact decode stages through raw `serde_json::Value`.
   - Evidence: `crates/ploke-records/src/protocol/mod.rs` deserializes `input`, `output`, and `artifact` as values before converting by `procedure_name`.
   - This violates the invariant and must be replaced with typed procedure payload decoding.

2. Some monitor/projection readers parse raw JSON for operator output.
   - Examples include `agent-turn-trace.json`, `prototype1_observation_*.jsonl`, `slice.jsonl`, and selected successor-completion inspection paths in `cli_facing.rs`.
   - These must deserialize into named typed records or named typed projection structs because the project owns their JSON/JSONL shapes.

3. Tool-call argument records still include an anonymous JSON field.
   - Evidence: `crates/ploke-eval/src/record.rs` has `ToolCallRecord.arguments: serde_json::Value`.
   - Replace with typed tool argument records, a typed enum, or a typed parse/error record; do not inspect persisted arguments through `serde_json::Value`.

## Closed In Current Coverage Pass

- Browser DTO deserialize roundtrip coverage:
  `crates/ploke-tree-browser/src/lib.rs` now roundtrips a populated `PlaybackBrowserModel` with nested evaluation/surface/protocol snapshots and verifies omitted optional projection fields deserialize to defaults.
- History index projection typed coverage:
  `crates/ploke-eval/src/cli/prototype1_state/history.rs` now appends two sealed blocks, deserializes `by-hash.jsonl`, `by-lineage-height.jsonl`, and `heads.json` as typed projection records, and checks them against sealed block-derived lineage/height/hash facts.
- Child-plan manifest classification:
  `messages/child-plan/*.json` is classified as a typed parent-owned message box. `ChildPlanFiles` roundtrips with nested `ChildFiles`, and the existing lock/unlock transition enforces receiver/path/generation constraints before the parent enters selection.
- Legacy eval History stored-block loading:
  `StoredSealedBlock.entries` now deserializes directly as `Vec<stored::StoredEntryAdmitted>`, and the typestate-private marker is consumed with `serde::de::IgnoredAny`. `Block<Sealed>` is still reconstructed only through the verified loader and `Block::verify_hash()`.
- Sealed evaluation evidence mirrors:
  `SealedEvaluationEvidence` now carries typed `SealedEvaluatorIdentity` and `SealedEvalSetIdentity`; `SealedComparedRunEvidence` now carries typed `SealedRunEvidence` with typed protocol summaries instead of raw run JSON snapshots. Traversal protocol-run checks read those typed summaries directly.

## Documentation Coverage

No current doc proves 100% typed deserialized coverage. The best references are family-specific:

- `docs/active/agents/2026-05-09_ploke-records-protocol-handoff.md`
  Strong protocol/shared-record direction, not global coverage proof.
- `docs/active/agents/2026-05-09_records-emission-clean-sweep-handoff.md`
  States the broad typed-passive-surface goal, while still noting producer normalization work.
- `docs/active/agents/2026-05-09_run-playback-coarse-history-handoff.md`
  Best nested History deserialization proof.
- `docs/active/agents/2026-05-09_run-playback-typed-observability-plan.md`
  Explicitly says not every replay-relevant passive DTO has `Record` metadata.
- `docs/workflow/evalnomicon/drafts/prototype1-persistence-map-2026-05-03/`
  Best historical artifact-family inventory, but it also documents missing join objects and aggregate records.

## Real-Run Families To Keep In Scope

Observed or documented Prototype 1 run roots include:

- `/home/brasides/.ploke-eval/campaigns/p1-3gen-15nodes-run12/prototype1`
- `/home/brasides/.ploke-eval/campaigns/p1-edit-surface-history-long-20260508-1/prototype1`
- `/home/brasides/.ploke-eval/campaigns/p1-occurrence-selection-long-20260509-2/prototype1`
- `/home/brasides/.ploke-eval/campaigns/p1-bounded-surface-long-20260509-2/prototype1`

Real-run file families include:

- `history/blocks/segment-*.jsonl`
- `history/index/by-hash.jsonl`
- `history/index/by-lineage-height.jsonl`
- `history/index/heads.json`
- `transition-journal.jsonl`
- `branches.json`
- `evaluations/*.json`
- `messages/child-plan/*.json`
- `nodes/*/node.json`
- `nodes/*/runner-request.json`
- `nodes/*/runner-result.json`
- `nodes/*/results/*.json`
- `nodes/*/invocations/*.json`
- `nodes/*/successor-ready/*.json`
- `nodes/*/successor-completion/*.json`
- `nodes/*/channels/*/*.jsonl`
- `nodes/*/streams/*/*.log`

`scheduler.json` may be typed, but it is a projection for this track and should not be used as authority.

## Priority Work To Reach 100%

1. Replace every production `serde_json::Value` reader/parser over owned persisted JSON/JSONL with a concrete record type, typed enum, or named typed projection struct.
2. Add coverage tests that fail on wrong nested shapes for each owned persisted family, including monitor/debug/projection JSONL.
3. Replace `ToolCallRecord.arguments: serde_json::Value` with typed persisted argument/error records.

## Suggested Verification Commands

Use bounded output:

```bash
cargo test -p ploke-records playback 2>&1 | tail -n 80
cargo test -p ploke-records protocol::tests 2>&1 | tail -n 80
cargo test -p ploke-records history::tests 2>&1 | tail -n 80
cargo test -p ploke-records evaluation::tests 2>&1 | tail -n 80
cargo test -p ploke-tree fs_run_store_loads_typed_transition_journal_in_append_order 2>&1 | tail -n 80
cargo test -p ploke-tree typed_transition_journal_reports_bad_source_line 2>&1 | tail -n 80
cargo test -p ploke-tree-browser --features projection 2>&1 | tail -n 80
cargo test -p ploke-eval fs_block_store_projection_indexes_deserialize_and_match_sealed_blocks 2>&1 | tail -n 80
cargo test -p ploke-eval child_plan_files_deserializes_nested_child_manifest 2>&1 | tail -n 80
cargo test -p ploke-eval fs_block_store_history_segment_deserializes_as_passive_record 2>&1 | tail -n 80
cargo test -p ploke-eval candidate_case_exposes_optional_non_mechanized_sections 2>&1 | tail -n 80
```

Before inspecting real-run JSONL, use metadata and width caps:

```bash
ls -lh <path>
wc -l <path>
tail -n 3 <path> | cut -c 1-400
rg -n '<pattern>' <path> | head -n 5 | cut -c 1-400
```
