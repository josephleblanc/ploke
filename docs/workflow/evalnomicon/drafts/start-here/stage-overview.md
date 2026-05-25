# Prototype 1 Loop Stage Overview

Prototype 1 is a runtime succession loop over Artifacts, Runtimes, and
lineage-local authority. It is not a flat `baseline -> patch -> rerun` loop.
The key reason is simple: mutating an Artifact does not mutate the already
running parent binary. A candidate Artifact must hydrate a fresh child Runtime
before its modified behavior can be evaluated.

The current public phase surface is `DiagnosedPhase` in
[`run/core.rs`](../../../../../crates/ploke-eval/src/cli/prototype1_state/run/core.rs).
`prototype1-doctor` reports one phase. `prototype1-step` advances at most one
phase. `prototype1-continue` repeats the same diagnosis and advance loop until
it reaches `complete`, `blocked`, or its guard limit.

## Stage List

| Stage | Status | Doctor phase | What advances it | Main evidence written or read |
| --- | --- | --- | --- | --- |
| Setup and admission | implemented | before doctor | `prototype1-setup` | `campaign.json`, `closure-state.json`, `prototype1/run-profile.toml`, `run-profile.commitment.json`, `.ploke/prototype1/parent_identity.json`, scheduler state |
| Baseline eval | implemented | `baseline_eval` | `advance_eval_closure` | baseline run records under `~/.ploke-eval/instances/prototype1/<campaign>/...`, `closure-state.json` |
| Baseline protocol | implemented | `baseline_protocol` | `advance_protocol_or_block` | protocol artifacts referenced by baseline run records, `closure-state.json` |
| Child planning | implemented | `child_plan` | `resolve_profile_child_plan` | `messages/child-plan/<parent-node-id>.json`, broad-harness requests/results when configured |
| Materialize child Artifact | implemented | `materialize` | `run_planned_child(..., stop_after = Materialize)` | child node worktree, node projection, runner request, transition journal |
| Build child Runtime | implemented | `build` | `BuildChild` over `C2 -> C3` | child binary path, node status, build journal entries |
| Spawn child Runtime | implemented | `spawn` | `SpawnChild` over `C3 -> C4` | invocation JSON, channel files, streams, runtime id, node status |
| Observe child result | implemented | `observe` | `ObserveChild` over `C4 -> C5` | runner result, treatment evidence, branch evaluation report |
| Select successor | implemented | `select` | `select_successor_for_profile` | successor selection journal record and selection seal material |
| Handoff successor | partially implemented | `handoff` | `spawn_and_handoff_prototype1_successor` | active checkout update, sealed History block, successor invocation, ready/completion records |
| Terminal | implemented | `complete` | no-op | read-only status |
| Stop branch | implemented as diagnosis, operator procedure outside state machine | `blocked` | no loop advance | blocker text, doctor output, artifacts selected for repair or abandon/restart |

The materialize/build/spawn/observe rows are implemented as live phases. Their
`C1` through `C5` names are transition vocabulary for the typed model; the
source still preserves scaffold warnings around parts of that migration.

## Setup And Admission

Setup is not one of the doctor phases, but it is a required loop stage.
`prototype1-setup` admits the run profile, creates or adopts campaign state,
creates the generation-0 parent identity, writes the parent identity into the
active checkout, and commits that identity as the parent witness.

The admitted profile is the loop configuration authority for target selection,
model defaults, search bounds, generation mode, selection policy, protocol
policy, MBE settings, and local control. The current profile schema and
validation live in
[`profile.rs`](../../../../../crates/ploke-eval/src/cli/prototype1_state/profile.rs).
The operator reference is
[`prototype1-run-profile.md`](../../../../../crates/ploke-eval/docs/prototype1-run-profile.md).

Important setup distinction:

```text
active parent checkout = where the Parent Runtime runs
campaign root = where durable loop state is stored
child worktree = temporary Artifact surface for one candidate
```

A child worktree is not the successor's long-lived home. A selected successor
Artifact is installed into the stable active checkout before successor launch.

## Baseline Eval

For generation 0, diagnosis enters `baseline_eval` when eval closure is not
complete or closure state is missing. This phase runs baseline evaluation for
the admitted campaign target set. It produces run artifacts and updates closure
state. Candidate planning does not begin until baseline eval and baseline
protocol have both completed.

The stage is mechanically complete only when closure says eval is complete. A
later run review may still decide that the run artifacts are benchmark-useless
or internally contradictory.

## Baseline Protocol

