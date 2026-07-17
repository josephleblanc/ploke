# Prototype 1 R12 Policy Stops Routed to Successor Handoff

Status: source repaired; historical R12 replay and the serial `ploke-eval`
suite pass; fresh multi-generation live validation pending.

## Broken Contract

At R12, the selected candidate is a traversal coordinate, not successor-handoff
authority. The walk controller and driver must route from the continuation
disposition: policy stops go to R13a without checkout permission, while only a
`Continue*` disposition may enter R13b/R13c and mutate the active checkout.

## Evidence

The same incomplete controller contract appeared under multiple policy stops:

- Campaign `p1-dbdual-broad3g1x3-live-20260623-114924`: the continuation policy
  returned `StopHistoricalTraversalBudget`, but `walk` refused R13a because a
  candidate coordinate was present. See
  `docs/active/agents/2026-06-23_p1-dbdual-broad3g1x3-live-run/README.md`.
- Campaign
  `p1-v11-keeponly-g35f-oropenai-3g1x3-p3-20260716-185301`: the admitted
  `prototype1/run-profile.toml` sets
  `require_keep_for_continuation = true` and
  `explore_from_rejected = false`; the profile commitment SHA-256 is
  `7a92bef48096e563b4b1287207a006b320029f0bf88ca5314caa102900340275`.
  The selected child `node-cb1b41e21e01ddc7` completed, but
  `prototype1/evaluations/branch-aef83be6f4105a58.json` records
  `overall_disposition = reject` because `tool_calls_failed` regressed from 2
  to 3. The controller committed R12 with evidence
  `8092f1459627d8b6ddc9d838419e95be6dc1d346848eeaef2892d5e882c8030d`,
  then rejected the requested R13a target before admitting fence 19:

```text
target r13a is the stopped/no-selection branch, but R12 has
selected-successor evidence; use --until r13b or --until r14b with
--allow git-changes
```

The preserved incident artifacts are replayed from
`crates/ploke-eval/src/tests/fixtures/prototype1-r12-selected-reject-20260716/`.
The original run remains stopped at R12; no History block was sealed and the
selected candidate was not installed.

The related max-generation incident remains documented separately in
`2026-07-08-prototype1-max-generation-handoff.md`.

## Source Trace

Before repair:

1. `typestate/context.rs::Collected::has_successor_selection` correctly
   reported only that selection evidence existed.
2. `walk/controller.rs::{step_at, ensure_branch_target}` treated that evidence
   as proof that R13b was the valid branch.
3. `driver/control.rs::advance` independently required checkout permission for
   every selected R12.
4. `cli_facing.rs::live_successor_continuation_decision` and
   `live_edges.rs::r12_to_r13` already computed the semantic disposition and
   would have returned R13a for `StopSelectedBranchRejected`.

The repair extracts `preview_successor_continuation` as the shared read-only
authority calculation. R12 exposes that preview to the controller and driver;
only `disposition.allows_successor()` admits checkout mutation. The canonical
edge validates the handoff permit before recording a continuation decision or
selected-successor journal entry. Reconstruction also uses the preview instead
of the DB-writing live wrapper.

## Docs/Policy Expectation

- `crates/ploke-eval/docs/prototype1/run-profile.md` defines
  `require_keep_for_continuation` and the separate rejected-exploration opt-in.
- `crates/ploke-eval/docs/reference/knobs/rejected-successor-continuation.md`
  states that a selected rejected child normally stops with
  `StopSelectedBranchRejected`.
- `crates/ploke-eval/docs/appendices/prototype1-typestate-notebook.md` separates
  candidate selection from continuation authorization and reserves handoff for
  the `Continue*` dispositions.

No digest, History, checkout, profile, or strict-validation rule should be
weakened to make a policy stop advance.

## Current Repro Coverage

```text
cargo test -p ploke-eval r12_reject_replay -- --nocapture
cargo test -p ploke-eval generation_cap_stops_direct_child_handoff_at_max_generation -- --nocapture
cargo test -p ploke-eval historical_selection_rejects_exhausted_parent_turn_budget -- --nocapture
cargo test -p ploke-eval continuation_decision_stops_on_generation_limit_and_rejects_non_keep -- --nocapture
cargo test -p ploke-eval -- --test-threads=1
```

The historical replay loads the real strict profile, commitment, controller
journal, transition journal, child plan, terminal channel, selected child, and
reject evaluation. It drives the production R12 controller branch resolver and
driver edge with checkout permission disabled and proves:

- selection evidence remains present;
- the preview is `StopSelectedBranchRejected`;
- explicit and default operator targets resolve to R13a;
- R13b is rejected because continuation does not authorize handoff;
- a continuable rejected-exploration preview with an R13a permit fails before
  changing the owner DB bytes, transition journal, History, or checkout;
- the transition is R12 to R13a;
- exactly one stopped-successor record is appended;
- History remains absent; and
- Git HEAD and worktree status remain unchanged.

The other regressions preserve max-generation, historical-budget, and rejected
selection policy behavior. The serial crate suite passes with 1,373 tests, 0
failures, and 45 ignored tests. A parallel run exposed an existing shared
`PLOKE_EVAL_HOME` endpoint-lock interference; the isolated affected test and
the serial suite both pass.

## Missing Repro / Validation

Run a fresh strict live campaign from the repaired source. It must prove both
operator branches end to end:

1. a selected policy-stop candidate reaches R13a/R14a without
   `--allow-git-changes`; and
2. a later kept/continuable candidate reaches R13b/R14b only with explicit
   checkout authority, seals History, installs the exact candidate Artifact,
   and starts the successor runtime from the verified handoff.

## Fix Direction

Keep the continuation disposition as the sole R12 branch authority. Preserve
`has_successor_selection` for evidence/provenance queries, preserve all digest
and History validation, and fail before durable mutation when an authorized
handoff lacks the exact controller permit.
