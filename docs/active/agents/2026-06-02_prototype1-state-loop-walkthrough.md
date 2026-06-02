# 2026-06-02 Prototype 1 state-loop walkthrough

Status: draft walkthrough / implementation audit
Command: `ploke-eval loop prototype1-state`
Base branch: `feature/ploke-loop`
Scope: `crates/ploke-eval`, with live-model/protocol boundaries into `ploke-protocol` where relevant.

## Purpose

This guide explains the current implementation path for `ploke-eval loop prototype1-state` so future work on the Prototype 1 observable self-improving loop can reason about configuration, execution phases, disk writes, persisted data, live API/model routing, and parallelism opportunities.

## Reading notes

- File/line references are to the source snapshot on branch `docs/prototype1-state-loop-walkthrough`, based at `3a4ac77e`.
- The focus is the typed parent turn path in `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`, plus C1-C4 child transitions.
- This document intentionally distinguishes setup/admission (`prototype1-setup`) from the runtime turn (`prototype1-state`).

## Outline

1. Command entrypoint and configuration loading.
2. Campaign config and admitted run-profile config.
3. Full parent-turn execution path.
4. Child C1-C4 state-machine path.
5. Disk-write boundaries.
6. Persisted data types and originating runtime types.
7. Live API calls and where models are selected.
8. Parallelism that exists today and missed/weak parallelism opportunities.
9. Practical debugging/stabilization notes.

## Draft evidence spine

The verified high-level path is:

- `crates/ploke-eval/src/main.rs` parses the CLI and calls `Cli::run()`.
- `crates/ploke-eval/src/cli.rs` dispatches `LoopSubcommand::Prototype1State(cmd)` to `cmd.run().await`.
- `Prototype1StateCommand::run` calls `run_turn` in `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`.
- `run_turn` resolves repo/campaign/config/admitted run shape, validates parent identity or successor handoff, appends parent-start evidence, establishes baseline, plans children, runs fanout, compares child treatments, selects a successor, and may hand off to the next parent.
- Each child proceeds through C1 materialize, C2 build, C3 spawn/ready, and C4 observe/result.

The expanded walkthrough below will replace this outline with detailed evidence tables and step-by-step notes.