Diagnosis enters `baseline_protocol` when baseline eval is complete but
protocol closure is not. This phase runs the admitted protocol policy over the
baseline evidence. The protocol result can guide later targeting and selection
metrics, but it is not by itself proof that the model improved anything.

When provider route, request shape, reasoning policy, or token budget is the
risk, `prototype1-doctor --live-protocol-preflight` is the current preflight
surface. It is a live provider call and should be used deliberately.

## Child Planning

Diagnosis enters `child_plan` when the active parent has no valid child-plan
message. Planning verifies the active parent, reserves child budget from the
admitted search policy, and resolves the configured candidate generator.

For the current broad-harness path, the parent publishes request-bound harness
requests, waits for admitted submitted results, and writes a child-plan message
whose children are bound to the current parent node and expected child
generation. The child-plan message is a typed message box, not an arbitrary
scratch file.

The child-plan receiver checks:

- recipient parent node id
- child generation equals parent generation plus one
- planned child node records and runner requests match the parent-owned plan
- broad-harness children carry request-bound workspace and artifact evidence

## Materialize

Diagnosis enters `materialize` when the next planned child node is still
`Planned`. The current step path calls `run_planned_child` with a materialize
stop. The typed scaffold names this as `C1 -> C2`:

```text
C1 = parent binary over parent artifact world
C2 = parent binary over child artifact world
```

The semantic point is that the parent remains the same already-running binary,
but the candidate Artifact has been realized in a separate workspace.

## Build

Diagnosis enters `build` when a child node is `WorkspaceStaged`. The transition
uses `BuildChild` to move from `C2` to `C3`:

```text
C3 = parent binary over child artifact world, child binary present
```

Build failures are recorded as rejected transition results rather than being
lost as control-flow noise. On success, the child binary is promoted to the
node-owned binary path and the buildable Artifact is persisted for later
selection/handoff.

## Spawn

Diagnosis enters `spawn` when a child node is `BinaryBuilt`. The transition
uses `SpawnChild` to move from `C3` to `C4`:

```text
C4 = parent binary over child artifact world, child runtime acknowledged
```

Spawn writes a persisted child invocation and uses the parent-child channel to
observe readiness. The child invocation role is deliberately limited: a child is
a leaf evaluator for one node. It is not allowed to recurse or promote itself.

## Observe

Diagnosis enters `observe` when a child node is `Running`. The transition uses
`ObserveChild` to observe terminal child output and move toward `C5`:

```text
C5 = parent has observed one terminal child state
```

The parent reads the runner result and treatment evidence. When the child
succeeds, the parent compares the treatment evidence against the parent
baseline and writes a branch evaluation report. This comparison is the main
bridge from child execution to selection input.

## Select

Diagnosis enters `select` when all planned children are terminal and
`select_successor_for_profile` can resolve an admitted successor candidate.
Selection combines current-generation child outcomes with any admitted History
candidate scope requested by the run profile.

Selection does not mean the selected child has promoted itself. It means the
parent has applied the admitted policy over available evidence and written a
successor selection record. The successor still needs handoff before it can
become the next parent.

## Handoff

Diagnosis enters `handoff` after a selected successor marker exists and before
successor completion. The handoff path:

1. rehydrates the selected Artifact payload;
2. validates the selection and selected branch/node identity;
3. installs the selected Artifact into the stable active checkout;
4. builds the successor binary from that active checkout;
5. moves the parent to a retired state;
6. seals and appends a History block;
7. writes a successor invocation;
8. launches the successor;
9. waits for the successor ready acknowledgement.

The source-level authority claim is local and narrow. History and Crown gate
the lineage transition. They do not prove distributed consensus, global process
uniqueness, or correctness of LLM judgment.

## Complete And Blocked

`complete` means diagnosis sees no next active phase for the current parent
turn. It is a terminal status for the current control command, not a global
statement that the campaign was scientifically successful.

`blocked` means required state, prompt references, child-plan shape, provider
preflight, or other consistency checks failed. `prototype1-step` refuses to
advance from `blocked`. The operator branch is to freeze evidence, classify the
failure, and decide repair-and-resume or abandon-and-restart.

## Step Versus Continue

`prototype1-step` diagnoses once, advances one phase if possible, diagnoses
again, and prints status. Child phases run with cap 1.

`prototype1-continue` repeats diagnosis and advance until `complete`,
`blocked`, or the 256-advance guard trips. Child phases use the admitted
effective control cap. Do not run overlapping step/continue commands against
the same campaign or worktree.
