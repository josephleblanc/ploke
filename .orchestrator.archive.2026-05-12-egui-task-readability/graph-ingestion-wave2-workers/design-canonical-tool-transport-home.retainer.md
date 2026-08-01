# Canonical Tool Transport Home Retainer

Changed files: none.

Current owners:

- `ToolName`: `crates/ploke-core/src/tool_types.rs`; re-exported by `ploke-tui`.
- `ToolUiPayload`: `crates/ploke-tui/src/tools/ui.rs`; re-exported by `ploke-records::tool_contracts`.
- `ToolErrorWire`, `ToolRetryContext`: `crates/ploke-tui/src/tools/error.rs`; re-exported by `ploke-records::tool_contracts`.
- Argument DTOs: `crates/ploke-tui/src/tools/*`; re-exported by `ploke-records::tool_contracts` and wrapped by `ToolCallArguments`.

Dependency shape:

- Current passive graph path is `ploke-tree -> ploke-records(tool-contracts) -> ploke-tui -> ploke-core`.
- This pulls broad TUI/runtime dependencies into passive readers.

Recommendation:

- Keep `ToolName` in `ploke-core`.
- Move passive tool transport DTO ownership into `ploke-records::tool_contracts`.
- Make `ploke-tui` import/re-export those DTOs instead of defining them.
- Remove `ploke-records -> ploke-tui`.
- Do not copy or mirror DTOs; preserve one canonical owner.

