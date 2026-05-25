# Records Worker Report - Tool UI Contract Boundary - 2026-05-11

Task: `records-tool-ui-contract-boundary`

Changed files:

- `crates/ploke-tui/src/tools/error.rs`
- `crates/ploke-tui/src/tools/mod.rs`
- `crates/ploke-tui/src/tools/create_file.rs`
- `crates/ploke-tui/src/tools/ns_patch.rs`
- `crates/ploke-tui/src/tools/validators.rs`
- `crates/ploke-records/src/tool_contracts.rs`

Implemented:

- Added typed `ToolRetryContext`, `ToolLlmErrorPayload`, and typed LLM error
  inspection values.
- Changed `ToolErrorWire.llm` from `serde_json::Value` to the typed payload.
- Changed `ToolError.retry_context` from `serde_json::Value` storage to typed
  retry context.
- Re-exported shared UI/error DTOs through
  `ploke_records::tool_contracts`.
- Added a `ploke-records` roundtrip test for typed tool error wire retry
  context.
- Updated allowed tool callsites to build typed retry contexts instead of
  `json!`.

Verification reported by worker:

- `cargo check -p ploke-records --features tool-contracts`
- `cargo check -p ploke-records`
- `cargo test -p ploke-records --features tool-contracts tool`
- `cargo test -p ploke-tui validators`

Handoff:

- Docs that still describe agent-turn payload parsing as blocked need review and
  likely update after this patch is accepted.
- The next implementation step is agent-turn passive record ownership/loading,
  not changes in `ploke-eval`.
