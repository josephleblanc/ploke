# Eval Orchestration Protocol

**Date:** 2026-04-12
**Task Title:** Eval orchestration protocol
**Task Description:** Define a concrete sub-agent coordination protocol for eval and harness work that minimizes drift, prevents self-certification, and survives orchestrator replacement.
**Related Planning Files:** `docs/active/plans/evals/eval-design.md`, `docs/active/CURRENT_FOCUS.md`, `docs/active/workflow/README.md`, `docs/active/agents/phase-1-audit/AUDIT_SYNTHESIS.md`

## Purpose

This protocol is the control plane for eval and harness work. It exists to keep
parallel implementation aligned with `eval-design.md` while preventing false
confidence, handoff drift, and loss of design intent.

This document does not replace the existing workflow hierarchy:

- `docs/active/CURRENT_FOCUS.md` remains the top-level recovery entry point
- `docs/active/workflow/*` remains the operational source of truth
- `docs/active/plans/evals/eval-design.md` remains the central design/rationale doc

This protocol adds execution discipline on top of those documents.

## Precedence And Startup Sequence

When this protocol and a live workflow artifact disagree on operational state,
the live workflow artifact wins unless the discrepancy is explicitly called out
and resolved.

Startup sequence for real eval work:

1. read `docs/active/CURRENT_FOCUS.md`
2. read `docs/active/workflow/README.md`
3. read `docs/active/workflow/readiness-status.md`
4. read `docs/active/workflow/handoffs/recent-activity.md`
5. read `docs/active/plans/evals/phased-exec-plan.md`
6. read the active orchestration doc for the current sprint

If the active planning document changes, the orchestrator must update
`CURRENT_FOCUS.md` in the same change set and mark the prior planning doc
superseded with a forward link.

## Core Rules

1. One orchestrator owns the control plane at a time.
2. Workers do not certify their own work as accepted, verified, or done.
3. Every implementation claim must point to concrete evidence.
4. The orchestrator is the only role allowed to mark a task `accepted`.
5. For Layer 0-1 eval infrastructure work, independent verification is required before acceptance unless the orchestrator explicitly records an exception.
6. If a task would weaken correctness, invariants, validation, schema guarantees, or import semantics, stop and escalate before implementation.
7. Lower-layer measurement and replay work takes priority over higher-layer tool optimization whenever interpretation is blocked.

## Roles

### Orchestrator

Owns:

- priority and sequencing
- mapping each task to layer/workstream (`A1`-`A5`, `H0`)
- packet creation
- acceptance criteria
- acceptance / rejection / re-scope decisions
- updates to central coordination state

Must not:

- delegate away ownership of design intent or acceptance
- mark a task accepted based on worker confidence alone

### Worker

Owns:

- bounded implementation or audit work inside the assigned scope
- producing claims plus evidence
- reporting exactly what was not checked

Must not:

- broaden scope without reporting it
- claim "verified", "fixed", "done", or "works" without attached evidence
- silently change invariants, schemas, or shared interfaces

### Verifier

Owns:

- adjudicating specific claims against concrete evidence
- checking whether acceptance criteria are actually satisfied
- flagging unverified assumptions, missing checks, and drift

Must not:

- redesign the task
- expand into a second implementation pass unless explicitly reassigned
- mark a task accepted

## Canonical Artifacts

The protocol uses four artifact types.

1. `CURRENT_FOCUS.md`
   Entry point and recovery pointer.
2. Active orchestration doc
   The current control-plane document in `docs/active/agents/...`.
3. Task packet
   The bounded unit of delegated work.
4. Worker or verifier report
   Short structured report tied to a packet.

The active orchestration doc should link to all active packets and reports so a
replacement orchestrator can resume from documents instead of chat history.

The active orchestration doc must contain a compact current-state table with, at
minimum:

- `task_id`
- `status`
- `owner`
- `layer_workstream`
- `packet_link`
- `latest_report_link`
- `next_action`

The table is the default resume surface. Packet and report docs hold the detail.

## Required Task Packet Fields

Every delegated task gets a task packet with these fields:

- `task_id`
- `title`
- `date`
- `owner_role`
- `layer_workstream`
- `related_hypothesis`
- `design_intent`
- `scope`
- `non_goals`
- `owned_files`
- `dependencies`
- `acceptance_criteria`
- `required_evidence`
- `report_back_location`
- `status`

Rules:

- `scope` must be narrow and falsifiable
- `owned_files` should be disjoint across parallel workers when possible
- `acceptance_criteria` must describe observable outcomes, not effort
- `required_evidence` must be specific enough that a verifier can answer yes/no
- each acceptance criterion should be numbered so worker claims can map to it directly

## Task States

Use these states exactly:

- `proposed`
- `ready`
- `in_progress`
- `implemented_unchecked`
- `implemented_self_checked`
- `independently_checked`
- `accepted`
- `blocked`
- `superseded`

State meanings:

