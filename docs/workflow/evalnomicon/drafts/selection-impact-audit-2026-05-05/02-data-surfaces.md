# Prototype 1 Child Selection Impact Audit: Data Surfaces

Date: 2026-05-05

Scope: persisted data, History, telemetry, and projection impact of selecting a Prototype 1 child node.

## Summary

Selecting a child currently feeds two nearby but distinct paths:

- The typed successor-selection path builds `SelectionInput` from one persisted branch evaluation report, evaluates only the operational domain, records a `SuccessorRecord::Selected` into the transition journal, updates scheduler continuation state, and may trigger successor handoff.
- Older/projection paths still infer selected branches from mutable scheduler/branch-registry documents, branch summaries, and dashboard ranking. These are useful operator/model attention surfaces, but they do not have Crown/History authority.

The main risk is not that telemetry directly becomes selection authority. The main risk is that operational metrics copied into branch evaluation JSON can be treated downstream as if they prove oracle/adjudication/protocol correctness, or as if a selected branch in mutable projections is equivalent to a sealed History admission.

## Causal Evidence Map

### 1. Child eval/evidence -> branch evaluation report

Branch evaluation compares baseline and treatment closure rows. For each baseline instance, it reads compressed run records only when the closure row is complete, derives `OperationalRunMetrics`, compares baseline vs treatment, and stores the copied metrics plus per-instance status in `Prototype1BranchEvaluationReport`.

Key reads:

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6100-6210`
  - Builds `Prototype1BranchEvaluationReport`.
  - Reads `baseline_record_path` and `treatment_record_path`.
  - Records `baseline_metrics`, `treatment_metrics`, `evaluation`, and `status`.
  - Sets `overall_disposition = Keep` only when no rejection reasons exist.
- `crates/ploke-eval/src/operational_metrics.rs:36-69`
  - Defines the metric fields consumed by branch evaluation and successor selection.
  - Explicitly separates `nonempty_valid_patch`, `convergence`, and `oracle_eligible`.
- `crates/ploke-eval/src/operational_metrics.rs:106-118`
  - Documents `nonempty_valid_patch` as a conservative pre-oracle proxy and makes `oracle_eligible` depend on a nonempty submission artifact.
- `crates/ploke-eval/src/branch_evaluation.rs:25-85`
  - Programmatic branch disposition compares operational metrics only.
- `crates/ploke-eval/src/cli/prototype1_process.rs:1897-1922`
  - Persists the branch evaluation JSON at `prototype1/evaluations/<branch-id>.json`.

Preservation note: this surface is programmatic operational evidence. It is not an external oracle result, not LLM adjudication, and not protocol-review evidence.

### 2. Runner result -> observed child -> successor-selection input

The child runner writes an attempt-scoped result that points to the evaluation report. Parent-side observation loads that report and carries it as the successful child observation.

Key reads:

- `crates/ploke-eval/src/intervention/scheduler.rs:107-130`
  - `Prototype1RunnerResult` stores `node_id`, `branch_id`, `status`, `disposition`, optional `treatment_campaign_id`, and optional `evaluation_artifact_path`.
- `crates/ploke-eval/src/cli/prototype1_process.rs:1595-1631`
  - Successful runner results copy `report.evaluation_artifact_path` and are written attempt-scoped plus latest-result.
- `crates/ploke-eval/src/cli/prototype1_state/c4.rs:132-142`
  - Loads a `Prototype1BranchEvaluationReport` from JSON.
- `crates/ploke-eval/src/cli/prototype1_state/c4.rs:374-407`
  - Requires `runner_result.evaluation_artifact_path`, loads the report, and records `ObservedChildResult::Succeeded { evaluation_artifact_path, overall_disposition }`.
- `crates/ploke-eval/src/cli/prototype1_state/journal.rs:165-184`
  - The observe-child journal result records only the evaluation path and branch disposition, not the full report contents.

Impact: selection is fed by the report the runner result points to. If the report has missing baseline/treatment metrics, those omissions flow through as missing comparisons rather than independent proof.

### 3. Branch report -> typed successor-selection decision

The typed successor-selection module is intentionally separate from the operator-facing selection cache. Its current default registry evaluates one domain: operational.

Key reads:

- `crates/ploke-eval/src/successor_selection/mod.rs:1-25`
  - Declares generation-local successor-selection evidence and default `decide`.
- `crates/ploke-eval/src/successor_selection/evidence.rs:9-41`
  - `SelectionInput` contains candidate, branch disposition, evaluation report path, and `RunComparison` records with optional parent/child metrics.
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6820-6845`
  - Converts `Prototype1BranchEvaluationReport.compared_instances` into `SelectionInput`.
  - Drops per-instance `evaluation` reasons except through report-level `overall_disposition` and metrics/status.
