# Walk summary rejects valid pre-child-plan campaigns

Status: fixed and live verified; typed phase field remains later snapshot work

Discovered: 2026-07-13

## Broken Contract

`loop walk summary` is the server-independent durable campaign discovery view.
It should render every admitted phase, including generation-0 baseline eval,
baseline protocol, and the pre-child-plan parent state. The absence of
`prototype1/messages/child-plan` is valid in those phases and must be rendered
as “not produced yet,” not treated as a malformed run manifest.

At the same time, the fix must not make a missing child-plan directory
universally optional. If downstream child, report, selection, or handoff
evidence requires child-plan authority, absence remains an integrity error.

## Reproduction

After explicitly selecting the fresh Stage 0 parent:

```text
ploke-eval loop walk use \
  /home/brasides/.ploke-eval/worktrees/p1-stage0-g35f-pplxembed-3g1x3-p3-20260713-1

ploke-eval loop walk summary -v
```

Observed result:

```text
failed to read run manifest
'.../prototype1/messages/child-plan':
No such file or directory (os error 2)
```

At reproduction time ordinary Prototype 1 doctor reported
`phase = baseline_eval`; after the embedding quota attempt it reported
`phase = blocked` with only baseline failure evidence. Neither state should
have child-plan authority.

## Source Boundary

```text
walk::summary::run
-> WalkSummary::load
-> load_generations
-> read_dir_optional(messages/child-plan)
-> fs::read_dir
```

`read_dir_optional` currently maps `NotFound` to
`PrepareError::ReadManifest`; despite its name it is not optional. The helper
is also used for node and report discovery, so changing it globally would
silently weaken integrity checks.

## Missing Regression

Add production-read-model coverage for both sides of the contract:

1. an admitted baseline campaign with no child-plan directory renders a typed
   empty/not-yet-produced plan state;
2. a campaign with downstream child-dependent evidence but a missing
   child-plan directory fails with an explicit authority/integrity blocker.

The preferred repair is to reuse the canonical diagnosed phase in the future
typed loop snapshot rather than infer phase from directory existence.

## Fix

`WalkSummary::load` now decides whether child-plan authority is required from
canonical durable session phase plus downstream node, report, terminal broad
harness, and transition-journal evidence. A missing child-plan directory is
accepted only when those sources still prove a pre-plan generation-0 state.
R8-or-later session state, later-generation identity, multiple node records,
parent reports, terminal edit-harness results, damaged session evidence, or
later transition evidence all keep the reader fail-closed.

The child-plan loader handles `NotFound` locally. The shared directory helper
and malformed existing JSON behavior remain strict.

Regression coverage proves:

1. an absent pre-authority child-plan directory produces an empty generation
   list;
2. the same absence fails with an explicit missing-authority error when
   downstream evidence requires the plan;
3. malformed existing child-plan JSON still fails closed.

Live verification against
`p1-stage0-g35f-pplxembed-3g1x3-p3-20260713-1` now renders
`child_plan_count = 0`, `planned_generations = []`, and `generations = []`
through the production `loop walk summary --format json` command.

## Operator Context Observation

Before this reproduction, `walk summary` invoked from the new worktree showed
an older campaign because persisted `walk use` context takes precedence over
cwd. That behavior is documented, but the selected run must be prominent in UI
and CLI snapshots so an operator cannot mistake one campaign for another.
