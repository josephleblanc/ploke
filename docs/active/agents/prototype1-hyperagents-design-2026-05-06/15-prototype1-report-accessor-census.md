# Agent 15: Prototype 1 Report Accessor Census

Date: 2026-05-06

Scope: `crates/ploke-eval/src/cli/prototype1_state/report.rs`,
`metrics.rs`, and `cli_facing.rs` path/load/report helpers, with
`crates/ploke-eval/src/campaign.rs`,
`crates/ploke-eval/src/intervention/scheduler.rs`, and
`crates/ploke-eval/src/intervention/branch_registry.rs` read only where they
define campaign, scheduler, node, runner, or branch accessors.

## Summary

The current code already contains most of the operators needed to locate,
load, decode, join, summarize, and project Prototype 1 campaign/runtime/
evaluation files. They are not one coherent operator yet.

The closest reusable lower evidence layer is the `EvidenceStore` /
`FsEvidenceStore` plus `Document` / `EvidencePointer` machinery in
`crates/ploke-eval/src/cli/prototype1_state/history_preview.rs`, consumed by
`metrics.rs`. It discovers the filesystem evidence surface, decodes JSON, and
keeps path/hash refs. `metrics.rs` then assembles rows with `SourceRef`s and
selection-source authority labels. This should be the seed of the operator
feeding archive selection and operator reports, but it still uses JSON-field
inspection and degraded joins rather than typed evidence bundles.

The rest of the scoped reports should remain projections over that lower
evidence layer. `report.rs`, monitor timing, branch status/show/apply reports,
loop traces, and runner reports are useful operator views, but they either
drop source hashes, read mutable latest files, infer identity from paths, or
summarize away compared-instance provenance.

## Locator Operators

Campaign roots and campaign-local files:

- `campaign_manifest_path(campaign_id)` returns
  `<campaigns_dir>/<campaign_id>/campaign.json`
  (`crates/ploke-eval/src/campaign.rs:233`).
- `campaign_closure_state_path(campaign_id)` returns
  `<campaigns_dir>/<campaign_id>/closure-state.json`
  (`crates/ploke-eval/src/campaign.rs:237`).
