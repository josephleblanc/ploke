# P0D/P0E Verification Report

## implemented

- No code changes were made in this verification pass.
- Inspected the existing `ploke-eval` implementation in [record.rs](/home/brasides/code/ploke/crates/ploke-eval/src/record.rs:463), [record.rs](/home/brasides/code/ploke/crates/ploke-eval/src/record.rs:795), and [record.rs](/home/brasides/code/ploke/crates/ploke-eval/src/record.rs:985), plus the targeted tests in [introspection_integration.rs](/home/brasides/code/ploke/crates/ploke-eval/tests/introspection_integration.rs:140) and [setup_phase_integration.rs](/home/brasides/code/ploke/crates/ploke-eval/tests/setup_phase_integration.rs:335).
- Verified `P0D` and `P0E` explicitly on top of accepted `P0C`; this report does not reopen or broaden the accepted `ploke-db` helper slice from [2026-04-12_P0C_report.md](/home/brasides/code/ploke/docs/active/agents/2026-04-12_eval-infra-sprint/2026-04-12_P0C_report.md).

## claims

- Claim D1 for `P0D` acceptance criterion 1: `TurnRecord::db_state()` exposes a turn-level DB snapshot handle by returning `DbState::new(self.db_timestamp_micros)` in [record.rs](/home/brasides/code/ploke/crates/ploke-eval/src/record.rs:809).
- Claim D2 for `P0D` acceptance criterion 2: `DbState::lookup(name)` answers exact-name existence at the turn snapshot by querying a fixed relation set (`function`, `struct`, `enum`, `trait`, `method`, `const`, `static`, `macro`, `type_alias`) through `db.raw_query_at_timestamp(...)`, returning `Ok(Some(NodeInfo))` on first match and `Ok(None)` if none are found in [record.rs](/home/brasides/code/ploke/crates/ploke-eval/src/record.rs:985).
- Claim D3 for `P0D` acceptance criterion 3: present and absent lookup behavior is independently supported by targeted artifact-backed tests for `GlobSet`, `new`, and a definitely-missing sentinel in [introspection_integration.rs](/home/brasides/code/ploke/crates/ploke-eval/tests/introspection_integration.rs:140).
- Claim E1 for `P0E` acceptance criterion 1: `RunRecord::replay_query(turn, db, query)` exists and returns `Result<QueryResult, ReplayError>` in [record.rs](/home/brasides/code/ploke/crates/ploke-eval/src/record.rs:463).
- Claim E2 for `P0E` acceptance criterion 2: `replay_query()` executes against the recorded turn snapshot rather than implicit present time because it resolves `timestamp_for_turn(turn)` and passes that value to `db.raw_query_at_timestamp(query, timestamp)` in [record.rs](/home/brasides/code/ploke/crates/ploke-eval/src/record.rs:469); accepted `P0C` is the dependency that proves the helper rewrites `@ 'NOW'` to the supplied timestamp.
- Claim E3 for `P0E` acceptance criterion 3: targeted replay queries succeeded both against the recorded ripgrep run artifact and a focused fixture-backed `RunRecord` in [introspection_integration.rs](/home/brasides/code/ploke/crates/ploke-eval/tests/introspection_integration.rs:227) and [setup_phase_integration.rs](/home/brasides/code/ploke/crates/ploke-eval/tests/setup_phase_integration.rs:335).
- Acceptance disposition: `P0D` meets its packet acceptance criteria on top of `P0C`.
- Acceptance disposition: `P0E` meets its packet acceptance criteria on top of `P0C`.

## evidence

- Code inspection:
  - `RunRecord::replay_query(...)` resolves a turn timestamp then calls `db.raw_query_at_timestamp(...)` in [record.rs](/home/brasides/code/ploke/crates/ploke-eval/src/record.rs:463).
  - `TurnRecord::db_state()` returns a `DbState` wrapper over `turn.db_timestamp_micros` in [record.rs](/home/brasides/code/ploke/crates/ploke-eval/src/record.rs:795).
  - `DbState::lookup(...)` performs exact-name lookup over the fixed relation list and returns `NodeInfo` / `None` / `DbError` in [record.rs](/home/brasides/code/ploke/crates/ploke-eval/src/record.rs:985).
  - `DbState::query(...)` is the matching raw-query surface over the same timestamp in [record.rs](/home/brasides/code/ploke/crates/ploke-eval/src/record.rs:1089).
