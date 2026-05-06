# Agent 11: Successor Selection Registry and Archive Gap

Scope: `crates/ploke-eval/src/successor_selection`, especially `registry.rs`, plus direct callers observed in `crates/ploke-eval/src/cli/prototype1_state` and `crates/ploke-eval/src/intervention/scheduler.rs`.

## Summary

Current Prototype 1 successor selection is a generation-local child selector, not a HyperAgents-style archive parent selector.

The implemented registry is a small domain-evaluation scaffold. It can evaluate a `SelectionInput` and emit structured `DomainFinding` records, but the default registry contains only the operational domain, and `SuccessorDecision::from_findings` folds only the operational verdict into `Accepted`, `Stop`, or later `ExploreFrom`.

The available candidate population is the active parent turn's planned direct children: generation `parent.generation + 1`, `parent_node_id == active_parent.node_id`, bounded by `child_budget.max`, run in batches of `child_budget.min`, and stopped early if an accepted child appears. There is no implemented archive-wide population of admitted parents, no archive frontier ranker, and no persisted score object that can compare older parents or sibling lineages outside the current generation.

## Current Successor Selection Module

- `successor_selection` documents itself as generation-local evidence used by a ruling parent to decide whether a completed child should become the next successor (`crates/ploke-eval/src/successor_selection/mod.rs:1`).
- `PROCEDURE_ID` is `successor-selection:v1` (`crates/ploke-eval/src/successor_selection/mod.rs:21`).
- `decide(input)` constructs `SelectionRegistry::default()` and delegates to the registry (`crates/ploke-eval/src/successor_selection/mod.rs:23`).
- `decide_generation(inputs)` iterates inputs in caller-provided order. The first `Accepted` decision wins immediately; if none are accepted, it considers only `BranchDisposition::Reject` inputs and selects the best rejected child as `ExploreFrom` by an internal tuple score (`crates/ploke-eval/src/successor_selection/mod.rs:28`).
- `CandidateRef` stores only `node_id`, `branch_id`, and `generation`; it does not carry `campaign_id`, `parent_node_id`, runtime id, artifact id, History block id, or archive admission status (`crates/ploke-eval/src/successor_selection/mod.rs:102`).

## Registry Contents

- `SelectionRegistry` has a private `domains: Vec<DomainKind>` field (`crates/ploke-eval/src/successor_selection/registry.rs:14`).
- `DomainKind` currently has one variant: `Operational` (`crates/ploke-eval/src/successor_selection/registry.rs:8`).
- `Default` hard-codes `vec![DomainKind::Operational]` (`crates/ploke-eval/src/successor_selection/registry.rs:19`).
- `SelectionRegistry::decide` maps each configured domain through `evaluate_domain` and passes the collected findings to `SuccessorDecision::from_findings` (`crates/ploke-eval/src/successor_selection/registry.rs:27`).
- The domain model already names future surfaces: `DomainName::{Operational, Protocol, Patch, Oracle, Adjudication}` (`crates/ploke-eval/src/successor_selection/domains/mod.rs:12`). Only `Operational` is implemented and registered.

This is reusable as a plug-in policy scaffold, but not yet a policy plug-in boundary. A new domain can be added structurally, but it cannot affect selection unless `DomainKind`, registry construction/configuration, and the decision fold are extended. Today `SuccessorDecision::from_findings` looks up the operational finding and ignores other domain names when choosing the outcome (`crates/ploke-eval/src/successor_selection/decision.rs:21`).

## Evidence, Scores, and Provenance References

`SelectionInput` contains:

- `candidate: CandidateRef`;
- `branch_disposition: BranchDisposition`;
- `evaluation_artifact_path: PathBuf`;
- `comparisons: Vec<RunComparison>`.

Each `RunComparison` contains `instance_id`, optional parent metrics, optional child metrics, and string `status` (`crates/ploke-eval/src/successor_selection/evidence.rs:9`).

The operational domain:

