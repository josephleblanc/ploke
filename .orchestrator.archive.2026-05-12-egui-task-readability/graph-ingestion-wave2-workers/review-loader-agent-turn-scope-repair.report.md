Findings:
- No blocking findings.
- Direct run-dir discovery matches the current producer shape:
  `<instance>/runs/run-<timestamp>-<arm>-<uuid8>/agent-turn-trace.json`
  and `agent-turn-summary.json`.
- The loader now checks direct `run_root.join(file_name)` paths and no longer
  uses recursive agent-turn discovery.
- Regression coverage creates direct files plus nested lookalikes and asserts
  nested lookalikes are ignored.

Verification:
- `cargo test -p ploke-tree load_record_set_loads_agent_turn_trace_and_summary_evidence 2>&1 | tail -n 40` passed.
- Targeted `rg` found no recursive agent-turn discovery in `fs.rs`.

Changed files: none.
