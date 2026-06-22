# Prototype 1 Eval Store — Debugging and Research Query Workloads

Status: active planning note / workload review.

Related files:

- [`relational-data-model.md`](relational-data-model.md)
- [`database-planning-notes.md`](database-planning-notes.md)
- [`cozo-feature-map.md`](cozo-feature-map.md)
- [`codegraph-eval-join-model.md`](codegraph-eval-join-model.md)
- [`storage-plan.md`](storage-plan.md)
- [`open-questions.md`](open-questions.md)
- [`crates/ploke-eval/docs/prototype1/debugging-playbook.md`](../../../../crates/ploke-eval/docs/prototype1/debugging-playbook.md)
- [`crates/ploke-eval/docs/prototype1/phase-map.md`](../../../../crates/ploke-eval/docs/prototype1/phase-map.md)
- [`crates/ploke-eval/docs/prototype1/recovery.md`](../../../../crates/ploke-eval/docs/prototype1/recovery.md)
- [`docs/workflow/evalnomicon/src/prototype1/runtime-loop.md`](../../../workflow/evalnomicon/src/prototype1/runtime-loop.md)
- [`docs/workflow/evalnomicon/src/prototype1/selection-and-evaluation.md`](../../../workflow/evalnomicon/src/prototype1/selection-and-evaluation.md)
- [`docs/workflow/evalnomicon/src/prototype1/persistence-and-observability.md`](../../../workflow/evalnomicon/src/prototype1/persistence-and-observability.md)

## Purpose

This note reviews the eval-store schema from the perspective of actual loop debugging, run review, and research-analysis workloads.

The central question is: **what would we want to query during and after Prototype 1 runs that is currently reconstructed by hand from ad hoc files?**

The answer should shape the database more than the current filesystem layout does. The file store is useful migration evidence, but it should not force a one-file-one-table model.

## Source sample

This pass sampled current loop docs and recent run/bug evidence, including:

- clean configured stops:
  - [`2026-06-09 ... terminal-outcome`](../run-reviews/2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-terminal-outcome.md)
  - [`2026-06-08 selectfix terminal-status`](../run-reviews/2026-06-08-p1-selectfix-replay-5g1x2-a2-20260607-224502/terminal-status.md)
- provider/environment failures:
  - [`2026-06-09 treatment auth 401`](../run-reviews/2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-184302-treatment-auth-401.md)
  - [`2026-06-10 malformed_function_call`](../../bugs/2026-06-10-direct-google-malformed-function-call-finish-reason.md)
- lifecycle/observability gaps:
  - [`2026-06-06 request-only no diagnostics RCA`](../run-reviews/2026-06-06-p1-admissionfix-g35flash-p25flash-20260606-090815-parent-request-only-rca.md)
  - [`2026-06-09 late child result recovery`](../../bugs/2026-06-09-prototype1-late-child-result-recovery-corrupts-node-state.md)
- edit/index/validation ambiguity:
  - [`2026-06-09 post-apply refresh wrong crate`](../../bugs/post-apply-freshness/2026-06-09-prototype1-post-apply-refresh-selects-wrong-crate.md)
  - [`2026-05-24 cargo validation and trace summary ambiguity`](../../bugs/2026-05-24-cargo-tool-validation-and-trace-summary-ambiguity.md)

These files are run-scoped evidence, not proof that each issue remains live. They are useful because they show the query shapes reviewers repeatedly needed.

## Main thesis

The natural shape is not “the current files, but in Cozo.”

The natural shape is closer to four cooperating layers:

1. **Event spine:** phase, attempt, channel, harness-slot, model/tool/edit, validation, provider, and import events with causal ordering.
2. **Dimension/entity spine:** campaign, profile commitment, parent epoch, runtime, attempt, child/candidate, artifact, benchmark instance, model/provider route, code graph/artifact scope.
3. **Evidence refs:** raw records, logs, full responses, snapshots, protocol artifacts, History refs, and blob refs with hashes and sensitivity labels.
4. **Derived/query projections:** current phase, liveness, candidate status, validation coverage, provider-vs-merit disposition, benchmark trend facts, and UI/research rollups.

