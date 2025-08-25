# Implementation log 006 — M0 hardening: stabilize SSoT test; advice on concurrency fuzz test (2025-08-19)

Summary
- Fixed a race in the new SSoT test by ensuring the EventBus task subscribes to the indexing broadcast channel before injecting a status.
- Documented guidance for the pre-existing flaky concurrency fuzz test.

Changes
- crates/ploke-tui/src/event_bus/mod.rs
  - tests::ssot_forwards_indexing_completed_once: add a short sleep after spawning run_event_bus to let it subscribe to index_tx, then inject IndexingStatus::Completed.

Rationale
- tokio::sync::broadcast delivers messages only to receivers that have subscribed at the time of send.
- The test previously sent IndexingStatus immediately after spawning the EventBus loop, causing a race where the loop might not have subscribed yet, leading to a missed message and a timeout.

Advice: app_state::tests::test_concurrency_with_fuzzing
- The test fires 50 user+embed command pairs with random delays and then sleeps 500ms before asserting chat.messages >= 50.
- This is inherently timing-sensitive and may be flaky under CI/load.
- Recommended follow-ups (not implemented in this commit):
  - Replace fixed sleep with a bounded wait loop that checks messages.len() until it reaches the expected count or a timeout (e.g., 2s).
  - Introduce per-command completion channels (oneshot) similar to test_fix_with_oneshot and await them.
  - Alternatively, add an AppEvent or counter in state to signal when the work queue drains, and await that in the test.

Next steps
- Keep implementing M0: extend E2E tool-call correlation tests and DB persistence once the ploke-db contract is available.
- Consider adding a small readiness handshake for EventBus in tests (barrier or a Ready event) to remove sleeps entirely.
