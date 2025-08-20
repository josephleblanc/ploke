ploke-tui code review (2025-08-19)

Executive summary
- The codebase shows a clear move toward a modular, testable architecture: App is thinner, view logic is componentized (ConversationView, InputView), commands are parsed and executed via a dedicated module, and state mutations are serialized through a state manager.
- EventBus with realtime/background separation, plus a dedicated indexing channel, is a solid foundation for decoupling; however, there is duplication and some ambiguity that can be simplified.
- RAG and LLM paths are evolving; tool-call management has a practical design (dispatch-and-await pattern), but there are deprecated paths that should be cleaned up.
- There are a few correctness risks:
  - Potential duplicate IndexingCompleted events from two different sources.
  - FileManager’s save_content writes a temp file but doesn’t rename to a final destination.
  - Some sync/blocking calls remain in UI code (block_in_place) that could stall the TUI if used.
  - EventBus priority split can still be a source of confusion; several .expect/.unwrap are present across modules.
- Testing is on the right track (keymap tests, benches, a thorough state/embedding test). Recommended to add rendering snapshot tests as planned in refactor.md Phase 6.

Architecture observations

1) AppState and State Manager
- AppState holds shared services (db, embedder, io, rag) and guards (RwLock/Mutex). State changes are done via StateCommand and handled in app_state::dispatcher::state_manager; this is good for concurrency and testability.
- The design promotes a single place to mutate state, reducing race conditions (further strengthened by oneshot synchronization in chat/embedding flows).

2) EventBus
- Two broadcast channels (realtime, background) plus a dedicated index_tx are used. AppEvent::priority chooses the channel at send time.
- run_event_bus currently forwards indexing status into App events. However, indexing::index_workspace also sends completion events directly, risking duplicates.

3) UI and Rendering
- ConversationView and InputView encapsulate layout state and rendering, eliminating per-frame allocations by iterating over cached paths.
- Measurement and rendering share matching wrap widths (reserving a gutter), a good invariant for consistent scroll behavior.
- Mouse hit-testing maps clicks to messages correctly via virtual line offsets; paging/scroll math looks guarded with saturating arithmetic.

4) LLM and Tool Calls
- LLM session extraction isolates per-request orchestration (retries, tool calls, timeout). await_tool_result correlates completions by (request_id, call_id) on the realtime channel.
- Tool calls are currently routed as SystemEvent::ToolCallRequested. Comments note this as a deprecated path—cleaning this up to dedicated events will simplify control flow.

5) RAG integration
- Hybrid, BM25-only, dense search handlers are present with clear user feedback via SysInfo messages. Context assembly constructs a system-prompt + code + conversation prompt and forwards to the LLM manager.

6) File I/O
- FileManager subscribes to background events and handles filesystem operations. It currently reads queries from ./query/<file>, and emits WriteQuery to the app thread for execution.
- SaveRequested writes a temp file but never performs an atomic rename to a final path, and it logs the directory path as if it were the file—this needs correction.

7) Config and model registry
- Default OpenRouter models are merged with user overrides. API keys are resolved from env first; this is sensible. Ensure sensitive values are never logged at info/debug.

Strengths
- Solid concurrency model: actor-like state manager guarded by channels; background work done asynchronously; snapshot-oriented rendering.
- Incremental refactor plan documented in refactor.md; the code closely follows it.
- Good unit test coverage for keymaps, and a deep integration test around embedding/indexing.
- Clean separation of cross-crate responsibilities (db, rag, embedder, io).

Issues and risks (with recommendations)
- Duplicate indexing completion events
  - Problem: run_event_bus forwards IndexStatus::Completed as AppEvent::IndexingCompleted, while indexing::index_workspace also sends AppEvent::IndexingCompleted in its task result path.
  - Impact: UI may receive duplicate “Indexing Succeeded” and trigger UpdateDatabase twice.
  - Recommendation: Choose a single source of truth. Prefer letting run_event_bus be the only forwarder of IndexingStatus into AppEvent. Remove the direct AppEvent::IndexingCompleted emission from indexing::index_workspace.

