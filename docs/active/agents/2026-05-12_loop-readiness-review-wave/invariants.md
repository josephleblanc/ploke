# Prototype 1 Invariants Review

Worker: `loop-invariants-reviewer`
Date: 2026-05-12

Scope: recent Prototype 1 structural-carrier and broad-harness changes, compared against:

- `crates/ploke-eval/src/cli/prototype1_state/mod.rs`
- `docs/workflow/evalnomicon/drafts/runtime/loop.md`
- `docs/design/drafts/edit-surface/hyperagents-prompt-posture.md`
- doc comments in `crates/ploke-eval/src/cli/prototype1_state/history.rs`

Changed files reviewed:

- `crates/ploke-eval/src/cli/prototype1_state/backend.rs`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_result.rs`
- `crates/ploke-eval/src/cli/prototype1_state/history.rs`
- `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs`
- `crates/ploke-eval/src/cli/prototype1_state/parent.rs`

## 2026-05-13 Update

Commit `bd00056b Add broad harness request fanout` supersedes the high-severity
finding below that default `BroadHarnessRequest` complete runs reject before
child planning. Complete mode now has a request-bound continuation path:

```text
Parent<Ready>
  -> HarnessRequestBatch
  -> headless ploke-tui attempt per request slot
  -> SubmittedBroadHarnessResult
  -> AdmittedBroadHarnessResult
  -> ChildPlan when admitted.len() >= child_budget.min
```

The structural concerns about verified publication loading, durable grant
projection shape, and compatibility aliases remain relevant. The live broad
path is runnable, but it is still a prototype workspace-diff admission path, not
the full formal `SurfaceGrant` / `CheckedProposal` end state.

## Findings

### High: default `prototype1-state` is not ready for a long complete loop

The CLI default candidate generator is now `BroadHarnessRequest`:

- `crates/ploke-eval/src/cli.rs:634-636`

But complete live runs explicitly reject that generator:

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:745-754`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:8171-8174`

The rejection text says the broad-harness request path is pending until a typed request-to-child-plan receipt is implemented. The active loop docs expect a generation to synthesize descendants, record candidates, evaluate children, compare outcomes, and hand off to one successor:

- `docs/workflow/evalnomicon/drafts/runtime/loop.md:145-179`

For a 15-generation / 128-node run, this means the default command shape is not operationally ready. A complete run must either select `deterministic-tui-tools` explicitly or land the missing broad-harness receipt/child-plan transition first.

### High: published request hashes are stored, not verified by a load transition

The new structural carrier is a real improvement: `request::Request<request::Broad, request::Published>` carries kind and state, and `Parent<AwaitingHarnessPlan>` stores a `request::Reference<request::Broad, request::Published>`:

- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:210-224`
- `crates/ploke-eval/src/cli/prototype1_state/parent.rs:44-51`
- `crates/ploke-eval/src/cli/prototype1_state/parent.rs:981-1003`

However, `Request<K, S>` derives `Deserialize`, stores `request_hash` as a normal field, and exposes the request fields crate-wide:

- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:210-224`

The hash is computed only by constructor/update paths:

- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:1040-1050`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:1090-1110`

Submitted-result verification compares the submitted binding to the stored published request, but does not require a verified `Published` load state:

- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_result.rs:129-188`
- `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1338-1364`

This leaves request publication closer to a deserialized record with a hash field than a verified publication carrier. It is not yet History authority, but it is on the broad harness authority path. The History docs explicitly call for authoritative carriers to serialize without treating deserialization as an implicit constructor:

- `crates/ploke-eval/src/cli/prototype1_state/history.rs:325-329`

Smallest invariant-preserving direction: add an explicit verified-load transition for request publications, or manually deserialize through a record type and mint `Request<Broad, Published>` only after recomputing and checking the preimage hash.

### Medium: grant records still erase the checked/admitted discriminant at the durable boundary

The active grant carriers are structurally better than the prior flattened names:

- `grant::Grant<grant::Checked>`
- `grant::Grant<grant::Admitted>`
- `grant::Coordinate<grant::Checked>`
- `grant::Coordinate<grant::Admitted>`

Definitions:

- `crates/ploke-eval/src/cli/prototype1_state/history.rs:2925-2945`
- `crates/ploke-eval/src/cli/prototype1_state/history.rs:3057-3136`

But the durable coordinate sum remains `#[serde(untagged)]`:

- `crates/ploke-eval/src/cli/prototype1_state/history.rs:2950-2955`

The underlying record structs do not deny unknown fields:

- `crates/ploke-eval/src/cli/prototype1_state/history.rs:2960-2970`

That makes the serialized boundary less explicit than the active typestate. A mixed or future-expanded coordinate shape can be accepted by the first matching untagged variant, which is exactly where durable records should be projections of allowed transitions rather than ambiguous bags. This conflicts with the History doc direction that sealed/persisted forms should remain transition projections and not upgrade convenience records into authority:

- `crates/ploke-eval/src/cli/prototype1_state/history.rs:203-208`
- `crates/ploke-eval/src/cli/prototype1_state/history.rs:325-329`

