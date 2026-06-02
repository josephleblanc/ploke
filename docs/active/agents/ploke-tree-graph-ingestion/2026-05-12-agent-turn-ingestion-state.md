# Agent-Turn Graph Ingestion State - 2026-05-12

This note folds forward the wave2 worker/reviewer reports and the post-compact
feature-boundary correction for the agent-turn family.

## Reported Progress

- Passive agent-turn records have a typed owner in
  `ploke_records::agent_turn`.
- `ploke-records` exposes `agent_turn` only with the `tool-contracts` feature,
  keeping default `ploke-records` from pulling the canonical tool DTO home.
- `ploke-tree` has a `RunRecordSet` loader path for direct run-root
  agent-turn trace and summary evidence.
- Store deserialization uses named `ploke_records::agent_turn`
  types rather than production `serde_json::Value` field walking.
- `Graph::from_records` attaches agent-turn trace/summary artifacts as passive
  evidence. Ambiguous node-scoped artifacts do not weak-join to runtime or
  operation nodes by node id.
- The loader test is
  `cargo test -p ploke-tree load_record_set_loads_agent_turn_trace_and_summary_evidence`.

## Current Gaps

- Dependency cleanup remains unresolved: `ploke-tree` reaches agent-turn parsing
  through `ploke-records/tool-contracts`, and the canonical tool DTOs currently
  live in `ploke-tui`.
- Do not resolve the dependency issue by copying or mirroring `ToolName`,
  `ToolUiPayload`, `ToolErrorWire`, or related tool DTOs into `ploke-records`.
  The next architecture lane should move passive tool transport DTOs once to a
  shared canonical home, while keeping tool execution and validation authority
  in `ploke-tui`.
- Agent-turn graph ingestion is artifact-level passive evidence. It does not yet
  create dedicated tool call/result operation nodes.
- Provider attempt/retry/timeout and full-response sidecars are not covered by
  the agent-turn family. They need a separate passive owner, typed JSONL loader,
  and graph attachment path.
- Do not use private eval CLI projection shapes as graph authority for
  provider/full-response evidence.

## Verification Evidence

- `cargo test -p ploke-records agent_turn 2>&1 | tail -n 80` passed with
  zero tests under default features.
- `cargo test -p ploke-records --features tool-contracts agent_turn 2>&1 | tail -n 80` passed.
- `cargo test -p ploke-records --features tool-contracts tool_call_arguments 2>&1 | tail -n 80` passed.
- `cargo test -p ploke-tree load_record_set_loads_agent_turn_trace_and_summary_evidence 2>&1 | tail -n 80` passed.
- `cargo test -p ploke-tree graph 2>&1 | tail -n 80` passed.
- `cargo check -p ploke-tree --tests 2>&1 | tail -n 80` passed.
