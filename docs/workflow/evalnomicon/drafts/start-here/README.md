# Start Here

This directory is a working entrypoint for the Prototype 1 loop. It is meant to
make the loop reviewable before material is promoted into
[`src/prototype1/`](../../src/prototype1/index.md) or the operator reference in
[`crates/ploke-eval/docs`](../../../../../crates/ploke-eval/docs/README.md).

## Source Priority

When these drafts disagree, use this order:

1. Current code in
   [`prototype1_state/mod.rs`](../../../../../crates/ploke-eval/src/cli/prototype1_state/mod.rs),
   [`run/core.rs`](../../../../../crates/ploke-eval/src/cli/prototype1_state/run/core.rs),
   and
   [`history.rs`](../../../../../crates/ploke-eval/src/cli/prototype1_state/history.rs).
2. Current operator references under
   [`crates/ploke-eval/docs`](../../../../../crates/ploke-eval/docs/README.md).
3. Current Evalnomicon pages under [`src/prototype1/`](../../src/prototype1/index.md).
4. Drafts under [`docs/workflow/evalnomicon/drafts`](../README.md).
5. Workflow skills, treated as operator procedure, not runtime authority.

## Reading Order

- [`stage-overview.md`](stage-overview.md)
  Detailed phase-by-phase map of the loop, from setup through handoff.
- [`operator-workflow.md`](operator-workflow.md)
  How setup, diagnostic step, run review, and blocker repair fit the loop.
- [`evidence-and-artifacts.md`](evidence-and-artifacts.md)
  Authority and evidence surfaces, including what is History authority and what
  is only an operational projection.
- [`phase-state-probes.md`](phase-state-probes.md)
  How to infer finer-grained runtime state from durable files while a phase is
  in progress.
- [`source-map.md`](source-map.md)
  Code and existing-doc map for refreshing these drafts against current source.
- [`review-notes.md`](review-notes.md)
  Synthesis, drift risks, and follow-up questions for the next documentation
  pass.

## Status Labels

Use these labels inside the draft set:

- `implemented`: enforced by current source or current command behavior.
- `partially implemented`: current source enforces part of the claim, but the
  typed model, History model, or controller integration still has named gaps.
- `intended`: target architecture described by source comments or docs but not
  fully enforced.
- `operator rule`: guidance from workflow skills or operator docs.
- `not claimed`: stronger interpretation that Prototype 1 explicitly does not
  rely on.
