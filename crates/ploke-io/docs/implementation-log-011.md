# Implementation Log 011 — Watcher Scaffolding and Builder/Handle Wiring

Date: 2025-08-19

Summary
- Implemented Phase 4 scaffolding: added a feature-gated watcher module that uses notify to watch configured roots and broadcast file change events via tokio::broadcast.
- Extended IoManagerBuilder with enable_watcher(bool) and with_watcher_debounce(Duration).
- Added IoManagerHandle::subscribe_file_events() (feature-gated) and wired a shared broadcast channel between the watcher thread and handle subscribers.
- Introduced the "watcher" Cargo feature and optional "notify" dependency.

Rationale
- Provide a minimal, opt-in watcher infrastructure to unblock downstream components that want to subscribe to file change events, without impacting default builds or runtime cost.

Changes Made
- Cargo.toml:
  - Added optional dependency notify = "6" and new feature "watcher".
- src/lib.rs:
  - Added feature-gated watcher module and re-exported FileChangeEvent and FileEventKind.
- src/builder.rs:
  - IoManagerBuilder gains enable_watcher and with_watcher_debounce; build() creates a broadcast channel and spawns the watcher thread when enabled and roots are configured.
- src/handle.rs:
  - IoManagerHandle now holds an optional broadcast::Sender<FileChangeEvent> (feature-gated) and exposes subscribe_file_events().
- src/watcher.rs (new):
  - Minimal watcher that maps notify events to FileChangeEvent and broadcasts them. Debounce is applied via poll interval config as a starting point.

Tests/Verification
- Builds without the "watcher" feature remain unchanged.
- With "watcher" feature enabled, the crate compiles and exposes subscribe_file_events(); runtime emits events when files change under configured roots.

Impact/Risks
- The current debouncing is best-effort (uses poll interval). Future iterations can add proper coalescing and async debounce.
- The event type lives in ploke-io behind a feature; future coordination with ploke-core may move these types upstream.

Next Steps
- Phase 4: refine debounce/coalescing, add origin correlation for echo suppression.
- Phase 7: continue path policy hardening (canonicalization, symlink policy).
- Maintain 2-log window: remove the oldest implementation log after this addition.

References
- docs/production_plan.md
- src/{builder.rs,handle.rs,watcher.rs}
