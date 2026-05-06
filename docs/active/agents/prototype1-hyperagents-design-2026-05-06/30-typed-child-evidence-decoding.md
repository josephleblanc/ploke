# Worker A: Typed Child Evidence Decoding

Date: 2026-05-06

## Scope

Owned files changed:

- `crates/ploke-eval/src/cli/prototype1_state/evidence.rs`
- `docs/active/agents/prototype1-hyperagents-design-2026-05-06/30-typed-child-evidence-decoding.md`

Observed concurrent tracked changes outside this ownership in `cli.rs`,
`cli_facing.rs`, and `history_preview.rs` were left untouched.

## Behavior

`prototype1_state::evidence` now prepares each `Document` through a single
typed-first decode pass before indexing and attaching evidence. The grouped
child/evaluation behavior is preserved: child identity, branch placement,
runtime placement, compared run paths, operational metrics, source refs,
`EvidenceClass`, treatment, path/hash/ref id, and existing conservative
diagnostics are still carried as before.

Each grouped `EvidenceSource` now also records:

- `parse.status`: `typed`, `json_fallback`, `parse_failure`, or
  `not_applicable`;
- `parse.record`: the expected typed source record when known;
- `parse.diagnostic`: typed decode or JSON parse failure detail when present;
- `facts`: field-level origin markers for extracted facts, using `typed`,
  `json_fallback`, `path_fallback`, `filename_fallback`, or `parse_failure`.

Typed decode is diagnostic-only evidence provenance. It does not rank, score,
select successors, admit History entries, or upgrade degraded/projection
sources into authority.

## Types Decoded

Typed source records attempted in this slice:

- `Prototype1NodeRecord`
- `Prototype1BranchEvaluationReport`
- `Prototype1RunnerRequest`
- `Prototype1RunnerResult`

`NodeRecord` and `Evaluation` were the prioritized records and have explicit
regression coverage. Runner request/result decoding was easy to add because the
types are available in `crate::intervention`; it only changes parse/fact
provenance and preserves the same grouped field surface.

## Fallbacks

If a known document class fails typed deserialization but still has a valid JSON
payload, extraction falls back to the existing `serde_json::Value` key reads and
records `parse.status = json_fallback` plus the typed decode diagnostic.

If the payload is not valid JSON, extraction records `parse.status =
parse_failure`; path or filename fallback can still recover facts where the old
behavior already did so.

Path/filename fallback remains conservative:

- `node_id` can still come from `prototype1/nodes/<node-id>/...`;
- runtime ids for runtime-scoped classes can still come from
  `<runtime-id>.json`;
- evaluation `branch_id` can still come from the evaluation filename stem.

## Tests

Passed:

```text
cargo fmt --all
cargo test -p ploke-eval --lib prototype1_state::evidence
cargo check -p ploke-eval
```

The test target now runs 10 evidence tests. New coverage:

- typed `Prototype1NodeRecord` and `Prototype1BranchEvaluationReport` decode
  records `typed` parse status and typed fact origins;
- typed NodeRecord parse failure degrades to JSON fallback without erasing
  child evidence and emits a diagnostic.

Existing warnings remain, mostly dead-code warnings in Prototype 1 History
scaffolding and existing `syn_parser` warnings.

## Remaining Typed Gaps

- `Invocation`, `AttemptResult`, `SuccessorReady`, and `SuccessorCompletion`
  still use JSON/path fallback in this slice.
- `Prototype1BranchRegistry` and scheduler projection records still use JSON
  fallback.
- Preserved compared run `record.json.gz` paths are not opened as `RunRecord`.
- Protocol artifacts, run registrations, run artifact refs, provider stream
  telemetry, and sealed History entries are still outside this evidence slice.

## Lines To Inspect

- `evidence.rs:461`: `EvidenceSource` now carries parse status and fact origins.
- `evidence.rs:566`: prepared document carrier used to decode once and reuse
  the same source metadata for indexing/attachment.
- `evidence.rs:580`: typed parsed-document enum and fallback states.
- `evidence.rs:1104`: document preparation pipeline.
- `evidence.rs:1236`: coordinate extraction with typed/json/path/filename
  origins.
- `evidence.rs:1437`: typed evaluation projection preserving compared run refs.
- `evidence.rs:1973`: typed NodeRecord/Evaluation success test.
- `evidence.rs:2030`: typed parse failure JSON fallback test.
