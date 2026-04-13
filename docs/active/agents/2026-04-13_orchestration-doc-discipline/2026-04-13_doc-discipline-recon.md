# 2026-04-13 Doc Discipline Recon

- date: 2026-04-13
- task title: locate existing archive and stale-doc discipline for orchestration/eval workflow docs
- task description: determine whether the repo already contains a specific protocol or convention for retiring, archiving, or superseding stale orchestration and eval-workflow documents before proposing any new process changes
- related planning files: `docs/active/plans/evals/eval-design.md`, `docs/active/agents/2026-04-12_eval-orchestration-protocol/2026-04-12_eval-orchestration-protocol.md`, `docs/active/workflow/README.md`, `docs/active/workflow/handoffs/recent-activity.md`

## Purpose

This is a narrow reconnaissance packet. The goal is to answer one question:

- do we already have a protocol or durable convention for stale/archived/superseded
  orchestration and eval-workflow docs, and if so where does it live?

## Packet `DDR-1A`

- owner_role: worker
- status: ready
- scope:
  - search the active eval/orchestration/workflow docs for explicit archive,
    supersede, stale-doc, or retirement rules
  - prioritize `eval-design.md`, the active orchestration protocol, workflow docs,
    templates, and recent meta-process packets
- non_goals:
  - do not propose a new process yet
  - do not modify docs
- owned_files:
  - read-only review of `docs/active/**`, `docs/workflow/**`, and `AGENTS.md`
- acceptance_criteria:
  1. identify any explicit existing rule or convention relevant to stale docs
  2. distinguish exact rules from weak conventions or scattered examples
  3. cite the highest-signal sources directly
- required_evidence:
  - exact file references
  - concise finding summary
  - explicit not-checked areas

## Packet `DDR-1B`

- owner_role: worker
- status: ready
- scope:
  - search adjacent process/template/history surfaces for the same question,
    including control-plane templates, S3/meta-process artifacts, and any archive
    boundary docs that might have been intended to generalize
- non_goals:
  - do not propose a new process yet
  - do not modify docs
- owned_files:
  - read-only review of `docs/active/agents/**`, relevant templates, and process notes
- acceptance_criteria:
  1. identify whether a stale-doc rule exists outside the main workflow/orchestration docs
  2. distinguish active protocol from historical habit
  3. cite the strongest supporting or contradictory evidence
- required_evidence:
  - exact file references
  - concise finding summary
  - explicit not-checked areas

## Decision Rule

- if both packets converge on “no single explicit rule exists,” treat that as the
  current state and avoid acting like a hidden protocol already exists
- if either packet finds a clear existing rule, treat that as the controlling starting point
