# Decisions requiring USER review

Purpose
- This file is the single queue for blockers and directional decisions that require USER approval.
- Only add items that block progress or significantly determine future direction.
- For each item, include: context, options (with tradeoffs), recommended default, and any deadline.

Open decisions (M0 focus)
1) Chat history file path and naming
   - Context: FileManager::save_content currently writes a temp file; final path needs to be stable and user-visible.
   - Options:
     a) Save to CWD/.ploke_history.md (simple, visible, per-project).
     b) Save to XDG documents dir (~/.local/share/ploke/history.md) (centralized).
     c) Per-session timestamped files under logs/ (no overwrites, harder to find).
   - Recommended: (a) for M0; add config later.
   - Blocker: minor (unblock M0 once chosen).
    - USER: Neither, save to database with metadata instead. Later we can add config options for users that may wish to inspect/edit the files, etc.

2) Typed LLM tool events migration timing
   - Context: SystemEvent::ToolCallRequested is deprecated; migration risk to break routing.
   - Options:
     a) Land typed events in M0 with compatibility bridge; remove old in M1.
     b) Defer typed events entirely to M1.
   - Recommended: (a) to get telemetry attached to typed events early.
   - Blocker: none if bridge used; confirm direction.
    - USER: Agreed, get the longer-term single source of truth working early, avoid tech debt.

3) Tool-call payload persistence (privacy)
   - Context: Storing full arguments_json and outcome_json may include secrets/paths.
   - Options:
     a) Store full JSON; rely on logs redaction patterns later.
     b) Store only hashes by default; full payload behind debug flag/config.
   - Recommended: (b) for M0; full payload optional via config in M1.
   - Blocker: affects ploke-db schema defaults.
    - USER: We should transition out of absolute paths and into local paths instead. This is a longer-term solution that will avoid issues with deserializing the hashed values. May require updating many files, ultimately worth it. Should unify our approach to file-handling in any case to make project more maintainable. Consider cost/benefit of moving all IO logic into `ploke-io` or keeping minimal layer as-is in `ploke-tui` for currently functioning operations.

4) EventBus channel capacities (defaults)
   - Context: Avoid lag; keep realtime snappy.
   - Options:
     a) realtime=100, background=1000, index=1000 (current).
     b) Lower realtime to 64; raise background to 4096 for heavy indexing.
   - Recommended: (a) retain; instrument lag metrics first.
   - Blocker: none.
    - USER: Confirm. Expose in config for power-users, use sane default (a), change later on evidence from benchmarks (early optimization is the source of all evil, etc - DK)

5) Observability log retention
   - Context: logs/logs/ploke.log rotation schedule and retention.
   - Options:
     a) daily rotation, keep 7 days.
     b) daily rotation, user-configurable retention (M1).
   - Recommended: (a) in M0; add config in M1.
   - Blocker: none.
    - USER: Agreed

How to use
- Add new items at the bottom; include the four required parts (context, options, recommendation, blocker).
- Reference the related implementation-log-XXX.md entries when a decision is finalized.
