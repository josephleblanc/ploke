# Prototype 1 Cross-Runtime Communication Audit

Scope: code-path audit from `loop prototype1-state`, starting at
`crates/ploke-eval/src/cli/prototype1_state/mod.rs` and
`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`. No runtime artifacts
were modified.

## Executive Summary

The typed loop is trying to model a graph edge:

```text
Parent<Node G> -> Child<Node G+1> -> Successor Parent<Node G+1>
```

The live persisted model does not carry that edge directly. It stores
`parent_branch_id` in `branches.json`, reconstructs `parent_node_id` later from
`(parent_branch_id, generation - 1)`, and lets the typed parent consumer require
`candidate.generation == parent_identity.generation + 1` plus
`candidate.parent_node_id == parent_identity.node_id`.

That reconstruction explains the recent run10 failure. First children of active
gen0 parent `node-f7b9c198a79ca49a` were registered as generation 2 under
synthetic parent `node-c47370c1ae880bf4`, because the producer inferred source
generation from branch-registry ancestry and treated the root parent artifact
branch as an unknown treatment parent.

## Intended Contract

`mod.rs` describes the intended semantic sequence: a parent creates temporary
child checkouts, evaluates child runtimes, selects a successor artifact, advances
the stable active checkout, launches the successor from that stable checkout,
and then the old parent exits (`crates/ploke-eval/src/cli/prototype1_state/mod.rs:80`,
`:319`, `:420`). It also states the critical invariant: a runtime may become
`Parent<Checked>` only after active checkout, artifact-carried parent identity,
and scheduler node agree (`mod.rs:224`).

The live code enforces this at parent start:

- `ParentIdentity` stores `campaign_id`, `parent_id`, `node_id`, `generation`,
  `parent_node_id`, `branch_id`, optional `artifact_branch`, and `created_at`
  (`identity.rs:24`).
- `ParentIdentity::from_node` copies identity from a scheduler node
  (`identity.rs:42`).
- `Parent<Unchecked>::check` validates checkout through the backend, then checks
  identity generation and branch against the scheduler node (`parent.rs:59`).
- The git backend additionally requires a clean active checkout, matching branch
  when `artifact_branch` is set, a HEAD commit message equal to the parent
  identity message, and a HEAD commit carrying `parent_identity.json`
  (`backend.rs:965`).

## Edge Map

