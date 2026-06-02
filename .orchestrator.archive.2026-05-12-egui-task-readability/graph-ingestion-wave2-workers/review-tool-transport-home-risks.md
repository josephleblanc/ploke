# Tool Transport Home Risk Review

Changed files: none.

Findings:

1. High: `ploke-records/tool-contracts` still pulls the full TUI/runtime dependency graph because it depends on `ploke-tui` and re-exports from `ploke_tui::tools`.
2. Medium: public API drift remains likely while passive DTOs are defined inside active TUI tool modules.
3. Medium: `ToolErrorWire` lives beside non-passive error machinery, so re-exporting it compiles more than the passive surface needs.

Serde / copy invariant:

- No current serde regression found by the reviewer.
- Legacy `ToolErrorCode` aliases and custom `ToolLlmErrorPayload` deserialize behavior were reported present.
- No DTO mirror was found outside the current owner.
- If moved, keep exactly one owner and make old paths `pub use` that canonical owner.

Commands reported by reviewer:

- `cargo check -p ploke-records --features tool-contracts --no-default-features`
- `cargo test -p ploke-records --features tool-contracts tool_error_wire`
- `cargo tree -p ploke-records --features tool-contracts --no-default-features`

