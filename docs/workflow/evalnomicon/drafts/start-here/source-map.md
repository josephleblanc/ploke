# Source Map

Use this page to refresh the draft set against current source.

## Core Runtime And Authority Files

| File | Why it matters |
| --- | --- |
| [`mod.rs`](../../../../../crates/ploke-eval/src/cli/prototype1_state/mod.rs) | Conceptual model for Artifact, Runtime, Journal, History, Crown, Parent, Child, Successor, and intended handoff ordering. |
| [`history.rs`](../../../../../crates/ploke-eval/src/cli/prototype1_state/history.rs) | Local History invariant model, block sealing, lineage authority, Crown claims, and current claim boundaries. |
| [`run/core.rs`](../../../../../crates/ploke-eval/src/cli/prototype1_state/run/core.rs) | Current doctor, prompt, step, continue, diagnosis phases, phase advancement, and active control loop. |
| [`cli_facing.rs`](../../../../../crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs) | Setup, child planning, broad harness admission, child fanout helpers, selection, parent turn reports, and lower-level `prototype1-state`. |
| [`prototype1_process.rs`](../../../../../crates/ploke-eval/src/cli/prototype1_process.rs) | Child evaluation process seam, successor install/build/seal/spawn, ready wait, cleanup, and runner result validation. |
| [`profile.rs`](../../../../../crates/ploke-eval/src/cli/prototype1_state/profile.rs) | Admitted run-profile schema, validation, defaults, commitment, and protocol/search policy projection. |
| [`parent.rs`](../../../../../crates/ploke-eval/src/cli/prototype1_state/parent.rs) | Parent role states, startup evidence, child-plan message box, child file payloads. |
| [`invocation.rs`](../../../../../crates/ploke-eval/src/cli/prototype1_state/invocation.rs) | Attempt-scoped child and successor bootstrap contracts. |
| [`journal.rs`](../../../../../crates/ploke-eval/src/cli/prototype1_state/journal.rs) | Transition journal schemas and replay helpers. |
| [`identity.rs`](../../../../../crates/ploke-eval/src/cli/prototype1_state/identity.rs) | Artifact-carried parent identity file schema and path helpers. |

## Typed Child Transition Files

| File | Transition vocabulary |
| --- | --- |
| [`c1.rs`](../../../../../crates/ploke-eval/src/cli/prototype1_state/c1.rs) | `C1 -> C2`, materialize child Artifact: parent binary remains active while child Artifact world is realized. |
| [`c2.rs`](../../../../../crates/ploke-eval/src/cli/prototype1_state/c2.rs) | `C2 -> C3`, build child Runtime: promoted child binary appears. |
| [`c3.rs`](../../../../../crates/ploke-eval/src/cli/prototype1_state/c3.rs) | `C3 -> C4`, spawn child Runtime and observe readiness. |
| [`c4.rs`](../../../../../crates/ploke-eval/src/cli/prototype1_state/c4.rs) | `C4 -> C5`, observe terminal child result and treatment evidence. |

The `c*` files still carry scaffold warnings in their module headers. Treat
them as transition vocabulary and partially wired implementation, not as proof
that every old controller path has been replaced.

## Broad Harness And Edit Surface

| File | Why it matters |
| --- | --- |
| [`edit_surface/harness_request.rs`](../../../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs) | Typed broad-harness request, evidence roots, child evidence binding, prompt rendering. |
| [`edit_surface/harness_result.rs`](../../../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_result.rs) | Submitted/admitted harness result payloads. |
| [`edit_surface/tui_adapter.rs`](../../../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs) | Headless TUI attempt loop, tool/proposal observation, request admission, result summaries. |
| [`edit_surface/request_policy.rs`](../../../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/request_policy.rs) | Request policy and bounded edit-surface checks. |
| [`backend.rs`](../../../../../crates/ploke-eval/src/cli/prototype1_state/backend.rs) | Workspace backend abstraction and git worktree implementation. |

## Selection And Scoring

| File | Why it matters |
| --- | --- |
| [`selection.rs`](../../../../../crates/ploke-eval/src/cli/prototype1_state/selection.rs) | Selection material types and selected Artifact payloads. |
| [`successor.rs`](../../../../../crates/ploke-eval/src/cli/prototype1_state/successor.rs) | Successor records used in transition journal and handoff state. |
| [`score/`](../../../../../crates/ploke-eval/src/cli/prototype1_state/score/) | Operational/protocol scoring components and selection profiles. |

## Command Surfaces

Current operator-facing loop commands are described in
[`prototype1-loop-operator.md`](../../../../../crates/ploke-eval/docs/prototype1-loop-operator.md).
The key commands are:

- `ploke-eval loop prototype1-setup`
- `ploke-eval loop prototype1-doctor`
- `ploke-eval loop prototype1-prompt`
- `ploke-eval loop prototype1-step`
- `ploke-eval loop prototype1-continue`
- `ploke-eval loop prototype1-state`
- `ploke-eval loop prototype1-runner`
- `ploke-eval loop prototype1-harness`

Refresh this list with help output before writing operator instructions.

## Existing Docs To Cross-Check

| Doc | Use |
| --- | --- |
| [`crates/ploke-eval/docs/prototype1-loop-operator.md`](../../../../../crates/ploke-eval/docs/prototype1-loop-operator.md) | Practical command, phase, artifact, and implementation map. |
| [`crates/ploke-eval/docs/prototype1-run-profile.md`](../../../../../crates/ploke-eval/docs/prototype1-run-profile.md) | Admitted profile reference. |
| [`src/prototype1/index.md`](../../src/prototype1/index.md) | Canonical Evalnomicon Prototype 1 section. |
| [`src/prototype1/runtime-loop.md`](../../src/prototype1/runtime-loop.md) | Short runtime-loop summary. |
| [`src/prototype1/history-crown.md`](../../src/prototype1/history-crown.md) | Current book-level History/Crown summary. |
| [`src/prototype1/persistence-and-observability.md`](../../src/prototype1/persistence-and-observability.md) | Evidence-family and projection warning summary. |
| [`drafts/runtime/loop.md`](../runtime/loop.md) | Richer runtime-succession draft. |

## Refresh Commands

Use narrow source checks:

```bash
rg -n "enum DiagnosedPhase|fn diagnose|fn advance|DiagnosedPhase::" crates/ploke-eval/src/cli/prototype1_state/run/core.rs
rg -n "resolve_profile_child_plan|run_planned_child|select_successor_for_profile|live_successor_continuation_decision" crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs
rg -n "spawn_and_handoff_prototype1_successor|validate_prototype1_successor_continuation|record_prototype1_successor_ready" crates/ploke-eval/src/cli/prototype1_process.rs
rg -n "RUN_PROFILE_SCHEMA_VERSION|struct Prototype1RunProfile|fn validate" crates/ploke-eval/src/cli/prototype1_state/profile.rs
```

Refresh operator surfaces with:

```bash
./target/debug/ploke-eval loop prototype1-setup --help
./target/debug/ploke-eval loop prototype1-doctor --help
./target/debug/ploke-eval loop prototype1-step --help
./target/debug/ploke-eval loop prototype1-continue --help
```

