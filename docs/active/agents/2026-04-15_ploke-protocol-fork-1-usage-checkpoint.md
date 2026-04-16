# Ploke-Protocol Fork-1 Usage Checkpoint

- date: 2026-04-15
- task title: ploke-protocol fork-1 usage checkpoint
- task description: usage-based checkpoint for the first fork after the major `ploke-protocol` rewrite, comparing the new tool-call review protocol against the older CLI-first `inspect tool-calls` workflow
- related planning files: `docs/active/agents/2026-04-15_ploke-protocol-control-note.md`, `docs/active/agents/2026-04-15_ploke-protocol-architecture-checkpoint.md`, `docs/workflow/evalnomicon/src/meta-experiments/02-prototing-protocol.md`

## Scope

This fork did not change the architecture.

It pressure-tested the first live protocol against the older useful
CLI-first workflow and fixed one immediate contract bug in the protocol output.

## What Changed

- updated the experiment journal with a distilled framing block quote in
  `docs/workflow/evalnomicon/src/meta-experiments/02-prototing-protocol.md`
- fixed the `tool-call-review` contract in
  `crates/ploke-protocol/src/tool_calls/review.rs`
  - removed `index` from adjudicated `Judgment`
  - aligned the prompt and typed output so the protocol parses successfully

## Live Test Result

The new protocol now runs successfully through the CLI.

Tested against run:

- `tokio-rs__tokio-4519`

Tested calls:

- call `0`
- call `7`
- call `8`

Observed result:

- all three calls were judged `appropriate`
- all three judgments were `high` confidence

## Comparative Read

The old `inspect tool-calls` workflow remains more useful right now.

Why:

- it exposes trajectory and turning points across the trace
- it supports richer local inspection with `--full`
- it helps a reviewer notice search repetition, context bloat, and possible
  late-stage thrash

The new protocol is now operational but not yet competitively informative.

Why:

- the evidence packet is still too thin
- the question is too permissive
- the current unit judgment does not yet surface a clearly valuable non-obvious
  metric

## Current Conclusion

Do not prioritize persistence yet.

The next design pressure should be on protocol usefulness, not on storing more
outputs from a weak first protocol.

The first protocol is now:

- mechanically functioning
- machine-readable
- not yet strong enough to count as a worthwhile first NOM in practice

## Best Next Step

Refine or replace the current first protocol so it targets a more informative
value than generic appropriateness.

Likely better targets:

- redundancy
- recoverability
- search thrash
- marginal usefulness of a call within local trace context

Only after one protocol is both live and genuinely informative should protocol
artifact persistence move to the front of the queue.
