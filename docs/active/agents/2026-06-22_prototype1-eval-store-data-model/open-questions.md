# Prototype 1 Eval Store Data Model — Open Questions

Status: active question list.

This list keeps unresolved decisions only. Resolved decisions live in [`README.md`](README.md), [`implementation-plan.md`](implementation-plan.md), [`storage-plan.md`](storage-plan.md), and [`relational-data-model.md`](relational-data-model.md).

## First implementation slice

These questions must be resolved once in the first-slice contract section of [`database-planning-notes.md`](database-planning-notes.md) before Phase 3/4 implementation.

- What is the exact narrow Rust method for the fixed first slice around `R4c -> R5` ParentStarted/resource evidence?
  - Candidate shape: `put_parent_started(...)`, `put_transition_event(...)`, or `put_evidence_ref(...)` with a typed parent-start envelope.
- What deterministic id should the first slice use?
  - Candidate shape: `sha256(campaign_id || parent_id || transition || source_stream_id || source_event_index || content_sha256)` for imported/journal-derived evidence; generated UUID only for direct live writes that also record source cursor/hash.
- What exact filesystem bytes/paths are part of the `fs` parity assertion for `R4c -> R5`?
- What exact semantic envelope/hash is compared in `dual-strict`?
- What is the write-order and failure behavior in `dual-strict` if the filesystem write succeeds and DB write fails?

## Common axes and enums

Canonical axes are `store_scope`, `producer_role`, `visibility_scope`, `source_class`, `evidence_class`, and `validation_status`.

- What are the minimal enum values needed for the first slice only?
- Should `store_scope = campaign` exist for parent-start/resource evidence, or should the first slice always use `store_scope = parent` with `parent_id` present?
- Which field identifies the parent runtime in the first slice: `parent_id`, `runtime_id`, both, or a composite?
- Should selected successors receive a new `parent_id` row at ready time or only after entering `Parent<Ready>`?

## Remote execution

- What is the minimal bootstrap package for a remote child?
- How should a remote child return large evidence: inline channel payload, object-store refs, git refs, or all of the above?
- Which child-local facts are never imported into parent by default?
- When a child becomes the selected successor, which child-local facts are revalidated, inherited, or discarded?

## Trace/log storage

- Should the first trace slice ingest optional `prototype1_observation_*.jsonl`, or should `observe::Step` write directly to `EvalStore`?
- Which log payloads can be stored inline, and which must remain path/object-store refs?
- What redaction/sensitivity labels are needed for full LLM responses and tool payloads?
- Should `TimingTrace` stderr markers be retired in favor of structured `observe::Step` events?

## Passive mirror

- Does `records/mirror.cozo.sqlite` remain always-on while `DbEvalStore` is introduced?
- Should the mirror become profile-gated once DB writes are stable?
- Do we need a one-time backfill importer from the passive mirror into typed `eval_record_ref` rows?

## Live transition test gate

- What is the exact source-derived inventory of current transition outcomes?
- Which transitions can be tested by direct edge functions, and which need the `loop walk` CLI surface for faithful setup?
- What minimal admitted fixture or restore point should seed each transition in isolation?
- What checkpoint format should preserve campaign tree, active checkout/artifact refs, run artifacts, and DB snapshots with hash verification?
- Which provider/model/profile should be the default live confidence suite target?
- What exact opt-in variables and feature flags should gate the live API suite?

## Relations and schemas

- What is the exact DDL for the first slice relations and lookup paths?
- Should `eval_record_ref` store full JSON inline for small compatibility records, or only refs/hashes?
- How should duplicate dual-write records be detected during `dual-strict` mode?
- Should `eval_successor` remain a derived projection, or become a base relation after handoff writes stabilize?
- Where should basic validation command evidence stop and code-graph-backed validation coverage begin?

## Authority boundaries

- How should `MessageBox` lock/unlock events be represented without making payload mirrors authoritative?
- What minimum channel receipt/import fact is enough for parent selection evidence?
- How should sealed History entries cite `eval_*` rows without making those rows mutable dependencies of sealed blocks?
- Which policy surface commitments must be included in History versus stored as queryable overlays?

## Code graph overlays (follow-on, not first storage pass)

- What id should connect eval policy overlays to code graph nodes across artifacts?
- Should the first follow-on bridge use `eval_code_snapshot`, `eval_code_ref`, and `eval_code_link`, or can `eval_code_ref` alone carry enough subject linkage for that overlay slice?
- Can we capture a reliable Cozo `Validity` coordinate for the code graph when an eval event is written, or do we need a separate snapshot/digest relation first?
- Which event writers can resolve `code_node_id` immediately, and which should emit path/span refs for later backfill?
- How should immutable/mutable/protected labels be scoped to artifact/tree versions?
- What is the successor validation rule for inherited policy overlays?
- Do benchmark-target code graphs and Ploke self-edit code graphs need separate relation prefixes, or is artifact/snapshot scope enough?

## Debugging and recovery queries

- Should missing expected evidence be stored as explicit `eval_evidence_warning` / gap rows, or derived only at query time?
- What is the minimal first-class lifecycle record for a broad harness slot before the full headless-TUI diagnostic exists?
- Should provider/infrastructure failure classification live on `eval_attempt`, `eval_evaluation`, `eval_model_exchange`, or a separate normalized failure relation?
- What DB query should block destructive re-entry when terminal child channel/result evidence exists but comparison/evaluation evidence is missing?
- Which projection-staleness checks should be materialized for operator tools versus computed on demand?

## Research and annotation queries

- Should human run reviews and adjudicator findings be ingested as non-authoritative `eval_review` / `eval_review_finding` rows?
- What stable experiment/run grouping id should support before/after harness comparisons across campaigns?
- Which benchmark-performance metrics are first-class columns versus derived reports: keep/reject, validation coverage, provider failure rate, oracle/MBE result, cost/tokens, protocol coverage?
- Which Cozo-specific capabilities should be exposed through optional backend traits: recursive lineage queries, graph algorithms/fixed rules, temporal projections, vector/text/LSH trace search, backup/export snapshots, or custom fixed-rule analyses?
- Which algorithm-ready edge projections should be materialized first: causal runtime graph, artifact/operation graph, agent-turn graph, or research/failure graph?
- Should `eval_causal_edge` and related graph inputs be physical relations, query-time rules, or backend-specific views?
