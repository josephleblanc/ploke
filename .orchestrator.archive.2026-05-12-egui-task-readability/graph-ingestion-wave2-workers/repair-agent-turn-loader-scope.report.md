Completed:
- Agent-turn loader discovery now checks only direct files under the concrete
  run root:
  - `run_root/agent-turn-trace.json`
  - `run_root/agent-turn-summary.json`
- Recursive run-root filename discovery was removed.
- The loader test now includes nested lookalike files under
  `nodes/child/output/` and asserts they are ignored.

Changed files:
- `crates/ploke-tree/src/store/fs.rs`

Checks:
- `rustfmt crates/ploke-tree/src/store/fs.rs`
- `cargo test -p ploke-tree load_record_set_loads_agent_turn_trace_and_summary_evidence 2>&1 | tail -n 40` passed.
- `cargo check -p ploke-tree 2>&1 | tail -n 40` passed.

Remaining issue:
- The dependency boundary is still pending in the records lane:
  `ploke-tree` still reaches agent-turn records through the current
  `ploke-records/tool-contracts` feature.
