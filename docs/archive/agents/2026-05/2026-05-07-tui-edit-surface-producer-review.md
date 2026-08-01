# TUI Edit Surface Producer Review

Date: 2026-05-07

Task title: Review live deterministic TUI edit-surface producer slice

Task description: Review the current uncommitted `cli_facing.rs` producer for
`candidate-generator=tui-edit-surface`, the producer note, and the preceding
edit-surface implementation/review docs. Focus on honesty and safety of the
deterministic direct-splice producer, child budget behavior, compile safety, UI
authority boundaries, resolver bypass, child plan projection, duplicate
handling, short-run readiness, and missing History evidence.

Related planning files:

- `AGENTS.md`
- `docs/archive/agents/2026-05/2026-05-07-prototype1-edit-surface-implementation-plan.md`
- `docs/archive/agents/2026-05/2026-05-07-edit-surface-resolver-extraction-review.md`
- `docs/archive/agents/2026-05/2026-05-07-edit-surface-backend-bridge-review.md`
- `docs/archive/agents/2026-05/2026-05-07-tui-edit-surface-producer-note.md`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
- `crates/ploke-eval/src/cli/prototype1_state/backend.rs`
- `crates/ploke-eval/src/cli/prototype1_state/identity.rs`

## Findings

### Medium: child budget authority still comes from ambient defaults

The live TUI producer is budget-bounded internally, but the parent-selection
entry point always supplies `Prototype1SearchPolicy::default().child_budget`
instead of a policy carried by the typed parent/handoff path
(`cli_facing.rs:697-711`). That means the producer defaults to `min=2, max=6`
(`scheduler.rs:45-54`) even when the setup/controller path was configured with
different child budget values (`cli_facing.rs:1192-1249`).

This matches the existing Prototype 1 state runtime compatibility bridge, which
also recovers fanout budget from defaults and carries a TODO saying budget
authority should come from bootstrap/handoff/History
(`cli_facing.rs:6837-6848`). It is therefore acceptable for a controlled
short-run, but the producer should not be described as honoring scheduler or
campaign child budget authority. It honors the ambient default unless the test
helper calls `publish_tui_edit_surface_child_plan` directly with an explicit
budget.

### Medium: compile-safe comment append is plausible, not proven by the producer

The direct-splice producer appends one EOF line comment to
`crates/ploke-tui/src/tools/code_edit.rs` (`cli_facing.rs:891-898`,
`cli_facing.rs:930-976`). For the normal initialized parent path, this is
probably compile-safe because parent node ids are deterministic
`node-<16 hex>` values (`scheduler.rs:256-260`) and the target file is valid
Rust before the splice.

The producer itself does not parse Rust, run a child compile, or escape the
interpolated seed before inserting it into source text. `ParentIdentity` is
loaded from JSON and `validate_for_command` checks schema/campaign/node
equality but not character safety (`identity.rs:89-122`, `identity.rs:143-161`).
Normal bootstrapping constrains the id through deterministic node-id checking
(`cli_facing.rs:6414-6443`), so this is not a blocker for the intended short
run. The honest claim is "comment-only deterministic splice expected to compile
on normal parent identities", not "compile-safe by construction".

### Medium: live child plans still lack durable edit-surface History evidence

The producer validates each `EditProposal` through
`GitWorktreeBackend::validate_edit_surface_candidate`, which builds
`surface::Grant`, `surface::Check`, a staged TUI proposal, applied writes, and
an `ArtifactDelta` before returning `CheckedSurfaceEdit`
(`backend.rs:859-1077`). The child projection then maps only selected
compatibility fields into `Prototype1NodeRecord`, `TreatmentBranchNode`, and
`ChildFiles`: base artifact id, patch id, derived artifact id, proposed content,
hashes, and producer id (`cli_facing.rs:493-567`, `cli_facing.rs:773-786`).

That is enough for current child materialization, but it is still not the
History model requested by the implementation plan. The plan expects durable
grant/bounds/proposal/run/touched-span/check/delta evidence tied to the
Artifact. The current child plan format still does not carry the full
`surface::Check`, `ArtifactDelta`, grant/bounds digests, or resolver/producer
run evidence as first-class History records. This should remain a visible
limitation before any long-running or audit-significant use.

### Low: bypassing `resolve_code_edit_request` is acceptable only because this is not claiming TUI resolver evidence

