# Phase 3 Edit Surface Integration Review

Date: 2026-05-07

Task title: Review first bounded edit-surface candidate-generation integration slice

Task description: Review the Phase 3 CLI/config dispatch slice for bounded
`ploke-tui` edit-surface candidate generation, focusing on fail-closed
semantics, legacy fallback risks, authority/projection boundaries, and whether
the slice is safe to commit before the live backend path exists.

Related planning files:

- `docs/archive/agents/2026-05/2026-05-07-prototype1-edit-surface-implementation-plan.md`
- `docs/archive/agents/2026-05/2026-05-07-prototype1-edit-surface-model.md`
- `docs/archive/agents/2026-05/2026-05-07-edit-surface-phase3-integration-slice.md`
- `crates/ploke-eval/src/cli.rs`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`

## Verdict

No blocking findings for committing this as a non-live first integration slice.

The slice does not make `tui-edit-surface` live. That is the correct behavior
for the current authority state: the path fails before publishing a `ChildPlan`
because backend-owned after-Artifact validation is not wired. The change is
safe only under that non-live interpretation.

## Findings

No correctness blockers found.

## CLI Semantics

The CLI adds explicit candidate-generation and edit-surface knobs:

```text
--candidate-generator legacy
--candidate-generator tui-edit-surface
--edit-surface ploke-tui-tools
```

`legacy` remains the default, which preserves current behavior for existing
operators and scripts. Explicit `--candidate-generator tui-edit-surface`
dispatches through the new path instead of silently using legacy target
selection.

The current `--edit-surface` flag is inert when `--candidate-generator legacy`
is selected. That is acceptable in this slice because there is only one edit
surface value and the legacy path is explicitly the default. If more surfaces
or generators are added, parse/config validation should reject meaningless
combinations rather than letting a surface flag look active while legacy runs.

## Fail-Closed Behavior

The requested `tui-edit-surface` path fails closed before child-plan publication:

- `resolve_child_plan` builds `CandidateGenerationConfig` from the command
  before selecting or reusing a child plan.
- If an existing child-plan file is present and `tui-edit-surface` was
  requested, it returns `ExistingPlanLacksEditSurfaceEvidence` instead of
  reading the legacy plan.
- If no child plan exists, `run_parent_target_selection` dispatches to
  `run_tui_edit_surface_parent_target_selection`, which returns
  `BackendOwnedAfterArtifactValidationMissing`.
- The error text names the missing backend-owned after-Artifact validation
  rather than presenting this path as implemented.

This preserves the important property: a requested bounded edit-surface
candidate generator cannot accidentally produce children through the legacy
plan path.

## Legacy Plan Reuse

The slice correctly avoids reusing an existing legacy `ChildPlan` when
`candidate-generator=tui-edit-surface` is requested. This is necessary because
the current child-plan format does not carry edit-surface check/delta evidence.

That behavior matches the model: proposal storage or existing plan files are
not authority unless they carry admitted transition evidence. Reusing them would
silently bypass the requested generator.

## Authority And Projection

The slice does not grant authority to `ploke-tui`, projection files, or existing
child-plan state. It introduces only CLI selection and a non-live dispatch
boundary.

The blocker message correctly names the next authority-bearing operation:

```text
materialize edit -> recompute derived Artifact identity and touched hashes
-> construct backend-owned post-apply Artifact
-> validate with edit_surface::tui::Apply::validate
-> convert checked ArtifactDelta into child materialization evidence
```

That is aligned with the edit-surface model. The TUI harness may propose and
apply as an executor, but `ploke-eval` must own the checked Artifact transition
before any child runtime is hydrated.

## Test Adequacy

Current tests cover:

- CLI parsing of explicit `tui-edit-surface` and `ploke-tui-tools`;
- default candidate generator remains `legacy`;
- config dispatch selects legacy or TUI edit-surface path;
- TUI edit-surface errors are surfaced as fail-closed `PrepareError` values;
- checked edit-surface carriers still cannot materialize child files without
  backend-owned after-Artifact validation;
- existing child-plan reuse is represented as an explicit error.

This is adequate for the non-live first slice.

Before the next integration slice or any live run, add a filesystem-level
`resolve_child_plan` test that creates an existing child-plan file and verifies
that `candidate-generator=tui-edit-surface` rejects it without calling
`receive_existing_child_plan`. Also add a no-child-plan test that exercises the
actual async dispatch path and proves the failure occurs before any child-plan
file is written.

## Validation

Bounded checks run:

```bash
cargo check -p ploke-eval 2>&1 | tail -n 80
cargo test -p ploke-eval candidate_generation --lib 2>&1 | tail -n 40
cargo test -p ploke-eval tui_edit_surface --lib 2>&1 | tail -n 60
cargo test -p ploke-eval checked_edit_surface_candidates --lib 2>&1 | tail -n 60
cargo test -p ploke-eval loop_prototype1_state --lib 2>&1 | tail -n 50
```

All passed. The package still emits existing warnings during check/test.

## Commit Safety

Safe to commit as a non-live first integration slice.

Do not treat this as live-loop ready. The next backend slice must provide the
backend-owned derived Artifact identity, touched after-hashes, checked
`ArtifactDelta`, and conversion into current child materialization evidence
before `tui-edit-surface` may publish a `ChildPlan`.