| Edge | Writer | Reader | Expected meaning | Actual persisted fields | Mismatch / assumption |
| --- | --- | --- | --- | --- | --- |
| Initial root node in `scheduler.json`, `nodes/<node>/node.json`, `runner-request.json` | `register_root_parent_node` (`scheduler.rs:229`) called by setup (`cli_facing.rs:174`) | `Parent<Unchecked>::load` (`parent.rs:47`), `run_parent_target_selection` (`cli_facing.rs:300`) | Gen0 artifact admitted as initial parent node | Node: `generation=0`, `parent_node_id=None`, `branch_id=artifact_branch`, `workspace_root=repo_root`; request argv is `loop prototype1-state --repo-root ...` (`scheduler.rs:278`) | Good for gen0, but root parent is only represented in scheduler/identity, not as a branch-registry parent. |
| Initial `parent_identity.json` in active checkout | `write_parent_identity` plus git commit (`cli_facing.rs:190`, `identity.rs:141`) | `resolve_prototype1_parent_identity` (`cli_facing.rs:2387`), backend validation (`backend.rs:965`) | Artifact-carried authority for current parent | `parent_id == node_id`, `generation`, `parent_node_id`, `branch_id`, `artifact_branch` | Good authority carrier, but downstream candidate production mostly receives only `branch_id`, not this full identity. |
| Parent start journal | `Prototype1StateCommand::run_turn` appends `ParentStarted` (`cli_facing.rs:2585`) | Monitor/replay paths; not used to authorize candidate production | Parent runtime entered typed path | `parent_identity`, `repo_root`, optional `handoff_runtime_id`, `pid` (`journal.rs:181`) | Observability only. It does not become a producer-side parent lease or candidate registration authority. |
| Refill target selection from active parent | `resolve_next_candidate_node_id` calls `run_parent_target_selection` on empty candidates (`cli_facing.rs:2312`) | `run_prototype1_loop_controller` (`cli_facing.rs:380`) | Active parent asks producer to synthesize next children | Controller input sets `source_branch_id = parent_identity.branch_id`, dry-run target selection (`cli_facing.rs:310`) | The producer loses `parent_identity.node_id` and `generation`; only branch id crosses the seam. |
| Branch registry source/branches in `branches.json` | `record_synthesized_branches` (`branch_registry.rs:216`) called at `cli_facing.rs:500` | Scheduler staging (`cli_facing.rs:585`), branch resolution (`branch_registry.rs:583`) | Synthesis result from one parent/target source; candidate branches | Source: `source_state_id`, optional `parent_branch_id`, `operation_target`, selected branch; branch: `branch_id`, `candidate_id`, `patch_id`, content hashes, status (`branch_registry.rs:64`) | `parent_node_id`, parent generation, generator runtime, and active parent identity are absent. Existing source nodes are matched only by `source_state_id` and `target_relpath` (`branch_registry.rs:255`), so parent provenance can be overwritten by later calls. |
| Generation inference | `prototype1_source_generation` (`cli_facing.rs:3321`) | Producer staging at `cli_facing.rs:597` and legacy child eval at `prototype1_process.rs:1520` | Determine child generation from source ancestry | No file; inferred from registry source-node parent branch chain | Unknown `parent_branch_id` returns `depth + 1` (`cli_facing.rs:3333`). Root parent branches are unknown to the branch registry, so first children become generation 2 after caller adds 1. |
| Scheduler treatment node in `scheduler.json`, `node.json`, `runner-request.json` | `register_treatment_evaluation_node` (`scheduler.rs:688`) | Candidate resolver (`cli_facing.rs:2140`), C1/C2/C3 loads, child runner | Candidate child node to materialize/evaluate | Node includes `parent_node_id`, `generation`, `source_state_id`, optional graph provenance, `parent_branch_id`, `branch_id`, paths, status (`scheduler.rs:740`); request mirrors most fields and runner argv (`scheduler.rs:775`) | `parent_node_id` is reconstructed as `prototype1_node_id(parent_branch_id, generation - 1)` (`scheduler.rs:710`), not read from parent identity. With wrong generation, it creates plausible but nonexistent parents. |
| Candidate resolution by typed parent | `runnable_candidate_nodes` reads scheduler/registry (`cli_facing.rs:2140`) | `resolve_next_candidate_node_id` (`cli_facing.rs:2276`) | Find one runnable direct child of current parent | Filters by required generation, selected instance, parent node id, frontier/status, selected branch (`cli_facing.rs:2154`) | Consumer is semantically correct: it requires generation `parent + 1` and matching `parent_node_id`. It cannot recover from producer-invented generation/parent. |
| Materialized child worktree / active checkout state | `MaterializeBranch::transition` (`c1.rs:516`) via `GitWorktreeBackend::realize` (`backend.rs:694`) | `BuildChild::load`/transition (`c2.rs:237`, `:331`), child runner | Temporary child artifact surface for one node | Node/request `workspace_root` updated (`c1.rs:590`, `scheduler.rs:562`); journal records materialize before/after (`c1.rs:539`, `:637`) | Worktree path is a handle, not identity. Durable artifact identity is partial: target content commit may be later recorded, but whole-artifact identity is still mostly git branch/commit convention. |
| Child binary build | `BuildChild::transition` writes binary and node status (`c2.rs:331`, `:515`) | `SpawnChild::transition` loads request and binary (`c3.rs:507`) | Hydrate child runtime from candidate artifact | Node status `BinaryBuilt`; binary at `nodes/<node>/bin/ploke-eval`; build journal before/after (`c2.rs:367`, `:550`) | Build result does not carry parent identity or generator runtime; relies on node identity already being correct. |
| Child artifact commit and child parent identity | `persist_prototype1_buildable_child_artifact` (`prototype1_process.rs:644`) | Successor install/validation, later parent start | Make child artifact recoverable and parent-capable | Commits target and `.ploke/prototype1/parent_identity.json` in child worktree; journal `ChildArtifactCommitted` includes previous parent and child identity (`prototype1_process.rs:667`, `:694`) | This is the strongest provenance record, but it occurs after candidate resolution/build. It cannot fix bad node generation produced before materialization. |
| Child invocation JSON and argv | `write_child_invocation` from C3 (`c3.rs:530`) or legacy helper (`prototype1_process.rs:732`) | `execute_prototype1_runner_invocation` (`prototype1_process.rs:1376`) | Attempt-scoped bootstrap for one leaf evaluator | Invocation fields: `role=child`, `campaign_id`, `node_id`, `runtime_id`, `journal_path`; argv is `loop prototype1-runner --invocation ... --execute --format json` (`invocation.rs:37`, `:90`) | Good role split. It deliberately does not duplicate node/request fields, so wrong node provenance remains wrong. |
| Child ready/completion journal and results | Child writes ready and result through `record_prototype1_child_ready` and `record_attempt_runner_result` (`prototype1_process.rs:226`, `:1047`) | Parent `ObserveChild` waits for `Child<ResultWritten>` and reads `results/<runtime>.json` (`c4.rs:226`, `:294`) | Child runtime acknowledged, evaluated, and wrote terminal result | Ready carries runtime, generation, refs, paths, pid (`journal.rs:131`); result carries campaign/node/generation/branch/status/disposition/eval path (`scheduler.rs:109`) and is written both attempt-scoped and as latest node result (`prototype1_process.rs:1054`) | Result records do not include parent identity, parent node id, parent branch id, evaluator/policy identity beyond evaluation artifact path. |
| Continuation decision | `decide_node_successor_continuation` and `record_continuation_decision` (`scheduler.rs:663`, `:676`) | Successor validation (`prototype1_process.rs:376`) | Scheduler says selected child is allowed to continue | `last_continuation_decision`: disposition, selected branch id, next generation, total node count (`scheduler.rs:47`) | Mutable singleton, not attempt-scoped selection authority. It contains branch id, not selected node id, parent identity, or runtime id. |
| Successor artifact install into active checkout | `prepare_prototype1_active_successor_runtime` and `install_prototype1_successor_artifact` (`prototype1_process.rs:453`, `:466`) | Successor process parent start (`cli_facing.rs:2564`) | Stable active checkout advances to selected child artifact before successor runs | Selects branch, verifies target content, switches active checkout, validates new parent identity, appends successor checkout and `ActiveCheckoutAdvanced` entries (`prototype1_process.rs:459`, `:526`, `:538`, `:544`, `:565`) | Correct intended order. It relies on child identity having been committed on the selected branch. |
| Successor invocation JSON, argv, ready and completion docs | `spawn_and_handoff_prototype1_successor` writes invocation, stream paths, ready wait (`prototype1_process.rs:851`); successor writes ready/completion (`prototype1_process.rs:306`, `:333`) | Successor `acknowledge_prototype1_state_handoff` (`cli_facing.rs:2406`); predecessor waits on ready path (`prototype1_process.rs:818`) | Next parent acknowledges handoff then runs one typed turn | Invocation: `role=successor`, `campaign_id`, `node_id`, `runtime_id`, `journal_path`, `active_parent_root`; argv is `loop prototype1-state --campaign ... --repo-root ... --handoff-invocation ...` (`invocation.rs:49`); ready/completion include node/runtime/pid/status (`invocation.rs:348`) | Handoff validation checks campaign, node id, active root, and continuation decision (`cli_facing.rs:2429`, `:2466`), but does not bind to an immutable selection record. |

