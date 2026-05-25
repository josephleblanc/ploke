# Prototype 1 Crown/Handoff Typestate Review

Date: 2026-04-29

Scope: Rust type and visibility barriers around Prototype 1 Crown, History, successor invocation, and handoff. Inspected:

- `crates/ploke-eval/src/cli/prototype1_state/history.rs`
- `crates/ploke-eval/src/cli/prototype1_state/inner.rs`
- `crates/ploke-eval/src/cli/prototype1_state/parent.rs`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
- `crates/ploke-eval/src/cli/prototype1_process.rs`
- `crates/ploke-eval/src/cli/prototype1_state/invocation.rs`

## Summary

The recent direction is visible in the main handoff path: `spawn_and_handoff_prototype1_successor` takes `Parent<Selectable>` and calls `parent.lock_crown()` before `spawn_prototype1_successor` (`crates/ploke-eval/src/cli/prototype1_process.rs:859`, `crates/ploke-eval/src/cli/prototype1_process.rs:873`, `crates/ploke-eval/src/cli/prototype1_process.rs:915`). `Parent<S>` and `Crown<S>` are not `Clone` or `Copy`, and `Block<Open>::seal` is private behind `Crown<Locked>::seal` (`crates/ploke-eval/src/cli/prototype1_state/history.rs:1084`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1218`).

However, the current APIs do not make it hard enough to accidentally spawn or admit a successor outside the intended handoff transition. The strongest barriers are local to `Parent<Selectable> -> Parent<Retired>` and `Crown<Locked> -> Block<Sealed>`, but the executable successor surface is crate-visible, cloneable, file-deserializable, and validated against mutable scheduler state rather than a Crown/History handoff proof.

## Findings

### High: `Crown<Locked>` can be forged inside `prototype1_state` without `Parent<Selectable>`

`Crown<S>::for_lineage` is generic over `S` and `pub(super)` (`crates/ploke-eval/src/cli/prototype1_state/inner.rs:74`). Because `parent.rs` can call it from a sibling module (`crates/ploke-eval/src/cli/prototype1_state/parent.rs:430`), other modules under `prototype1_state` can also mint `Crown::<crown::Locked>::for_lineage(...)` directly. `Crown<crown::Ruling>::lock` is also `pub(crate)` (`crates/ploke-eval/src/cli/prototype1_state/inner.rs:88`).

Misuse path: a future agent working in `cli_facing`, `history_preview`, or another `prototype1_state` sibling can construct a locked Crown without moving a `Parent<Selectable>` into `Parent<Retired>`, then call `Crown<Locked>::seal` (`crates/ploke-eval/src/cli/prototype1_state/history.rs:1224`). That violates the claim that the live handoff path is the only way to produce locked lineage authority.

Recommendation: remove the generic externally visible constructor. Do not expose `Crown<S>::for_lineage` to the whole `prototype1_state` subtree. Co-locate Crown construction with the parent transition, or make `Parent` carry a private `Crown<crown::Ruling>`/lineage capability that is transformed by `Parent<Selectable>::lock_crown`. Avoid a generic constructor that can instantiate advanced states.

### High: successor invocation authority is crate-visible and does not require a locked Crown or retired Parent

`SuccessorInvocation` is a cloneable authority wrapper (`crates/ploke-eval/src/cli/prototype1_state/invocation.rs:113`) with a `pub(crate) fn new` constructor (`crates/ploke-eval/src/cli/prototype1_state/invocation.rs:218`), `pub(crate)` launch args (`crates/ploke-eval/src/cli/prototype1_state/invocation.rs:269`), and a `pub(crate)` writer (`crates/ploke-eval/src/cli/prototype1_state/invocation.rs:340`). None of those APIs requires `Parent<Retired>`, `Crown<Locked>`, or a handoff transition witness.

Misuse path: any crate code can build a `SuccessorInvocation`, write it, call `launch_args`, and use `std::process::Command` to run `loop prototype1-state --handoff-invocation ...`. The private `spawn_prototype1_successor` helper helps concentrate the intended process spawn (`crates/ploke-eval/src/cli/prototype1_process.rs:752`), but the public invocation API exposes all data needed to bypass that helper.

Recommendation: make executable successor invocation creation a projection of the handoff transition. For example, return a move-only `Successor<Spawnable>` token from `Parent<Selectable>::lock_crown` or from a combined handoff transition, and require that token to create/write launch args. Remove `Clone` from authority wrappers or separate cloneable persisted data from move-only executable authority.

### High: handoff invocation loading accepts file authority instead of a typed Crown/History proof

The persisted `Invocation` is fully deserializable with public fields (`crates/ploke-eval/src/cli/prototype1_state/invocation.rs:91`). `load_executable` just loads and classifies the file (`crates/ploke-eval/src/cli/prototype1_state/invocation.rs:327`). The successor CLI path accepts a `--handoff-invocation`, checks campaign/node/repo-root (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3050`), then validates only the mutable scheduler continuation decision (`crates/ploke-eval/src/cli/prototype1_process.rs:377`).

Misuse path: a developer or external process can write a JSON invocation with role `successor`, matching campaign/node/root, and start `prototype1-state --handoff-invocation` manually. If `scheduler.last_continuation_decision` still names the node branch as `ContinueReady`, `validate_prototype1_successor_continuation` accepts it (`crates/ploke-eval/src/cli/prototype1_process.rs:388`). No sealed block, Crown lock record, predecessor runtime id, or retired-parent evidence is required.

