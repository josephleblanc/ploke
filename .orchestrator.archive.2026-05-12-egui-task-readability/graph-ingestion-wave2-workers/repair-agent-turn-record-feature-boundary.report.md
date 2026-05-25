Completed:
- `agent_turn` is exposed without requiring `tool-contracts`.
- `tool-contracts` no longer enables `dep:ploke-tui`.
- Passive tool DTOs are owned directly in `ploke-records`.
- `ToolArgumentsJson` uses `serde_json::value::RawValue`.

Changed files:
- `crates/ploke-records/Cargo.toml`
- `crates/ploke-records/src/lib.rs`
- `crates/ploke-records/src/tool_contracts.rs`

Checks:
- `cargo check -p ploke-records` passed.
- `cargo check -p ploke-records --locked` passed.
- `cargo check -p ploke-records --features tool-contracts` passed.
- `cargo test -p ploke-records agent_turn` passed.
- `cargo test -p ploke-records tool_call_arguments` passed.
- `cargo check -p ploke-tree --tests` passed.
- `cargo test -p ploke-tree load_record_set_loads_agent_turn_trace_and_summary_evidence` passed.
- `cargo check -p ploke-eval` passed with existing warnings.
- `cargo tree -p ploke-tree -e normal | rg "ploke-tui|ploke-records"`
  showed `ploke-records` only, no `ploke-tui`.

Remaining note:
- `ploke-tui` remains as an unused optional dependency entry in
  `ploke-records/Cargo.toml`.
