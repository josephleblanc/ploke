# Prototype 1 Execution Paths

This compares the current execution paths for:

- `ploke-eval loop prototype1`
- `ploke-eval loop prototype1-continue`
- `ploke-eval loop prototype1-state`

The comparison is source-facing. It describes the command wiring and control
flow in the current checkout, not the long-term architecture by itself.

Editable Excalidraw map: [`prototype1-state-execution-path.excalidraw`](./prototype1-state-execution-path.excalidraw).

## Short Version

| Command | Main role | Entry point | State source | What it advances |
| --- | --- | --- | --- | --- |
| `prototype1` | Legacy loop wrapper for campaign/baseline/synthesis stages | `Prototype1LoopCommand::run` | CLI args, prepared batch, campaign manifest, optional run profile | Baseline eval, baseline protocol, issue detection, intervention synthesis, optional intervention apply |
| `prototype1-continue` | Resumable operator control loop over the active parent checkout | `prototype1_state::run::resume` | `.ploke/prototype1/parent_identity.json`, admitted run profile, campaign artifacts, transition journal | Repeated diagnosed phases until complete, blocked, or guard limit |
| `prototype1-state` | Direct typed parent runtime turn | `Prototype1StateCommand::run_turn` | Explicit command args, parent identity or successor handoff invocation, admitted profile when present | One parent turn: baseline check, child plan, child fanout, observe, selection, possible successor handoff |

The practical split is:

```text
prototype1
  older campaign/baseline/synthesis wrapper

prototype1-continue
  restartable outer controller for the active parent checkout

prototype1-state
  direct typed parent-turn runner and successor entrypoint
```

`prototype1-step` is not one of the three commands compared here, but it shares
the same diagnosis and advance machinery as `prototype1-continue`. The
difference is execution mode: `step` advances one diagnosed phase with child
cap 1, while `continue` loops and uses the admitted effective parallel cap for
child phases.

## Shared CLI Dispatch

All three commands enter through `LoopSubcommand` in
[`cli.rs`](../../../crates/ploke-eval/src/cli.rs):

```text
LoopSubcommand::Prototype1(cmd)
  -> cmd.run()

LoopSubcommand::Prototype1Continue(cmd)
  -> prototype1_state::run::resume(cmd)

LoopSubcommand::Prototype1State(cmd)
  -> cmd.run()
```

That shared dispatch hides a real architectural split. `prototype1` is wired to
the older loop controller in
[`cli_facing.rs`](../../../crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs).
`prototype1-continue` is wired to the resumable controller in
[`run/core.rs`](../../../crates/ploke-eval/src/cli/prototype1_state/run/core.rs).
`prototype1-state` is wired back through `cli_facing.rs`, but into the typed
parent-turn path rather than the older wrapper.

## `prototype1`

`prototype1` runs `Prototype1LoopCommand::run`, which builds a
`Prototype1LoopControllerInput` and calls `run_prototype1_loop_controller`.

The current path does this:

1. Parse or load the prepared batch.
2. Create or load the campaign.
3. Admit the operator run profile when one is supplied.
4. Resolve the search policy from the admitted profile or CLI flags.
5. Advance baseline eval closure.
6. Advance baseline protocol closure when `--stop-after` reaches
   `baseline-protocol`.
7. Load closure state.
8. For eligible complete baseline instances, run issue detection and
   intervention synthesis when `--stop-after` reaches `target-selection`.
9. Apply the selected intervention candidate when `--stop-after` reaches
   `intervention-apply` and the run is not dry.
10. Write treatment-evaluation projections for staged child nodes.
11. Print a `Prototype1LoopReport`.

Important current boundaries:

- The default stop point is `intervention-apply`.
- Non-dry active execution at `--stop-after compare` is refused. The source
  tells operators to run typed child evaluation through `prototype1-state`
  instead.
- Source-branch continuation in this older wrapper is refused. Successor
  continuity is expected to arrive through History, channel, and artifact
  handoff.
- This command can prepare child-node records and projections, but it is not
  the active typed child runtime / successor handoff loop.

Use this path when you are looking at the older baseline/synthesis wrapper or
the campaign setup surface behind it. Do not treat it as the current authority
for typed child execution.

## `prototype1-continue`

`prototype1-continue` runs `prototype1_state::run::resume`.

The current path is a diagnose-and-advance loop:

```text
loop:
  resolve active parent context
  diagnose current phase
  if blocked: refuse
  if complete: print status and return
  advance the diagnosed phase
  stop if the 256-advance guard trips
```

It resolves the active parent context from the parent checkout:

- repo root from `--repo-root` or the current directory;
- parent identity from `.ploke/prototype1/parent_identity.json`;
- campaign manifest from the identity's campaign id;
- admitted run profile from `prototype1/run-profile.toml`;
- effective control from the admitted profile's `[control]` section.

