# Child Entry Surface Admission Audit

Date: 2026-04-30

Focus: whether child process entry independently recomputes/checks surface admission before it marks itself running/ready or begins evaluation, versus relying on parent-side validation.

## Summary

Current child process entry does not independently recompute or check the partitioned surface before it marks itself `Running`, records `Child<Ready>`, transitions to `Child<Evaluating>`, or starts branch evaluation. The live parent/controller paths do compute the child surface before build/spawn and again after child artifact persistence, but that check is a caller-side precondition, not a child-entry barrier.

This is enough for the normal loop to proceed if the parent-side path is the only way a child is launched and the child workspace/binary are not changed between parent validation and child start. It is not enough to claim that a child runtime independently admits its own current Artifact before evaluation.

## Documented Implemented Invariants

- `prototype1_state::mod` says successor handoff checks the current clean Artifact tree against the sealed History head before the next runtime enters the parent path, and that successor handoff computes a partitioned `SurfaceCommitment` committed into the sealed block. It further claims child evaluation validates this surface before hydrating a child runtime and again after the child Artifact is persisted (`crates/ploke-eval/src/cli/prototype1_state/mod.rs:97`, `crates/ploke-eval/src/cli/prototype1_state/mod.rs:101`, `crates/ploke-eval/src/cli/prototype1_state/mod.rs:106`).
- The parent/controller code does perform a pre-build surface check in the typed state command path before `BuildChild` (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3301`) and performs the post-persist check through `persist_prototype1_buildable_child_artifact` (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3331`, `crates/ploke-eval/src/cli/prototype1_process.rs:798`).
- The older `run_prototype1_branch_evaluation_via_child` parent path similarly validates before build and after persistence (`crates/ploke-eval/src/cli/prototype1_process.rs:1839`, `crates/ploke-eval/src/cli/prototype1_process.rs:1855`).
- The backend-level surface computation treats `crates/ploke-eval` as immutable and rejects an immutable-surface change (`crates/ploke-eval/src/cli/prototype1_state/backend.rs:446`, `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1217`, `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1238`). It hashes tool-description files as the mutated partition and an empty ambient partition (`crates/ploke-eval/src/cli/prototype1_state/backend.rs:1245`, `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1339`).
- Successor entry has a stronger implemented barrier than child entry: `acknowledge_prototype1_state_handoff` validates continuation and History admission before writing successor ready and before entering the typed parent run (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3118`, `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3120`). That admission recomputes the current clean tree and surface from the active checkout and verifies them against the sealed head (`crates/ploke-eval/src/cli/prototype1_process.rs:397`, `crates/ploke-eval/src/cli/prototype1_process.rs:419`, `crates/ploke-eval/src/cli/prototype1_process.rs:427`).

## Documented Aspirational Or Deferred Invariants

- `history.rs` says startup admission should become an explicit procedure: `ProducedBy(SelfRuntime, CurrentArtifact)` followed by `AdmittedBy(CurrentArtifact, Lineage, Policy, History)` before entry to the ruling parent path (`crates/ploke-eval/src/cli/prototype1_state/history.rs:65`).
- The same docs explicitly narrow the current implementation: bootstrap admission, full authenticated state roots, policy-surface digest checks, and process uniqueness are partial or absent (`crates/ploke-eval/src/cli/prototype1_state/history.rs:19`).
- `history.rs` also says the current implementation does not yet enforce a uniform bootstrap/predecessor admission carrier or structural type-state representation of the child/successor surface gate (`crates/ploke-eval/src/cli/prototype1_state/history.rs:242`).
- Surface semantics are documented as a static reconstruction witness that should be computed by checking out Artifacts and hashing declared surfaces, not by executing the candidate runtime (`crates/ploke-eval/src/cli/prototype1_state/history.rs:1184`). That is consistent with parent-side pre-execution validation, but not with a claim that the child recomputes its own admission at entry.

## Ambiguous Or Conflicting Claims

- `mod.rs` says child evaluation validates the surface before hydrating a child runtime and after persistence (`crates/ploke-eval/src/cli/prototype1_state/mod.rs:106`). In actual code, this is true only in the parent/controller launch paths, not inside child process entry.
- `history.rs` still says child execution relies on the older bounded-target validation path (`crates/ploke-eval/src/cli/prototype1_state/history.rs:201`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:215`). That statement is stale if read as "no child surface check exists", because the parent now calls `validate_child_surface`. It is accurate if read as "child entry has no structural History/surface admission carrier."
- `validate_child_surface` computes a `SurfaceCommitment` and rejects immutable-surface drift, but it does not compare the child checkout to a sealed History head. Calling this "admission" is therefore ambiguous for children: it is a parent-side surface preservation check, not History admission (`crates/ploke-eval/src/cli/prototype1_process.rs:435`).

## Actual Child Entry Behavior