- `implemented_unchecked`: code or docs changed, but no meaningful checking was done
- `implemented_self_checked`: worker produced evidence, but no independent verification yet
- `independently_checked`: verifier confirms the evidence supports the narrow claims
- `accepted`: orchestrator confirms acceptance criteria are met and updates the control plane

Only the orchestrator changes a task to `accepted` or `superseded`.

## Evidence Policy

Workers and verifiers report evidence, not confidence.

Allowed evidence forms:

- command executed
- tests executed
- sample artifact inspected
- diff inspected
- specific file/line references
- concrete output summary

Disallowed evidence forms:

- "should work"
- "looks correct"
- "probably fixed"
- "tests pass" without naming the test command
- "verified" without describing what was checked

Required wording:

- use `claims` for propositions being asserted
- each claim must cite the acceptance criterion it is intended to satisfy
- use `evidence` for supporting artifacts
- use `not_checked` for anything not validated
- use `unsupported_claims` for any attempted claim that the evidence did not support
- use `risks` for realistic ways the claim could still be wrong

## Default Verification Policy

### Mandatory independent verification

Require a verifier before acceptance when a task touches:

- `crates/ploke-eval/` run schema, replay, inspection, metrics, manifest, or failure classification
- shared interfaces used by multiple eval components
- phase status or readiness claims
- automation that could create trivially passing tests or false negatives

### Usually self-check is sufficient before acceptance

The orchestrator may accept after self-check only for:

- isolated doc-only changes
- narrow refactors with no behavior change and clear diff evidence
- housekeeping that does not affect measurement or interpretation

Any exception for Layer 0-1 work should be recorded explicitly in the active
orchestration doc.

## Verifier Budget

Verifier passes must stay bounded.

Default verifier budget:

- inspect only the packet, required evidence, cited files, and cited commands
- no repo-wide exploration unless an escalation trigger fires
- no implementation edits unless the orchestrator reassigns the task
- stop after three targeted checks if the packet still cannot be adjudicated cleanly

If the budget is exhausted, the verifier returns `revise` or `blocked` with the
specific missing evidence or ambiguity.

## Delegation Loop

1. Orchestrator creates or updates the active orchestration doc.
2. Orchestrator creates a task packet with bounded scope and acceptance criteria.
3. Worker executes only that packet.
4. Worker submits a structured report with claims and evidence.
5. If required, verifier checks the claims against the evidence and submits a verifier note.
6. Orchestrator decides one of:
   - accept
   - reject and request revision
   - split into smaller packets
   - escalate due to invariant or scope risk
7. Orchestrator updates the active orchestration doc and, when appropriate, `CURRENT_FOCUS.md` or workflow artifacts.

When packet state changes materially, the orchestrator should also update the
relevant living workflow artifact, usually `recent-activity.md` or a task-specific
handoff, so the workflow record and the orchestration record do not diverge.

## Escalation Triggers

Escalate back to the orchestrator before continuing if any of the following is true:

- the task touches files outside its declared ownership
- a shared schema or API boundary must change
- a correctness guardrail would need to be weakened
- existing uncommitted changes conflict with the packet scope
- acceptance criteria cannot be met without broadening the task
- the evidence contradicts the original hypothesis or packet framing

For eval work, also escalate if the task reveals:

- parse fidelity problems that invalidate downstream conclusions
- replay or inspection gaps that make a result non-attributable
- evidence that a test is tautological or trivially passing

## Drift Prevention Rules

1. Keep one active orchestration doc for the current sprint or focus area.
2. Split broad work until each packet has one clear acceptance boundary.
3. Prefer disjoint write scopes for parallel workers.
4. When a packet changes design intent, update the task packet first, then implementation.
5. Do not let reports become mini-design docs; they should stay short and structured.
6. If the active planning document changes, update `CURRENT_FOCUS.md` immediately.
7. Chat history is optional context; documents must be sufficient for recovery.
8. Keep workflow metadata (`owning_branch`, `review_cadence`, `update_trigger`) aligned with the living artifact rules when the protocol touches workflow docs.

## Acceptance Rule

A task is accepted only when all three conditions hold:

1. the evidence supports the claims
2. the claims satisfy the packet acceptance criteria
3. no unresolved risk remains that would invalidate interpretation at the task's layer

This is intentionally stricter for Layer 0-1 work than for ordinary feature work.

## Suggested Directory Layout

For a concrete orchestration effort, keep documents together:

- `docs/active/agents/<date>_<topic>/...`
- task packets under the same directory
- worker and verifier reports under the same directory

This keeps recovery local and avoids scattering control-plane state.

## Recommended Next Use

Use this protocol to drive the next eval sprint in this order:

1. create an active orchestration doc for the eval-infra sprint
2. define the first packets around the P0 audit gaps
3. require independent verification for each Layer 0-1 packet
4. only move on to tool-design hypothesis work once the inspection/replay surface is genuinely usable
