# How to add a new tool with config

This guide is the canonical checklist for adding a new tool to `ploke-tui`. It reflects the
current split between `ploke-core` (tool names/descriptions) and `ploke-tui` (tool wiring).

## A. Create the tool type and module

1. Add a new module in `crates/ploke-tui/src/tools/` (e.g., `list_dir.rs`).
2. Define a label type:

```rust
pub struct ListDir;
```

## B. Implement `Tool` for the label type

```rust
impl Tool for ListDir {
  // methods
}
```

Before wiring, define:
- Params/owned params structs (borrowed + owned).
- Output struct serialized into `ToolResult.content` (JSON).
- Any helper enums or validation helpers needed for clean parsing.

### 1) `name`

- Add a new variant to `ToolName` in `crates/ploke-core/src/tool_types.rs`.
- Update `ToolName::ALL` and `ToolName::as_str`.
- This is the name exposed to the LLM as a tool call.

### 2) `description`

- Add a new variant to `ToolDescr` in `crates/ploke-core/src/tool_types.rs`.
- The new variant should be marked with `#[serde(rename = "<description>")]`.
- This description is exposed to the LLM and should provide concise instructions on when/where/how
  to use the tool.

### 3) `schema`

- Create a `lazy_static` with the JSON Schema for the tool parameters.

e.g.
```rust
lazy_static::lazy_static! {
    static ref CODE_EDIT_PARAMETERS: serde_json::Value = json!({
        "type": "object",
        "properties": {
            "edits": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "file": { "type": "string", "description": "Absolute or workspace-relative file path." },
                        "canon": { "type": "string", "description": "Canonical path to the node, e.g. crate::module::Item" },
                        "node_type": { "type": "string", "description": "Node type (function|struct|enum|...)." },
                        "code": { "type": "string", "description": "Replacement code for the node." }
                    },
                    "required": ["file", "canon", "node_type", "code"],
                    "additionalProperties": false
                }
            },
            "confidence": {
                "type": "number",
                "minimum": 0.0,
                "maximum": 1.0,
                "description": "Optional confidence indicator for the edit proposal."
            }
        },
        "required": ["edits"],
    });
}
```

- Ensure tests are added for the JSON schema value and that the output contains the expected
  descriptions.

### 4) Error handling

- Prefer `ToolInvocationError` mapping and `ToolError` helpers in
  `crates/ploke-tui/src/tools/error.rs`.
- For user-facing validation failures, return `tool_ui_error(...)` from `execute`.
- For I/O failures, return `tool_io_error(...)` or map them to `ToolErrorCode::Io`.

## C. Register the tool with the runtime

1. Add the new module to `pub mod ...` in `crates/ploke-tui/src/tools/mod.rs`.
2. Update `process_tool` to deserialize, execute, and emit events for the new tool.
3. Add the tool to the LLM tool list in `crates/ploke-tui/src/llm/manager/mod.rs`.
4. If the tool needs other crates, wire their contexts inside `Tool::build` or `execute`.
5. If the tool needs a shared output type, add it to `ploke-core` instead of duplicating.

## D. Execution + tests

1. Implement `Tool::execute` with the correct ctx wiring before sending work to other crates.
2. Emit `emit_err` on any deserialization/validation failures (see `ToolInvocationError` in
   `crates/ploke-tui/src/tools/error.rs` for mapping guidance).
3. If the tool returns structured data, add a `ToolUiPayload` summary for UI visibility.
4. Add unit tests covering schema, `into_owned`, and at least one execution path (mock dependencies when possible).
5. Run the relevant `cargo test -p <crate>` targets before submitting changes.

## E. Cross-crate work

- When adding supporting APIs (e.g., new IoManager requests), document the data flow in the same PR.
- Keep typed responses in `ploke-core` when they are shared between tools.
- Update any helper docs or diagrams so future tool authors can follow the new pattern.
