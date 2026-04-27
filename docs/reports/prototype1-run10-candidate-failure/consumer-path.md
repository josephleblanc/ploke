# Prototype 1 Run10 Candidate Failure: Consumer Path

## Summary

Campaign `p1-3gen-15nodes-run10` failed before `C1::load` ran. The typed
parent resolver loaded a generation-0 parent identity, computed that the next
child must be generation 1, and found zero runnable scheduler nodes matching
that generation and parent node id.

The scheduler data explains the empty result:

- Parent identity recorded in `transition-journal.jsonl`:
  `node_id=node-f7b9c198a79ca49a`,
  `generation=0`,
  `branch_id=prototype1-parent-p1-3gen-15nodes-run10-gen0`.
- `prototype1/scheduler.json` contains no generation-1 nodes.
- The three staged treatment nodes are generation 2:
  `node-a3b258a61220d9eb`, `node-d39773b381f66e41`,
  `node-ea09000c0ee5151c`.
- Those generation-2 nodes all have
  `parent_node_id=node-c47370c1ae880bf4`, not the active parent
  `node-f7b9c198a79ca49a`.

So the consumer-side failure is correct under the current predicates, but the
error message hides the decisive facts: zero generation-1 nodes existed, while
three frontier nodes existed at generation 2 under a computed missing parent.

## Consumer Control Path

The live command path is:

1. `Prototype1StateCommand::run_turn` resolves the parent identity, checks the
   parent role, appends `ParentStarted`, then asks for the next child candidate
   before loading `C1` (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:2564`,
   `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:2609`,
   `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:2625`).
2. `resolve_next_candidate_node_id` computes
   `required_generation = parent_identity.generation + 1`
   (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:2283`).
3. It calls `runnable_candidate_nodes` with that generation and
   `parent_identity.node_id` as the required parent
   (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:2306`).
4. If no candidate exists, it runs target selection once, then resolves again
   with the same generation and parent predicates
   (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:2312`,
   `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:2317`).
5. Only after a node id is resolved does `C1::load` load the node, resolve its
   treatment branch, read the target file, and verify the live source content
   equals the branch source content
   (`crates/ploke-eval/src/cli/prototype1_state/c1.rs:390`,
   `crates/ploke-eval/src/cli/prototype1_state/c1.rs:396`,
   `crates/ploke-eval/src/cli/prototype1_state/c1.rs:404`,
   `crates/ploke-eval/src/cli/prototype1_state/c1.rs:414`).

Because node-id inference failed, run10 never reached the `C1::load` predicates.

## Runnable Candidate Predicates

`runnable_candidate_nodes` reads `scheduler.json` and applies these predicates
in order:

1. Required generation, if supplied:
   `node.generation == required_generation`
   (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:2157`).
2. Active selected instance, if the active selection belongs to this campaign:
   `node.instance_id == selected_instance`
   (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:2115`,
   `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:2162`).
3. Required parent node, if supplied:
   `node.parent_node_id.as_deref() == Some(parent_identity.node_id.as_str())`
   (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:2168`).
4. Runnable status/frontier:
   if `frontier_node_ids` is empty, status must not be `succeeded` or `failed`;
   otherwise the node id must be in `frontier_node_ids`
   (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:2173`).
5. Selected branch narrowing:
   collect branch-registry `selected_branch_id`s for the selected instance, if
   any, and narrow to candidates whose `node.branch_id` is selected
   (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:2185`,
   `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:2197`).

Important detail: selected-branch narrowing is non-fatal. If selected branch ids
exist but none match the already-filtered candidates, the function leaves the
candidate set unchanged (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:2203`).
In run10, the zero result happens before this can help: the generation and
parent filters eliminate every scheduler node.

When the final candidate list is empty, the current diagnostic reports only
purpose, generation, optional selected instance, and scheduler path
(`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:2236`). It does not
report counts by generation, parent, frontier membership, or selected branch.

## Parent Identity And Branch Provenance

Parent identity is artifact-carried. The identity record stores `parent_id`,
`node_id`, `generation`, `parent_node_id`, and `branch_id`
(`crates/ploke-eval/src/cli/prototype1_state/identity.rs:24`). It is constructed
from the scheduler node mirror, with `parent_id == node.node_id`,
`node_id == node.node_id`, `generation == node.generation`, and
`branch_id == node.branch_id`
(`crates/ploke-eval/src/cli/prototype1_state/identity.rs:48`).

The parent role loader then loads the scheduler node named by that identity and
checks:

- the active checkout matches the identity through the backend
  (`crates/ploke-eval/src/cli/prototype1_state/parent.rs:65`);
- identity generation equals scheduler-node generation
  (`crates/ploke-eval/src/cli/prototype1_state/parent.rs:75`);
- identity branch id equals scheduler-node branch id
  (`crates/ploke-eval/src/cli/prototype1_state/parent.rs:78`);
- selected instance, when set, equals the parent node's instance
  (`crates/ploke-eval/src/cli/prototype1_state/parent.rs:81`).