## Run10 Failure Path

The active parent was generation 0 with node
`node-f7b9c198a79ca49a` and branch
`prototype1-parent-p1-3gen-15nodes-run10-gen0`.

On startup, the consumer computed `required_generation =
parent_identity.generation + 1` (`cli_facing.rs:2283`) and required
`parent_node_id == parent_identity.node_id` (`cli_facing.rs:2306`).

The refill producer passed only `source_branch_id =
parent_identity.branch_id` into target selection (`cli_facing.rs:318`) and then
stored that as `parent_branch_id` in the branch registry (`cli_facing.rs:500`,
`branch_registry.rs:261`).

During staging, the producer computed:

```text
generation = prototype1_source_generation(registry, source_node) + 1
```

(`cli_facing.rs:597`). Because the root parent artifact branch is not a branch
inside `branches.json`, `prototype1_source_generation` returned `depth + 1`
for an unknown parent branch (`cli_facing.rs:3333`). The caller added another
one, so first children were persisted as generation 2.

Then `register_treatment_evaluation_node` derived:

```text
parent_node_id = prototype1_node_id(parent_branch_id, generation - 1)
```

(`scheduler.rs:710`). With `generation=2`, that yielded synthetic
`node-c47370c1ae880bf4` instead of the real gen0 parent node
`node-f7b9c198a79ca49a`.

The typed parent consumer therefore correctly found no runnable generation 1
children of the active parent.

## Top 5 Blockers To A 3-Generation Run

1. Producer-side generation and parent reconstruction is wrong for root-parent
   branches. The producer must resolve `parent_branch_id` through scheduler or
   explicit parent identity before registering children.

2. Candidate records do not carry explicit parent identity. `branches.json`
   stores `parent_branch_id`, but not `parent_node_id`, parent generation, or
   previous `ParentIdentity`; scheduler reconstructs provenance after the fact.

3. Continuation authority is a mutable scheduler singleton. `last_continuation_decision`
   can validate a successor by selected branch id, but it is not an immutable
   attempt-scoped `SuccessorSelection` binding selected node, branch, parent,
   runtime, and policy.

4. First-class runtime provenance is incomplete. Child and successor invocations
   carry `runtime_id`, but synthesis/branch registration usually lacks generator
   runtime identity, so generation and operation provenance are reconstructed
   from branch handles.

5. Observability does not record typed stopped/blocked states before child
   execution. A parent can start, refill target selection, and fail candidate
   resolution with only `ParentStarted` plus scheduler/registry side effects,
   forcing provenance reconstruction across several files.
