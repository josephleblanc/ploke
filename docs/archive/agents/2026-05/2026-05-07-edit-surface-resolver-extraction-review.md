# Edit Surface Resolver Extraction Review

Date: 2026-05-07

Task title: Review stage-free TUI resolver extraction and eval-side conversion

Task description: Review the current uncommitted resolver extraction in
`ploke-tui`, the resolver tests, and the `ploke-eval` conversion from resolved
TUI writes into authority-side edit proposals.

Related planning files:

- `AGENTS.md`
- `docs/archive/agents/2026-05/2026-05-07-prototype1-edit-surface-implementation-plan.md`
- `docs/archive/agents/2026-05/2026-05-07-edit-surface-backend-bridge-review.md`
- `crates/ploke-tui/src/rag/tools.rs`
- `crates/ploke-tui/src/rag/tests/apply_code_edit_tests.rs`
- `crates/ploke-eval/src/cli/prototype1_state/backend.rs`

## Findings

No blocking findings.

The resolver extraction is stage-free. `apply_code_edit_tool` still performs
parse-failure and empty-request handling, then delegates only concrete
resolution to `resolve_code_edit_request`, and all proposal mutation remains in
`stage_semantic_edit_proposal` (`tools.rs:622-657`). The new resolver returns
`Vec<WriteSnippetData>` or `ToolError`; it reads path context and queries the DB,
but it does not write the proposal store, emit tool events, create previews, or
approve/apply edits (`tools.rs:660-898`). The added TUI tests check success and
failure paths without proposal-store mutation.

Existing `apply_code_edit_tool` behavior is preserved for the reviewed paths.
The extracted code keeps the previous splice and canonical resolution flow,
continues to ignore `Patch` entries before staging so patch-only requests still
fall through to the existing "No edits provided" staging rejection, and routes
resolver failures back through `tool_call_failed_error` (`tools.rs:649-653`,
`tools.rs:892-898`). The full `apply_code_edit_tests` filter passes, covering
canonical success, not-found, wrong-type, duplicate request detection, preview
generation, auto-confirm, and multiple-file behavior.

No UI proposal state or auto-apply state leaks into authority. The eval bridge
still creates the `tui::Proposal::stage` value inside the backend validation
path with `auto_apply: false`, obtains a `surface::Grant` check, validates the
reported writes against a backend-built after artifact, and only then extracts
the applied delta (`backend.rs:1007-1062`). This keeps proposal storage and UI
state as harness projection/evidence rather than authority.

The eval-side conversion uses backend-owned content hashes, not
`TrackingHash`. `proposal_from_resolved_writes` explicitly strips each resolved
write to a repository-relative path, validates that relpath, reads the parent
checkout file, and fills `ProposedTouch::expected_file_hash` with
`content_hash(&source_content)` (`backend.rs:226-249`). The regression test
first proves that feeding the TUI `TrackingHash` directly is rejected as
`StaleEditBaseHash`, then proves the converted backend-owned hash validates.

Path normalization/security is preserved at both conversion and validation
boundaries. The converter rejects absolute writes outside `repo_root` via
`strip_prefix`, then calls `validate_normal_repo_relpath` before reading the
target (`backend.rs:235-239`, `backend.rs:262-273`). The validator independently
deduplicates touched paths, requires a normal repository-relative relpath, checks
the edit-surface allowlist, and only then reads from disk (`backend.rs:872-896`).
The prior prefix-with-`..` escape is covered by `validate_normal_repo_relpath`,
which rejects absolute paths, `.`, `..`, roots, and prefixes
(`backend.rs:1791-1810`).

## Residual Risks

The tests are adequate for committing this bridge slice, but live wiring should
add a couple of focused conversion-path cases: an absolute resolved write
outside `repo_root`, and a relative resolved write containing `..` passed
directly into `proposal_from_resolved_writes`. The underlying validation code is
already present; these would document the conversion boundary explicitly.

This is still a single-file text bridge, not durable whole-Artifact History
admission. That limitation matches the backend bridge review and should remain
visible when wiring the live producer.

## Test Evidence

Commands run:

```text
cargo test -p ploke-tui resolve_code_edit_request --lib 2>&1 | tail -n 40
cargo test -p ploke-tui apply_code_edit_tests --lib 2>&1 | tail -n 50
cargo test -p ploke-eval edit_surface_bridge --lib 2>&1 | tail -n 40
cargo test -p ploke-eval edit_surface_resolved_write_conversion --lib 2>&1 | tail -n 40
cargo check -p ploke-tui 2>&1 | tail -n 60
cargo check -p ploke-eval 2>&1 | tail -n 60
```

Results:

- `resolve_code_edit_request`: passed, 4 tests.
- `apply_code_edit_tests`: passed, 22 tests.
- `edit_surface_bridge`: passed, 8 tests.
- `edit_surface_resolved_write_conversion`: passed, 1 test.
- `cargo check -p ploke-tui`: passed with existing warnings.
- `cargo check -p ploke-eval`: passed with existing warnings.

## Commit Recommendation

Safe to commit and proceed to live wiring for the TUI proposal producer. The
live wiring should continue to treat `resolve_code_edit_request` output as
harness evidence and route all authority through `proposal_from_resolved_writes`
plus `validate_edit_surface_candidate`; it should not persist UI proposal state,
`auto_apply`, or `TrackingHash` as authority.