For child candidates, parent-child provenance comes from
`Prototype1NodeRecord.parent_node_id`, `generation`, `parent_branch_id`, and
`branch_id` (`crates/ploke-eval/src/intervention/scheduler.rs:76`).
`register_treatment_evaluation_node` computes the child `node_id` from the
treatment branch id and generation, then computes `parent_node_id` from
`branch.parent_branch_id` and `generation - 1`
(`crates/ploke-eval/src/intervention/scheduler.rs:703`,
`crates/ploke-eval/src/intervention/scheduler.rs:710`).

Branch-registry provenance is separate. A source node stores
`source_state_id`, optional `parent_branch_id`, `instance_id`,
`target_relpath`, selected branch id, and branches
(`crates/ploke-eval/src/intervention/branch_registry.rs:64`). Recording
synthesized branches matches or creates a source node by only
`source_state_id` and `target_relpath`, then writes `parent_branch_id` when one
is supplied (`crates/ploke-eval/src/intervention/branch_registry.rs:255`,
`crates/ploke-eval/src/intervention/branch_registry.rs:279`).

The target-selection refill path passes the active parent branch as
`source_branch_id` (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:310`,
`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:318`) and then records
it as the selected target's `parent_branch_id`
(`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:534`).
When staging nodes, it computes:

```text
generation = prototype1_source_generation(registry, source_node) + 1
```

(`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:597`).

`prototype1_source_generation` treats a source node with a `parent_branch_id`
whose branch is not found in any registry source as depth `1`
(`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3330`,
`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3338`). In run10, the
source node's `parent_branch_id` is the root parent artifact branch
`prototype1-parent-p1-3gen-15nodes-run10-gen0`, not a treatment branch present
under any registry source. Therefore staging computed generation `2`, not
generation `1`, and `register_treatment_evaluation_node` computed the parent
node id from that parent branch at generation `1`, producing
`node-c47370c1ae880bf4` instead of the real generation-0 parent
`node-f7b9c198a79ca49a`.

## Run10 Predicate Outcome

For run10, after the refill:

- Scheduler nodes by generation: one generation-0 root parent, zero
  generation-1 nodes, three generation-2 treatment nodes.
- Required by the parent resolver:
  `generation=1`, `parent_node_id=node-f7b9c198a79ca49a`.
- Existing frontier nodes:
  `node-f7b9c198a79ca49a`, `node-a3b258a61220d9eb`,
  `node-d39773b381f66e41`, `node-ea09000c0ee5151c`.
- Selected branch registry:
  `selected_branch_id=branch-bd104387ee90926a`.
- Closest candidate by branch:
  `node-a3b258a61220d9eb`, but it is generation 2 and has
  `parent_node_id=node-c47370c1ae880bf4`.

The consumer asks for `Parent<Ready>(generation 0) -> Child(generation 1)`.
The scheduler projection only offers `Child(generation 2)` under a synthetic
missing parent. No generation-1 candidate can be inferred.

## Diagnostics That Should Exist

When zero candidates are found, the error should include a compact candidate
diagnostic block produced from the same scheduler and registry state:

- requested predicates: required generation, required parent node id, selected
  instance, selected branch ids, frontier mode;
- scheduler totals by generation and status;
- counts after each predicate: generation, selected instance, parent node,
  frontier/status, selected branch narrowing;
- near misses: nodes with matching generation but wrong parent, nodes with
  matching parent but wrong generation, and selected-branch nodes rejected by
  generation or parent;
- refill result, when `run_parent_target_selection` was attempted: number of
  staged nodes and their generation/parent ids;
- branch provenance for selected source nodes: `source_state_id`,
  `parent_branch_id`, selected branch id, computed source generation, and the
  parent node id that `register_treatment_evaluation_node` would derive.

For run10, that diagnostic would have made the core problem visible in one
screen: `required_generation=1`, `required_parent=node-f7b9...`, `generation1=0`,
`generation2=3`, and selected branch `branch-bd104...` staged as
`node-a3b258...` with `parent_node_id=node-c473...`.

## Minimal Fix Plan

1. Add a private candidate-diagnostic helper beside `runnable_candidate_nodes`.
   It should be a read-only projection over scheduler and branch registry state,
   not another mutable status path.
2. Use that helper only in the empty-candidate branch of
   `resolve_prototype1_candidate_node_id`, and include the diagnostic after the
   existing human-readable error prefix.
3. After `run_parent_target_selection` refills the scheduler, re-run the helper
   before returning the final empty-candidate error so the message shows what
   the refill staged.
4. Tighten the generation/provenance contract in the staging path: a root parent
   branch used as `parent_branch_id` must resolve to the active
   `ParentIdentity` node at generation 0, not to a synthetic generation-1 node.
   Practically, either pass the active `ParentIdentity` into staging or store an
   explicit parent node id on the branch/source provenance before registering
   treatment nodes.
5. Add regression coverage for the exact run10 shape: parent identity generation
   0, target-selection refill from the root parent branch, selected branch
   present, and no explicit `--node-id`. The assertion should prove that the
   selected treatment node is staged as generation 1 with
   `parent_node_id == active_parent.node_id`, or that the empty-candidate
   diagnostic reports the generation/parent mismatch.
