# Indexing Freeze Investigation

- Started: 2025-12-27
- Scope: ploke-tui indexing freeze when running `/index start <path>` against `tests/fixture_crates/fixture_nodes`.
- Goal: add a regression test with a 1-minute timeout and test-specific logging, and record observations.

## Timeline

- Created this log document to track steps and observations.
- Added new regression test `crates/ploke-tui/tests/indexing_freeze_repro.rs` with a 60s timeout and per-test logging.
- Logging target: `indexing_freeze_test` writing to `crates/ploke-tui/tests/reports/indexing_freeze_test.log`.
- Ran `cargo test -p ploke-tui indexing_freeze_repro -- --nocapture` but the test filter did not match; no tests executed.
- Observed empty log file; fixed test logging to keep the tracing subscriber guard alive for test duration.
- Re-ran `cargo test -p ploke-tui --test indexing_freeze_repro -- --nocapture`; test passed in ~0.03s.
- Log file `crates/ploke-tui/tests/reports/indexing_freeze_test.log` now contains test steps (start, index send, marker wait, completion).
- Switched indexing test delay control to a safe atomic flag (`set_indexing_test_delay_ms`) instead of env var.
- Updated regression test to use a 5s artificial delay and a 200ms marker wait to ensure it fails if indexing blocks the state manager.
- Ran `cargo test -p ploke-tui --test indexing_freeze_repro -- --nocapture`; test passed and logged to `crates/ploke-tui/tests/reports/indexing_freeze_test.log`.
- Guarded test-only indexing delay with `test_harness` feature and removed unsafe env access.
- Ran `cargo test -p ploke-tui --test indexing_freeze_repro --test indexing_non_blocking -- --nocapture`; both tests passed.
- Git history review: latest commits touching indexing path are 406eafba (parse failure logging) and 6bac6dc3/ea21e56f (focus handling + path policy unification). These introduced canonicalized crate roots and path policy enforcement (require_absolute + DenyCrossRoot) wired into indexing IO root updates.
- Added app-loop regression test using ratatui TestBackend: `crates/ploke-tui/tests/indexing_freeze_app_loop.rs`.
- Adjusted app-loop test timing (startup delay + 1s message wait) after initial failure to allow for app loop startup.
- Added baseline input check to app-loop test to confirm input simulation works before indexing.
- Identified lock scope issue in `process_with_rag`: read lock on chat was held across awaits and attempted to write to chat, causing deadlock. Refactored to snapshot chat state before awaits.
- App-loop regression test now passes after fixing chat lock scope in `process_with_rag`.
- Ran `cargo test -p ploke-tui --test indexing_freeze_app_loop -- --nocapture`.
- Added a system-lock regression test in `crates/ploke-tui/tests/indexing_freeze_app_loop.rs` that holds a `state.system` read lock, triggers `AppEvent::IndexingCompleted`, then verifies input processing; this reproduces the UI stall.
- Ran `cargo test -p ploke-tui --test indexing_freeze_app_loop -- --nocapture`; new test fails with `app loop did not process input while indexing completion event was pending`.
- Potential fix: move `SystemState` mutation out of the UI event handler into `state_manager` (`RecordIndexCompleted`), and snapshot `SystemState` fields before IO in `scan_for_change` to avoid long-lived system read locks.
- Ran `cargo test -p ploke-tui --test indexing_freeze_app_loop -- --nocapture`; system-lock regression test now passes (potential fix, not yet confirmed in live UI).
- Investigating "indexing completes but 0 nodes indexed": logs show parse succeeds, but indexer reports 0 unembedded nodes; `run_parse` was parsing without writing to DB. Potential fix: call `transform_parsed_graph` in `crates/ploke-tui/src/parser.rs` so initial indexing populates the DB.
- Confirmed: UI responsiveness restored and indexing now populates targets after applying `RecordIndexCompleted` + `scan_for_change` lock scope reduction + `run_parse` DB transform.
- Follow-up (longer-term): replace direct UI mutation of `SystemState` with a snapshot/event-driven model (single writer task + watch channel) to avoid lock contention by design.
