# Prototype 1 Record Usefulness Triage

Status: active planning ledger / cleanup deferred.

Short description: tracks persisted record families and files that may be legacy, compatibility-only, projection-only, superseded, or cleanup/refactor candidates. This ledger is intentionally **not** for critical authority surfaces or first-class DB candidates.

Related files:

- [`relational-data-model.md`](relational-data-model.md)
- [`schema-review.md`](schema-review.md) — archived/folded provenance only
- [`typestate-persistence-ledger.md`](typestate-persistence-ledger.md)
- [`storage-plan.md`](storage-plan.md)

## Purpose

Prototype 1 has accumulated persisted records while the loop architecture changed quickly. Some records are still emitted for compatibility, operator display, recovery, tests, passive mirroring, or legacy controller paths, but are no longer the right semantic objects for the future database model.

Use this ledger to decide, later and deliberately, whether each questionable record should be:

- retained only as compatibility/debug evidence;
- ingested only as `eval_record_ref` / `eval_log_ref` with explicit legacy/source class;
- replaced by typed transition, channel-import, selection, candidate, or attempt facts;
- hidden behind a new semantic API while old files remain during migration;
- retired after downstream consumers migrate.

## Boundary: what does **not** belong here

Keep these categories separate from cleanup/refactor triage:

- **Critical authority, not generic DB:** History/BlockStore, Channel/Transport, MessageBox lock/unlock, artifact/worktree/backend mutation. These belong in [`storage-plan.md`](storage-plan.md) and the typestate ledger.
- **Critical DB candidates:** evaluations, selection decisions, candidate membership, transition evidence, structured traces, log/blob refs, imports, and policy overlays. These belong in [`relational-data-model.md`](relational-data-model.md).
- **Authority guardrails:** rows that say “do not flatten History/Channel/MessageBox into EvalStore” belong in the storage plan, not in this cleanup ledger.

This file should answer: “Which persisted surfaces might be legacy, misleading, redundant, or cleanup candidates?” It should not mix that with “which surfaces are critical but use a different backend/authority model?”

## Classification labels

| Label | Meaning |
| --- | --- |
| Compatibility | Useful for migration/debug/replay, but not semantic authority. |
| Projection-only | Mutable or rebuildable view over stronger evidence. |
| Superseded | A newer typed relation/event/authority path should replace this as the semantic object. |
| Ref-only | Keep as path/blob/hash evidence; do not normalize initially. |
| Cleanup candidate | Likely removable, gateable, or replaceable after consumers migrate. |
| Unknown | Needs source audit before deciding. |

## Current cleanup/refactor triage

