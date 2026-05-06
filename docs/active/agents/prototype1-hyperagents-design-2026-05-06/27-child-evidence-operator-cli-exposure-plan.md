# Agent 27: Child Evidence Operator CLI Exposure Plan

Date: 2026-05-06

Scope:
- Read reports `19` through `24`.
- Inspected `crates/ploke-eval/src/cli.rs`,
  `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`,
  `history_preview.rs`, `evidence.rs`, `metrics.rs`, and `report.rs`.
- Source code was not modified.

## Goal

Expose the new child evidence grouping to operators with the smallest
read-only CLI/projection surface, so humans can inspect the data that will later
feed successor selection before any selector consumes it.

The exposure should answer:

- which child groups exist;
- which node/branch/runtime coordinates were recovered;
- which source files or journal lines contributed evidence;
- which evaluation report and compared run record paths are attached;
- which sources were unplaced;
- which diagnostics warn about ambiguous or degraded joins;
- which authority treatment each source currently carries.

It should not select a successor, admit History entries, strengthen mutable
projection evidence, or duplicate the grouping joins in CLI rendering code.

## Current Shape

Agent 19 added the core grouping in
`crates/ploke-eval/src/cli/prototype1_state/evidence.rs`.
`ChildEvidenceSet` serializes `children`, `unplaced`, and `diagnostics`;
`ChildEvidence` nests runtime, branch, evaluation, document, journal, and
diagnostic evidence; `EvidenceSource` preserves class, kind, treatment, and
`EvidencePointer`.

`history_preview::EvidenceStore` already exposes
`child_evidence()`, and `FsEvidenceStore::child_evidence()` builds it from the
same `transition_journal()` and `documents()` reads used by the History preview.
That is the right producer boundary: the operator view can be a projection over
the existing grouping, not a new filesystem scan.

The command family already has two read-only access paths:

- top-level `history`, with `metrics` and `preview`;
- monitor-local `loop prototype1-monitor`, with `history-metrics` and
  `history-preview`.

That means the lowest-risk operator exposure can reuse the existing History
projection family instead of inventing an unrelated inspect surface.

## Option Comparison

### Add A Metrics Mode

This would extend `MetricSlice` with a child-evidence view and route it through
`metrics::run`.

Pros:
- reuses existing `history metrics` and `history-metrics` command paths;
- already has `--rows`, `--generation`, table/JSON handling;
- operators already expect ranked/selection-adjacent views here.

Cons:
- `metrics::build` still has its own `Assembly` and duplicate joins over
  `documents()` and `transition_journal()`;
- a metrics mode pressures child evidence into rows, ranks, cohorts, and
  dashboard fields before the evidence grouping is the source of truth;
- it would blur "inspect evidence" with "score candidates".

Verdict: useful later, but not first. The first exposure should not make the
metrics projection the public owner of child evidence.

### Add A Report Section

This would add a child evidence section to `prototype1-monitor report`.

Pros:
- the monitor report is already the default operator view;
- it can show compact counts and warnings without adding CLI syntax;
- `report.rs` already labels itself as provisional and not sealed History.

Cons:
- `report::Report::load` uses typed scheduler/registry/journal/evaluation
  readers and `load_evaluations`, not `FsEvidenceStore::child_evidence`;
- adding the section cleanly would either duplicate source grouping or require
  beginning the report integration before humans can inspect the raw grouping;
- the default report would get denser and less focused.

Verdict: good second step for a summary section, but too invasive as the first
operator exposure.

### Extend History Preview Output

This would add a child-evidence slice to the existing History preview command
shape.

Pros:
- child evidence is produced by `history_preview::FsEvidenceStore`;
- the existing preview path is already read-only and source/provenance focused;
- table/JSON projection can be added as a thin wrapper around
  `ChildEvidenceSet`;
- it preserves the distinction between evidence grouping and selector input.

Cons:
- overloading `history-preview` could confuse History-shaped entries with child
  evidence bundles unless the option and output title are explicit;
- `Prototype1HistoryPreviewCommand` currently slices entries and diagnostics,
  so child evidence would add a different top-level view under one command.

Verdict: best implementation layer, but the user-facing name should be explicit
enough that operators do not confuse it with sealed History preview entries.

### Add A New Inspect Subcommand

This would add a separate command such as `history child-evidence` and a
monitor alias such as `loop prototype1-monitor child-evidence`.

Pros:
- most explicit operator intent;
- keeps child evidence from being collapsed into metrics or report rows;
- can still call the existing `FsEvidenceStore::child_evidence()` producer;
- makes JSON output of the raw grouping straightforward and stable enough for
  human inspection.

Cons:
- touches CLI enum wiring and parse tests;
- adds one more command name to the already broad Prototype 1 surface.

Verdict: recommended first exposure, if implemented as a very thin projection
under the existing History/monitor command families rather than as a broad new
inspection subsystem.

## Recommendation

Add a narrow read-only `child-evidence` projection under the existing History
projection family:

```text
ploke-eval history --campaign <id> child-evidence --format table
ploke-eval history --campaign <id> child-evidence --format json
ploke-eval loop prototype1-monitor --campaign <id> child-evidence --format table
ploke-eval loop prototype1-monitor --campaign <id> child-evidence --format json
```

The monitor-local form should be only an alias for operator convenience. The
top-level `history child-evidence` command is the canonical surface because the
data is a read-only evidence projection, not a live monitor status report.

Do not add ranking, selection, scoring, or filtering in the first slice. The
first table output should be compact:

```text
prototype1 child evidence
----------------------------------------
schema_version: prototype1-child-evidence.v1
campaign_id: ...
children: N
unplaced: N
diagnostics: N

children
----------------------------------------
node=<id> parent=<id|none> gen=<n|?> branch=<id|none> runtimes=<n> branches=<n> evaluations=<n> documents=<n> journal=<n> diagnostics=<n>

evaluations
----------------------------------------
node=<id> branch=<id> disposition=<...> compared=<n> artifact=<path|none>

diagnostics
----------------------------------------
warning [<source_ref>]: ...
```

JSON output should serialize the `ChildEvidenceSet` or a wrapper that adds only
campaign metadata. The wrapper may include:

```text
schema_version
generated_at
campaign_id
manifest_path
prototype_root
evidence
```

If the team wants the absolute smallest code delta, skip the wrapper and print
`ChildEvidenceSet` directly in JSON. For table output, include campaign and
manifest paths from the command context.

## Exact Files And Functions To Change

Primary implementation:

- `crates/ploke-eval/src/cli.rs`
  Add `ChildEvidence(Prototype1ChildEvidenceCommand)` to
  `HistorySubcommand` and `Prototype1MonitorSubcommand`. Add a small parser
  struct with `format: InspectOutputFormat`. Optional later fields:
  `generation`, `node`, `branch`, `diagnostics`.

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
  Import `Prototype1ChildEvidenceCommand`. In `Prototype1MonitorCommand::run`,
  route the monitor alias to the same helper as the History command. In
  `HistoryCommand::run`, route `HistorySubcommand::ChildEvidence` to the same
  helper. Add `run_child_evidence(campaign_id, manifest_path, command)`.

- `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs`
  Add a small public crate-visible projection function, for example
  `pub(crate) fn build_child_evidence(campaign_id, manifest_path)`. It should
  construct `FsEvidenceStore::new(manifest_path)`, call `child_evidence()`, and
  return either `ChildEvidenceSet` or a wrapper with campaign metadata. Keep it
  beside `build`/`run` because this is the same evidence-store projection
  family.

- `crates/ploke-eval/src/cli/prototype1_state/evidence.rs`
  Add only rendering/accessor support if needed. The current structs already
  derive `Serialize` and contain the data needed for JSON. Prefer implementing
  the table printer in the new CLI helper or in a small `print` method that
  does not perform joins.

Tests:

- `crates/ploke-eval/src/cli.rs`
  Add parse tests for:
  `history --campaign c child-evidence --format json` and
  `loop prototype1-monitor --campaign c child-evidence --format json`.

- `crates/ploke-eval/src/cli/prototype1_state/evidence.rs`
  Existing grouping tests are sufficient for producer correctness after Agent
  24. Add no new grouping tests unless the projection wrapper changes fields.

Verification:

```text
cargo fmt --all
cargo test -p ploke-eval --lib groups_child_documents_with_source_refs
cargo test -p ploke-eval --lib ambiguous_runtime_and_branch_joins_are_not_reused
cargo test -p ploke-eval --lib branch_metadata_conflicts_are_diagnostic
cargo test -p ploke-eval cli::tests::history_child_evidence_should_parse
cargo test -p ploke-eval cli::tests::prototype1_monitor_child_evidence_should_parse
cargo check -p ploke-eval
```

## Non-Goals For The First Exposure

- Do not make `metrics::build` consume `ChildEvidenceSet` in the same patch.
- Do not add a selection policy input projection in the same patch.
- Do not add History admission or sealed block reads.
- Do not call `Crown`, `BlockStore::append`, or History write APIs.
- Do not present `EvidenceClass::treatment()` strings as authority.
- Do not hide unplaced evidence or diagnostics in table output.

## Later Sequence

1. Add `history child-evidence` and monitor alias as the first human inspection
   surface.
2. Add a small child-evidence summary section to `prototype1-monitor report`
   once operators have validated the raw grouping.
3. Convert `metrics::build` to derive rows from `ChildEvidenceSet`, preserving
   dashboard-only scoring as a projection.
4. Add `ChildEvidence -> SelectionInput` projection with explicit dropped-field
   tests before successor selection consumes the grouping.
5. Only after that, add verified History query/admission work from Agent 22.

## Final Position

The smallest low-risk first exposure is a new explicit child-evidence
subcommand under the existing History projection family, backed directly by
`FsEvidenceStore::child_evidence()`. It is slightly more CLI wiring than a
flag on `history-preview`, but it keeps operator intent clear and avoids
mixing raw child evidence with metrics, reports, or sealed History language.
