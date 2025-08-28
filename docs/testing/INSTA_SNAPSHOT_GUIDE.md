# Insta Snapshot Guide

Purpose
- Establish a clear, reliable workflow for authoring, reviewing, and maintaining visual snapshot tests using `insta` and `cargo-insta`.
- Complement semantic assertions with human-reviewed golden snapshots for critical TUI views.

When to Use Snapshots
- Critical UI: overlays (Approvals, Model Browser, Context Items), error banners, and compact textual reports.
- Snapshots are an addition to semantic assertions, not a replacement.

Requirements
- Dev-deps in crate under test (`Cargo.toml`):
  - `insta = { version = "*", features = ["redactions"] }`
  - `regex = "*"` for simple string normalization and path/UUID redaction.
- Tests must render into a fixed `Rect` with deterministic ordering to minimize churn.
- Redact non-deterministic content: UUIDs, timestamps, absolute paths.

Authoring Snapshots
- Write tests that:
  - Construct or reuse deterministic state.
  - Render the target view into a `ratatui::backend::TestBackend` with a fixed size.
  - Convert the buffer to a String (rows joined by newlines).
  - Apply redactions via `regex` or `insta` filters (UUIDs -> `<UUID>`, PWD -> `<PWD>`, optional timestamp masks).
  - Assert semantics (presence/ordering) and call `insta::assert_snapshot!("case_name", redacted_string);`.

Running Snapshot Tests
- First run (no existing .snap files):
  - `cargo test -p ploke-tui --test approvals_overlay_render`
  - Tests will fail with “unreviewed snapshot” and write new snapshot candidates.
- Review and approve:
  - `cargo insta review`
  - Inspect diffs; Accept to approve new snapshots (or reject/skip as appropriate).
  - Commit the `.snap` files generated under `crates/<crate>/tests/snapshots/`.
- Re-run tests:
  - `cargo test -p ploke-tui --test approvals_overlay_render`
  - Should pass now that snapshots are approved.

Updating Snapshots After UI Changes
- If a snapshot test fails due to legitimate UI changes:
  - Run `cargo insta review` to inspect diffs.
  - Classify the change:
    - Trivial UI changes (spacing, harmless wording, border glyphs): adjust the test redactions or stabilize layout/sorting if feasible, then approve.
    - Expected UX improvements (new labels, added data): update semantic assertions and approve snapshots.
    - Regressions (missing labels, broken ordering, truncated content): fix the code, re-run tests, review, and only approve once the behavior is correct.

Failure Triage
- Trivial failure indicators:
  - Minor whitespace shifts with identical content, changed box drawing characters, or path prefixes.
  - Redaction misses (raw UUIDs or PWDs leaking into snapshots).
- Critical failure indicators:
  - Missing or out-of-order key labels or counts.
  - Empty or truncated details area unintentionally.
  - Panics or failure to render.
- Underlying issue vs. UI tweak:
  - If semantics still pass but snapshots fail, likely a non-critical UI tweak; adjust redactions or approve after review.
  - If semantics and snapshots fail together, investigate logic/regression in the underlying view/state assembly.

Good Practices
- Keep snapshot names descriptive and tied to viewport size: `approvals_unified_80x24`.
- Keep redaction helpers local to each test file for clarity.
- Avoid over-redacting; retain meaningful content in snapshots.
- Prefer increasing semantic checks when failure diffs are ambiguous.

Command Reference
- Generate snapshots on test run: `cargo test` (fails on first run with new snapshots).
- Review and approve: `cargo insta review`.
- Update all snapshots non-interactively (use sparingly): `INSTA_UPDATE=always cargo test`.
- Show snapshot help: `cargo insta --help`.

Integration With Guidelines
- This guide supplements `docs/testing/TEST_GUIDELINES.md`:
  - Snapshot tests are required in addition to semantic assertions for critical views.
  - Redactions and fixed Rect sizes are mandatory to maintain stability.
  - Human review is required before committing new or changed snapshots.

