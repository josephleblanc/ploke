# Runtime Playback Inventory

Status: draft survey track.

This folder maps the persisted and derived data that should eventually feed
`RuntimePlayback`. It is a documentation lane, not an implementation handoff.
The implementation thread should use these files to decide which typed records,
graph facts, joins, and playback steps need to exist before adding new CLI or
egui surfaces.

- [`record-surface-map.md`](record-surface-map.md)
  First code-backed map of current record owners, file locations, readers,
  graph coverage, History payloads, channel files, index DB snapshots,
  selection material, join keys, and duplicate record pressure.
- [`record-persistence-checklist.md`](record-persistence-checklist.md)
  Run-review checklist for marking expected Prototype 1 persistence surfaces as
  present, absent, playback gaps, manual joins, or operator convenience records.
- [`crate-boundary-inventory.md`](crate-boundary-inventory.md)
  Crate-by-crate map of writer, schema-owner, graph-loader, TUI, LLM, DB, and
  protocol responsibilities for playback evidence.
- [`playback-coverage-pass.md`](playback-coverage-pass.md)
  Checklist for finishing the survey across emitted files, graph gaps,
  sequencing gaps, and iterator placement.
- [`latest-run-emission-worksheet.md`](latest-run-emission-worksheet.md)
  Metadata-first worksheet for the newest observed run in `~/.ploke-eval`,
  including run-root, protocol-root, campaign, registry, batch, and TUI
  proposal file families.
- [`egui-questions-and-aggregates.md`](egui-questions-and-aggregates.md)
  Candidate `ploke-egui` aggregates, benchmark-improvement evidence, and
  operational metrics that should be derived from the shared playback cursor.

## Survey Rule

Every item in this inventory should be described by the same facts:

- owner type and crate;
- writer boundary;
- persisted file path;
- reader boundary;
- current `ploke_tree::Graph` representation, if any;
- semantic object from the Prototype 1 model;
- join key;
- playback step family and granularity;
- known duplicates, partial mirrors, or sequence gaps.

The goal is not to make a larger pile of report structs. If an item is already
owned by a persisted type, the preferred future shape is to load that owner
type, add borrowed graph accessors or graph facts, and render from those facts.

## Ordering Rule

Playback order should preserve the current authority ladder:

1. sealed History lineage order;
2. admitted History entry order;
3. typed transition, channel, protocol, invocation, and evaluation evidence;
4. runtime-local and attempt-local order;
5. agent-turn event order;
6. timestamps for profiling, correlation, and anomaly detection.

Timestamps are important for latency and cost analysis. They should not replace
History or typed transition order as the causal spine.
