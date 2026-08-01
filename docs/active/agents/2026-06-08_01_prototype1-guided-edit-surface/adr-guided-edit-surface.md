# ADR: Prototype 1 guided edit surface

## Status

PARTIALLY IMPLEMENTED.

## Context

Prototype 1 currently has a working parent turn and child fanout loop, but the
broad candidate generator still asks child patch-generation attempts to infer
the useful target surface from a large prompt and sparse evidence roots. Recent
runs prove that loop control can continue through multiple parent handoffs, but
run reviews still show weak branch-level semantic coverage and noisy target
choice:

- `p1-selectfix-replay-5g1x2-a2-20260607-224502` stopped cleanly after
  generation 4 children with direct-Google eval and protocol routes, but the
  final reports explicitly do not claim every branch is semantically useful.
- The current workspace baseline gate is red: `cargo test --workspace 2>&1 | rg
  -A 8 E0` produced cargo exit `101`, while the `rg` filter hid the failing
  output. A follow-up unfiltered test probe is in progress.

The larger semantic object is not a single pre-edit prompt. It is a bounded
planning and admission procedure between protocol review and mutable-surface
publication:

```text
protocol/run evidence -> PlanReview -> graph-bounded request -> child patch generation
```

The planning procedure must preserve evidence provenance, target-pipeline
intent, mutable-surface scope, and missing-data diagnostics. Collapsing it into
one untyped prompt would lose the information needed for History admission,
run-review replay, selection-score analysis, and future route/cost/trust
reporting.

## Decision

Add a parent-side planning stage before child patch generation.

The stage asks a higher-capacity direct-Google model to review recent protocol
output, agent traces, run artifacts, branch evaluations, and current code. It
must return a structured plan containing:

- target pipeline to improve;
- cited protocol, agent-trace, run-review, and code evidence;
- the wider pipeline touched by the proposed edit;
- intended behavioral improvement;
- missing evidence or missing telemetry that weakens the plan;
- recommended mutable surface policy.

The planning stage is advisory for patch generation and reportable for
selection analysis. It is not History authority by itself and does not promote
a child. Promotion remains gated by child self-eval, protocol/oracle evidence
when configured, selection policy, and sealed History handoff.

Implementation note, 2026-06-08: the broad harness parent path now writes a
pre-child planning prompt and artifact before broad TUI patch generation starts.
Production uses a direct-Google planner call; test builds write a deterministic
planner stub artifact to verify sequencing without a live provider.

Add a restricted broad-edit policy that derives mutable targets from the code
graph rather than from the whole workspace. The initial policy is:

```text
seeds = code items defined in crates/ploke-tui/src/tools/mod.rs modules
mutable = N nearest graph items from the Cozo code graph
direction = protocol findings + tool failures + PlanReview target pipeline
protected = policy-bearing ploke-eval surface remains immutable
```

The new policy should extend the existing edit-surface carriers rather than
create a mirror schema. The current reusable carriers are:

- `edit_surface::graph::{Projection, Bounds, Rule, Target, Span}`;
- `edit_surface::surface::{EditObjective, ObjectiveSpec, Grant, SurfaceRequest}`;
- `edit_surface::harness_request::{PublishedBroadHarnessRequest, RequestAdmissionBinding}`;
- `ploke-records` run-profile, branch, evaluation, selection, History, and run
  record DTOs.

Implementation note, 2026-06-08: request JSON and prompts now carry
`graph_restriction` with configurable `nearest_items` from
`execution.broad_tui.graph_nearest`. This is visible to the planner and patching
model. Admission still uses the existing `WorkspaceExceptPlokeEval`
protected-core gate; rejecting broad harness submissions outside the material
Cozo graph-neighborhood remains a required follow-up before the policy is fully
enforced.

Add multi-instance Prototype 1 loop support as a first-class run-profile path.
`target.instances` already exists and `target.eval_instances()` already returns
the intended cohort. The ADR direction is to keep `legacy` generation
single-instance, while `broad-harness-request` and later graph-guided policies
must preserve the full target cohort through setup, child self-eval, MBE/oracle
execution, branch evaluation, and selection evidence.

Add a parallel persistent Cozo records mirror, separate from the current file
authority. It should ingest the same typed persisted record families rather
than becoming a second authority:

- campaign, profile, scheduler, branch, node, invocation, channel, runner,
  evaluation, protocol, run, History, and selection records;
- append/projection metadata sufficient to rebuild views;
- missing-record and manual-join diagnostics.

Add missing-data tracking for selection-score mechanisms as report-only
telemetry first. The initial target is to record which data needed by
`ploke-selection-score` is present, absent, manually joinable, or not
applicable at selection time. This follows the June 5 selection-score notes:
new route-cost, episode-status, eval-set, evaluator-trust, artifact-diff, and
scheduler-distribution metrics should be reported before they influence
selection.

## Affected Pipelines

Parent runtime and child-plan publication:

- `prototype1-state` is the parent turn that resolves identity, establishes
  baseline evidence, plans children, observes results, selects successors, and
  may launch the next parent.
- The planning stage belongs before broad-harness request publication and
  before `ChildPlanFiles` admission, not inside child self-eval.
- Existing `PublishedBroadHarnessRequest` and request admission bindings should
  carry the selected policy and surface evidence so child results cannot drift
  back into unbound text-branch candidates.

Edit-surface and broad TUI:

- The broad harness currently uses `BroadEditPolicy::WorkspaceExceptPlokeEval`.
  The graph-guided policy should narrow this, not weaken the protected-core
  invariant.
