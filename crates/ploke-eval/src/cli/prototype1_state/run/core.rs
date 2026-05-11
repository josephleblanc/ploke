#[allow(
    dead_code,
    reason = "task-stack:prototype1-run-core-parent-turn-spine structural placeholder for run-loop refactor"
)]
pub(crate) fn run_parent_turn() {
    // Required shape:
    //   Selection<DirectChild> -> gate -> Continuation<Allowed> -> spawn
    //   Selection<DirectChild> -/-> spawn
    //
    // gate:
    //   Parent<Selectable>
    //   × AdmittedRunProfile
    //   × ArtifactInventory<Admitted>
    //   × Selection<DirectChild>
    //   × Spend<Unspent>
    //   -> Continuation<Allowed> + Continuation<Stopped>
    //
    // let:
    //   p   : Parent<Selectable>
    //   pol : AdmittedRunProfile
    //   inv : ArtifactInventory<Admitted>
    //   sel : Selection<DirectChild>
    //   a   = artifact(sel)
    //
    // gate(...) = Continuation<Allowed> =>
    //   a ∈ inv
    //   parent(a) = artifact(p)
    //   generation(a) = generation(artifact(p)) + 1
    //   surface(a) = policy_surface(pol)
    //   generation(a) ≤ pol.max_generations
    //   |inv| ≤ pol.max_total_nodes
    //
    // admit(inv, a, pol) -> ArtifactInventory<Admitted> =>
    //   cargo_check(a)
    //   surface(a) = policy_surface(pol)
    //   |inv| + 1 ≤ pol.max_total_nodes
    //
    // plan(p, pol) -> ChildPlan<Admitted> =>
    //   pol.children.min ≤ |children(plan)| ≤ pol.children.max
    //
    // persist(child) =>
    //   child ∈ children(plan(p, pol))
    //
    // select(...) -> Selection<DirectChild> =>
    //   artifact(sel) ∈ { artifact(child) | child ∈ children(plan(p, pol)) }
    //
    // handoff(Continuation<Allowed>) =>
    //   Parent<Selectable> -> Parent<Retired>
    //   Spend<Unspent> -> Spend<Spent>
    //
    // stop | failure | timeout | Continuation<Stopped> -/-> spawn
    // cleanup -/-> remove(ArtifactInventory<Admitted> | History)
}
