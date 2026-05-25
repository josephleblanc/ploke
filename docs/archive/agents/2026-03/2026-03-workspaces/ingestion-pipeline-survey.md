# Ingestion Pipeline Survey

Survey basis: direct code inspection only. No sub-agent report artifacts were available in this session.

## Summary

The workspace-aware parser and transform path already exists, and the DB/indexer can already process all unembedded nodes once they are in Cozo. The remaining gap is orchestration: the TUI still resolves and indexes a single target directory/crate, while save/load and backup semantics are still crate-focused.

## Current State

- `syn_parser::parse_workspace` already reads `[workspace]`, normalizes members/exclude paths, supports optional member selection, and returns a `ParsedWorkspace` with both workspace metadata and per-crate parse outputs. See [`crates/ingest/syn_parser/src/lib.rs`](crates/ingest/syn_parser/src/lib.rs#L66).
- `ploke-transform::transform_parsed_workspace` already inserts `workspace_metadata` and then transforms each parsed crate graph into the DB. See [`crates/ingest/ploke-transform/src/transform/workspace.rs`](crates/ingest/ploke-transform/src/transform/workspace.rs#L13).
- The schema bootstrap already includes `WorkspaceMetadataSchema` in `create_schema_all`. See [`crates/ingest/ploke-transform/src/schema/mod.rs`](crates/ingest/ploke-transform/src/schema/mod.rs#L84).
- `IndexerTask::run` is DB-wide: it iterates all primary node relations, embeds every unembedded node, and optionally finalizes BM25. See [`crates/ingest/ploke-embed/src/indexer/mod.rs`](crates/ingest/ploke-embed/src/indexer/mod.rs#L241).
- The TUI already has a `StateCommand::IndexWorkspace`, but the legacy `/index start` path still sends one directory string and the handler still resolves one target dir before calling `run_parse`. See [`crates/ploke-tui/src/app/commands/exec.rs`](crates/ploke-tui/src/app/commands/exec.rs#L852) and [`crates/ploke-tui/src/app_state/handlers/indexing.rs`](crates/ploke-tui/src/app_state/handlers/indexing.rs#L36).

## Missing Pieces

- `/index <workspace>` does not yet explicitly resolve a workspace manifest and fan out through `parse_workspace` / `transform_parsed_workspace` from the TUI path.
- `/index` inside a workspace still uses the current focus/current directory heuristic, not a workspace manifest-driven batch flow.
- There is no workspace-level persistence model for loaded crates, backup locations, or member metadata.
- `/save db` and `/load crate` are still single-crate workflows keyed by crate name/prefix, not workspace-aware workflows. See [`crates/ploke-tui/src/app_state/database.rs`](crates/ploke-tui/src/app_state/database.rs#L234).
- The planned workspace management commands are absent: `/workspace status`, `/workspace update`, `/workspace rm <crate>`, `/load <workspace>`, and `/load crates ...`.
- Indexing completion bookkeeping is still crate-focus oriented, so workspace status/update flows do not yet have a first-class place to record per-workspace state.

## Likely Touchpoints

- [`crates/ingest/syn_parser/src/lib.rs`](crates/ingest/syn_parser/src/lib.rs#L66) and [`crates/ingest/syn_parser/src/discovery/mod.rs`](crates/ingest/syn_parser/src/discovery/mod.rs#L1) for explicit workspace-root discovery and member fan-out.
- [`crates/ingest/ploke-transform/src/transform/workspace.rs`](crates/ingest/ploke-transform/src/transform/workspace.rs#L13) for any additional workspace-level validation or batching.
- [`crates/ploke-tui/src/app/commands/exec.rs`](crates/ploke-tui/src/app/commands/exec.rs#L846) and [`crates/ploke-tui/src/app_state/handlers/indexing.rs`](crates/ploke-tui/src/app_state/handlers/indexing.rs#L36) for command parsing and orchestration.
- [`crates/ploke-tui/src/app_state/database.rs`](crates/ploke-tui/src/app_state/database.rs#L234) for workspace-aware save/load, conflict checks, and backup naming.
- [`crates/ploke-tui/src/app_state/commands.rs`](crates/ploke-tui/src/app_state/commands.rs) for new workspace commands and payloads.
- [`crates/ingest/ploke-embed/src/indexer/mod.rs`](crates/ingest/ploke-embed/src/indexer/mod.rs#L180) if progress, cancellation, or BM25 finalization need workspace-scoped metadata.

## Open Risks

- Do not weaken manifest/member validation to make workspace indexing “forgiving”; partial or mismatched member sets should still fail loudly.
- Avoid treating `current_dir()` as a workspace root substitute once workspace commands are added; it can silently index the wrong tree.
- Re-index/update semantics need a clear source of truth for stale detection. The current indexer counts unembedded rows, but it does not provide workspace-aware file-hash reconciliation.
- If workspace batching is added on top of the existing DB-wide indexer, make sure crate removal/update flows do not silently leave stale rows behind.