- FileManager save_content incomplete
  - Problem: save_content writes to temp_path = path.join(".ploke_history").with_extension("md") but never renames into a real output path; log states “saved to {path}” where path is a directory.
  - Impact: Users believe history saved, but file remains in temp path; data loss risk.
  - Recommendation: Decide a final target file (e.g., path.join(".ploke_history.md") or default_history_path()). Use atomic rename from temp to final. Log the final path, not the directory.

- Remaining blocking calls in UI
  - Problem: App::list_models uses block_in_place + Handle::block_on; while the refactor moves model listing to async (commands::exec list_models_async), this dead code risks reintroduction of blocking behavior.
  - Recommendation: Remove or gate the UI-blocking version. Keep async approach only.

- Event priority confusion
  - Problem: Two broadcast channels for AppEvent can be confusing (especially since priority is derived from the event variant). Some system events are mixed priority.
  - Recommendation: Consider a single AppEvent channel and embed priority metadata in the event, or split event types more strictly by channel. This reduces misrouting.

- Deprecated tool-call path
  - Problem: Tool calls travel through SystemEvent::ToolCallRequested while comments say it’s a temporary compatibility path.
  - Recommendation: Introduce a dedicated LlmToolEvent in AppEvent with a typed payload; remove the deprecated SystemEvent variant. This simplifies routing, avoids mixing unrelated concerns in SystemEvent.

- Error handling (unwrap/expect)
  - Problem: There are numerous .expect/.unwrap calls in control paths (e.g., FileManager, many handlers).
  - Impact: Potential panics under edge conditions.
  - Recommendation: Replace with Result-returning helpers and emit AppEvent::Error with severities. Use ResultExt/ErrorExt to standardize logging.

- Observability
  - Problem: tracing setup exists but is commented out in main. Without a guard or env flag, it stays off.
  - Recommendation: Initialize tracing by default with sensible EnvFilter; if terminal rendering conflicts, gate ANSI only at stdout and keep file logging always-on.

- InputView/ConversationView consistency
  - The measurement/render invariants look consistent (same wrap width rule). Keep tests in Phase 6 to ensure no regressions.

- Testing gaps
  - Missing snapshot tests for ConversationView and InputView rendering; property tests and criterion benches are planned but not all present. The keymap tests are good; extend to view snapshots (TestBackend), and smoke tests for event routing.

Performance notes
- Message wrapping uses textwrap; width adjustments are consistent. Measurement caches item heights for efficient scrolling. Avoid logging in hot render paths except at trace level.
- indexer and rag paths are async; UI sets needs_redraw flags and avoids blocking (good).
- Consider bounding broadcast capacities and monitoring lag to avoid memory growth under heavy background traffic.

Security and safety
- API keys resolved from env; ensure provider configs with keys from config files aren’t logged. Avoid printing provider.api_key anywhere.
- IoManager path policy is external to this crate; ensure edits requested by tools respect absolute path policies and roots to prevent cross-root writes.

Maintainability
- Module structure is good; commands parser/executor are clean and testable.
- refactor.md is an excellent guide; continue implementing Phase 6 test/bench tasks.
- Prefer returning small helper results instead of mixing logging with state mutation; thin handlers around well-named functions keep intent clear.

Actionable next steps (prioritized)
1) De-duplicate indexing completion emissions (choose run_event_bus as sole forwarder).
2) Fix FileManager save_content to atomically rename and log the correct final file path.
3) Remove/block dead blocking paths in App; keep async list_models only.
4) Introduce typed LlmToolEvent and retire SystemEvent::ToolCallRequested.
5) Add ConversationView/InputView snapshot tests (TestBackend) per refactor.md.
6) Audit .expect/.unwrap paths in background handlers and replace with emitted errors.

