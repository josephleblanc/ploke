Observability Status — 2025-08-26 20:47:58Z

Scope
- Evaluate current observability across the main interaction paths in ploke: UI input → State Manager → RAG → LLM → tool calls → editing (staging/apply) → DB/index updates, plus indexing and model selection flows.

What’s Implemented
- Tracing
  - File-based tracing is initialized in ploke-tui/src/tracing_setup.rs (rolling daily logs at logs/ploke.log). Console logging is disabled, preserving TUI integrity.
  - LLM session writes diagnostic artifacts to target/test-output/openrouter_e2e (request plans, responses, errors, tool call observation snapshots).

- Event instrumentation
  - AppEvent split by priority (Realtime/Background). EventBus records and forwards indexing and selected system events. run_event_bus emits IndexingStarted/Progress/Completed/Failed with throttled lag warnings.
  - LLM session (llm/session.rs) logs request dispatch, tool call observation, and tool outcome handling with structured messages.
  - Tool dispatch (rag/dispatcher.rs) logs tool routing and unsupported tool names.
  - RAG context assembly (rag/context.rs) logs counts of parts/messages included.

- Persistence (db observability)
  - Conversation turns upserted on MessageUpdated (observability.rs → ploke_db::observability::upsert_conversation_turn).
  - Tool lifecycle recorded: record_tool_call_requested and record_tool_call_done, with latency computed from start/end and idempotency guards. get_tool_call used to correlate.

Gaps And Opportunities
- No consolidated “session trace” for a request in TUI. Proposal: Trace overlay to render a compact, navigable timeline.
- No persisted RAG retrieval events (query, strategy, top_k, scores, selected snippets). Proposal: add retrieval_event table; persist in rag/tools.rs::handle_request_context.
- No persisted edit proposal/approval/apply lifecycle (beyond tool_done JSON). Proposal: proposal and apply_result tables referencing files and new hashes.
- Usage/cost not captured from provider responses; not persisted or surfaced. Proposal: parse usage, compute cost from registry pricing, persist on conversation turns and summarize per session.
- BM25/dense search timings present only in logs; add counters/histograms exposed to the trace overlay.
- Indexing (scan_for_change) success/failure/reporting not summarized in a per‑session view. Proposal: emit and persist a brief summary per scan.

Next Actions
- Implement the TUI “Session Trace” overlay with per‑request timeline and jump‑to logs.
- Extend observability.rs to persist: retrieval events, proposal lifecycle, apply results, and usage/cost.
- Add counters for RAG and LLM tool timings and collect simple histograms in memory for display.

