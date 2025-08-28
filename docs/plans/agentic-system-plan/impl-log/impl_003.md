Updates made per request

- Snapshot gating removed
    - Un-gated insta assertions in crates/ploke-tui/tests/approvals_overlay_render.rs so
visual snapshots always run.
    - Redactions kept for stability (UUID → , PWD → ).
    - Redactions kept for stability (UUID → , PWD → ).
- 
Test harness usage
    - I evaluated TEST_APP (crates/ploke-tui/src/test_harness.rs). It isn’t practical to
use for current overlay tests:
    - Render tests need direct access to `Arc<AppState>` to stage proposals; `App` doesn’t
expose a public getter.
    - Key tests need to intercept `StateCommand`s; `TEST_APP` encapsulates the `cmd_tx`
without exposing the receiver.
- Conclusion: leaving these tests with minimal, focused state setup is more maintainable
until we expose safe getters in the harness.
Conclusion: leaving these tests with minimal, focused state setup is more maintainable
until we expose safe getters in the harness.
- 
Insta guide
    - Added docs/testing/INSTA_SNAPSHOT_GUIDE.md:
    - How to write, run, review (`cargo insta review`), and maintain insta snapshots.
    - Failure triage: trivial vs critical changes; when to update redactions vs. fix code.

- Lint and warnings cleanup
    - Replaced deprecated Buffer::get with buffer.cell((x, y)).expect(...).symbol() in
render tests.
    - Removed an unnecessary mut in rag/editing.rs.
    - Note: unrelated bench warnings (criterion::black_box) remain outside this change
scope.
    - Note: unrelated bench warnings (criterion::black_box) remain outside this change
scope.
- 
Tests run status
    - Regular tests pass (semantic assertions).
    - Snapshot tests fail as expected with “unreviewed snapshots” and have
generated .snap.new files.
    - Next: run `cargo insta review` locally (you installed cargo-insta) to approve
snapshots; commit the approved `.snap` files.
- Command examples:
    - Generate and fail-on-new snapshots: `cargo test -p ploke-tui --test
approvals_overlay_render`
    - Review snapshots: `cargo insta review`
    - Rerun: `cargo test -p ploke-tui --test approvals_overlay_render`

Additional progress beyond step 2

- Proposal Persistence (step 2): Verified existing implementation and added tests
    - crates/ploke-tui/tests/proposals_persistence.rs: roundtrip, missing file, corrupted
file — all pass.
    - crates/ploke-tui/tests/proposals_persistence.rs: roundtrip, missing file, corrupted
file — all pass.
- 
Post-Apply Rescan (step 3): Implemented SysInfo and added test
    - crates/ploke-tui/src/rag/editing.rs: emits “Scheduled rescan of workspace…” after
approve.
    - crates/ploke-tui/tests/post_apply_rescan.rs: asserts SysInfo presence — passes.

What I didn’t change

- Did not attempt to retrofit TEST_APP for render/key tests because it would require public
getters or a command-capture facility. If you want, I can add minimal test-only accessors
and refactor tests to use them.

Next actions

- Approve snapshots:
    - Run cargo test -p ploke-tui --test approvals_overlay_render followed by cargo insta
review, then commit approved snapshots/*.snap.
- I can proceed to the next plan slice (Open-in-editor spawn test with a mockable command
runner), or focus on any refinement you prefer (e.g., exposing test harness accessors and
refactoring tests to use TEST_APP).