- `crates/ploke-eval/src/successor_selection/registry.rs:19-40`
  - Default registry contains only `DomainKind::Operational`.
- `crates/ploke-eval/src/successor_selection/domains/mod.rs:12-20`
  - Names future domains: protocol, patch, oracle, adjudication.
  - They are not currently active in the default registry.
- `crates/ploke-eval/src/successor_selection/domains/operational.rs:15-72`
  - Skips comparisons missing parent or child metrics.
  - `Keep` with comparable metrics and no regressions becomes `Better`.
  - Zero comparable instances becomes `Inconclusive`.
- `crates/ploke-eval/src/successor_selection/decision.rs:20-55`
  - `Better -> Select`, `Mixed -> ContinueWithRisk`, otherwise `Stop`.
  - `ContinueWithRisk` still sets `selected_branch_id`.

Risk: selection evidence refs point at the evaluation artifact path, but the decision does not persist source record digests, metric derivation version, provider attempt refs, protocol artifact refs, or oracle/adjudication refs. The report path is necessary but not sufficient to recover full evidence strength.

### 4. Successor decision -> scheduler, journal, successor handoff

After a successful child observation, parent-side code computes the successor decision, derives a continuation decision, appends a successor transition journal record, updates scheduler continuation state, and may spawn the successor if continuation is ready.

Key reads:

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5736-5796`
  - Loads node and scheduler.
  - Calls `successor_selection::decide(selection_input_from_child_report(...))`.
  - Calls `decide_continuation`.
  - Appends `JournalEntry::Successor(SuccessorRecord::selected_with_decision(...))`.
  - Persists the continuation decision.
- `crates/ploke-eval/src/cli/prototype1_state/successor.rs:20-29`
  - `State::Selected` stores the continuation decision and optional `SuccessorDecision`.
- `crates/ploke-eval/src/cli/prototype1_state/successor.rs:90-105`
  - `selected_with_decision` embeds the full `SuccessorDecision` in the transition journal record.
- `crates/ploke-eval/src/intervention/scheduler.rs:627-660`
  - `decide_continuation` uses only selected branch id, selected branch disposition, generation limits, node count, and scheduler policy.
- `crates/ploke-eval/src/intervention/scheduler.rs:676-685`
  - Persists `last_continuation_decision` to `scheduler.json`.
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5797-5835`
  - Successor handoff is skipped unless the continuation disposition is `ContinueReady`.

Impact: the transition journal is the best persisted record of the typed successor-selection decision. Scheduler state is a mutable projection of the continuation effect.

### 5. Successor handoff -> sealed History

When continuation is ready, handoff seals and appends a local History block before spawning the successor. The sealed block commits successor/artifact identity and admitted artifact claim material, but the selection decision itself is not the authority payload in the block.

Key reads:

- `crates/ploke-eval/src/cli/prototype1_process.rs:1123-1228`
  - Prepares successor runtime, creates a `SealBlock::from_handoff`, seals via `Parent<Selectable>::seal_block_with_artifact`, appends to `FsBlockStore`, and only then proceeds toward spawn.
- `crates/ploke-eval/src/cli/prototype1_state/inner.rs:197-220`
  - `Parent<Selectable>::seal_block_with_artifact` opens a block, admits an artifact claim, locks Crown, and seals.
- `crates/ploke-eval/src/cli/prototype1_state/history.rs:2452-2489`
  - `SealBlock` commits `crown_lock_transition`, `selected_successor`, `active_artifact`, claims, and timestamp.
  - `from_handoff` is explicitly a compatibility constructor with empty claims until the parent-side path supplies admitted claims.
- `crates/ploke-eval/src/cli/prototype1_state/history.rs:3023-3145`
  - Crown methods gate open/admit/seal transitions and check lineage.