Recommendation: bind successor admission to an attempt-scoped handoff record produced by the predecessor transition. The successor should verify a sealed predecessor block or at least a durable handoff proof that includes parent runtime id, selected node, active artifact, expected generation, invocation runtime id, and Crown-lock evidence. `load_executable` should validate schema/version and role-specific invariants, not merely classify JSON.

### Medium: `Parent<Selectable>` can be reached through a replay-style child-plan receive path

`Parent<Ready>::planned_from_locked_child_plan` is `pub(crate)` and converts a ready parent to `Parent<Planned>` without consuming a `Locked<ChildPlan>` witness (`crates/ploke-eval/src/cli/prototype1_state/parent.rs:407`). The live replay path then separately loads `Locked<ChildPlan>` from a file (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:397`) and unlocks it (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:429`).

Misuse path: crate code can manufacture `Parent<Planned>` from any checked/ready parent, then combine it with a separately loaded or stale child-plan message to reach `Parent<Selectable>`. The child plan validates parent node and generation (`crates/ploke-eval/src/cli/prototype1_state/parent.rs:131`), but it is not tied to the current parent runtime attempt or Crown epoch.

Recommendation: make the child-plan receive transition a single method that consumes both `Parent<Ready>` and `Locked<ChildPlan>` and returns `(Parent<Selectable>, Received<ChildPlan>)`. If replay is intended, encode replay as an explicit transition with attempt/generation/epoch checks rather than a public status cast.

### Medium: ready acknowledgement can be forged and is not content-checked by the predecessor

The parent waits for the ready path and only deserializes the record, discarding its contents (`crates/ploke-eval/src/cli/prototype1_process.rs:826`). `write_successor_ready_record` is `pub(crate)` and the ready record fields are public (`crates/ploke-eval/src/cli/prototype1_state/invocation.rs:349`, `crates/ploke-eval/src/cli/prototype1_state/invocation.rs:410`). The successor CLI writes ready before entering the parent turn (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3110`).

Misuse path: crate code can write a ready file directly, or an external process can create a syntactically valid file at the expected path. The predecessor will treat the handoff as acknowledged even if the intended successor process did not write the record. This does not itself spawn another runtime, but it weakens the durable evidence around the handoff boundary.

Recommendation: validate the loaded ready record against expected campaign id, node id, runtime id, and, where possible, successor pid. Restrict the ready writer to the successor acknowledgement transition. Prefer a signed or hash-linked ready record once sealed History is wired.

### Medium: side effects make successor runtime material available before Crown lock

`spawn_and_handoff_prototype1_successor` prepares the successor runtime before it locks the Crown (`crates/ploke-eval/src/cli/prototype1_process.rs:867`, `crates/ploke-eval/src/cli/prototype1_process.rs:873`). Preparation validates continuation, installs the selected artifact into the active checkout, and builds the successor binary (`crates/ploke-eval/src/cli/prototype1_process.rs:461`). The module docs say the parent locks lineage authority before the successor process is spawned (`crates/ploke-eval/src/cli/prototype1_process.rs:43`), which is true, but they also say the predecessor must not remain ruling-capable after the successor runtime is executable (`crates/ploke-eval/src/cli/prototype1_process.rs:74`).

Misuse path: during the window after install/build and before `lock_crown`, the active checkout and binary can represent the selected successor while the in-memory parent has not moved to `Parent<Retired>`. This is probably a narrow operational window in the current function, but it contradicts the stronger wording and gives future edits a place to insert accidental process launch or additional lineage mutation before retirement.

Recommendation: either narrow the comments to the implemented guarantee, or split the transition so installation/building and spawnability have distinct typed states. A stronger model would make the artifact install produce a `Successor<Prepared>` value, then require `Parent<Selectable> -> Parent<Retired>` plus that value to produce `Successor<Spawnable>`.

## Positive Barriers

- `Parent<S>` fields are private and `Parent<S>` is not `Clone` or `Copy` (`crates/ploke-eval/src/cli/prototype1_state/parent.rs:49`).
- `Crown<S>` fields are private and `Crown<S>` is not `Clone` or `Copy` (`crates/ploke-eval/src/cli/prototype1_state/inner.rs:60`).
- The intended live handoff function now requires `Parent<Selectable>` and returns `Parent<Retired>` (`crates/ploke-eval/src/cli/prototype1_process.rs:859`).
- `Block<Open>::seal` is private; crate callers must use `Crown<Locked>::seal` (`crates/ploke-eval/src/cli/prototype1_state/history.rs:1084`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1224`).
- History docs accurately disclose that live handoff still does not persist or verify sealed blocks before successor admission (`crates/ploke-eval/src/cli/prototype1_state/history.rs:104`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:193`).

## Bottom Line

It is not yet hard enough to accidentally spawn a successor runtime outside the intended handoff transition. The handoff function itself crosses `Parent<Selectable> -> Parent<Retired>` before calling the private spawn helper, but the surrounding APIs allow the same executable successor surface to be constructed from a plain invocation file and mutable scheduler state. The most important follow-up is to make executable successor authority a move-only projection of the Crown-lock transition, rather than a cloneable crate-visible JSON wrapper.