- `load_existing_prototype1_campaign(campaign_id)` joins manifest path,
  decoded manifest, resolved campaign config, closure-state path, and a
  slice dataset path fallback beside the manifest
  (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:287`).
- `prototype1_campaign_root(manifest_path)` and `report.rs::prototype_root`
  both derive `<campaign>/prototype1` from the campaign manifest parent
  (`cli_facing.rs:1825`, `report.rs:610`).

Scheduler, node, and runner paths:

- `prototype1_scheduler_path(manifest_path)` returns
  `<campaign>/prototype1/scheduler.json`
  (`crates/ploke-eval/src/intervention/scheduler.rs:240`).
- `prototype1_node_dir(manifest_path, node_id)` returns
  `<campaign>/prototype1/nodes/<node-id>`
  (`intervention/scheduler.rs:248`).
- `prototype1_node_record_path`, `prototype1_runner_request_path`, and
  `prototype1_runner_result_path` locate `node.json`,
  `runner-request.json`, and latest `runner-result.json`
  (`intervention/scheduler.rs:252`, `:256`, `:260`).
- `prototype1_trace_path(manifest_path)` in `cli_facing.rs` returns
  `<campaign>/prototype1-loop-trace.json`, outside the `prototype1/` root
  even though `prototype1_monitor_locations` describes a legacy loop trace
  under `prototype1/prototype1-loop-trace.json`
  (`cli_facing.rs:7523`, `:1862`). That mismatch is a locator drift risk.

Branch and evaluation paths:

- `prototype1_branch_registry_path(manifest_path)` returns
  `<campaign>/prototype1/branches.json`
  (`crates/ploke-eval/src/intervention/branch_registry.rs:156`).
- `treatment_branch_id(source_state_id, target_relpath, candidate_id)` is a
  deterministic branch id from a hash of those three coordinates
  (`branch_registry.rs:160`).
- `report.rs::Report::load` constructs `evaluation_dir` as
  `<campaign>/prototype1/evaluations` and reads every `.json` file there
  (`report.rs:76`, `:81`, `:629`).
- `history_preview::FsEvidenceStore::documents` also treats
  `<campaign>/prototype1/evaluations` as the `Evaluation` evidence class
  (`history_preview.rs:84`).

Monitor discovery:

- `prototype1_monitor_locations(manifest_path, repo_root)` is the operator
  location index for humans. It enumerates campaign manifest, prototype root,
  scheduler, branch registry, transition journal, evaluations, nodes,
  node record, runner request/result, invocation, attempt result, successor
  ready/completion, child worktree/build products, and active parent identity
  (`cli_facing.rs:1832`).
- `peek_prototype1_monitor_locations` expands those locations by collecting
  existing files under the prototype root before printing excerpts
  (`cli_facing.rs:4110`).

## Typed Readers And Decoders

Campaign:

- `load_campaign_manifest(campaign_id)` reads and decodes `CampaignManifest`
  and verifies the embedded campaign id matches the requested campaign
  (`campaign.rs:243`).

Scheduler/node/runner:

- `load_or_default_scheduler_state(campaign_id, manifest_path)` decodes
  `Prototype1SchedulerState` or fabricates a default if `scheduler.json` is
  absent (`intervention/scheduler.rs:385`).
- `load_scheduler_state(manifest_path)` is the strict scheduler reader
  (`intervention/scheduler.rs:402`).
- `load_node_record`, `load_runner_request`, `load_runner_result`, and
  `load_runner_result_at` decode typed node/request/result records
  (`intervention/scheduler.rs:493`, `:505`, `:517`, `:525`).
- `Prototype1RunnerCommand::run` joins scheduler, node, runner request,
  optional latest runner result, and filesystem existence checks into
  `Prototype1RunnerReport` (`cli_facing.rs:1478`).

Branch registry:

- `load_or_default_branch_registry(campaign_id, manifest_path)` decodes
  `Prototype1BranchRegistry` or fabricates an empty default if `branches.json`
  is absent (`branch_registry.rs:185`).
- `resolve_treatment_branch(campaign_id, manifest_path, branch_id)` scans the
  registry and returns `ResolvedTreatmentBranch` with source node fields and
  the matched branch (`branch_registry.rs:583`).
- `active_branch_selection_for_target` finds an active target and its branch by
  `target_relpath`, `source_state_id`, and `active_branch_id`
  (`branch_registry.rs:547`).

Journals and evaluations:

- `report.rs::load_journal(path)` uses `PrototypeJournal::load_entries` and
  returns an empty vector if the journal file is missing (`report.rs:617`).
- `report.rs::load_evaluations(dir)` scans direct `.json` files and decodes
  `Prototype1BranchEvaluationReport` (`report.rs:629`).
- `history_preview::FsEvidenceStore::transition_journal` loads stored journal
  entries from `prototype1_transition_journal_path` with pointers and hashes
  (`history_preview.rs:65`).
- `history_preview::FsEvidenceStore::documents` decodes scheduler, branch
  registry, evaluations, nested invocation/result/successor documents, node
  records, runner requests, and runner results as classified `Document`s
  (`history_preview.rs:70`).

Diagnostic/runtime logs:

- Monitor timing `Report::load` reads scheduler/node/latest-runner-result
  records and joins observation evidence, agent turn traces, run artifacts,
  stream timings, and provider HTTP events into `NodeEvidence`
  (`cli_facing.rs:2374`).
- `parse_observation_log` decodes JSONL observation lines, filters by campaign,
  and extracts either `ProviderHttpEvent`s or timed `ObservationStep`s
  (`cli_facing.rs:3877`). It contains a temporary bridge that may attribute
  unscoped provider attempts to a campaign when any line in the file mentions
  that campaign (`cli_facing.rs:3927`).

## Join Operators

Current join keys are useful but unevenly preserved:

- `report.rs::selected_trajectory` joins scheduler nodes by `node_id`,
  starts from `last_continuation_decision.selected_next_branch_id` or max
  generation, then walks `parent_node_id` backward (`report.rs:661`).
- `report.rs::DedupedFields::from_sources` unions campaign ids, node ids,
  generations, runtime ids, branch ids, candidate ids, source state ids, and
  target relpaths across scheduler, branch registry, and journal entries
  (`report.rs:514`).
- `metrics.rs::Assembly` builds `branch_to_node: BTreeMap<String, String>` and
  joins evaluations to nodes by `branch_id` (`metrics.rs:1001`, `:1197`).
- `metrics.rs::apply_node`, `apply_request`, `apply_result`, and
  `apply_invocation` populate a row by `node_id`, with fallbacks from path
  stems when a document lacks a node id (`metrics.rs:1030`, `:1047`, `:1060`,
  `:1072`).
- `metrics.rs::apply_evaluation` keys evaluation totals by `branch_id`, falling
  back to the evaluation filename stem if the field is missing
  (`metrics.rs:1104`).
- `metrics.rs::apply_selection_projection` recursively scans scheduler and
  branch-registry JSON for `selected_next_branch_id` and selected branch
  statuses, marking those selections as `mutable_projection`
  (`metrics.rs:1129`, `:1799`).
- `metrics.rs::apply_journal` treats successor selected records in the
  transition journal as selection evidence with `transition_journal` authority
  (`metrics.rs:1010`).
- `metrics.rs::cohorts` groups rows by `(lineage: None, parent_node_id,
  generation)` and emits a diagnostic that lineage is unavailable
  (`metrics.rs:1283`).
- `metrics.rs::Trajectory::from_decisions` projects selected rows into a
  degraded parent/generation trajectory and records ambiguity/incompleteness
  diagnostics (`metrics.rs:542`).
- `cli_facing.rs::run_prototype1_loop_controller` joins closure rows to
  protocol availability, issue detection/synthesis, branch registry source
  nodes, scheduler nodes, branch evaluations, selected branch id, and
  continuation decision (`cli_facing.rs:735`).
- `prototype1_source_generation` recursively follows `parent_branch_id` through
  branch-registry source nodes; typed parent runs bypass that with
  `parent.generation + 1` in `prototype1_child_generation`
  (`cli_facing.rs:6521`, `:6547`).

## Summary And Projection Builders

`report.rs`:

- `Report::load` builds a provisional aggregate containing source paths,
  scheduler view, registry view, journal view, evaluation view, deduped fields,
  and a hard-coded list of weak/missing fields (`report.rs:75`).
- `SchedulerView::from_state` counts scheduler statuses and embeds
  `selected_trajectory` (`report.rs:162`).
- `RegistryView::from_registry` counts source nodes, active targets, branches,
  selected branches, and branch statuses (`report.rs:262`).
- `JournalView::from_entries` counts journal kinds and flags
  `imported_as_legacy_evidence: true` (`report.rs:317`).
- `EvaluationView::from_reports` folds `Prototype1BranchEvaluationReport`s into
  branch/disposition counts and operational totals (`report.rs:375`).

`metrics.rs`:

- `build(campaign_id, manifest_path)` uses `FsEvidenceStore`, applies journal
  entries first, then classified documents, then finishes a `Dashboard`
  (`metrics.rs:73`).
- `SourceRef::{from_pointer, from_document}` preserve evidence class, ref id,
  path, and hash (`metrics.rs:955`).
- `Dashboard::slice` and `Dashboard::print` produce summary/cohort/trajectory
  views without adding source facts (`metrics.rs:127`, `:170`).
- `ScoreDerivation::from_row`, `assign_ranks`, `generations`, `cohorts`, and
  `Trajectory` create heuristic dashboard ranks and degraded trajectories
  (`metrics.rs:390`, `:1239`, `:1270`, `:1283`, `:489`).
- `strongest_selection_authority` prefers `transition_journal` over
  `mutable_projection` (`metrics.rs:1828`).

`cli_facing.rs`:

- `Prototype1LoopReport` is a legacy/controller trace projection containing
  campaign paths, selected targets, staged nodes, branch evaluation summaries,
  selected next branch id, and pending stages (`cli_facing.rs:6810`).
- `Prototype1LoopBranchEvaluationSummary` copies compact operational totals
  from a full branch evaluation report or synthesizes a reject row from a
  runner failure (`cli_facing.rs:6874`, `:6403`, `:6458`).
- `select_most_promising_branch` is the old branch selector over those compact
  summaries (`cli_facing.rs:6492`). It is generation-local and does not read
  source refs.
- `prototype1_branch_status_report` projects branch registry state into flat
  rows (`cli_facing.rs:6559`).
- `Prototype1BranchShowReport`, `Prototype1BranchApplyReport`,
  `Prototype1RunnerReport`, `Prototype1StateReport`, and monitor timing
  `Report` are CLI-facing operator views (`cli_facing.rs:6962`, `:6981`,
  `:6893`, `:6911`, `:1967`).
- `selection_input_from_child_report` converts a node plus
  `Prototype1BranchEvaluationReport` into `SelectionInput`, but drops
  baseline/treatment record paths and carries only metrics/status plus the
  evaluation artifact path (`cli_facing.rs:7020`).

## Where Provenance Is Lost

- `report.rs::EvaluationView::from_reports` preserves the evaluation artifact
  path per row but drops source document hash, loader ref id, compared-instance
  run record paths, per-instance reasons, and branch-registry summary
  provenance (`report.rs:375`).
- `report.rs::DedupedFields::from_sources` deduplicates ids without retaining
  which source carried each id, so it cannot answer conflict or custody
  questions (`report.rs:514`).
- `report.rs::selected_trajectory` is a scheduler-only path through mutable
  nodes and `last_continuation_decision`, not an authority-backed lineage
  (`report.rs:661`).
- `metrics.rs` keeps `SourceRef` path/hash on rows, which is good, but it often
  extracts fields from raw `serde_json::Value` and uses fallback path stems.
  That preserves discoverability, not typed validity (`metrics.rs:1030`,
  `:1104`, `:1905`).
- `metrics.rs::collect_selected_branches` recursively treats any matching JSON
  field shape as a selected branch. It labels this `mutable_projection`, but
  the extraction itself is shape-based rather than typed (`metrics.rs:1799`).
- `metrics.rs::cohorts` explicitly records that lineage is unavailable and
  groups by degraded parent/generation coordinates (`metrics.rs:1283`).
- `selection_input_from_child_report` loses compared-instance
  `baseline_record_path` and `treatment_record_path`, even though the source
  report has them (`cli_facing.rs:6995`, `:7020`).
- `summarize_prototype1_branch_evaluation` copies operational counts out of
  the full report into `Prototype1LoopBranchEvaluationSummary`, dropping
  per-instance metrics, reasons, and paths (`cli_facing.rs:6403`).
- `summarize_prototype1_failed_branch_evaluation` invents
  `<runner-result-only>` when no evaluation path is available, which is honest
  as a marker but not a resolvable evidence ref (`cli_facing.rs:6458`).
- Monitor timing joins observation events by node id or branch id and turn
  traces by path mention, useful for operators but not enough for selection
  without stronger runtime/campaign binding (`cli_facing.rs:2374`).
- `parse_observation_log` has a temporary campaign-attribution bridge for old
  unscoped provider attempts; this must not feed selection as authoritative
  evidence (`cli_facing.rs:3927`).
- `Prototype1BranchApplyCommand::run` documents that manual apply reconstructs
  a candidate from a branch handle and lets lower code derive text-file surface
  identities, not whole-worktree artifact identity (`cli_facing.rs:1231`).
- `select_treatment_branch` and related registry accessors recover missing
  artifact/patch ids from text-file fallbacks, useful for continuity but not
  equivalent to original artifact provenance (`branch_registry.rs:452`).

## Should These Feed Selection?

Use as the evidence-operator seed:

- `history_preview::EvidenceStore` / `FsEvidenceStore` as the filesystem
  discovery boundary because it classifies evidence and computes source
  pointers/hashes.
- `metrics.rs::SourceRef`, `SelectionSource`, and `Assembly` as the current
  proof that rows can carry source provenance and distinguish journal authority
  from mutable projections.
- Typed readers from `campaign.rs`, `scheduler.rs`, and `branch_registry.rs`
  where they decode `CampaignManifest`, `Prototype1SchedulerState`,
  `Prototype1NodeRecord`, `Prototype1RunnerRequest`,
  `Prototype1RunnerResult`, and `Prototype1BranchRegistry`.

Keep as projections:

- `report.rs::Report` and all `*View` types.
- `cli_facing.rs` monitor location/peek/watch/timing reports.
- `Prototype1LoopReport`, `Prototype1LoopBranchEvaluationSummary`,
  `Prototype1BranchStatusReport`, branch show/apply reports, runner report,
  and state report.
- `select_most_promising_branch`, unless explicitly documented as the legacy
  controller heuristic. It should not become the archive selector.

Required shape for the real operator:

- It should emit typed evidence bundles, not CLI report rows. A candidate
  bundle should carry `campaign_id`, `node_id`, `parent_node_id`, generation,
  `runtime_id`, `branch_id`, `candidate_id`, branch/evaluation/report paths,
  baseline and treatment run record refs, source hashes, derivation version,
  and authority class.
- It should expose joins by campaign, node, runtime, branch, parent, and
  evaluation artifact as named relations, not ad hoc maps hidden inside a view.
- It should let operator reports, metrics dashboards, and selection inputs be
  projections over the same evidence bundle.
- It should preserve degraded status when evidence came from mutable scheduler
  or branch-registry projections, path-stem fallback, unscoped observation logs,
  latest runner result, or text-file artifact fallback.

## Design Implication

Do not promote existing reports into the selection source of truth. Promote the
lowest common evidence accessors into a typed, digest-backed evidence operator,
then make current reports and selection inputs consume that operator.

The practical migration path is:

1. Reuse `FsEvidenceStore` discovery and `SourceRef` path/hash identity.
2. Add typed decoding for the known evidence classes before JSON-field
   extraction.
3. Build an explicit candidate/evaluation bundle keyed by campaign, node,
   runtime, branch, and parent relation.
4. Project `metrics::Dashboard`, `report.rs::Report`, monitor timing, and
   successor/archive selection inputs from that bundle.

That preserves current operator ergonomics while preventing mutable or lossy
views from becoming the causal authority for HyperAgents-style archive
selection.
