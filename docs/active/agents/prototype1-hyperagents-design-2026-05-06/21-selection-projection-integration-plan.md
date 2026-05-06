# Agent 21: Selection Projection Integration Plan

Date: 2026-05-06

Scope:
- Primary: `crates/ploke-eval/src/successor_selection`.
- Direct callers: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`, `successor.rs`, and continuation use in `intervention/scheduler.rs`.
- Inputs read: reports `11`, `14`, `15`, `16`, `17`, and `18`. Report `19` was not present in this directory when checked.
- Source code was not modified.

## Summary

Keep current `SelectionInput` generation-local and lossy by design. It should be
derived from a richer child evidence grouping, not expanded into the grouping
itself and not made responsible for locating evaluation, run, protocol, or
History evidence.

The immediate integration shape is:

```text
FsEvidenceStore / typed readers
-> child evidence grouping
-> policy-specific projection
-> SelectionInput
-> successor_selection::decide_generation
```

The evidence grouping is the durable join surface. It should preserve
baseline/treatment run record refs, protocol refs, source hashes, child runtime
refs, parent relation, artifact/patch refs, and authority treatment. The current
selector can then keep consuming only candidate identity, branch disposition,
evaluation artifact path, and operational metrics.

This tees up archive traversal because archive selection can later enumerate
admitted or degraded child evidence groupings across generations and lineages,
while the existing successor selector remains the local parent-turn continuation
policy over direct children.

## Current Projection Boundary

`SelectionInput` contains only:

- `candidate: CandidateRef`;
- `branch_disposition: BranchDisposition`;
- `evaluation_artifact_path: PathBuf`;
- `comparisons: Vec<RunComparison>`.

`CandidateRef` contains only:

- `node_id`;
- `branch_id`;
- `generation`.

`RunComparison` contains only:

- `instance_id`;
- optional `parent_metrics`;
- optional `child_metrics`;
- string `status`.

The operational domain reads only that projection. It emits field-level metric
comparisons and one evidence ref, currently `path:<evaluation_artifact_path>`.
`SuccessorDecision` then records procedure id, candidate node id, selected
branch id, branch disposition, outcome, findings, and rationale.

That shape is coherent for the current selector. It is not sufficient as an
evidence locator, an archive admission bundle, or a replayable archive parent
selection record.

## Exact Fields Dropped Today

`selection_input_from_child_report` derives `SelectionInput` from
`Prototype1BranchEvaluationReport` plus `Prototype1NodeRecord`.

From `Prototype1BranchEvaluationReport`, the projection preserves:

- `overall_disposition` as `branch_disposition`;
- `evaluation_artifact_path`;
- each compared instance's `instance_id`;
- each compared instance's `baseline_metrics` as `parent_metrics`;
- each compared instance's `treatment_metrics` as `child_metrics`;
- each compared instance's `status`.

It drops these top-level report fields:

- `baseline_campaign_id`;
- `branch_id` from the report itself;
- `treatment_campaign_id`;
- `branch_registry_path`;
- `treatment_campaign_manifest`;
- `treatment_closure_state_path`;
- `reasons`.

It drops these per-instance report fields:

- `baseline_record_path`;
- `treatment_record_path`;
- `evaluation`.

From `Prototype1NodeRecord`, the projection preserves:

- `node_id`;
- `branch_id`;
- `generation`.

It drops these node fields:

- `schema_version`;
- `parent_node_id`;
- `instance_id`;
- `source_state_id`;
- `operation_target`;
- `base_artifact_id`;
- `patch_id`;
- `derived_artifact_id`;
- `parent_branch_id`;
- `candidate_id`;
- `target_relpath`;
- `node_dir`;
- `workspace_root`;
- `binary_path`;
- `runner_request_path`;
- `runner_result_path`;
- `status`;
- `created_at`;
- `updated_at`.

From the direct caller context, the projection also does not carry:

- `campaign_id`;
- `manifest_path`;
- child runtime id observed by `run_planned_child`;
- child plan index or fanout ordering evidence;
- transition-journal pointers for materialize/build/spawn/observe;
- successor selected-record pointer after the decision is written;
- sealed History block, head, or admission status.

These omissions are acceptable for the current operational selector, but they
must be preserved below the projection if protocol, patch, oracle,
adjudication, archive traversal, or replay need to explain the decision later.

## Child Evidence Grouping

Add the missing structure as a grouping below selection, most likely by
expanding the read-only `history_preview::FsEvidenceStore` path described in
reports `14`, `15`, and `18`.

The grouping should be keyed by child evidence identity, not by the selection
procedure:

```text
campaign_id
parent_node_id
generation
node_id
branch_id
runtime_id when available
evaluation_artifact_path
```

It should carry source refs for every document that contributed facts:

- `EvidencePointer` or `SourceRef` for scheduler, branch registry, node record,
  runner request, runner result, invocation, attempt result, evaluation report,
  and transition journal rows;
- content hash and source class from `Document` / `EvidencePointer`;
- authority treatment such as sealed, sealed ingress, transition journal,
  attempt-scoped, degraded evidence, mutable projection, or report projection.

It should preserve direct-child structure:

- `parent_node_id`;
- child `generation`;
- `child_generation == parent.generation + 1` validation result when known;
- `parent_branch_id` and selected source relation when available;
- `candidate_id`, `source_state_id`, and `target_relpath`;
- `operation_target`, `base_artifact_id`, `patch_id`, and
  `derived_artifact_id`.

This grouping is the place to locate evidence. `SelectionInput` should not scan
the filesystem, parse reports, load run records, discover protocol artifacts,
or classify authority.

## Preserve Baseline And Treatment Run Refs

Each compared instance in the grouping should retain a structured pair:

```text
instance_id
baseline:
  campaign_id
  record_path
  metrics
  run_registration_ref when available
  run_artifact_refs when available
  source_ref/hash for the record path carrier
