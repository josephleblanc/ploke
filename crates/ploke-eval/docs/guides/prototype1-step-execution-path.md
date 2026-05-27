# `prototype1-step` Execution Path

This guide is a source-grounded map for:

```sh
ploke-eval loop prototype1-step --repo-root <active-parent-checkout>
```

It intentionally starts at the CLI command boundary and follows the live code
one step at a time. Keep this document narrow: it should describe the current
execution path, not the whole Prototype 1 architecture.

## Prerequisites

`prototype1-step` does not bootstrap a Prototype 1 run. It controls an already
prepared parent checkout. In the normal path, that means `loop prototype1-setup`
has already completed successfully from the worktree that will become the
parent checkout.

At minimum, the active checkout and campaign must already have:

- a committed parent identity at `.ploke/prototype1/parent_identity.json`;
- a campaign manifest at the campaign path named by that parent identity;
- a campaign config that resolves its dataset sources, model route, instance
  root, batch root, required procedures, and eval/protocol policies;
- an admitted Prototype 1 run profile at `prototype1/run-profile.toml` beside
  the campaign manifest;
- a matching `prototype1/run-profile.commitment.json` digest for that admitted
  profile;
- a generation-0 parent node registered for the campaign and active checkout.

The command must be run with `--repo-root` pointing at that active parent
checkout, or from that checkout as the current directory. A child worktree is
not enough, because child worktrees do not carry parent control state.

The baseline eval, baseline protocol, child planning, child execution,
selection, and handoff phases do not all need to be complete before
`prototype1-step` runs. Those are the phases `prototype1-step` diagnoses and
advances. Provider credentials, prompt files, or protocol readiness may still
block a later phase, but they are not substitutes for parent checkout setup.

## Scope

`prototype1-step` is the bounded controller path for an active Prototype 1
parent checkout. It diagnoses the current parent phase, advances at most one
phase, then diagnoses again and prints the resulting status.

This is distinct from:

- `prototype1-continue`, which loops through phases until terminal or blocked.
- `prototype1-state`, which drives a direct typed parent turn.
- `prototype1`, the older wrapper/controller surface.

## CLI Dispatch

The binary entrypoint is `crates/ploke-eval/src/main.rs`.

1. `main` calls `Cli::try_parse()`.
2. On parse success, tracing is initialized from `cli.debug_tools`.
3. `main` calls `cli.run().await`.

The top-level command enum contains `Command::Loop(LoopCommand)` in
`crates/ploke-eval/src/cli.rs`.

`Cli::run` dispatches `Command::Loop(cmd)` to `cmd.run().await`. Any returned
`PrepareError` is printed to stderr and becomes `ExitCode::FAILURE`.

Inside `LoopCommand::run`, the relevant arm is:

```rust
LoopSubcommand::Prototype1Step(cmd) => prototype1_state::run::step(cmd).await
```

The parsed command payload is `Prototype1ControlCommand`:

```rust
pub struct Prototype1ControlCommand {
    pub repo_root: Option<PathBuf>,
    pub format: InspectOutputFormat,
}
```

So `prototype1-step` has only two CLI inputs of its own:

- `--repo-root`: the active parent checkout root, defaulting to current
  directory inside the control layer.
- `--format`: table or JSON status output.

## Step Controller

The live controller entrypoint is:

```rust
pub(crate) async fn step(command: Prototype1ControlCommand) -> Result<(), PrepareError>
```

in `crates/ploke-eval/src/cli/prototype1_state/run/core.rs`.

Its control shape is:

1. Resolve the active runtime context from `command.repo_root`.
2. Diagnose the active parent state.
3. If diagnosis is `Blocked`, return an error and do not advance.
4. If diagnosis is not `Complete`, advance exactly one diagnosed phase in
   `ExecuteMode::Step`.
5. Diagnose again through `diagnose_command`.
6. Render the post-step status in the requested format.