The earlier resolver review said live wiring should route TUI resolver output
through `proposal_from_resolved_writes` plus backend validation when it is using
resolved TUI writes. This producer does not use TUI resolver output at all. It
constructs backend `EditProposal` values directly with one normal repo-relative
target path, EOF byte span, backend-computed base hash, and no reported after
hash (`cli_facing.rs:930-976`).

That bypass is acceptable as an interim deterministic producer because
authority still flows through the backend surface validator, not UI proposal
state or rendered TUI storage. The limitation is semantic: the produced
candidates are not evidence that `resolve_code_edit_request` can resolve a live
LLM edit request, and they do not exercise `AppState`, proposal storage, preview
generation, auto-apply behavior, DB-backed canonical resolution, or duplicate
request detection from the TUI tool path.

### Low: child plan projection is structurally consistent but remains a compatibility projection

The live producer writes treatment evaluation projections before wrapping each
checked candidate back into `ChildFiles` (`cli_facing.rs:739-786`). It then
validates the generated child plan recipient, generation, non-emptiness,
direct-child relationship, and deterministic producer id before writing the
plan (`cli_facing.rs:821-872`). Tests cover that the published plan has three
children for an explicit `min=2,max=3` budget, that each child points to the TUI
target, and that node/request files exist (`cli_facing.rs:8934-8969`).

This is correct for the current legacy child-carrier bridge. It still means
checked edit-surface facts are projected into legacy node/branch fields rather
than admitted as a typed History transition.

### Low: duplicate handling is adequate for deterministic content dedupe

The direct producer dedupes proposed full file contents before validation
(`cli_facing.rs:950-976`), and the checked-candidate path dedupes by backend
proposed content hash after validation (`cli_facing.rs:899-925`). The focused
test proves duplicate replacements cannot satisfy a higher minimum
(`cli_facing.rs:8900-8932`). This is adequate for the deterministic splice
slice.

The dedupe is content-based, not semantic. Two different candidates can still
be meaningless variants that differ only by comment text. That is expected for
this interim producer and should be scored as a proposal-quality limitation, not
a safety issue.

## Non-Issues

No UI state authority leak was found in this slice. The producer imports backend
proposal carriers and constructs them directly; it does not read or persist
`AppState.proposals`, UI status, preview storage, `auto_apply`, or rendered TUI
proposal state. Backend validation stages a `tui::Proposal` with
`auto_apply: false`, checks it against a `surface::Grant`, and validates the
reported writes against a backend-owned after artifact before extracting a delta
(`backend.rs:1020-1059`).

The producer is honest about being deterministic and interim. The producer id is
`prototype1:tui-edit-surface:deterministic-v1` (`cli_facing.rs:714-715`), and
the inserted comment explicitly says the next step is replacing deterministic
direct-splice generation with LLM proposal production (`cli_facing.rs:891-898`).
The producer note also says it is a minimal parent-side producer and does not
claim LLM-driven generation.

Missing target behavior is fail-closed. If
`crates/ploke-tui/src/tools/code_edit.rs` is absent from the parent checkout,
the producer returns `MissingEditSurfaceTarget` before publishing a plan
(`cli_facing.rs:937-945`).

## Short-Run Readiness

This slice is acceptable for a controlled live short run of
`candidate-generator=tui-edit-surface` against `edit-surface=ploke-tui-tools`
when the goal is to exercise parent-side plan publication, bounded child fanout,
backend surface validation, and traversal plumbing with deterministic,
low-value edit candidates.

Do not use the resulting run as evidence that the full requested edit-surface
system is complete. It does not prove LLM proposal production, TUI resolver
resolution, durable History admission of checked edit-surface transitions,
budget authority carried through History/handoff, or compile success of every
generated child beyond the normal short-run build/evaluation path.

## Check Evidence

Commands run:

```text
cargo test -p ploke-eval tui_edit_surface --lib 2>&1 | tail -n 60
cargo check -p ploke-eval 2>&1 | tail -n 80
```

Results:

- `tui_edit_surface` filter passed: 4 tests.
- `cargo check -p ploke-eval` passed with existing warnings.

## Recommendation

Keep the slice as an interim deterministic producer if the next step is a short
observability run. Before calling the live `tui-edit-surface` path complete,
wire a real proposal source through the resolver/conversion bridge, carry child
budget authority from the active policy/History path, and persist checked
edit-surface evidence into History rather than only projecting it into legacy
child files.
