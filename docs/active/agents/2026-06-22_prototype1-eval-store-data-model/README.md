# 2026-06-22 — Prototype 1 Eval Store Data Model

Status: active planning folder.

Short description: planning packet for replacing Prototype 1 shared-filesystem assumptions with an owner-scoped, backend-agnostic persistence model. The first implementation target is ordinary eval evidence behind `fs | database | dual-strict`; History, Channel, MessageBox, artifact/worktree mutation, bootstrap, journals, and logs keep their own authority boundaries or ports.

## Canonical files

Read these first when implementing:

- [`implementation-plan.md`](implementation-plan.md) — canonical phased implementation plan, first-slice decision, and migration gates.
- [`slice-by-slice-implementation-plan.md`](slice-by-slice-implementation-plan.md) — consolidated code-checked slice plan with concrete files/functions to change per slice.
- [`implementation-log.md`](implementation-log.md) — implementation/run log template for autonomous slice execution, test evidence, live API usage, checkpoints, and commits.
- [`storage-plan.md`](storage-plan.md) — authority boundaries, Domain-C `EvalStore` backend model, config sketch, and migration boundary guardrails; it no longer carries a competing implementation sequence.
- [`persistence-port-map.md`](persistence-port-map.md) — broader port/trait map for all persisted surfaces so implementation does not collapse them into `EvalStore`.
- [`relational-data-model.md`](relational-data-model.md) — canonical logical relation model, common axes, and relation status rules.
- [`database-planning-notes.md`](database-planning-notes.md) — physical schema slices, first-slice contract checklist, key strategy, Cozo-specific implementation notes, and validation requirements.
- [`live-transition-test-plan.md`](live-transition-test-plan.md) — live/source-derived transition test gate.
- [`transition-persistence-dependency-matrix.md`](transition-persistence-dependency-matrix.md) — producer/consumer persisted-data matrix and checkpoint strategy.

Supporting references:

- [`typestate-persistence-ledger.md`](typestate-persistence-ledger.md) — source-checked transition ledger of current writes, reads, hard gates, passive mirror behavior, and trace/log sinks.
- [`cozo-feature-map.md`](cozo-feature-map.md) — source-checked Cozo feature review and backend trait implications.
- [`codegraph-eval-join-model.md`](codegraph-eval-join-model.md) — follow-on join model for eval evidence over the parsed Ploke code graph; not part of the first file/db migration pass.
- [`debugging-and-research-query-workloads.md`](debugging-and-research-query-workloads.md) — workload review for debugging, trajectory reconstruction, benchmark aggregation, and schema pressure beyond file-shaped persistence; workload ideas must be promoted through `relational-data-model.md` before implementation.
- [`record-usefulness-triage.md`](record-usefulness-triage.md) — cleanup/refactor ledger for legacy, projection-only, compatibility, or superseded records.
- [`open-questions.md`](open-questions.md) — remaining modeling and implementation questions.
- [`schema-review.md`](schema-review.md) — archived source-checked review; folded into `relational-data-model.md` and kept only as provenance.

## Current decisions

- Do not treat “the database” as one ambient shared object. Stores are owner-scoped: parent, child runtime, successor runtime, treatment run, imported evidence, passive mirror, etc.
- Use separate common axes instead of one overloaded owner field: `store_scope`, `producer_role`, `visibility_scope`, `source_class`, `evidence_class`, and `validation_status`.
- `History` / sealed `Block` remains a dedicated authority path.
- `Channel<R, T: Transport>` remains runtime communication authority.
- `MessageBox` remains typed lock/unlock authority; payloads may be mirrored, but generic record writes do not replace the box.
- Artifact/worktree mutation remains `WorkspaceBackend`/artifact authority; DB rows may carry refs and hashes only.
- `records/mirror.cozo.sqlite` is a compatibility/debug mirror, not the future `DbEvalStore` schema and not proof of typed DB parity.
- Structured traces and logs are persisted evidence and need first-class `eval_trace_event` / `eval_log_ref` treatment.
- Code-graph overlay relations are follow-on capability after file/db parity for ordinary eval evidence; first pass should not require them.
- Parent-visible child evidence must cross a channel or explicit import/admission edge; child-local DB/codegraph facts must not be merged into the parent graph by default.
- Storage migration must be gated by isolated tests for the full current live typestate transition inventory derived from source; provider-facing transitions use live API calls in the confidence suite.
- To avoid 30–35 minute full-loop waits between narrow changes, use persisted dependency checkpoints and producer/consumer contract tests before reserving full live loops for canaries.

## First implementation slice

The first concrete storage slice is now fixed:

1. keep default `backend = fs`;
2. add config and fs-only `EvalStore` scaffolding;
3. move the parent-owned `R4c -> R5` ParentStarted/resource evidence writer behind the narrowest useful `EvalStore` method;
4. add DB rows for that slice only; and
5. enable `dual-strict` for that slice with deterministic semantic envelope/hash comparison.

The exact first-slice method/envelope, DB fields, ids, hashes, enum values, and dual-strict failure behavior must be written once in `database-planning-notes.md` before coding.

Do **not** start with `JsonRecordFile::emit`, sealed History, channel envelopes as transport, child-plan MessageBox authority, invocation delivery, or code-graph overlays.

## Working goal

Design relations that can answer lifecycle questions such as:

- What happened during this parent generation?
- Which children were planned, launched, observed, compared, and selected?
- Which runtime produced this record/log/artifact?
- Which data is parent-owned, child-local, successor-local, treatment-local, or imported?
- Which code items were protected/mutable under the active policy surface?
- Which LLM/tool-call trace led to a candidate patch or failed admission?
