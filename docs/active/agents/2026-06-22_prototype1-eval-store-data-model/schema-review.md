# Prototype 1 Relational Data Model Review — Archived

Status: archived / folded into [`relational-data-model.md`](relational-data-model.md).

This file used to contain a source-checked review of the first relational model draft. The accepted findings were folded into [`relational-data-model.md`](relational-data-model.md) and the implementation-facing decisions were consolidated into:

- [`README.md`](README.md) — current decisions and first-slice summary;
- [`implementation-plan.md`](implementation-plan.md) — phase gates and fixed first implementation slice;
- [`database-planning-notes.md`](database-planning-notes.md) — physical slices, key strategy, and validation requirements;
- [`open-questions.md`](open-questions.md) — unresolved questions only.

## Folded findings

The review approved and incorporated these changes:

- preserve History, Channel, MessageBox, and artifact/backend authority boundaries;
- keep stores owner-scoped rather than one ambient shared DB;
- split the old overloaded ownership vocabulary into `store_scope`, `producer_role`, `visibility_scope`, `source_class`, `evidence_class`, and `validation_status`;
- add lineage and parent-epoch identity;
- keep `runtime_id`, `attempt_id`, and `run_id` semantically distinct;
- add operation/patch/apply/build provenance;
- normalize candidate sets, membership, evaluation, selection, and continuation facts;
- make admitted profile/config commitments first-class;
- preserve channel envelope identity, receipts, and explicit import/admission facts without replacing channel transport;
- expand invocation/bootstrap refs without treating generic DB rows as executable bootstrap authority;
- add artifact surface, workspace, binary, baseline, closure/protocol, trace, and evidence-warning relations;
- model agent-turn drilldown as an ordered event timeline.

## Historical provenance

The review cross-checked current behavior against source and docs such as:

- `crates/ploke-eval/src/cli/prototype1_state/live_edges.rs`
- `crates/ploke-eval/src/cli/prototype1_state/channel.rs`
- `crates/ploke-eval/src/cli/prototype1_state/invocation.rs`
- `crates/ploke-eval/src/cli/prototype1_state/observe.rs`
- `crates/ploke-eval/src/record_emission.rs`
- `crates/ploke-records/src/{scheduler,branch,evaluation,journal,history,invocation,selection}.rs`
- `crates/ploke-eval/docs/prototype1/*`
- `docs/workflow/evalnomicon/src/prototype1/*`

Do not treat this archived file as an independent schema source. If it disagrees with [`relational-data-model.md`](relational-data-model.md), the latter is canonical.