Files map into this model as sources and refs. They should not define the model.

## Loop-time workloads

### 1. Phase/liveness diagnosis

Current loop docs emphasize `prototype1-doctor`, `walk summary`, and read-only replay before mutation. A DB-backed store should make the doctor-style query first-class:

```text
Given repo-root/campaign/parent identity:
  show current phase;
  show authority evidence used for that phase;
  show stale projections that disagree;
  show the next safe effectful edge;
  show blockers with source refs.
```

This requires append evidence plus derived phase views, not a mutable scheduler table. `scheduler.json` and `node.json` can be compatibility refs; the phase query should prefer typed transition, channel, message-box, evaluation, and History refs.

### 2. Missing-evidence diagnosis

The request-only/no-diagnostics RCA shows the needed query:

```text
For each published harness slot:
  request exists?
  workspace materialized?
  launch record exists?
  provider/model request made?
  terminal diagnostic/result exists?
  child-plan admitted/rejected it?
```

The important DB feature is a **gap row**, not just positive evidence. Absence is a bug class. Every expected slot should produce either a terminal result, a terminal diagnostic, or an explicit missing/fallback record that can be joined into child-plan rejection.

### 3. Provider/infrastructure versus candidate merit

The auth-401 and provider-429 reviews repeatedly needed to separate:

- model/candidate quality;
- provider auth/capacity failure;
- harness setup failure;
- validation setup failure;
- selection policy rejection.

The schema should make `failure_domain` or equivalent queryable on attempts/evaluations, while still preserving the raw provider error in refs. A provider-aborted child should not disappear into generic `treatment_failed` if the research question is “did this patch improve the benchmark?”

### 4. Late-result and idempotent recovery

The late-child result bug needs a causal query:

```text
observe_child timeout event at T1
child channel terminal result at T2 > T1
runner result mirror written at T2
branch evaluation missing
re-entry attempted at T3
node projection downgraded after T3
```

This is a strong argument for:

- explicit event indexes/cursors, not timestamp-only ordering;
- attempt-scoped terminal evidence separate from mutable node projections;
- idempotent recovery guards that query terminal channel/result evidence before materialize/build;
- `eval_evidence_warning` rows for contradictory projections.

### 5. Agent trajectory review

Run reviews and the runtime-playback docs ask for chains like:

```text
model request -> provider response -> tool call -> tool result visible to model
  -> model quote/interpretation -> edit proposal -> apply/stage result
  -> validation command -> final response/terminal state
```

This is the research trajectory object. It is more than a compressed run record and more than a log. It needs ordered `eval_model_exchange`, `eval_tool_event`, `eval_edit_event`, `eval_validation_event` or equivalent fields, with sidecar refs for full payloads.

### 6. Post-apply code graph freshness

See also [`codegraph-eval-join-model.md`](codegraph-eval-join-model.md). The wrong-crate refresh bug points to a code-graph/eval overlay workload:

```text
Which files changed?
Which crate roots own those paths?
Which crate roots were rescanned?
Which code graph snapshot did later lookup use?
Did stale-anchor failures follow an unrelated refresh?
```

That suggests first-class index/refresh events. They should join touched paths to code graph artifact scope, crate root, tracking hash, and later tool failures. This is a better model than reading logs for `scan_for_change in crate_name: globset`.

### 7. Validation semantics

The cargo ambiguity bug shows that lifecycle success is not semantic validation success.

Useful query fields include:

- command/cwd/manifest path;
- changed-path coverage;
- target benchmark instance;
- exit code and semantic `ok`;
- failed tests or compile errors summarized;
- whether stdout/stderr/tool result was model-visible;
- whether later model text or actions responded to the failure;
- whether final validation was weaker than the candidate claim.

A `ToolCompleted` event with `ok=false` must not be counted as successful validation merely because the tool lifecycle completed.

## Research workloads