- `crates/ploke-eval/src/cli/prototype1_state/history.rs:2688-2716`
  - Intended block content includes stochastic evaluation refs and uncertainty summaries when policy uses them for admission.
- `crates/ploke-eval/src/cli/prototype1_state/history.rs:203-208`
  - Mutable scheduler, branch, invocation, ready/completion, and monitor reports may be cited as evidence/projections but are not History authority until admitted or imported under policy.

Impact: History currently protects successor admission around artifact/surface/head checks. It does not currently seal the full stochastic selection evidence chain as admitted block entries.

### 6. Monitor/report/metrics projections -> future attention

The reporting and metrics surfaces are explicit projections, but they can steer future operator/model attention by highlighting selected rows, selected trajectories, and dashboard ranks.

Key reads:

- `crates/ploke-eval/src/cli/prototype1_state/report.rs:1-7`
  - Report module says it is provisional aggregate evidence, not sealed History.
- `crates/ploke-eval/src/cli/prototype1_state/report.rs:75-113`
  - Report loads scheduler, branch registry, transition journal, and evaluation JSON.
  - It lists weak fields, including missing sealed Crown authority in current records and distributed tool/model/prompt/full-response evidence.
- `crates/ploke-eval/src/cli/prototype1_state/report.rs:150-181`
  - Scheduler view includes `selected_trajectory` and `last_continuation_decision`.
- `crates/ploke-eval/src/cli/prototype1_state/report.rs:625-654`
  - Loads all JSON evaluation reports from the evaluations directory.
- `crates/ploke-eval/src/cli/prototype1_state/report.rs:695-715`
  - Evaluation summaries aggregate only treatment operational metrics.
- `crates/ploke-eval/src/cli/prototype1_state/metrics.rs:1-6`
  - Metrics are projections and do not strengthen mutable scheduler/registry/node files.
- `crates/ploke-eval/src/cli/prototype1_state/metrics.rs:73-109`
  - Metrics build from evidence documents plus transition journal.
- `crates/ploke-eval/src/cli/prototype1_state/metrics.rs:1010-1025`
  - Transition-journal successor selections are marked with authority `transition_journal`.
- `crates/ploke-eval/src/cli/prototype1_state/metrics.rs:1129-1144`
  - Scheduler and branch-registry selected-branch projections are marked `mutable_projection`.
- `crates/ploke-eval/src/cli/prototype1_state/metrics.rs:1799-1817`
  - Selection projection recursively collects any `selected_next_branch_id` or object with `status: selected` and `branch_id`.
- `crates/ploke-eval/src/cli/prototype1_state/metrics.rs:1828-1838`
  - `transition_journal` is stronger than `mutable_projection` only inside the metrics projection.

Risk: recursive selected-branch collection is intentionally broad. It can mark a branch selected from mutable JSON fields that are not typed successor decisions. The `selection_authority` field mitigates this, but downstream prose must preserve that distinction.

## Existing Algebra And Missing Structure

The formal procedure draft separates recorded artifacts from forwarded evidence and treats procedure outputs as typed states rather than bare values:

- `docs/workflow/evalnomicon/drafts/formal-procedure-notation.md:90-118`
  - Distinguishes target metrics, evidential outputs, and supporting metric states.
- `docs/workflow/evalnomicon/drafts/formal-procedure-notation.md:120-140`
  - Separates `Rec(s)` from `Fwd(s)`.
- `docs/workflow/evalnomicon/drafts/formal-procedure-notation.md:195-207`
  - Merge should preserve branch provenance unless explicitly discarded.

Prototype 1 already has several structural carriers, but selection evidence still lacks one durable joined object that ties a selected child to all evidence classes used or excluded:

- `docs/workflow/evalnomicon/drafts/prototype1-persistence-map-2026-05-03/08-synthesis.md:20-28`
  - Useful join spine: campaign -> node/branch/runtime -> runner result -> evaluation report -> compared run records -> provider/protocol evidence.
- `docs/workflow/evalnomicon/drafts/prototype1-persistence-map-2026-05-03/08-synthesis.md:102-112`
  - Gaps include no single authoritative object tying the IDs and paths together, provider retry evidence only in logs/telemetry, metrics/history preview not reading sealed History blocks, and live sealed blocks appearing as zero-entry handoff authority.

