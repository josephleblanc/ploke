Findings:
- Medium: agent-turn passive records are typed, but the module is gated behind
  `tool-contracts`, which currently pulls `ploke-tui` into passive readers.
- No production `serde_json::Value` walking was found in the agent-turn record
  owner. Test JSON literals are fixture-only.

Verification:
- `cargo check -p ploke-tree --tests 2>&1 | tail -n 40` passed.
- `cargo test -p ploke-records agent_turn --features tool-contracts 2>&1 | tail -n 40` passed.
- `cargo test -p ploke-tree load_record_set_loads_agent_turn_trace_and_summary_evidence 2>&1 | tail -n 40` passed.

Decision:
- Review is not accepted until the feature boundary is repaired so `ploke-tree`
  can parse passive agent-turn records without depending on `ploke-tui`.