The diagnosis phases are:

```text
baseline_eval
baseline_protocol
child_plan
materialize
build
spawn
observe
select
handoff
complete
blocked
```

The advance mapping is direct:

| Diagnosed phase | Advance function |
| --- | --- |
| `baseline_eval` | `advance_eval_closure` |
| `baseline_protocol` | `advance_protocol_or_block` |
| `child_plan` | `resolve_profile_child_plan` |
| `materialize` | `run_planned_child(..., stop_after = Materialize)` |
| `build` | resume `C2`, run `BuildChild` |
| `spawn` | resume `C3`, run `SpawnChild` |
| `observe` | resume `C4`, run `ObserveChild`, then compare successful treatment evidence |
| `select` | reconstruct terminal child outcomes and append successor selection |
| `handoff` | reselect, then spawn and hand off successor or append stopped successor |

The child-phase difference from `prototype1-step` is the cap. In continuous
mode, child phases take up to the admitted `parallel_cap`; in step mode they
take one target. This is why `prototype1-continue` is the better fit for
operator-driven long runs once setup has already admitted a profile.

Important current boundaries:

- It resumes within the admitted campaign/profile. It does not widen search
  bounds or rewrite the admitted run profile.
- It refuses from `blocked` instead of guessing a repair.
- It prints the same status shape as doctor when it reaches `complete`.
- It depends on durable files to reconstruct current state, especially parent
  identity, child plan, node records, runner results, transition journal, and
  successor records.

## `prototype1-state`

`prototype1-state` runs `Prototype1StateCommand::run`, which records failed
successor turns when appropriate and delegates to `run_turn`.

The current path is a direct typed parent turn:

1. Resolve repo root from `--repo-root` or current directory.
2. Resolve campaign from `--campaign` or parent identity.
3. Resolve run shape from the admitted run profile when present; otherwise use
   command defaults.
4. Ensure baseline closure state exists.
5. If `--init-parent-identity` is set, write and commit generation-0 parent
   identity and return.
6. Otherwise resolve parent identity from either `--handoff-invocation` or the
   active checkout.
7. Load the parent as `Parent<Unchecked>`.
8. Acknowledge successor handoff if this is a successor invocation.
9. Check startup and enter `Parent<Ready>`.
10. Append `ParentStarted`.
11. Establish parent baseline.
12. Resolve or publish the child plan.
13. Run child fanout.
14. Select successor when `stop_after = Complete`.
15. If the continuation decision allows it, install the selected artifact,
   seal History, spawn successor, and wait for ready acknowledgement.
16. Print a `Prototype1StateReport`.

The child path inside this turn is the typed sequence:

```text
C1 -> C2: materialize child Artifact
C2 -> C3: build child Runtime binary
C3 -> C4: spawn child Runtime and observe ready
C4 -> C5: observe terminal child result
```

When a successful child is observed, the parent compares treatment evidence
against the parent baseline and writes the branch evaluation report. That
report is then available to selection.

Important current boundaries:

- `--stop-after materialize|build|spawn` is a debug slice. Non-complete modes
  intentionally run one child rather than a full search turn.
- `--stop-after complete` is the live parent-turn path. It can run fanout,
  select a successor, and hand off.
- `--handoff-invocation` is the successor entrypoint. The previous parent
  writes the invocation; the successor validates it before becoming the next
  active parent turn.
- When an admitted profile exists, it overrides command defaults for run shape.

## How They Overlap

All three paths share campaign and closure concepts. They all eventually read
or write under:

```text
~/.ploke-eval/campaigns/<campaign-id>/
~/.ploke-eval/campaigns/<campaign-id>/prototype1/
```

`prototype1-continue` and `prototype1-state` overlap most strongly. Both can
advance baseline, plan children, materialize, build, spawn, observe, select,
and hand off. The difference is control style:

- `prototype1-state` executes a parent turn directly from the command shape.
- `prototype1-continue` repeatedly diagnoses persisted state and advances the
  next recoverable phase.

`prototype1` overlaps with them only in the older campaign/baseline/synthesis
frontier. It can write projected child-node state, but current active typed
child execution belongs to `prototype1-state` and the resumable
`prototype1-continue` wrapper.

## Operator Choice

Use `prototype1` when checking or exercising the older wrapper through
baseline/protocol/synthesis/intervention-apply behavior.

Use `prototype1-state` when you need a direct one-turn typed run, a controlled
`--stop-after` slice, initial parent identity setup, or successor handoff
entrypoint behavior.

Use `prototype1-continue` when a campaign has already been admitted and you
want the active parent checkout to move forward from its current durable phase
without manually choosing the next transition.

For external state inspection during these phases, especially the child
`observe` stage, see
[`phase-state-probes.md`](../../workflow/evalnomicon/drafts/start-here/phase-state-probes.md).