- The policy-bearing `ploke-eval` surface remains out of ordinary mutable scope
  because `prototype1_state/mod.rs` and `history/mod.rs` both describe the
  digest-preservation invariant for admitted descendants.

Protocol and run review:

- Protocol outputs become inputs to `PlanReview`, but protocol text does not
  become promotion authority.
- Run reviews remain the quality gate for saying whether a branch produced
  useful benchmark progress.
- The planning stage must cite evidence paths so later review can reconstruct
  the chain from protocol/tool findings to target choice and patch outcome.

History and successor selection:

- Sealed History blocks remain the authority-bearing substrate. Mutable
  projections such as scheduler, branch registry, request files, and the new
  Cozo mirror are evidence/projections until admitted.
- Selection-score data should be sealed or referenced only after the current
  selection machinery can prove the underlying records and eval-set identities.

Multi-SWE-Bench and MBE:

- `target.instances` defines the intended eval cohort for multi-instance loops.
- MBE/oracle execution must cover exactly the configured cohort when enabled.
- Branch evaluation and selection evidence must preserve the compared instance
  set and any missing treatment instances.

Records graph and Cozo mirror:

- File records stay the write authority for current Prototype 1.
- The parallel Cozo DB is a durable read-side mirror and join/projection store.
- Direct DB inspection remains out of normal log-reading workflow; use typed
  loaders or planned query surfaces.

Implementation note, 2026-06-08: `JsonRecordFile` now mirrors emitted JSON
records into `$PLOKE_EVAL_HOME/records/mirror.cozo.sqlite` after the file write
succeeds. The mirror stores record family, schema, format, path, content hash,
timestamp, and JSON payload in a separate `prototype1_record` relation.

## Relationship To Existing Documentation

`crates/ploke-eval/src/cli/prototype1_state/mod.rs` defines the broader
Artifact/Runtime/Parent/Child/Successor/History model. This ADR keeps the
planning stage inside that model: a parent runtime may plan and publish child
patch-generation work, but a child still has to produce evaluated evidence and
a successor still has to satisfy handoff authority.

`crates/ploke-eval/src/cli/prototype1_state/history/mod.rs` states that History
is the sealed authority surface, while scheduler, branch registry, reports, and
database side tables are projections or evidence sources. This ADR preserves
that boundary by making PlanReview and the Cozo mirror non-authoritative until
their facts are admitted or referenced by existing History entries.

The walkthrough worktree at
`/home/brasides/code/ploke-walkthrough` documents the live
parent turn, config planes, broad-harness request path, turn-live replay, and
model/API boundaries. This ADR follows its distinction between setup/admission
and runtime parent turns, and it keeps model/provider provenance explicit:
planning, parent patch generation, eval, protocol, and embeddings may have
different route needs, but this feature should use direct-Google generation and
protocol routes while OpenRouter remains allowed only for embeddings.

The June 5 selection-score notes recommend report-only instrumentation before
selector-driving use. This ADR adopts that posture for missing-data tracking
and rejects a new aggregate selector that mixes cost, trust, protocol,
embedding, and oracle evidence prematurely.

## Consequences

Positive:

- Child patch generation receives a narrower, evidence-backed target surface.
- Protocol and tool failures become actionable without becoming promotion
  authority.
- Multi-instance eval becomes a real loop behavior instead of a profile-only
  parse path.
- The Cozo mirror can support run review, graph policy, and selection analysis
  without weakening file/History authority.
- Selection-score integration starts with auditable missing-data evidence.

Negative:

- The parent turn gets another live model call and another persisted planning
  artifact.
- A graph-guided surface depends on current code graph freshness and clear
  failure behavior when graph evidence is missing.
- Multi-instance loops increase runtime cost, artifact volume, and review load.
- The Cozo mirror creates another durability surface that must be kept
  projection-only until its authority contract is explicit.

Neutral:

- `legacy` generation remains single-instance.
- Existing broad workspace policy can remain available for explicit comparison
  runs, but should not be the default for tool-pipeline improvement campaigns.

## Compliance And Guardrails

- Do not relax History, profile, import, or backup-fixture correctness to make
  old artifacts load.
- Do not treat PlanReview prose as selection authority.
- Do not let graph lookup failure silently widen the mutable surface.
- Do not let missing selection-score telemetry silently score as zero; record it
  as missing data with the mechanism and required carrier named.
- Do not route generation/protocol through OpenRouter for this feature unless
  the operator explicitly changes the run policy. OpenRouter is acceptable for
  embeddings only.
- Before any code symbol edit, run GitNexus impact analysis on the target
  symbol and respect high-risk warnings.

## Verification Plan

1. Baseline:
   - Record current `cargo test --workspace` status.
   - If red, classify the exact failure before implementation proceeds.
2. ADR/docs:
   - Keep this packet indexed from `docs/active/agents/readme.md`.
   - Keep implementation notes in `changelog.md`.
3. Unit tests:
   - Profile parsing/validation for multi-instance graph-guided runs.
   - Request rendering and request JSON roundtrip for the new planning/surface
     policy.
   - Missing-data tracking for selection-score report-only fields.
4. Live tests:
   - Direct-Google planning call behind `live_api_tests`.
   - Direct-Google parent patch-generation path with graph-restricted surface.
   - Multi-instance MBE/oracle loop when MBE is enabled.
5. Loop proof:
   - Run `prototype1-continue` on a fresh admitted campaign.
   - Prove at least three generations with successful parent/successor handoff.
   - Review generated branches with the run-review quality gate.
6. Campaign review:
   - Use Prototype 1 run-status tooling for final campaign synthesis.
   - Run MBE over valid produced patches where eligible.
   - Document trajectory, blockers, and cumulative diff review.