Smallest direction: use an explicitly tagged `grant::record::Coordinate` representation, or add `deny_unknown_fields` plus a verified record-to-carrier transition.

### Medium: broad-harness admission exists, but there is no live request-to-child-plan continuation yet

`GitWorktreeBackend::admit_submitted_broad_harness_result` validates request binding, live admission binding, workspace isolation, clean source tree, stale base, surface policy, and then persists candidate changes:

- `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1338-1441`

That matches the hyper-agent posture: the harness submits evidence only, and `ploke-eval` admits or rejects before minting a stronger loop object:

- `docs/design/drafts/edit-surface/hyperagents-prompt-posture.md:13-21`
- `docs/design/drafts/edit-surface/hyperagents-prompt-posture.md:87-93`

But the live parent-selection path still returns a pending request instead of consuming a submitted result into a child plan:

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6226-6235`

And broad-harness existing child plans are rejected because there is no typed receipt binding request identity, policy, objective, and surface evidence:

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:757-762`

This is structurally honest, but it is a blocker for the long loop path described in the runtime loop doc:

- `docs/workflow/evalnomicon/drafts/runtime/loop.md:181-210`

### Low: compatibility aliases keep flattened vocabulary on the active path

The request carriers exist, but two flattened names remain as crate-visible aliases:

- `PublishedBroadHarnessRequest`
- `RequestAdmissionBinding`

Aliases:

- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:29-33`

Active call sites still import or name those aliases:

- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_result.rs:5-8`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1232-1244`
- `crates/ploke-eval/src/cli/prototype1_state/backend.rs:23-24`

Because these are type aliases, the compiler still sees the structural carrier underneath. The risk is process drift: future code can keep treating the flattened alias as the semantic object. This is not an immediate correctness failure, but it weakens the structural naming gate as a feedback mechanism.

## Positive Findings

- `Parent<AwaitingHarnessPlan>` now stores a typed `request::Reference<request::Broad, request::Published>` instead of a locally flattened published-request identity. See `parent.rs:44-51`.
- `RequestAdmissionBinding` now aliases `request::Binding<surface::SurfacePolicyId>`, and construction from live `EditSurfaceAdmission` checks the coordinate target artifact. See `harness_request.rs:398-435`.
- The checked surface backend now mints `grant::Grant<grant::Checked>` from the applied authority plus checked transition before producing `CheckedSurface`. See `backend.rs:1297-1320`.
- Candidate binding refines checked grant evidence to admitted grant evidence through `Grant<Checked>::admit(Grant<Admitted>)`, with policy, writable target, target artifact, and runtime identity checks. See `history.rs:3090-3136` and `history.rs:3578-3605`.
- The broad harness prompt posture generally matches the HyperAgents design: evidence roots, broad outside-protected-core edits, protocol diagnoses as guidance, and submitted result as non-authoritative evidence. See `harness_request.rs:747-765` and `harness_request.rs:866-925`.
- The structural naming tripwire passes for `prototype1_state`.

## Open Questions

- Is `BroadHarnessRequest` intended to remain the CLI default while complete live runs reject it? If yes, long-loop runbooks need to say that `--candidate-generator deterministic-tui-tools` is currently required for complete autonomous runs.
- Should `request::Request<Broad, Published>` be loadable only through a verified `record -> checked publication` transition?
- Should `grant::AnyCoordinate` be changed to a tagged record now, before more persisted grant records exist?
- Should compatibility aliases be removed from active call sites now that the structural carriers exist?

## Residual Risk

Historical pre-`bd00056b` assessment: the change reviewed here was
directionally aligned with the stated invariants, but it was not yet ready as
the default path for a 15-generation / 128-node complete loop. At that time the
deterministic TUI path was still the runnable complete path, and broad harness
paused at request publication with no typed request-to-child-plan continuation.

Current assessment after `bd00056b`: broad harness is a runnable complete-mode
prototype path, but the remaining structural risks are verified publication
loading, durable grant projection shape, and convergence with the formal
`SurfaceGrant` / `CheckedProposal` proof spine.

The highest structural residual risk is persisted evidence loading: request publications and grant coordinates can still be deserialized as if they were already valid active carriers. That should be treated as a verification-boundary gap before relying on broad harness artifacts in longer runs.

## Verification

- `cargo check -p ploke-eval 2>&1 | tail -n 80`: passed; warnings only.
- `cargo test -p ploke-eval broad_harness_ 2>&1 | tail -n 80`: passed; 13 tests.
- `cargo test -p ploke-eval edit_surface 2>&1 | tail -n 80`: passed; 76 tests.
- `cargo run -p xtask -- check structural-naming --report-only crates/ploke-eval/src/cli/prototype1_state 2>&1 | tail -n 80`: passed; 49 Rust files checked.

Report changed files:

- `docs/active/agents/2026-05-12_loop-readiness-review-wave/invariants.md`

Blockers:

- Long complete loop readiness is blocked by the default `BroadHarnessRequest` generator being rejected for complete live runs until typed broad-harness request-to-child-plan receipt wiring exists.
- Request publication verification is blocked by direct deserialization into `Request<Broad, Published>` without a verified-load transition.
- Grant record durability is blocked by the untagged checked/admitted coordinate projection.
