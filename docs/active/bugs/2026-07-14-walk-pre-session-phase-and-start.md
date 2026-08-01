# Walk Pre-Session Phase and Start Contract

Status: fixed, regression-covered, and live verified.

Canary worktree:

```text
/home/brasides/.ploke-eval/worktrees/p1-stage4-observe-g35f-oaiembed-3g1x3-p3-20260714-004446
```

## Broken Contract

A completed Prototype 1 setup may strictly reconstruct typed bootstrap state at
R3 through R4c before any durable controller session exists. Walk status must
show both facts without conflating them:

- the reconstructed phase is operator-visible evidence;
- the empty `SessionVersion` is the exact pre-claim mutation guard;
- the only forward mutation is `Start` targeting the durable R3 session
  origin;
- `Step` and `Reset` do not exist until a session has been claimed.

The first guarded `Start` must replace the pre-session read cache only after the
empty durable version successfully claims R3.

## Evidence

After fresh setup, `walk show --format json` reconstructed the canary at R4c,
but `walk status --format json` returned this contradictory typed projection:

```text
rendered phase: r4c - parent ready
snapshot.phase: empty
snapshot.version: empty
snapshot.controller_attached: true
advertised edge: start_to_r0
```

`walk session-history` confirmed that no session, cursor, or controller journal
existed. The canary was stopped without advancing so that the mismatch remained
preserved as evidence.

## Source Trace

`serve_prepared` refreshes `WalkController` from strict setup/filesystem
evidence before serving. The regression introduced in `ce465c06a` then made
`status_response` derive its phase only from the durable session version. An
empty version therefore erased the reconstructed phase. `actions_for` also used
`controller_attached` as a proxy for session existence and projected a bootstrap
edge from `Empty`.

The same conflation blocked control: `WalkController::start_at` rejected the
pre-session reconstructed cache as an already-started walk before attempting
the exact empty-version claim.

The source boundaries are:

- `crates/ploke-eval/src/cli/prototype1_state/walk/server.rs`
  `status_response` and `actions_for`;
- `crates/ploke-eval/src/cli/prototype1_state/walk/controller.rs`
  `start_at`;
- `crates/ploke-eval/src/cli/prototype1_state/driver/control.rs`
  `validate_fresh_session` and `has_loop_effects`.

## Fix

Protocol v9 replaces the ambiguous phase/version pair with one validated
`WalkPosition` while retaining compatibility fields for older readers:

- `NoSession` means no durable cursor or reconstructed position is available;
- `Reconstruction { phase }` identifies strict pre-session evidence;
- `Unpositioned { version }` preserves a durable session identity whose journal
  has no committed cursor;
- `Session { version }` identifies the exact committed controller cursor;
- `Legacy { phase, version }` exists only when decoding a protocol-v8 status
  that omitted position authority.

Every v9 status must carry a non-legacy position whose phase and version agree
with the compatibility fields. Health and Show perform lightweight protocol
negotiation before returning structured status, while mutation requests still
carry the full checkout epoch. A session position additionally rejects an empty
cursor phase or noncanonical cursor evidence at serialization and decode time.

The server caches the complete controller observation captured while holding
the controller lock, including phase, attachment, blockers, and fresh-session
admission. Cache access remains bounded and nonblocking; contention cannot
claim that a controller is attached. Action derivation considers durable
session existence separately from controller attachment. Bare checkouts and
pre-session R5+ loop effects fail closed with disabled Start rather than
projecting a bootstrap edge. Recover is exposed only for a durable session with
a committed cursor and no indeterminate supervised job. A direct cursorless
recovery request is rejected before an operation record can be persisted, and
an indeterminate job must be explicitly resolved before session recovery can be
advertised or admitted.

The same fresh-session check now runs again in `Start` admission after exact
version comparison and before allocating or persisting an operation. Claim-time
validation remains in place for time-of-check/time-of-use safety.

`start_at` admits only an exact empty-version reconstruction at R3, R4a, R4b,
or R4c and delays clearing that read cache until session claim and
reconstruction have succeeded.

