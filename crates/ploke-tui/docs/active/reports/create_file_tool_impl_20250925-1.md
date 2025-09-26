# Create File Tool — Implementation Log (Part 1)

Scope
- Implement core types, IO path, TUI tool, staging, and auto-approval for a safe, strongly-typed create-file tool.

Decisions (based on answers in research doc)
- on_exists: support {error, overwrite}; default error.
- create_parents: allowed; enforced within root scope.
- File types: Rust-only (.rs) in M1; parsing required to compute TrackingHash.
- Auto-confirm: creations follow `auto_confirm_edits` behavior.

What’s implemented in this step
- ploke-core:
  - Added `OnExists`, `CreateFileData`, `CreateFileResult` in `io_types.rs`.
  - Added LLM-facing `CreateFileResult` in `rag_types.rs`.
  - Exported new types from `lib.rs`.
- ploke-io:
  - New `create.rs` with atomic create/write, path policy via parent-dir normalization, Rust-only, and TrackingHash computation.
  - `IoRequest::CreateFile` + handler branch in `actor.rs` (emits Created event when watcher enabled).
  - `IoManagerHandle::create_file(...)` public API in `handle.rs`.
- ploke-tui:
  - Tool module `tools/create_file.rs` with schema, typed params (borrowed/owned), and execution path delegating to `rag::tools::create_file_tool`.
  - Registered `ToolName::CreateFile` and route in `tools/mod.rs::process_tool`.
  - Exposed tool in LLM manager tools list.
  - Staging path: added `CreateProposal` and in-memory registry `AppState::create_proposals`.
  - Approval path: `rag::editing::approve_creations` mirrors `approve_edits`, applying via IoManager and emitting ToolCallCompleted.
  - Staging function `rag::tools::create_file_tool` builds previews (codeblock/diff), persists proposals, emits typed result, and auto-approves when configured.
  - Proposal persistence: added `save_create_proposals`/`load_create_proposals` alongside existing edit proposals.

Not included (future polish)
- CLI commands `create approve|deny <request_id>` (auto-confirm covers M1; can add commands later to manually control).
- Additional validations (e.g., deny binary content) beyond Rust parse; parse already enforces syntax.
- E2E tests and live-gated tests (will add in next step).

Safety notes
- Path normalization uses parent directory canonicalization and root containment check for non-existent files.
- Atomic create via temp file + fsync + rename; best-effort fsync of parent directory.
- Only `.rs` files accepted; TrackingHash computed from tokens of intended content.

Next steps
- Add unit tests for IO create path and tool schema.
- Add offline e2e test that stages and (auto-)applies a creation, then verifies existence and DB rescan trigger.