treatment:
  campaign_id
  record_path
  metrics
  run_registration_ref when available
  run_artifact_refs when available
  source_ref/hash for the record path carrier
evaluation:
  BranchEvaluationResult when available
  status
  reasons or per-instance notes when available
```

The current `RunComparison` projection should be derived from this pair by
dropping everything except `instance_id`, `baseline.metrics`,
`treatment.metrics`, and `status`.

For future replay, the grouping must keep the record paths even when metrics
are present. Otherwise a protocol or oracle domain cannot reopen the exact
baseline/treatment run records that produced a selection verdict.

## Preserve Protocol Refs For Future Domains

The selector already names future domains:

- `Operational`;
- `Protocol`;
- `Patch`;
- `Oracle`;
- `Adjudication`.

Only `Operational` is currently implemented and registered. The evidence
grouping should be ready for the others by carrying nested refs rather than
trying to force them through `SelectionInput` immediately.

For each baseline and treatment run, preserve:

- `RunRegistration` path or `run_id` plus storage root identity;
- `RunArtifactRefs`, especially `record_path`, `protocol_artifacts_dir`,
  `turn_trace`, `turn_summary`, `full_response_trace`, `msb_submission`, and
  `protocol_anchor`;
- exact `record.json.gz` path used for `OperationalRunMetrics`;
- `StoredProtocolArtifact` paths for raw protocol procedure evidence;
- `ProtocolArtifactRef` values from protocol aggregates, with the enclosing
  raw artifact refs available so `run_id`, `subject_id`, `schema_version`,
  `model_id`, and `provider_slug` can be recovered;
- `IssueArtifactRef` and the source `StoredProtocolArtifact` for issue
  detection evidence;
- aggregate derivation notes that state whether duplicate artifacts were
  collapsed or skipped.

Protocol and issue aggregate rows may be projected into future domain findings,
but the selected decision should cite the raw refs used to build those rows.
Rendered protocol reports and campaign triage reports should remain
operator-facing projections, not selection source facts.

## Projection Contract For Current Selection

Define the projection contract before changing selector behavior:

```text
ChildEvidenceGroup -> SelectionInput
```

The projection should:

- require the grouping to represent one direct child of the active parent;
- choose the candidate from the node record: `node_id`, `branch_id`,
  `generation`;
- choose `branch_disposition` from the branch evaluation report's
  `overall_disposition`;
- choose `evaluation_artifact_path` from the same evaluation report;
- map each compared instance into `RunComparison`;
- preserve the current order semantics used by `decide_generation`.

The projection should not:

- dereference run records;
- discover protocol artifacts;
- compute authority treatment;
- decide archive admission;
- decide whether a non-child archive coordinate is eligible.

For now, `SelectionInput` can remain unchanged. If a future implementation needs
the selection decision to cite the grouping, add a compact projection provenance
field to the surrounding successor journal record or decision envelope, not a
filesystem locator inside the operational input.

## Decision And Journal Preservation

The current direct caller appends `SuccessorRecord::selected_with_decision`,
embedding the continuation decision and `SuccessorDecision`. That record is the
right place to preserve decision replay context once the grouping exists.

Recommended future journal additions, without changing the current selector
semantics:

- selected grouping ref;
- projection procedure id, for example
  `child-evidence-to-selection-input:v1`;
- source refs for evaluation report, node record, and compared run records;
- authority treatment summary for the selected grouping;
- candidate set ref for all children considered in this generation.

This keeps `SuccessorDecision` as a policy result while letting the journal
recover the evidence projection that fed it.

## Archive Traversal Later

Archive traversal needs a population boundary that the current selector does
not have. The child evidence grouping creates that boundary.

Later archive selection can enumerate groupings by:

- sealed History block or degraded preview source;
- campaign and lineage head;
- parent node id and generation;
- admitted child artifact;
- `operation_target`, patch id, and derived artifact id;
- evaluation/protocol availability;
- authority treatment.

Then archive selection can produce a separate durable record:

```text
ArchiveCandidateSet
-> archive selection policy
-> ArchiveParentDecision
```

That record should not be `SuccessorDecision`. The current
`successor_selection` module should remain a generation-local continuation
policy over direct child evidence. Archive traversal should consume the same
grouping layer and may reuse `DomainFinding`-style rationale, but it needs its
own candidate population, score record, policy identity, and admission/authority
citations.

## Sequencing

1. Expand `history_preview::FsEvidenceStore` or a sibling read-only operator to
   emit child evidence groupings over existing `Document` and `EvidencePointer`
   records.
2. Decode known evidence classes into typed carriers before falling back to raw
   JSON-field extraction.
3. Preserve baseline/treatment `record_path` refs and run/protocol refs inside
   each compared-instance grouping.
4. Replace `selection_input_from_child_report` internals with a projection from
   the grouping, keeping the resulting `SelectionInput` unchanged.
5. Add projection provenance to the successor selected journal path when the
   grouping has a stable ref.
6. Build archive traversal over grouping enumeration and History authority
   status, not over `SelectionInput`.

## Guardrails

- Do not make `SelectionInput` a filesystem locator.
- Do not make rendered reports or metrics dashboards the selection source of
  truth.
- Do not treat scheduler, branch registry, node mirror, or latest runner result
  as admitted authority without an explicit authority treatment.
- Do not collapse `baseline_record_path` and `treatment_record_path` after
  metrics are derived.
- Do not encode future protocol, patch, oracle, or adjudication evidence as
  strings in operational rationale.
- Keep local successor continuation separate from archive admission and archive
  parent eligibility.