`ploke-walk-ui` renders `NO SESSION`, `RECONSTRUCTED`, `NO CURSOR`, `SESSION`,
and `LEGACY` as distinct source badges. A non-status response invalidates the
complete status snapshot instead of combining stale actions or blockers with a
newer response. Run selection generation-tags in-flight walk and query requests
so a reply from the prior run cannot repopulate the newly selected run.

## Docs and Policy Expectation

`docs/active/agents/2026-07-13_prototype1-loop-operator-control-plan.md`
defines completed setup/identity authority as the read-only bootstrap history
from which the mutable controller session begins at R3. It also requires the UI
and CLI to consume one typed snapshot without parsing prose.

## Regression Coverage

Focused production-backed setup coverage:

```text
cargo test -p ploke-eval fresh_setup_reconstruction_claims_first_walk_session_at_r3 -- --nocapture
```

Focused nonblocking status coverage:

```text
cargo test -p ploke-eval pre_session_health_uses_cached_reconstruction_while_controller_is_locked -- --nocapture
cargo test -p ploke-eval indeterminate_status_uses_durable_phase -- --nocapture
cargo test -p ploke-eval indeterminate_session_status_does_not_advertise_recover -- --nocapture
cargo test -p ploke-eval contended_cache_does_not_claim_an_attached_controller
```

Focused admission and compatibility coverage:

```text
cargo test -p ploke-eval start_rejects_failed_fresh_admission_before_operation_persistence
cargo test -p ploke-eval recover_without_session_rejects_before_operation_persistence
cargo test -p ploke-eval cursorless_v1_status_serializes_through_the_production_store
cargo test -p ploke-eval reconstructed_start_failure_preserves_controller_evidence
cargo test -p ploke-eval status_without_position_is_accepted_only_from_an_older_protocol
cargo test -p ploke-eval snapshot_rejects_session_position_with_empty_cursor_phase
cargo test -p ploke-eval snapshot_rejects_session_position_with_invalid_cursor_evidence
cargo test -p ploke-walk-ui
```

These focused tests pass. The production-backed setup test proves exact R4c
reconstruction followed by an R3 claim-and-release with a nonempty session ID,
nonzero journal revision, no transition edges, and a persisted inactive session
journal. The admission regression drives the production server request path and
proves rejection leaves no active job or durable operation record. The frozen
v8 wire regression exercises production decode and CLI JSON rendering without
permitting an explicit legacy position on v9.

No real protocol-v1 controller journal survives in the fixture corpus. The v1
compatibility regression therefore writes the supported cursorless
Created-plus-Acquired shape through the canonical journal serializer, then
reads it through the production Store, controller refresh, status response, and
v9 serializer. It is persisted compatibility coverage, not a historical replay
claim.

## Live Validation

Commit `5c9745b2a` (`Expose authoritative Prototype 1 walk state`) was built
before the preserved canary was advanced. Protocol-v9 status then reported
R4c from `position.source=reconstruction`, the exact empty session version, and
only the enabled `Start -> R3` controller mutation.

Guarded operation `9f994800-4e28-4f9d-8c13-7e9d7c0d0001` successfully claimed
session `7835dfe6-8ef5-437d-87f1-7a8e98c36663` at R3. The committed cursor has
canonical evidence
`c5e308e82010f86bcc0c97f56a43550a8e0465e585057bebc733ab69f06fd061`
and journal revision 3. Session history contains exactly v5 `Created`,
`Acquired(fence 1)`, and `Released(fence 1)` events, with no damage or
abandonment. Settled status exposes only the R3-to-R4a step as the forward
typestate action.

A second raw protocol-v9 Start request reused the old empty version under
operation `9f994800-4e28-4f9d-8c13-7e9d7c0d0002`. The server rejected it with
`stale_version`, returned the exact current R3 session version, and created no
durable operation record. The server then stopped cleanly. Doctor remained at
`baseline_eval` with no blockers, closure counts and timestamp were unchanged,
and the canary checkout remained clean at
`fa2b9149b7932504f3e39169c95d57125a5dad39`.
