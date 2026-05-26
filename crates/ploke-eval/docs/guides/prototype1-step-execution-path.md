# `prototype1-step` Execution Path

This guide is a source-grounded map for:

```sh
ploke-eval loop prototype1-step --repo-root <active-parent-checkout>
```

It intentionally starts at the CLI command boundary and follows the live code
one step at a time. Keep this document narrow: it should describe the current
execution path, not the whole Prototype 1 architecture.

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

If the active checkout has no parent identity, the command fails early. The
error text is explicit that child worktree cwd diagnosis is not supported in v1.
This is important for self-edit loop operation: `prototype1-step` is a parent
checkout controller, not a generic campaign artifact scanner.

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