That last re-diagnosis matters. The command reports the state after the phase
advance, not the state that was originally diagnosed.

Current source shape:

```rust
let diagnosis = diagnose(&resolve_context(command.repo_root.as_deref())?)?;
```

This line does two separate things:

- `resolve_context` turns an optional checkout path into the runtime facts that
  all later phases need.
- `diagnose` reads those facts plus persisted artifacts and chooses the next
  admissible phase.

If the diagnosis is blocked, `step` returns a `PrepareError` with the collected
blockers. It does not call `advance`.

If the diagnosis is complete, `step` also does not call `advance`; it only
prints the current terminal status.

All other phases go through:

```rust
advance(diagnosis, ExecuteMode::Step).await?;
```

`ExecuteMode::Step` is the local one-phase contract. It matters later for child
execution phases because it caps child-phase fanout to one matching child even
when the admitted profile allows more concurrency.

## Runtime Context Resolution

`step` begins by calling:

```rust
diagnose(&resolve_context(command.repo_root.as_deref())?)
```

`resolve_context` is the first authority boundary for this command. It builds a
`RuntimeContext` containing:

- `repo_root`: `--repo-root` or the current directory.
- `parent_identity`: loaded from the active checkout.
- `campaign_id`: copied from `parent_identity`.
- `manifest_path`: resolved from the campaign id.
- `resolved_campaign`: loaded campaign configuration.
- `admitted_profile`: loaded from `prototype1/run-profile.toml` beside the
  campaign manifest.
- `effective_control`: derived execution control from the admitted profile.

`campaign_id` is identity and location. It binds the active parent checkout to
one durable campaign tree and is used to find the campaign manifest and related
campaign artifacts.

`resolved_campaign` is the campaign manifest config after defaults and normal
resolution. It describes the eval campaign substrate: benchmark family, dataset
sources, model route, required procedures, instance and batch roots, eval
policy, protocol policy, and framework config.

`admitted_profile` is Prototype 1 loop policy for that campaign. It is the
setup-admitted run profile, checked by digest commitment, that controls search
limits, child fanout, generation source, protocol settings, model defaults, and
successor selection.

If the active checkout has no parent identity, the command fails early. The
error text is explicit that child worktree cwd diagnosis is not supported in v1.
This is important for self-edit loop operation: `prototype1-step` is a parent
checkout controller, not a generic campaign artifact scanner.

The current function proceeds in this order:

1. Resolve `repo_root`.
   If `--repo-root` is present, it is used directly. Otherwise the process
   current directory is used. This is only a path default; it is not authority.
2. Load `parent_identity` from the checkout.
   This is the bridge from filesystem checkout to Prototype 1 lineage. Without
   it, the command refuses to continue.
    - setup normally creates it from the registered root node; the explicit init path creates it directly from bootstrap facts.
3. Copy `campaign_id` from `parent_identity`.
   The active artifact names its campaign; the caller does not pass a separate
   campaign id to `prototype1-step`.
4. Resolve `manifest_path` from `campaign_id`.
   This locates the campaign root under the normal ploke-eval campaign layout.
5. Load `resolved_campaign`.
   This is the campaign manifest config used by eval/protocol closure work.
6. Load the admitted run profile from the campaign.
   Missing `prototype1/run-profile.toml` is a hard error because the step
   controller should run within the setup-admitted search/control bounds.
7. Derive `effective_control`.
   This folds optional control knobs into concrete values and rejects control
   settings that would widen admitted fanout.

The result is a `RuntimeContext`, not a mutable scheduler view. That distinction
is deliberate: later phase diagnosis may read projections for status, but the
active parent identity and admitted profile come from the parent checkout and
campaign setup artifacts.

### Clarification: How is Parent Identity first created?

The normal first creation path is `loop prototype1-setup`.

In [prepare_prototype1_parent_setup](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:203), setup does this:

