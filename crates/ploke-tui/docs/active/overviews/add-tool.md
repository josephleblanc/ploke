# How to add a new tool with config

## A. Create type label, e.g. 

```rust
pub struct NsPatch;
```

## B. implement `Tool` for the label type

```rust
impl Tool for NsPatch {
  // methods
}
```

On writing the `Tool` methods:

1. `name`

- add a new variant to `ToolName` enum
- this is the name that will be exposed to the llm as a tool call

2. `description`

- add a new variant to `ToolDescr`
- new variant should be marked with `#serde(rename = "<description>"]`
- this description will be exposed to the LLM, and should provide concise instructions on when/where/how to use the tool

3. `schema`

- create a `lazy_static` with the json representation of the tool

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

- ensure tests are added for the json value translation, and that the output contains the expected descriptions.

## C. Register the tool with the runtime

1. Add the new type to `pub mod ...` in `ploke/crates/ploke-tui/src/tools/mod.rs`.
2. Extend `ToolName` and `ToolDescr` enums with the new entry and description.
3. Update `process_tool` to deserialize, execute, and emit events for the new tool.
4. If the tool needs other crates, wire their contexts inside `Tool::build` or `execute`.

## D. Execution + tests

1. Implement `Tool::execute` with the correct ctx wiring before sending work to other crates.
2. Emit `emit_err` on any deserialization/validation failures.
3. Add unit tests covering schema, `into_owned`, and at least one execution path (mock dependencies when possible).
4. Run the relevant `cargo test -p <crate>` targets before submitting changes.

## E. Cross-crate work

- When adding supporting APIs (e.g., new IoManager requests), document the data flow in the same PR.
- Keep typed responses in `ploke-core` when they are shared between tools.
- Update any helper docs or diagrams so future tool authors can follow the new pattern.