Missing structure for this audit:

- A persisted `SelectionEvidence` or admitted History entry that records the selected child, compared run record refs, metric derivation identity, domain set actually evaluated, explicit non-use of oracle/adjudication/protocol evidence, and the selected decision hash.
- A policy distinction between `ContinueWithRisk` and `Select` in downstream continuation. Today both can set `selected_branch_id`; scheduler policy only sees branch id/disposition.
- Typed provider-attempt and timing records. Provider retries/timeouts currently remain telemetry/log evidence and should not be folded into selection claims without a durable attempt record.

## Risks

1. **Incomplete evidence can select.**
   Missing parent/child metrics are skipped by the operational domain. Zero comparable instances stops selection, but partial comparability can still produce a high-confidence-looking operational verdict from a reduced instance set.

2. **`ContinueWithRisk` still selects a branch id.**
   `SuccessorOutcome::ContinueWithRisk` sets `selected_branch_id`, so downstream continuation may treat it as a candidate for successor handoff unless stopped by scheduler policy or generation/node limits.

3. **Selection currently means operationally better, not adjudicated correct.**
   The active registry evaluates operational metrics only. Oracle/adjudication/protocol domains are named but inactive.

4. **Reports can amplify mutable selection.**
   Metrics and report projections display selected trajectories and ranks. They preserve source refs and authority labels, but any downstream summary that drops `selection_authority` may overstate mutable projections.

5. **Sealed History does not yet carry full selection evidence.**
   Handoff seals artifact/surface/head authority before successor spawn, but stochastic evaluation evidence and rejected/failed candidate evidence are only listed as intended block content when policy uses them.

## Verification Commands

Small code-path checks already run:

```sh
cargo test -p ploke-eval successor_selection --lib
cargo test -p ploke-eval journal_selection_attaches_transition_source --lib
```

Useful local inspection commands:

```sh
rg -n "SelectionInput|RunComparison|SuccessorDecision|successor_selection::decide|selected_with_decision|record_continuation_decision" crates/ploke-eval/src
rg -n "evaluation_artifact_path|Prototype1BranchEvaluationReport|compared_instances|operational_metrics" crates/ploke-eval/src/cli/prototype1_state crates/ploke-eval/src/cli/prototype1_process.rs crates/ploke-eval/src/operational_metrics.rs
rg -n "selection_authority|mutable_projection|transition_journal|collect_selected_branches" crates/ploke-eval/src/cli/prototype1_state/metrics.rs
rg -n "SealBlock|selected_successor|stochastic evidence|mutable files" crates/ploke-eval/src/cli/prototype1_state/history.rs
```

Campaign-specific evidence checks:

```sh
jq -c '{branch_id,overall_disposition,compared_instances:[.compared_instances[]|{instance_id,status,has_baseline:(.baseline_metrics!=null),has_treatment:(.treatment_metrics!=null),eval:.evaluation.disposition}]}' ~/.ploke-eval/campaigns/<campaign>/prototype1/evaluations/*.json
jq -c 'select(.kind=="successor") | {node_id,state}' ~/.ploke-eval/campaigns/<campaign>/prototype1/transition-journal.jsonl
jq -c '{last_continuation_decision:.last_continuation_decision}' ~/.ploke-eval/campaigns/<campaign>/prototype1/scheduler.json
jq -c '{height:.state.header.common.block_height,successor:.state.header.selected_successor,claims:.state.header.claims,entry_count:.state.header.entry_count}' ~/.ploke-eval/campaigns/<campaign>/prototype1/history/blocks/segment-000000.jsonl
```

## Preservation Check

- Telemetry and timing metrics are diagnostic/projection surfaces unless promoted into typed records.
- LLM adjudication and external oracle results are not active successor-selection domains in the inspected default path.
- Branch evaluation and successor selection are programmatic operational procedures over copied metrics from run records.
- Scheduler, branch registry, monitor report, and metrics dashboard are mutable projections, not Crown/History authority.
- Sealed History currently authorizes local successor admission around artifact/surface/head commitments; it should not be described as proving the full evaluation chain unless selection evidence is admitted into the block under policy.