1. Prepares/loads the campaign and admits the run profile.
2. Chooses the primary instance.
3. Builds the gen0 parent artifact branch name:
   `prototype1-parent-<campaign>-gen0`.
4. Calls `register_root_parent_node(...)` to create the generation-0 parent node.
5. Checks out a fresh parent branch.
6. Creates the identity with:

```rust
ParentIdentity::from_node(campaign_id, &node, None, Some(artifact_branch))
```

7. Writes it to `.ploke/prototype1/parent_identity.json`.
8. Commits that file into the active checkout.
9. Validates the checkout against the identity.

There is also a manual bootstrap path: `loop prototype1-state --init-parent-identity`. That goes through [initialize_prototype1_parent_identity](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6182), requires `--identity-branch` and `--identity-instance`, checks the deterministic gen0 node id, then calls:

```rust
ParentIdentity::root_bootstrap(...)
```

So: setup normally creates it from the registered root node; the explicit init path creates it directly from bootstrap facts. `resolve_context` only reads it later.

### Concrete Example of a Branch Name: `prototype1-parent-<campaign>-gen0`

```text
prototype1-parent-p1-gemini35-flash-direct-15g2x3-20260525-035000-gen0
```

It appears in this actual parent identity file:

[.ploke/prototype1/parent_identity.json](</home/brasides/.ploke-eval/worktrees/p1-gemini35-flash-direct-15g2x3-20260525-035000/.ploke/prototype1/parent_identity.json:1>)

Relevant fields from that file:

```json
{
  "campaign_id": "p1-gemini35-flash-direct-15g2x3-20260525-035000",
  "generation": 0,
  "branch_id": "prototype1-parent-p1-gemini35-flash-direct-15g2x3-20260525-035000-gen0",
  "artifact_branch": "prototype1-parent-p1-gemini35-flash-direct-15g2x3-20260525-035000-gen0"
}
```

This is the worktree-local file that parent-control commands would read for that checkout.

## First Diagnostic Boundary

After context resolution, `diagnose` decides which `DiagnosedPhase` applies:

```rust
BaselineEval
BaselineProtocol
ChildPlan
Materialize
Build
Spawn
Observe
Select
Handoff
Complete
Blocked
```

At this level, the important rule is that `prototype1-step` does not accept a
phase argument. The phase is derived from persisted parent state, closure state,
child-plan presence, child node statuses, and successor journal markers.

The command then advances exactly one phase by calling:

```rust
advance(diagnosis, ExecuteMode::Step).await
```

The phase dispatch is:

```rust
BaselineEval      => advance_baseline_eval(...)
BaselineProtocol  => advance_baseline_protocol(...)
ChildPlan         => advance_child_plan(...)
Materialize       => advance_child_phase(..., Step)
Build             => advance_child_phase(..., Step)
Spawn             => advance_child_phase(..., Step)
Observe           => advance_child_phase(..., Step)
Select            => advance_select(...)
Handoff           => advance_handoff(...)
Complete/Blocked  => no-op
```

For child phases, `ExecuteMode::Step` caps execution to one matching child even
if the admitted profile allows wider fanout. That cap is local to later child
phases; broad patch generation during `ChildPlan` has its own profile-derived
parallelism.

## Source Anchors

- `crates/ploke-eval/src/main.rs`: binary entrypoint.
- `crates/ploke-eval/src/cli.rs`: `Cli`, `Command::Loop`,
  `LoopSubcommand::Prototype1Step`, and `Prototype1ControlCommand`.
- `crates/ploke-eval/src/cli/prototype1_state/run/core.rs`: `step`,
  `resolve_context`, `diagnose`, `advance`, and phase-specific advance
  functions.
- `crates/ploke-eval/src/cli/prototype1_state/mod.rs`: architectural contract
  for Prototype 1 authority, History, child-plan messages, and persisted state.

## Next Slice

The next useful walkthrough section is `diagnose`: how it decides between
baseline, child-plan, child execution, selection, handoff, complete, and blocked
states.
