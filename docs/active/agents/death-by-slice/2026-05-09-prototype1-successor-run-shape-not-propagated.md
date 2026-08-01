# 2026-05-09 Prototype 1 Successor Run Shape Not Propagated

## Local Slice

`prototype1-state` exposes run-shape choices as CLI flags:

- `--candidate-generator tui-edit-surface`
- `--edit-surface ploke-tui-tools`
- `--successor-selection history-score-child-prop`
- `--successor-selection-metrics operational-and-protocol`
- `--successor-selection-seed 0`

That was sufficient for the first parent process invocation, so the surface
looked usable from the CLI.

## Missing Structure

The run shape is not persisted as campaign or runtime policy. It is only present
on the initial parent command line.

The successor parent invocation is constructed separately in
`crates/ploke-eval/src/cli/prototype1_state/invocation.rs` and currently
hardcodes only:

- campaign id;
- active parent root;
- handoff invocation path;
- `--stop-after complete`;
- `--format json`.

It does not carry candidate generation, edit surface, successor-selection
strategy, successor-selection metric inputs, or replay seed.

## How It Came Back

The intended overnight run was supposed to exercise the new edit surface and
score successors with operational plus protocol evidence. The first parent would
do that if launched with explicit flags.

The selected successor / next parent would not. Its generated argv falls back to
the `Prototype1StateCommand` defaults:

- `candidate_generator = Legacy`
- `successor_selection_metrics = Operational`

So the run would silently change behavior at generation 1. That is worse than a
simple launch failure because the campaign would appear to be the requested
experiment while actually running a different policy after the first handoff.

## Required Fix

Do not treat this as more CLI plumbing.

The run shape needs a durable structural carrier, probably in campaign/runtime
policy, with successor invocation projecting from that carrier. The successor
handoff should not reconstruct policy from ad hoc command defaults.

The invariant should be:

> A successor parent launched from a campaign must receive the same run-shape
> policy as the parent that selected it, unless the transition explicitly records
> a policy change.

The prevention mechanism should be type or constructor enforced:

- initial setup records a `RunShape` / equivalent policy carrier;
- parent execution reads that carrier instead of relying only on CLI defaults;
- successor invocation stores or references that carrier;
- `successor_parent_argv` cannot be constructed without the selected
  generation policy;
- tests assert that a successor invocation launched after a parent turn carries
  edit-surface generation and operational-plus-protocol metrics when those were
  selected for the campaign.

## Related Files

- `crates/ploke-eval/src/cli.rs`
  Defines the current CLI flags and defaults.
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
  Parent execution reads `candidate_generator` and
  `successor_selection_metrics` from the current command.
- `crates/ploke-eval/src/cli/prototype1_state/invocation.rs`
  Successor parent argv construction currently drops those choices.
