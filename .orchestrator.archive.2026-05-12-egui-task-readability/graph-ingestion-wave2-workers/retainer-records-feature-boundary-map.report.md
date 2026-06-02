Findings:
- The earlier review wording overstated the current dependency problem:
  `agent_turn` is not currently feature-gated, `tool-contracts = []` is empty,
  and no actual `ploke-tui` dependency appears in `ploke-tree`'s resolved graph.
- The remaining problem is still real: `ploke-tree` asks for the broad
  `tool-contracts` feature to parse passive agent-turn records, and
  `agent_turn` imports passive transport carriers from `tool_contracts`.

Smallest repair map:
- Add a small passive transport feature/module in `ploke-records`, such as
  `tool-transport`.
- Gate `agent_turn` behind `agent-turn = ["tool-transport"]`.
- Gate decoded tool contracts behind `tool-contracts = ["tool-transport"]`.
- Move passive wire carriers used by agent-turn into `tool_transport`.
- Keep decoded tool argument DTOs and `ToolArgumentsJson::decode_for_tool` in
  `tool_contracts`.
- Change `ploke-tree` to request `["protocol", "agent-turn"]`.

Verification:
- `cargo check -p ploke-records --no-default-features` passed.
- `cargo check -p ploke-records --no-default-features --features protocol` passed.
- `cargo test -p ploke-records agent_turn --no-default-features 2>&1 | tail -n 40` passed.

Changed files: none.
