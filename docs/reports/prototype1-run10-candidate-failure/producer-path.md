# Prototype 1 run10 candidate producer failure

## Summary

Campaign `p1-3gen-15nodes-run10` did produce candidate branch records after baseline protocol closure and intervention synthesis, but it registered the child scheduler nodes at generation 2 instead of generation 1. The later parent turn correctly searched for runnable generation 1 children of the root parent and found none.

The failure is therefore producer-side generation registration, not missing synthesis output.

Observed failure:

```text
batch selection is invalid: could not infer --node-id for next child candidate: no runnable generation 1 Prototype 1 nodes were found in /home/brasides/.ploke-eval/campaigns/p1-3gen-15nodes-run10/prototype1/scheduler.json
```

## Artifact evidence

- `/home/brasides/.ploke-eval/campaigns/p1-3gen-15nodes-run10/prototype1-loop-trace.json:2` records `stage_reached: target_selection`, with `dry_run: true`.
- `/home/brasides/.ploke-eval/campaigns/p1-3gen-15nodes-run10/prototype1-loop-trace.json:31` through `:40` records the baseline instance as eval/protocol complete, and `:42` through `:89` records one selected target after issue detection and synthesis.
- `/home/brasides/.ploke-eval/campaigns/p1-3gen-15nodes-run10/prototype1/branches.json:5` through `:63` contains one source node with three synthesized/selected branches. The source node has:
  - `source_state_id: prototype1-parent-p1-3gen-15nodes-run10-gen0` at `:7`
  - `parent_branch_id: prototype1-parent-p1-3gen-15nodes-run10-gen0` at `:8`
  - selected branch `branch-bd104387ee90926a` at `:18` and `:21`
- `/home/brasides/.ploke-eval/campaigns/p1-3gen-15nodes-run10/prototype1/scheduler.json:19` through `:116` contains four nodes: the root generation 0 parent plus three planned generation 2 nodes. There are no generation 1 nodes.
- `/home/brasides/.ploke-eval/campaigns/p1-3gen-15nodes-run10/prototype1/scheduler.json:40` through `:62` shows the selected candidate node `node-a3b258a61220d9eb` as `generation: 2`, with `parent_branch_id: prototype1-parent-p1-3gen-15nodes-run10-gen0`.
- `/home/brasides/.ploke-eval/campaigns/p1-3gen-15nodes-run10/prototype1/scheduler.json:41` shows `parent_node_id: node-c47370c1ae880bf4`. That node does not appear in scheduler `nodes`; it is the derived id for the root parent branch at generation 1, not the real root node id at generation 0.

## Producer path

The typed parent turn requires the next child generation to be `parent_identity.generation + 1`. For the root parent in this run, that is generation 1. This requirement is explicit in `resolve_next_candidate_node_id`:

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:2276` through `:2284` computes `required_generation`.
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:2306` through `:2311` asks for runnable candidates with that generation and the active parent node id.
- If none exist, `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:2312` through `:2315` calls `run_parent_target_selection`.
- It then resolves again at `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:2317` through `:2324`.

`run_parent_target_selection` builds a controller input from the active parent identity:

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:294` through `:327`.
- It sets `stop_after: TargetSelection` and `dry_run: true` at `:310` through `:313`.
- It sets `source_branch_id: Some(parent_identity.branch_id.clone())` at `:317` through `:319`.

The controller then runs baseline eval/protocol closure and synthesis:

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:426` through `:439` runs `advance_eval_closure` and `advance_protocol_closure`.
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:471` through `:492` persists issue detection and intervention synthesis.
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:500` through `:507` records synthesized branches, passing `input.source_branch_id.as_deref()` as `parent_branch_id`.

`record_synthesized_branches` persists that parent branch into the branch registry:

- `crates/ploke-eval/src/intervention/branch_registry.rs:216` through `:223` accepts `parent_branch_id`.
- `crates/ploke-eval/src/intervention/branch_registry.rs:261` through `:271` writes it when creating a source node.
- `crates/ploke-eval/src/intervention/branch_registry.rs:279` through `:282` overwrites it on an existing source node when present.

