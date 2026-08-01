---
name: prototype1-evalops
description: Use when turning a Prototype 1 campaign into a repeatable EvalOps review pipeline: campaign inventory, dry-run Kanban board planning, node/slot coverage verification, quality-gated run reviews, synthesis, backlog handoff, and conservative watchdog follow-through.
---

# Prototype 1 EvalOps

Use this skill when the task is bigger than one run review. The goal is to turn
Prototype 1 campaign artifacts into a deterministic review-and-development
pipeline, not just write a good narrative report.

Primary design note:

- `docs/active/agents/2026-06-02_prototype1-evalops-pipeline.md`

Companion review procedure:

- `docs/workflow/skills/ploke-run-review/SKILL.md`

## When To Use

Use this skill when the user asks to:

- make sure all campaign nodes and slots are covered;
- create or repair a run-review Kanban board for a Prototype 1 campaign;
- develop run review into a repeatable development pipeline;
- automate run-review fan-out/fan-in safely;
- verify that a review board covers every node, request slot, result slot,
  runner result, evaluation, and successor handoff;
- convert run-review findings into backlog, bug, evalnomicon, or rerun handoffs.

Do not use this as a substitute for `ploke-run-review` when the task is only a
single trace-bearing run review. Use `ploke-run-review` for the report itself;
use this skill for orchestration, coverage, and pipeline structure.

## Core Pipeline

Follow this order:

```text
campaign grounding
-> campaign inventory
-> dry-run board plan
-> node/slot coverage verification
-> Kanban graph materialization
-> review execution
-> review quality gate
-> synthesis
-> backlog/rerun decision
-> optional conservative watchdog
```

Do not skip directly to board creation. Coverage must be provable before Kanban
mutation.

## Stage 0: Ground The Campaign

Resolve the exact campaign before any board or watchdog work.

Required facts:

- campaign id and campaign root;
- intended repo/worktree root;
- active process state, if any;
- loop mode: baseline eval, broad-harness child attempt, successor handoff,
  post-exit artifact review, or self-improvement loop;
- current parent node and generation, if applicable;
- existing review docs and existing Kanban board, if any;
- requested profile/reasoning policy, if worker routing matters.

Stop and report instead of improvising if:

- the target repo path is missing;
- the campaign id is ambiguous;
- the active process/worktree does not match the claimed campaign;
- the user requested a specific assignee tier and the profile route has not been
  preflighted.

## Stage 1: Build Inventory

Write or refresh a deterministic, read-only inventory under project/campaign
scratch, for example:

```text
~/.ploke-eval/review-handoffs/<campaign-id>/INVENTORY.json
```

The inventory should cover:

- `campaign.json`, `closure-state.json`, `scheduler.json`;
- `prototype1/transition-journal.jsonl`;
- all authoritative campaign node directories under
  `<campaign-root>/prototype1/nodes/node-*`;
- each node's `node.json`, plus `runner-request.json`, `runner-result.json`,
  `results/*.json`, `invocations/*.json`, streams, and channel files when
  present;
- all `messages/edit-harness-request/*.json` slots;
- all `messages/edit-harness-result/*.json` slots;
- headless TUI traces;
- submitted results;
- candidate workspaces and commits;
- runner requests/results;
- treatment campaign roots;
- branch evaluations;
- successor-ready and successor-completion records;
- provider/quota failures, redacted;
- stale-running metadata reconciliations.

Use compact nested schema names. Keep field names to three semantic parts or
fewer; if a longer flattened name is tempting, introduce a nested object.

## Stage 2: Dry-Run Board Plan

Do not count fixture copies as campaign nodes. Candidate workspaces may contain
files such as
`prototype1/workspaces/edit-harness/<slot>/crates/ploke-eval/src/tests/fixtures/prototype1-node-150-handoff/node.json`;
those are fixture files inside the candidate checkout. The authoritative campaign
nodes are under `<campaign-root>/prototype1/nodes/node-*`.

Generate the proposed task graph before mutating Kanban.

The dry-run plan should include:

- board slug;
- task keys, titles, assignees, and parent keys;
- each node/slot/successor phase covered by each task;
- expected output paths;
- run-review skill path in every reviewer card body;
- read-only/doc-only worker boundaries.

Task classes:

- `coverage spine`: proves the artifact map and writes a coverage ledger;
- `node lifecycle`: classifies parent, failed child, runner success, selected
  successor, stale-running state, or terminal absence;
- `completed slot`: reviews one trace-bearing attempt;
- `incomplete slot`: records request-only/workspace-only/missing-join state;
- `runner success`: reviews treatment runner and patch/eval evidence;
- `successor handoff`: reviews the later continuity/handoff phase separately;
- `fan-in synthesis`: aggregates only quality-gated reviews.

## Stage 3: Coverage Verification

Compare the inventory with the dry-run plan or actual board.

The verifier must fail closed on:

- missing node coverage;
- missing request/result slot coverage;
- duplicate task ownership without an explicit retry relationship;
- synthesis not parent-gated on all required review tasks;
- selected-successor conflation, where runner success and successor handoff are
  covered by one task even though both phases exist;
- stale-running metadata accepted without reconciliation evidence;
- provider failure classified as benchmark success;
- board/worktree paths outside the grounded campaign/repo scope;
- reviewer task bodies that omit `docs/workflow/skills/ploke-run-review/SKILL.md`.

Only materialize or repair the board after this check passes, or after the user
explicitly accepts the listed gaps.

## Stage 4: Materialize Kanban Safely

Rules for board mutation:

- Use explicit `hermes kanban --board <slug> ...` commands.
- If a dispatcher may race graph creation, park roots while wiring the graph.
- Create child tasks with parent links at creation time.
- Put inventory and worker-contract paths in every card body.
- Put read-only/doc-only boundaries in every card body.
- Re-run coverage verification against actual board state after creation.

A created graph is not a control loop. If unattended progress is part of the
request, install and verify a scoped watchdog after the graph is correct.

## Stage 5: Review And Quality Gate

Each reviewer owns exactly one durable review or incomplete-state report for the
assigned scope. The report must follow `ploke-run-review` and include:

- scope and classification;
- evidence roots;
- execution path;
- at least one concrete trace chain when trace evidence exists;
- mechanical completion;
- benchmark usefulness;
- model/tool behavior;
- protocol/read-side blind spots;
- blockers and non-blocking bugs;
- follow-up handoff.

Before synthesis counts a report as durable evidence, check that it cites exact
paths, distinguishes mechanical completion from benchmark usefulness, lists
missing joins for incomplete states, and redacts provider project ids, endpoint
URLs, tokens, and credential paths.

## Stage 6: Synthesis And Handoff

Synthesis consumes inventory, coverage report, quality reports, and durable
review markdowns. It must classify findings into:

- observability bugs;
- provider/capability gates;
- protocol artifact gaps;
- run-loop correctness bugs;
- operator workflow issues;
- benchmark usefulness questions;
- non-actionable noise;
- positive adjudication candidates.

For each actionable finding, recommend the next target: Kanban repair card,
alive bug/issue note, evalnomicon design note, rerun decision, or deferred
finding. State the highest verified gate rather than summarizing only completed
card counts.

## Stage 7: Conservative Watchdog

Only add a watchdog after inventory, dry-run planning, and coverage verification
are deterministic.

Safe watchdog behavior:

- regenerate or update inventory;
- detect new trace-bearing slots;
- compute dry-run plan deltas;
- run the coverage verifier;
- create missing cards only if the verifier passes;
- dispatch at most one review worker per tick unless the user asks for broader
  fanout;
- report only state changes, blockers, dispatches, review completions, or final
  synthesis;
- stay silent on unchanged ticks.

Never let a watchdog advance the live campaign, edit production code, create
recursive cron jobs, or silently switch provider/worktree routes.

## Regression Campaign Shapes

Use known campaign shapes as fixtures once implementing tools:

- `...-100204`: terminal/provider-unavailable path with no child admission;
  tests absence-review behavior.
- `...-165628`: active self-improvement loop with parent/child/successor
  classification; tests live-loop grounding.
- `...-170054`: selected successor runner success plus later successor handoff
  `RESOURCE_EXHAUSTED`; tests runner-vs-handoff split and provider-failure
  classification.

These fixtures should validate inventory generation, dry-run planning, coverage
verification, and review classification without live provider calls.

## Common Pitfalls

- **Creating cards before inventory.** This repeats the old ad hoc failure mode.
  Build the campaign map first.
- **Treating selected successor as total coverage.** The selected child is one
  branch outcome, not proof that all nodes and slots were reviewed.
- **Conflating runner success with handoff success.** A treatment runner can
  produce a useful selected patch while the later successor handoff fails from a
  provider/quota error.
- **Counting provider failures as benchmark progress.** Failure paths are
  reviewable, but not patch or benchmark wins.
- **Letting watchdogs become the source of truth.** Watchdogs are follow-through
  mechanisms; inventory and coverage verification are the authority.
- **Summarizing board health by card count.** State coverage and highest verified
  gate instead.

## Verification Checklist

- [ ] Exact campaign/root/worktree grounded.
- [ ] Inventory includes every node and every request/result slot.
- [ ] Dry-run plan exists before Kanban mutation.
- [ ] Coverage verifier passes or reports exact gaps.
- [ ] Runner success and successor handoff are separate when both phases exist.
- [ ] Reviewer cards cite `docs/workflow/skills/ploke-run-review/SKILL.md`.
- [ ] Synthesis is parent-gated on all required reviews.
- [ ] Provider/quota failures are typed and redacted.
- [ ] Watchdog, if any, is scoped, conservative, and non-recursive.