The invocation-based child entry loads the executable child invocation, node, and runner request, then marks the node `Running` before any surface recomputation (`crates/ploke-eval/src/cli/prototype1_process.rs:1672`, `crates/ploke-eval/src/cli/prototype1_process.rs:1687`, `crates/ploke-eval/src/cli/prototype1_process.rs:1700`). It then records `Child<Ready>`, transitions to `Child<Evaluating>`, and starts branch evaluation (`crates/ploke-eval/src/cli/prototype1_process.rs:1706`, `crates/ploke-eval/src/cli/prototype1_process.rs:1714`, `crates/ploke-eval/src/cli/prototype1_process.rs:1720`).

The legacy campaign/node child entry behaves the same way: it loads node/request, marks the node `Running`, optionally records child ready, then begins evaluation (`crates/ploke-eval/src/cli/prototype1_process.rs:1612`, `crates/ploke-eval/src/cli/prototype1_process.rs:1623`, `crates/ploke-eval/src/cli/prototype1_process.rs:1629`, `crates/ploke-eval/src/cli/prototype1_process.rs:1631`).

Neither child entry path calls `validate_child_surface`, `WorkspaceBackend::surface_commitment`, `validate_prototype1_successor_history_admission`, or any sealed-head verifier. The child invocation schema also lacks `active_parent_root`, a stored `SurfaceCommitment`, or an expected immutable root; `Invocation::child` sets `active_parent_root: None` (`crates/ploke-eval/src/cli/prototype1_state/invocation.rs:128`, `crates/ploke-eval/src/cli/prototype1_state/invocation.rs:143`). `load_executable` only deserializes and classifies the invocation by role (`crates/ploke-eval/src/cli/prototype1_state/invocation.rs:351`, `crates/ploke-eval/src/cli/prototype1_state/invocation.rs:368`).

## Code Barriers Versus Caller Discipline

There are useful structural barriers, but not at the child surface gate.

- `SurfaceRoots` has private fields and a private constructor, so sibling modules cannot fabricate backend roots directly (`crates/ploke-eval/src/cli/prototype1_state/backend.rs:30`, `crates/ploke-eval/src/cli/prototype1_state/backend.rs:45`).
- `SurfaceCommitment` has partition-typed fields and a private constructor, and public creation is routed through backend roots (`crates/ploke-eval/src/cli/prototype1_state/history.rs:1206`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1220`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1233`).
- `Child<Starting> -> Child<Ready> -> Child<Evaluating> -> Child<ResultWritten>` is typed and records journal projections through transition methods (`crates/ploke-eval/src/cli/prototype1_state/child.rs:111`, `crates/ploke-eval/src/cli/prototype1_state/child.rs:145`, `crates/ploke-eval/src/cli/prototype1_state/child.rs:153`, `crates/ploke-eval/src/cli/prototype1_state/child.rs:161`).
- That child typestate does not carry a checked surface or admission witness. `Child::new` takes journal/runtime/generation/refs/paths/pid data and can transition to ready without any surface parameter (`crates/ploke-eval/src/cli/prototype1_state/child.rs:123`, `crates/ploke-eval/src/cli/prototype1_state/child.rs:144`).
- Parent startup has a local `Parent<Unchecked>::check` barrier, but it validates checkout identity/scheduler facts, not a child-entry surface admission witness (`crates/ploke-eval/src/cli/prototype1_state/parent.rs:334`, `crates/ploke-eval/src/cli/prototype1_state/parent.rs:347`).
- Child surface validation is currently a function boundary plus caller discipline. It is called by parent/controller paths before spawning the child, but the child binary does not recheck the condition after it starts (`crates/ploke-eval/src/cli/prototype1_process.rs:847`, `crates/ploke-eval/src/cli/prototype1_process.rs:1884`).

## Loop Completion Expectation

I expect the full loop structure to complete under the current implementation when candidates stay within the current ordinary surface policy: the parent validates that `crates/ploke-eval` is unchanged before child build/spawn, persists the child Artifact, observes the result, installs the selected Artifact, seals a History block with a surface commitment, launches the successor, and the successor validates current tree/surface against the sealed head before entering the parent path (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3301`, `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3465`, `crates/ploke-eval/src/cli/prototype1_process.rs:974`, `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3118`).

The missing child-entry surface check is therefore not an expected normal-loop blocker. It is an invariant gap: a directly invoked child, or a child whose workspace/binary is changed after parent validation, can mark itself running/ready and begin evaluation without independently recomputing the surface. The loop can complete while still failing the stronger claim that every runtime entry locally establishes surface admission before observable readiness or evaluation.

## Audit Finding

Finding: child entry relies on parent-side surface validation. It does not independently recompute/check surface admission before `Running`, `Child<Ready>`, `Child<Evaluating>`, or evaluation.

Severity: correctness gap for documented cross-runtime admission claims; not necessarily an operational blocker for the current happy-path loop.

Needed shape: if the intended invariant is independent child-entry admission, the child invocation should carry or derive the required parent surface context, and the transition to `Child<Ready>` should require a structural checked-surface carrier. If the intended invariant remains parent-side static reconstruction only, the docs should explicitly say the child does not self-admit and that child surface preservation is enforced by the parent launch path.