- Exact test commands run:
  - `cargo test -p ploke-eval --test introspection_integration lookup_ -- --nocapture`
    - result: `3 passed; 0 failed`
  - `cargo test -p ploke-eval --test introspection_integration replay_query_ -- --nocapture`
    - result: `3 passed; 0 failed`
  - `cargo test -p ploke-eval --test setup_phase_integration replay_query_works_with_run_record -- --nocapture`
    - result: `1 passed; 0 failed`
  - `cargo test -p ploke-eval replay_query_returns_error_for_missing_timestamp -- --nocapture`
    - result: `1 passed; 0 failed`
- Test coverage inspected:
  - `lookup("GlobSet")` returns a struct and `lookup("new")` returns a function/method in [introspection_integration.rs](/home/brasides/code/ploke/crates/ploke-eval/tests/introspection_integration.rs:140).
  - `lookup("ThisDoesNotExist12345")` returns `None` in [introspection_integration.rs](/home/brasides/code/ploke/crates/ploke-eval/tests/introspection_integration.rs:199).
  - `replay_query(1, ...)` returns struct rows and function counts, while `replay_query(99, ...)` errors, in [introspection_integration.rs](/home/brasides/code/ploke/crates/ploke-eval/tests/introspection_integration.rs:227).
  - A focused fixture-backed `RunRecord` with an explicit `TimeTravelMarker` successfully replays a query and rejects a nonexistent turn in [setup_phase_integration.rs](/home/brasides/code/ploke/crates/ploke-eval/tests/setup_phase_integration.rs:335).
  - Missing-timestamp behavior is covered at the unit level by verifying `timestamp_for_turn(1)` is absent when no time-travel marker exists in [record.rs](/home/brasides/code/ploke/crates/ploke-eval/src/record.rs:2091).
- Behavior notes required by the packets:
  - `lookup(name)` currently matches exact `name` equality only, over the fixed relation list named above; it does not do namespace-qualified lookup, fuzzy lookup, or ambiguity resolution.
  - `lookup(name)` does not depend on `RunRecord.phases.setup`; it uses the timestamp stored directly on `TurnRecord`, so missing setup data does not block it.
  - `replay_query(turn, query)` is raw-query oriented. The caller supplies CozoScript and must include at least one `@ 'NOW'` marker to satisfy the accepted `P0C` helper contract.
  - `replay_query(turn, query)` returns `ReplayError::TimestampNotFound(turn)` when the turn has no entry in `db_time_travel_index`; the current implementation does not surface `ReplayError::TurnNotFound` on that path.

## unsupported_claims

- I did not prove that `lookup()` covers every relation type a future inspection UX might need beyond the current fixed list.
- I did not prove disambiguation behavior when multiple nodes in the searched relations share the same name; the implementation returns the first match found by relation-order search.
- I did not prove parser-aware safety for arbitrary raw-query text in `replay_query()`; that remains the accepted `P0C` textual-rewrite contract, not a new guarantee from this pass.
- I did not prove full `cargo test -p ploke-eval` success or any workspace-wide regression status.

## not_checked

- `crates/ploke-eval/tests/test_introspection.rs` was not run in this verifier pass.
- I did not run additional `DbState::query(...)` tests beyond what is implied by `replay_query()` and the accepted `P0C` helper path.
- I did not inspect failure behavior for corrupted record artifacts, malformed DB rows, or stale external run artifacts under `~/.ploke-eval/`.
- I did not verify a `ploke-eval`-local differential test where the same replay query is shown to return different results at historical time vs present time; that semantic proof still comes from accepted `P0C`.

## risks

- `lookup()` is intentionally narrow: exact-name only, fixed relation set only, and first-hit wins. That is acceptable for `P0D` but is a real limitation for richer introspection use.
- `replay_query()` is a thin raw-query wrapper over the `P0C` helper. If callers omit `@ 'NOW'`, the call now fails fast under the accepted `P0C` contract.
- Nonexistent-turn handling is effectively "timestamp missing" handling today. That is coherent with the current code path, but the unused `ReplayError::TurnNotFound` variant means the error taxonomy is looser than the enum suggests.
- Artifact-backed integration tests rely on the local ripgrep run data under `/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209`, so this evidence depends on that artifact remaining representative and intact.

## next_step

- Treat `P0D` and `P0E` as independently checked and ready for orchestrator acceptance on top of accepted `P0C`.
- If the orchestrator wants a follow-up hardening packet, the highest-signal next step is not more scope in this packet; it is a narrow test/UX cleanup packet for:
  - differential historical-vs-present replay evidence local to `ploke-eval`
  - explicit ambiguity behavior for `lookup()`
  - deciding whether `ReplayError::TurnNotFound` should be used or removed