- skips comparisons without both parent and child metrics (`crates/ploke-eval/src/successor_selection/domains/operational.rs:19`);
- counts improved and regressed field-level metric comparisons (`crates/ploke-eval/src/successor_selection/domains/operational.rs:30`);
- maps keep-with-no-regressions to `Better`, keep-with-regressions to `Mixed`, rejected rows with regressions/no improvements to `Worse`, and missing comparable rows to `Inconclusive` (`crates/ploke-eval/src/successor_selection/domains/operational.rs:39`);
- emits one evidence ref, `path:<evaluation_artifact_path>` (`crates/ploke-eval/src/successor_selection/domains/operational.rs:61`);
- compares failed tool calls, patch failures, same-file retry counts/streaks, abort flags, nonempty valid patch, convergence, and oracle eligibility (`crates/ploke-eval/src/successor_selection/domains/operational.rs:75`).

The rejected-child exploration score is not a named or persisted score object. It is an internal tuple over child metrics only: oracle eligible, convergence, nonempty patch, patch attempted, fewer failed tool calls, and fewer total tool calls (`crates/ploke-eval/src/successor_selection/mod.rs:60`).

The direct input builder has richer source evidence than `SelectionInput` preserves. `Prototype1BranchEvaluationReport` contains baseline/treatment campaign ids, branch registry path, evaluation artifact path, treatment campaign manifest, closure state path, branch disposition, reasons, and compared instance rows with baseline/treatment record paths and metrics (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6995`). `selection_input_from_child_report` drops the baseline/treatment record paths and carries only metrics/status plus the evaluation artifact path into successor selection (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:7020`).

## Decision Types and Continuation Mapping

`SuccessorDecision` records procedure id, candidate node id, selected branch id, branch disposition, outcome, findings, and rationale (`crates/ploke-eval/src/successor_selection/decision.rs:8`).

`SuccessorOutcome` is:

- `Accepted`;
- `ExploreFrom`;
- `Stop`.

Operational `Better` maps to `Accepted`; `Mixed`, `Worse`, and absent/inconclusive operational findings map to `Stop` (`crates/ploke-eval/src/successor_selection/decision.rs:27`). `ExploreFrom` is introduced by `decide_generation` after no accepted child exists, not by the domain fold itself (`crates/ploke-eval/src/successor_selection/mod.rs:46`).

The decision maps to scheduler-facing `Prototype1SelectionPolicyOutcome` as:

- `Accepted` -> `Accepted`;
- `ExploreFrom` with branch disposition `reject` -> `ExploreFromRejected`;
- `ExploreFrom` otherwise -> `Accepted`;
- `Stop` -> no policy outcome (`crates/ploke-eval/src/successor_selection/decision.rs:64`).

Scheduler continuation policy then decides whether the chosen branch can advance. Defaults are `max_generations=1`, `max_total_nodes=32`, `child_budget=2..6`, `require_keep_for_continuation=true`, and `explore_from_rejected=true` (`crates/ploke-eval/src/intervention/scheduler.rs:15`). `decide_continuation_with_selection` stops on missing selection, rejected selection unless explicit exploration is allowed, stop-on-first-keep, generation cap, or total-node cap; otherwise it continues ready or continues from rejected exploration (`crates/ploke-eval/src/intervention/scheduler.rs:688`).

The selected decision is appended to the transition journal as `SuccessorRecord::selected_with_decision`, embedding both the continuation decision and `SuccessorDecision` (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5978`; `crates/ploke-eval/src/cli/prototype1_state/successor.rs:20`).

## Direct Callers and Candidate Population

The typed parent path is the active direct caller:

- `run_planned_child` observes a completed child and builds `SelectionInput` only for `ObservedChild::Succeeded` (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5369`).
- `accepted_selection` calls `successor_selection::decide` for completed child inputs and returns the first accepted decision (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5518`).
- `generation_selection` calls `accepted_selection` first, then calls `successor_selection::decide_generation` over completed child inputs if no accepted child exists (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5528`).
- The parent turn invokes `generation_selection` only when `stop_after == Complete`, then passes the resulting selected node into continuation and successor handoff (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5953`).

Candidate creation and narrowing are generation-local:

- Target selection stages branches from the selected source node, capped by `search_policy.child_budget.max`, and registers treatment evaluation nodes with the computed child generation and optional parent node id (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:940`).
- In typed parent runs, `prototype1_child_generation` returns `parent.generation + 1`; only untyped/legacy source traversal uses branch-registry source generation (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6547`).
- `runnable_candidate_nodes` filters scheduler nodes by optional required generation, active selected instance, optional parent node id, frontier status, and selected branch ids from the branch registry (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5010`).
- `runnable_child_plan_nodes` always supplies both `required_generation` and `parent_node_id` for the active parent (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5084`).
- `resolve_child_plan` states and enforces that Parent `k` may only materialize candidates produced as generation `k + 1`; explicit `--node-id` is rejected if its generation or parent does not match the active parent (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5180`).
- The child plan record stores `parent_node_id` and `child_generation = parent.generation + 1`, and receiver validation repeats that rule (`crates/ploke-eval/src/cli/prototype1_state/parent.rs:111`).
- `validate_child_plan` checks that the report's staged nodes exactly match the plan's direct children for the active parent (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:569`).
- Before fanout, the parent truncates planned nodes to `child_budget.max`; `run_child_fanout` runs batches of size `child_budget.min` and stops early when `accepted_selection` finds an accepted child (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5925`; `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5427`).

