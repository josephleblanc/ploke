# 2026-05-09 Ploke Records Protocol Handoff

Short restart handoff for the `ploke-records` / `ploke-tree` shared record-schema work.

## Scope

This thread moved Prototype 1 passive run data toward shared typed schemas that can be consumed by UI/projection crates without depending on `ploke-eval`.

The working rule is now hard:

> `ploke-records` public shared record surfaces must be typed. Use original passive types when possible. Use mirror types when original types carry functional/runtime authority or cannot be moved. Do not expose `serde_json::Value` / `JsonRecordValue` as a public field. If removing one is blocked, stop and identify the exact blocker before coding around it.

`ploke-eval` keeps functional typestate, authority, and active loop validation. `ploke-records` is passive schema. `ploke-tree` is read-only projection.

## Current State

Implemented in this thread:

- `ploke-records::protocol::Artifact` now exposes metadata plus typed `body: ArtifactBody`.
- Custom serde preserves the persisted file shape with top-level `input`, `output`, and `artifact` fields.
- Public `JsonRecordValue` fields were removed from `crates/ploke-records/src/protocol.rs`.
- Protocol input/output payloads reuse original `ploke-protocol` types:
  - `ToolCallSequence`
  - `SegmentedToolCallSequence`
  - `ToolCallNeighborhood`
  - `SegmentReviewSubject`
  - `LocalAnalysisAssessment`
- Nested procedure artifacts use typed mirror aliases built from reusable `ploke-protocol` graph wrappers:
  - `ProcedureArtifact`
  - `SequenceArtifact`
  - `StepArtifact`
  - `FanOutArtifact`
  - `ForkState`
- Provenance mirrors were added because the original active/runtime provenance types are not passive-deserializable as-is:
  - `MechanizedProvenanceMirror`
  - `JsonLlmProvenanceMirror`
  - typed OpenAI response mirror structs
- Mirror response structs use `#[serde(deny_unknown_fields)]`, so new provider fields fail instead of being silently dropped.
- `ploke-tree` can load protocol artifacts through an explicit `FsRunStore::with_protocol_artifacts_dir(...)`; it does not guess the instances path from a campaign root.

## Real Run Verified

Real run used for checks:

```text
p1-edit-surface-history-long-20260508-1
```

Campaign root:

```text
/home/brasides/.ploke-eval/campaigns/p1-edit-surface-history-long-20260508-1/prototype1
```

Protocol artifacts dir:

```text
/home/brasides/.ploke-eval/instances/prototype1/p1-edit-surface-history-long-20260508-1/treatments/branch-01187cd17226d1a4/instances/BurntSushi__ripgrep-2209/runs/run-1778270575083-structured-current-policy-7a8e5b98/protocol-artifacts
```

Observed protocol counts:

- 16 protocol artifact files
- 1 `tool_call_intent_segmentation`
- 10 `tool_call_review`
- 5 `tool_call_segment_review`
- 16 typed payloads

## Verification Commands

These passed after the typed protocol change:

```bash
cargo test -p ploke-records protocol::tests::all_artifacts_roundtrip -- --ignored
cargo test -p ploke-records protocol::tests::typed_payloads -- --ignored
cargo test -p ploke-records
cargo test -p ploke-tree
bash -o pipefail -lc 'cargo check -p ploke-eval 2>&1 | tail -n 120'
```

Real tree load with protocol artifacts also passed:

```bash
PLOKE_TREE_RUN_ROOT=/home/brasides/.ploke-eval/campaigns/p1-edit-surface-history-long-20260508-1/prototype1 \
PLOKE_TREE_PROTOCOL_ARTIFACTS_DIR=/home/brasides/.ploke-eval/instances/prototype1/p1-edit-surface-history-long-20260508-1/treatments/branch-01187cd17226d1a4/instances/BurntSushi__ripgrep-2209/runs/run-1778270575083-structured-current-policy-7a8e5b98/protocol-artifacts \
cargo test -p ploke-tree fs_run_store_loads_real_campaign -- --ignored --nocapture
```

## Post-Compact Update

The protocol split has been completed. `crates/ploke-records/src/protocol.rs`
was replaced by:

- `crates/ploke-records/src/protocol/mod.rs`: `Artifact`, `ArtifactBody`, constants, custom serde, public exports
- `crates/ploke-records/src/protocol/artifacts.rs`: concrete procedure artifact mirror aliases and payloads
- `crates/ploke-records/src/protocol/provenance.rs`: passive provenance and OpenAI response mirror types
- `crates/ploke-records/src/protocol/tests.rs`: real-run protocol tests and round-trip print helper

Current rough line-count outliers after the split:

- `journal.rs`: 587
- `scheduler.rs`: 443
- `branch.rs`: 411
- `history.rs`: 397
- `evaluation.rs`: 392
- `protocol/tests.rs`: 259
- `protocol/mod.rs`: 214
- `protocol/artifacts.rs`: 137
- `protocol/provenance.rs`: 74

Verification repeated after the split:

```bash
cargo fmt --all
cargo test -p ploke-records
cargo test -p ploke-tree
cargo test -p ploke-records protocol::tests::all_artifacts_roundtrip -- --ignored
cargo test -p ploke-records protocol::tests::typed_payloads -- --ignored
PLOKE_TREE_RUN_ROOT=/home/brasides/.ploke-eval/campaigns/p1-edit-surface-history-long-20260508-1/prototype1 \
PLOKE_TREE_PROTOCOL_ARTIFACTS_DIR=/home/brasides/.ploke-eval/instances/prototype1/p1-edit-surface-history-long-20260508-1/treatments/branch-01187cd17226d1a4/instances/BurntSushi__ripgrep-2209/runs/run-1778270575083-structured-current-policy-7a8e5b98/protocol-artifacts \
cargo test -p ploke-tree fs_run_store_loads_real_campaign -- --ignored --nocapture
cargo check -p ploke-eval
```

## Next Task

Revisit the `XMirror` naming pattern. Preferred direction:

- Use module context to shorten names, e.g. `protocol::provenance::Llm`, `protocol::provenance::Mechanized`, `protocol::response::OpenAi`.
- Add an explicit relationship to active types where useful from `ploke-records`, not from `ploke-protocol`, to avoid dependency cycles.
- Do not force a generic `Mirror<T>` unless it genuinely clarifies the relationship and serde shape.

## Watchouts

- Do not reintroduce public `serde_json::Value` or `JsonRecordValue` in `ploke-records::protocol`.
- Private `serde_json::Value` in custom serde is acceptable only as parser machinery, not as a public record surface.
- If a future artifact contains an unmodeled OpenAI/provider field, the mirror should fail deserialization and force an explicit type addition.
- `cargo check -p ploke-eval` passes, but the crate still emits many existing warnings.
