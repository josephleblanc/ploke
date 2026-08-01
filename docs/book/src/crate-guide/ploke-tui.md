# `ploke-tui`

> Imported from the generated Hermes code wiki at `~/.hermes/wikis/ploke` (generated `2026-06-03T03:55:43Z`, source commit `97b1a101b9363b88b8e41f1447cb36d76a3eb8a2`). Verify implementation details against current source before making changes.


`ploke-tui` is the release-facing terminal application. It owns the user interaction loop and wires together state management, parsing/indexing, RAG, LLM routing, tools, event delivery, observability, and filesystem I/O.

## Responsibilities

- Start the application binary and restore terminal state on panic or exit.
- Load user config from `~/.config/ploke/config.toml`, environment variables, and `.env`.
- Construct shared runtime services: `Database`, `EmbeddingRuntime`, `IoManagerHandle`, BM25 service, `IndexerTask`, optional `RagService`, `EventBus`, and `AppState`.
- Accept UI commands and route them to the `StateCommand` dispatcher.
- Run chat sessions and tool-call loops, including provider retries, cancellation, response tracing, tool-call validation, and chat-history updates.
- Expose TUI tools for code context, file reads, code edits, file creation, namespace patch/read, cargo, list-dir, and code-item lookup.

## Key Files

- [`crates/ploke-tui/src/main.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ploke-tui/src/main.rs) — `#[tokio::main]` binary that initializes tracing and calls `try_main`.
- [`crates/ploke-tui/src/lib.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ploke-tui/src/lib.rs) — application bootstrap and subsystem wiring.
- [`crates/ploke-tui/src/app_state/mod.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ploke-tui/src/app_state/mod.rs) — app-state module boundary and public re-exports.
- [`crates/ploke-tui/src/app_state/commands.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ploke-tui/src/app_state/commands.rs) — `StateCommand` enum and command grouping/validation migration surface.
- [`crates/ploke-tui/src/llm/manager/session.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ploke-tui/src/llm/manager/session.rs) — `run_chat_session`, chat-loop policy, tool-call loop handling, replay/tape support, and error normalization.
- [`crates/ploke-tui/src/tools/mod.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ploke-tui/src/tools/mod.rs) — tool registry, shared `Ctx`, validation/sanitization, tool-call records, and tool result shapes.

## Public API

- `try_main() -> color_eyre::Result<()>` — starts the full app runtime.
- `emit_app_event`, `emit_error_event`, `set_global_event_bus` — global event helpers used by error/reporting paths.
- `StateCommand` — main command boundary for chat, workspace, indexing, RAG, edit, and model operations.
- `CancelChatToken` — cancellation type re-exported from the LLM manager.
- Tool exports such as `RequestCodeContext`, `CodeEdit`, `CreateFile`, `NsPatch`, `NsRead`, and shared `ToolDefinition`/`ToolName` from `ploke-core`.

## Internal Structure

`try_main` is the best map of runtime composition: it builds an embedding processor from config, creates `EmbeddingRuntime`, initializes `ploke-db` with schema and multi-embedding support, spawns the `ploke-io` manager, creates the event bus, starts BM25, creates an `IndexerTask`, constructs a `RagService`, then places everything inside `AppState`. UI and background components communicate through `mpsc`, `broadcast`, and `watch` channels rather than direct cross-thread mutation.

The chat loop in `run_chat_session` is generic over a `Router + RouterCalibration`. It repeatedly performs `chat_step`, records provider attempts, handles `ChatStepOutcome::Content` or `ToolCalls`, validates tool arguments, dispatches tools, appends tool results to the request, and stops on completion, cancellation, or bounded error-repair exhaustion.

## Dependencies

- **Uses:** `ploke-db`, `ploke-embed`, `ploke-rag`, `ploke-io`, `ploke-llm`, `ploke-core`, `ploke-error`, `syn_parser`, `ploke-transform`, Ratatui/Crossterm, Tokio.
- **Used by:** release binary `ploke`, test harnesses, and operator workflows.

## Notable Patterns / Gotchas

- `StateCommand` still contains legacy variants plus newer grouped command variants; prefer grouped validated commands when adding new handlers.
- Tool names and descriptions moved toward `ploke-core` so `ploke-tui` and `ploke-llm` share one schema surface.
- The LLM loop has explicit repair-attempt limits independent from provider retry limits; do not convert those into unbounded retries.
- `try_main` currently creates `IoManagerHandle::new()`, which spawns its own runtime thread.