| Record / file | Current role | Current users / notes | Future treatment | Cleanup/refactor note |
| --- | --- | --- | --- | --- |
| `Prototype1SchedulerState` / `prototype1/scheduler.json` | Legacy mutable scheduler projection: policy fallback, frontier/completed/failed node ids, node summaries, last continuation projection. | Setup writes it. Some preview/metrics paths read it. Active typed execution should not use it to discover runnable children, decide selection, prove continuation, admit History, or validate successor causality. | **Compatibility / projection-only.** Do not add as first-class normalized relation initially. If ingested, use `eval_record_ref` with `source_class = compatibility_import` or `passive_mirror`. | Audit remaining policy fallback and preview consumers. Replace with admitted profile + typed candidate/selection/transition relations. |
| `Prototype1NodeRecord` / `nodes/<node>/node.json` | Mutable per-candidate node projection and path bundle. Carries current `node_id` join key and file paths. | Still widely used as compatibility join/path record in child plan, materialize/build/spawn, invocation/result paths. | **Compatibility / projection-only.** Do not model as authoritative `eval_node`. Use semantic relations: `eval_child`, `eval_attempt`, `eval_candidate_member`, `eval_invocation`, `eval_candidate_event`. Raw file can be `eval_record_ref`. | Keep until bootstrap/path consumers migrate. Later rename semantics away from “scheduler node” toward candidate/attempt vocabulary. |
| `nodes/<node>/runner-request.json` | Node-level runner/bootstrap work descriptor. | Used in child plan and invocation/bootstrap compatibility. | **Superseded / compatibility.** `eval_invocation` / bootstrap refs should be first-class; raw runner request is `eval_record_ref` or bootstrap payload ref. | Fold useful fields into invocation/bootstrap package; avoid parent/child shared-path dependency. |
| `nodes/<node>/runner-result.json` | Latest node-level result projection. | Useful for operator display and recovery; attempt-scoped result and channel terminal payload are stronger. | **Projection-only / cleanup candidate.** Prefer `eval_attempt`, channel terminal payload/import, and attempt result refs. | Retire latest-result authority assumptions; keep as display cache until consumers migrate. |
| `prototype1/branches.json` registry snapshots | Branch/candidate/evaluation projection and append stream. | Contains useful branch/candidate/provenance data; still partly legacy and partly a source for later normalized facts. | **Compatibility / needs split.** Extract semantic facts into candidate/patch/operation/evaluation relations. Raw records as `eval_record_ref`. | Audit which fields are superseded by candidate set, operation, patch, and selection rows. |
| `records/mirror.cozo.sqlite` / `prototype1_record` | Passive debug/compatibility mirror for `JsonRecordFile::emit`. | Currently lacks owner/runtime/artifact/import/authority context. | **Compatibility / cleanup candidate.** Do not treat as future `DbEvalStore`. | Decide later: always-on, profile-gated, or retired after typed DB parity. |
| `prototype1-loop-trace.json` | Legacy loop-controller trace outside the typed `prototype1_state` path. | Useful only for old controller diagnosis when present. | **Compatibility / cleanup candidate.** Prefer typed transition events and structured observation JSONL. | Audit remaining consumers; likely archive-only after typed trace ingestion. |
| `last-run.json` and monitor target files | Operator convenience pointers. | Useful for current operator UX but not loop semantics. | **Projection-only.** Keep outside semantic DB relations, or ingest as low-strength operator metadata if needed. | Do not let convenience pointers become replay/authority inputs. |
| `TimingTrace` stderr markers | Process timing diagnostics emitted to stderr. | Captured only when stderr is redirected; weaker than structured observe events. | **Superseded / cleanup candidate.** Prefer `eval_trace_event` from `observe::Step` / observation JSONL. | Retain only while structured timing coverage is incomplete. |
| Compressed aggregate `RunRecord` / `record.json.gz` | Aggregate run evidence and compatibility package. | Useful for egui/run-review and historical run evidence, but overlaps with future typed rows. | **Ref-only / compatibility.** Keep as blob/ref initially; derive typed rows over time. | Revisit after agent-turn/run/evaluation relations cover current consumers. |

## Immediate schema decision

`Prototype1SchedulerState` should not be a first-class relation in the initial relational model. The future schema should preserve semantic facts that currently appear inside or beside it, but through more precise relations:

- admitted profile / policy: `eval_profile_commitment`, `eval_policy_config`;
- candidate identity and membership: `eval_child`, `eval_candidate_set`, `eval_candidate_member`;
- status movement: `eval_candidate_event` from typed transitions/channel evidence;
- attempt lifecycle: `eval_attempt`, `eval_invocation`, `eval_channel_message`, `eval_channel_receipt`;
- selection: `eval_selection_decision`, `eval_selection_candidate`, `eval_continuation_decision`.

Raw scheduler snapshots can still be retained as compatibility evidence while migration proceeds.

## Follow-up audits

- Find every production read of `scheduler.json` and classify as: policy fallback, operator preview, compatibility recovery, or stale authority risk.
- Find every production read of latest `runner-result.json` and compare against attempt-scoped `results/<runtime>.json` and channel terminal payloads.
- Audit `branches.json` for which fields should become candidate/patch/operation/evaluation relations and which are legacy snapshots.
- Audit `RecordFamily` variants and mark only the questionable ones here; move clear first-class DB candidates back to the relational model.
- Audit old controller trace/report surfaces for remaining consumers.
- After DB parity for typed relations, decide whether to keep, gate, or retire `records/mirror.cozo.sqlite`.