The bad generation is created immediately after branch recording:

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:585` through `:618` stages scheduler nodes for each branch.
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:597` computes `generation = prototype1_source_generation(&registry, source_node) + 1`.
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:607` through `:614` passes that generation to `register_treatment_evaluation_node`.

The helper only walks branch-registry source nodes:

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3321` through `:3345`.
- If `parent_branch_id` is absent, it returns the current depth at `:3330` through `:3331`.
- If `parent_branch_id` is present but not found among registry branch ids, it returns `depth + 1` at `:3333` through `:3339`.

For run10, the source node's parent branch is the root parent artifact branch. That branch exists as the scheduler root node branch, not as a treatment branch inside the branch registry. The helper therefore returns `1`, and the caller adds one more, registering first children as generation 2.

`register_treatment_evaluation_node` then makes the wrong generation durable:

- `crates/ploke-eval/src/intervention/scheduler.rs:688` through `:704` takes the caller-supplied generation and derives the scheduler node id from branch id plus generation.
- `crates/ploke-eval/src/intervention/scheduler.rs:710` through `:717` derives `parent_node_id` from `parent_branch_id` and `generation - 1`.
- `crates/ploke-eval/src/intervention/scheduler.rs:740` through `:759` writes those values into `Prototype1NodeRecord`.
- `crates/ploke-eval/src/intervention/scheduler.rs:813` through `:828` persists the record into `scheduler.json` and adds it to `frontier_node_ids`.

This explains both symptoms in the artifact: no generation 1 candidates, and a non-existent `parent_node_id` derived for generation 1 of the root parent branch.

## Root cause

`prototype1_source_generation` treats an unknown `parent_branch_id` as one generation above the current source. That was unsafe once `run_parent_target_selection` began passing the root parent artifact branch as `parent_branch_id`.

The root parent branch is a valid parent branch, but it is represented by scheduler/parent identity state, not by a branch-registry treatment branch. The generation producer should not infer first-child generation solely from branch-registry ancestry.

## Minimal patch plan

1. Replace producer-side generation inference with scheduler-backed parent resolution.
   - Add a helper near `prototype1_source_generation`, for example `prototype1_child_generation(scheduler, source_node) -> Result<u32, PrepareError>`.
   - If `source_node.parent_branch_id` is `None`, return `1` for legacy/root synthesis.
   - If it is `Some(parent_branch_id)`, first find a scheduler node with `node.branch_id == parent_branch_id` and return `node.generation + 1`.
   - If no scheduler node matches, fall back to registry ancestry only when the parent branch is found in the registry. Do not silently add one for an unknown parent branch; return `InvalidBatchSelection` with the source/parent ids.

2. Use that helper at both producer registration sites.
   - `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:597` in the target-selection staging path.
   - `crates/ploke-eval/src/cli/prototype1_process.rs:1520` in `run_prototype1_branch_evaluation_via_child`.

3. Keep `register_treatment_evaluation_node`'s current parent id derivation initially.
   - Once the caller passes generation 1 for the root parent branch, `crates/ploke-eval/src/intervention/scheduler.rs:710` through `:717` derives the real root node id because it computes `prototype1_node_id(parent_branch_id, 0)`.
   - The same formula still works for later generations when the parent branch is a selected treatment branch with a scheduler node at the previous generation.

4. Add focused tests.
   - A run10-shaped test: scheduler contains root node `branch_id = prototype1-parent-...-gen0`, registry source has `parent_branch_id` equal to that root branch, and staged child nodes must be generation 1 with `parent_node_id` equal to the root scheduler node.
   - A successor-shaped test: scheduler contains a generation 1 selected branch node, a new source has `parent_branch_id` equal to that selected branch, and staged children must be generation 2.
   - A missing-parent test: unknown `parent_branch_id` should error instead of registering an invented generation.

This keeps the producer reliable without changing the branch registry into the authority for parent identity. The scheduler/parent node remains the durable source for generation and parent-node identity, while the branch registry continues to describe synthesized treatment branches and their lineage.