### Benchmark and harness performance over time

For papers and longitudinal analysis, the DB should support rollups by:

- campaign/profile commitment;
- code version / parent artifact / selected successor artifact;
- benchmark instance and dataset;
- model id, route source, provider, request parameters, token-budget policy;
- run mode: baseline, broad child, treatment, protocol, replay;
- outcome: applied patch, validation pass/fail, protocol coverage, selection disposition, oracle/MBE result when present;
- failure domain: provider, model behavior, tool/schema, harness lifecycle, validation setup, candidate merit, authority/History.

This is a star-schema-like workload over event/ref data. It does not require all raw payloads inline.

### Agent process metrics

Trajectory metrics that should be queryable without bespoke scripts:

- time/cost/tokens to first useful edit;
- repeated same-file repair loops;
- stale-index or stale-anchor loops;
- validation-after-edit rate;
- tool-output-used-before-next-edit examples;
- model/provider finish reasons by route and token-budget policy;
- hidden apply failures caught by later validation/protocol;
- final answer parity with persisted patch/eval facts.

These metrics are useful both for debugging and for process-reward style research.

### Harness change analysis

When the harness changes, we need to compare behavior across code versions and profile commitments:

```text
before/after PR or commit
  grouped by benchmark instance, model route, and phase
  with provider failures separated from model/candidate failures
  with confidence about validation coverage and protocol coverage
```

This argues for explicit `harness_version` / binary/artifact refs and profile commitment rows. Do not infer harness version only from local paths.

### Human and adjudicator annotations

Run reviews are currently Markdown documents with human judgments: “clean configured stop,” “provider failure,” “weak validation,” “benchmark usefulness uncertain,” etc.

Those should remain distinct from raw evidence, but they are valuable data. Consider a small annotation layer:

```text
eval_review
eval_review_finding
eval_annotation
```

A finding should cite source refs and carry reviewer/source identity. It should not overwrite raw attempt/evaluation facts. This would let research queries filter to human-reviewed labels without treating them as authority.

## Cozo feature opportunities, behind traits

See [`cozo-feature-map.md`](cozo-feature-map.md) for the source-checked feature review, including the algorithms/fixed-rule page. Implementation should not hard-code Cozo-only assumptions into high-level loop code. Still, Cozo features can inform the backend-specific implementation and query traits.

Feature families to consider from the Cozo docs (`https://docs.cozodb.org/en/latest/`) and current `ploke-db` practice:

| Feature family | Useful for | Suggested abstraction |
| --- | --- | --- |
| Recursive Datalog rules and graph algorithms/fixed rules | lineage paths, parent/successor ancestry, causal chains, artifact derivation paths, connectedness/cycle checks, shortest evidence paths | `LineageQuery`, `CausalPathQuery`, `EvalGraphQuery` |
| Aggregation/grouping | benchmark rollups, provider failure rates, validation coverage, cost/token summaries | `ResearchMetricsQuery` |
| Explicit relation keys + small lookup relations | idempotent imports, alternate lookup by parent/runtime/node/turn/tool | `EvalStoreSchema`, `EvalLookupQuery` |
| Transactions / batched writes | import one JSONL stream or one phase atomically | `BatchImportStore` |
| Validity/time-travel relations | materialized current projections and “state as of” debug views | `TemporalProjectionStore` |
| Vector/HNSW search | similar trajectories, similar prompts/failures, code/evidence retrieval | `TraceVectorSearch` |
| Text/BM25-style search where available or implemented in `ploke-db` | search logs/tool payload summaries/review findings | `TraceTextSearch` |
| Backup/export/restore | reproducible research snapshots and historical replay fixtures | `EvalSnapshotStore` |
| Fixed/custom rules where appropriate | specialized path scoring or graph algorithms | backend-specific extension trait |

High-level callers should ask for capabilities, not for Cozo scripts. For example, the loop can depend on `SelectionAuditQuery`, while the Cozo backend can implement that with recursive rules and joins.

## Schema pressure from incidents

