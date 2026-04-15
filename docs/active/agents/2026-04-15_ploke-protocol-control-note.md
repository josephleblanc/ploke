# Ploke-Protocol Control Note

- date: 2026-04-15
- task title: ploke-protocol control note
- task description: durable control note for the `ploke-protocol` architecture thread across forks, checkpoints, and restart surfaces
- related planning files: `docs/active/CURRENT_FOCUS.md`, `docs/active/agents/2026-04-15_protocol-cold-start-reference.md`, `docs/active/agents/2026-04-15_ploke-protocol-architecture-checkpoint.md`, `docs/active/workflow/handoffs/recent-activity.md`

## Purpose

This note is the lightweight continuity surface for the `ploke-protocol`
subtrack.

Use it to answer:

- which checkpoint is currently authoritative
- which fork/thread lineage the current work belongs to
- what the next intended slice is
- which older notes are context only

## Current Status

- workstream: `A1`
- status: active
- current_thread: `fork-1` of the 2026-04-15 protocol architecture line
- current_focus: usage-based pressure testing of the rewritten `ploke-protocol`
  architecture and selection of the next concrete implementation slice

## Authoritative Artifacts

### Current authority

- [2026-04-15_ploke-protocol-architecture-checkpoint.md](./2026-04-15_ploke-protocol-architecture-checkpoint.md)

This is the authoritative implementation checkpoint for the current crate
rewrite.

### Supporting references

- [2026-04-15_protocol-cold-start-reference.md](./2026-04-15_protocol-cold-start-reference.md)
- [2026-04-12_eval-infra-sprint/2026-04-14_ploke-protocol-bootstrap-handoff.md](./2026-04-12_eval-infra-sprint/2026-04-14_ploke-protocol-bootstrap-handoff.md)

## Thread Lineage

- `original-thread`
  - conceptual alignment
  - cold-start reconnaissance
  - protocol architecture rewrite
  - ended with the authoritative architecture checkpoint
- `fork-1`
  - resumes from the architecture checkpoint
  - pressure-tests the new protocol against the older CLI-first useful workflow
  - decides the next implementation slice from live usage rather than from
    architecture alone

## Intended Next Slice

Preferred next implementation order remains:

1. fix the current live protocol usage gap(s) exposed by interactive testing
2. persist protocol procedure artifacts beside the run in `ploke-eval`
3. add a second bounded protocol to pressure-test the architecture
4. move subject-building out of ad hoc CLI helpers into clearer `ploke-eval`
   adapter surfaces

## Supersession Rule

When a new fork or checkpoint becomes authoritative:

1. add it here under `Authoritative Artifacts`
2. update `current_thread`
3. move the previous authority to supporting context if superseded
4. leave a one-line statement describing what changed and why the authority
   moved

## Notes

- This note is intentionally small.
- Detailed implementation status belongs in checkpoint notes, not here.
- If chat context and repo state disagree, prefer this note plus the
  authoritative checkpoint over conversational memory.
