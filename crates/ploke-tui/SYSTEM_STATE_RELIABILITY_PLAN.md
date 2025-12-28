SYSTEM STATE RELIABILITY PLAN

Goal: eliminate UI lock contention and prevent future correctness bugs by design.
This plan replaces ad-hoc RwLock usage with a single-writer state manager and
read-only snapshots for the UI and background tasks.

Background problem
- SystemState is a global RwLock used by UI, indexing, and IO paths.
- Long-lived reads (IO/parse) combined with UI writes create stalls.
- Current correctness relies on convention (drop lock before await).

Design principles
- Single writer: only the state manager mutates SystemState.
- Snapshot reads: UI and workers read immutable snapshots (no locks).
- Message passing: updates flow through commands/events, not shared writes.
- Explicit boundaries: APIs prevent long-lived lock guards by construction.

Concrete architecture
1) Introduce SystemSnapshot
   - Add a lightweight immutable struct containing fields needed by UI/tools.
   - Example fields: focused_crate_id, focused_crate_root, stale flag, last_parse_failure.
   - Provide conversions from SystemStatus -> SystemSnapshot.

2) Add a watch channel for snapshots
   - AppState holds `system_snapshot_tx: watch::Sender<SystemSnapshot>`.
   - State manager updates snapshot after each SystemState mutation.
   - UI uses `watch::Receiver<SystemSnapshot>` for reads (no RwLock).

3) Make SystemState write-only from state manager
   - Remove direct `state.system.write()` from UI/event handlers.
   - Replace with StateCommand variants (e.g., RecordIndexCompleted).
   - Keep SystemState private to state_manager module or wrap it in a facade.

4) Restrict read access
   - For UI and tool paths, expose only SystemSnapshot.
   - For background tasks that need authoritative data, pass snapshots or
     a read-only view, not the lock guard.

5) Enforce lock scope
   - Make `SystemState` opaque outside app_state; provide accessor methods
     that return owned snapshots, not guards.
   - Use `#[deny(clippy::await_holding_lock)]` and add a lint pass.

6) Split hot fields (optional, if needed)
   - For extremely hot flags (indexing status, focus id), consider atomics
     or ArcSwap to avoid watch contention.

Migration steps (incremental)
1) Add SystemSnapshot struct + watch channel in AppState.
2) Update UI to read from snapshot receiver.
3) Move remaining SystemState writes into state_manager commands.
4) Replace direct reads in tools/handlers with snapshots.
5) Remove public access to SystemState or restrict to app_state module.
6) Add a regression test that simulates long IO while UI events fire.

Acceptance criteria
- UI draw loop never awaits a SystemState lock.
- State manager is the only writer to SystemState.
- All UI reads are snapshot-based.
- Regression tests cover: long scan + indexing completion does not stall.

Notes
- This plan avoids correctness depending on developer convention.
- It also makes future async refactors safer by construction.