| Incident/workload | Current pain | Schema pressure |
| --- | --- | --- |
| Clean configured stop reviews | Manual join across transition journal, channels, runner results, stale node/scheduler files | Derived terminal-chain query with projection-staleness warnings |
| Auth 401 / provider 429 | Provider aborts collapse into `treatment_failed` and poison merit analysis | `failure_domain`, provider error classification, route/model/request parameter rows |
| Request-only broad slots | No per-slot launch/exit/fallback record, so absence is unexplained | `eval_harness_slot` plus launch/terminal/fallback lifecycle events |
| Late child result | Terminal channel/result evidence arrives after timeout and direct re-entry mutates projections | event ordering, recovery guard queries, projection contradiction warnings |
| Malformed function call / truncation | Need compare finish reasons, token floors, tool choice, model family over time | model exchange params, finish reason, token usage, route policy, version/profile commitment |
| Wrong-crate refresh | Logs expose that refresh scanned unrelated crate | index/refresh events joined to touched paths and code graph artifact scope |
| Cargo ambiguity | Tool lifecycle success hides semantic failure or weak validation scope | basic validation event fields: command/manifest/model-visible payload/semantic status; codegraph-backed coverage rows for changed-path coverage |
| Manual run reviews | Findings are not queryable or aggregatable | optional review/annotation layer, source-cited and non-authoritative |

## Relation additions or refinements to consider

These are not mandatory first-slice relations, but the workload review suggests they will be useful:

```text
eval_harness_slot              -- one parent-side broad/edit slot, expected terminal evidence
eval_attempt_lifecycle_event   -- launch, started, joined, timed_out, killed, no_start, fallback_diag
provider-call dimensions       -- prefer fields inside eval_model_exchange first; promote eval_provider_call only if normalization demands it
eval_validation_event          -- command/cwd/manifest/semantic status; coverage joins can follow in `eval_validation_cover`
eval_refresh_event             -- touched path -> owning crate/root -> refreshed DB/artifact snapshot
eval_review                    -- human/adjudicator report metadata
eval_review_finding            -- source-cited non-authoritative judgments
eval_experiment                -- optional research grouping over campaigns/profile commitments
```

Some can start as views over existing event families. These are workload-driven candidates, not first-slice schema; promote them through [`relational-data-model.md`](relational-data-model.md) before implementation. The important part is preserving the query dimensions.

## First workload-driven queries to implement

For the first file-or-db storage pass, DB slices should be judged by whether they can answer these without bespoke filesystem walks while preserving current file output:

1. **Phase with authority evidence**: current parent phase, evidence used, stale projections excluded or warned.
2. **Harness slot coverage**: for each planned slot, request/workspace/launch/model-call/terminal/result status, using eval evidence rows and refs.
3. **Provider-vs-merit failures**: child/eval failures grouped by domain, model, provider, route, and benchmark instance.
4. **Late-result detector**: terminal child result after observe timeout with missing comparison/evaluation.

Follow-on code-graph/research queries after file/db parity:

5. **Validation coverage**: final validation commands joined to changed paths/code refs versus weak unrelated focused checks.
6. **Agent trajectory chain**: tool output visible to model -> next model action -> edit -> validation.
7. **Benchmark trend rollup**: success/keep/reject/provider-failure/cost by profile commitment and harness version.

If a proposed relation does not help the first-pass storage queries, it can probably stay as an `eval_record_ref` or be deferred to the follow-on overlay/research pass.

## Guardrails

- Do not weaken authority: sealed History, Channel transport, and MessageBox lock/unlock semantics remain separate.
- Do not merge child-local code graph facts into parent scope by default.
- Do not let compatibility imports or passive mirror rows silently become selection authority.
- Do not treat missing records as ordinary nulls when the lifecycle expected a record; emit evidence warnings/gap events.
- Do not make timestamps the only ordering mechanism.
- Do not store full LLM/provider payloads inline by default; keep refs, hashes, redaction/sensitivity labels, and normalized query dimensions.