There is also an older loop path that selects a branch directly from branch evaluation summaries using `select_most_promising_branch`. That function prefers kept branches, then scores oracle eligibility, convergence, nonempty submission, applied patch, keep disposition, fewer failed tool calls, and fewer total tool calls (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6492`). It does not call `successor_selection`, and it is not an archive parent selector.

## Reusable Pieces for Plug-In Policies

Useful reusable structure:

- `Domain` as a trait over `SelectionInput -> DomainFinding`;
- `DomainFinding` as a serializable evidence/rationale carrier;
- `MetricComparison` and `MetricDirection` for field-level deltas;
- `SuccessorDecision` as a journal-embeddable decision record with procedure id, findings, rationale, and selected branch;
- `SelectionInput` as a minimal branch-evaluation input for parent-vs-child operational comparisons;
- `Prototype1SelectionPolicyOutcome` and continuation dispositions for preserving the difference between accepted continuation and exploration from a rejected child.

Required changes for true plug-in policies:

- make registry construction policy/config driven rather than `SelectionRegistry::default()` with private hard-coded `Operational`;
- add a policy fold that can combine multiple domain findings instead of choosing solely from the operational verdict;
- make scores first-class if selection quality must be compared, audited, or replayed;
- carry provenance needed by archive policies: campaign id, parent node id, runtime id, artifact ids, branch evaluation report paths, baseline/treatment run record refs, evaluator/procedure ids, evidence digests, and History/Crown authority status;
- distinguish successor continuation from archive admission and archive parent eligibility.

## HyperAgents Archive Selection Gaps

The current code enforces direct-child, generation-local selection in several places. That is coherent for the present parent/child handoff path, but it leaves these gaps relative to HyperAgents-style archive parent/child selection:

- No archive population: there is no type that enumerates admitted archive entries across lineages/generations as parent candidates.
- No archive admission status in selection input: `SelectionInput` cannot tell whether a candidate is sealed, admitted, provisional, degraded, or merely a scheduler/branch-registry projection.
- No parent selector: selection ranks children of the active parent, not possible next parents from an archive/frontier.
- No archive provenance root: evidence refs are path strings, not committed digests tied to History/Crown blocks or artifact-local manifests.
- No reusable score record: operational and exploration scores are local computations; the selected score, tie-breaks, candidate pool, and losing candidates are not persisted as a replayable selection event.
- No policy identity beyond `successor-selection:v1`: the registry contents and fold are not serialized as a policy object with versioned parameters.
- No cross-generation exploration: rejected-child exploration can continue from a rejected direct child, but cannot choose a non-child archive coordinate.
- No separation between archive admission and successor handoff: a `SuccessorDecision` answers whether to continue from a child branch, not whether the child/evidence bundle belongs in a long-lived archive or whether that archive entry is eligible to be a future parent.

## Implementation Direction Suggested by the Audit

Do not replace the existing generation-local selector; it is a useful local continuation policy. Add an archive-facing layer above or beside it.

A small archive-aware slice would define an `ArchiveCandidate` or equivalent typed carrier with candidate identity, parent relation, generation, admitted authority status, artifact/runtime refs, evaluation refs, and policy evidence refs. Then an archive selection policy can reuse `DomainFinding`-style evidence and `SuccessorDecision`-style rationale while producing a different durable record: archive parent selection over an explicit candidate set, not direct-child successor continuation.

The existing direct-child selector can remain the local policy for deciding whether a just-finished child should be handed off immediately. HyperAgents archive selection needs a separate candidate-population boundary and a replayable archive-selection record before it can claim archive parent/child selection.
