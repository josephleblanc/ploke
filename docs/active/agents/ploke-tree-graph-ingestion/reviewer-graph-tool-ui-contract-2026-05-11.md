# reviewer-graph tool UI/error contract review 2026-05-11

Review scope: typed tool UI/error contract boundary introduced in `crates/ploke-tui/src/tools/*` and re-exported through `crates/ploke-records/src/tool_contracts.rs`. Review only; no source edits.

## Findings

1. Public serde deserialization leaves the public index API uninitialized. `ToolLlmErrorPayload` derives `Deserialize` while its `index` is `#[serde(skip)]` at `crates/ploke-tui/src/tools/error.rs:409` and `crates/ploke-tui/src/tools/error.rs:421`; the index is rebuilt only by `ToolErrorWire::parse` at `crates/ploke-tui/src/tools/error.rs:562`. Because `ToolErrorWire` and `ToolLlmErrorPayload` are re-exported from `ploke_records::tool_contracts` at `crates/ploke-records/src/tool_contracts.rs:14`, any public consumer using normal `serde_json::from_str::<ToolErrorWire>` gets a value whose `llm["..."]` indexing can panic. Either implement custom `Deserialize` that rebuilds the index, remove the `Index` API from the shared DTO, or make the indexing API total and independent of skipped derived state.

2. Old tool-error wire compatibility is not preserved for `code`. The new wire requires `ToolErrorCode` serde names such as `"invalid_format"` from `#[serde(rename_all = "snake_case")]` at `crates/ploke-tui/src/tools/error.rs:195`, and `ToolErrorWire::parse` is a strict `serde_json::from_str` at `crates/ploke-tui/src/tools/error.rs:564`. The replaced payload writer previously emitted `format!("{:?}", self.code)`, so existing persisted/replayed tool errors with `"code":"InvalidFormat"` will fail to parse and fall back to raw string handling in `crates/ploke-tui/src/llm/manager/session.rs:1009`. Add serde aliases or a compatibility parser if old run records must remain typed.

3. `serde_json::Value` was not fully removed from the production retry-context construction path. `ToolRetryContext` still accepts `From<serde_json::Value>` and recursively projects anonymous JSON at `crates/ploke-tui/src/tools/error.rs:151`, and `crates/ploke-tui/src/rag/tools.rs:533` plus `crates/ploke-tui/src/rag/tools.rs:569` still build retry contexts with `serde_json::json!` before calling `.retry_context(...)`. The persisted wire is typed after conversion, but production still stages shared error facts through anonymous JSON. That keeps a compatibility bridge, not a completed typed-persistence cleanup.

## Notes

- The `ploke_records::tool_contracts` change mechanically matches the existing re-export pattern: shared DTO identity still comes from `ploke_tui::tools` and is re-exported at `crates/ploke-records/src/tool_contracts.rs:14`. The dependency direction remains a boundary smell because enabling `ploke-records/tool-contracts` pulls `ploke-tui`, but this patch extends an existing pattern rather than inventing a new one.
- The new `ToolRetryContext` wire shape is bounded and first-order, which is the right direction for the typed-persistence invariant once the remaining `Value` bridge is removed or isolated as explicit legacy compatibility.

## Verification

- `cargo check -p ploke-records --features tool-contracts`: passed.
- `cargo test -p ploke-records --features tool-contracts tool_error_wire_roundtrips_typed_retry_context 2>&1 | tail -n 40`: passed, 1 test.
- `cargo test -p ploke-tui validators 2>&1 | tail -n 40`: passed, 10 tests.
- `cargo check -p ploke-records`: passed.

Changed files: this report only.
