Completed:
- Added graph evidence subjects/locators for agent-turn evidence.
- `Graph::from_records(&RunRecordSet)` ingests `PassiveEvidence.agent_turns`
  without filesystem discovery or deserialization.
- Nested `nodes/<node-id>/.../agent-turn-*.json` evidence joins to known
  branch/runtime/operation metadata when available.
- Agent-turn data remains evidence/metadata only and does not create History
  authority.

Changed files:
- `crates/ploke-tree/src/graph/types/evidence.rs`
- `crates/ploke-tree/src/graph/build.rs`
- `crates/ploke-tree/src/graph/build/passive.rs`
- `crates/ploke-tree/src/graph/build/run_attempts.rs`
- `crates/ploke-tree/src/graph/build/scheduler.rs`

Checks:
- `cargo test -p ploke-tree graph -- --nocapture 2>&1 | tail -n 80` passed.
- `cargo check -p ploke-tree 2>&1 | tail -n 80` passed.

Residual ambiguity:
- Agent-turn artifacts do not carry explicit runtime IDs, operation coordinates,
  or tool-call graph IDs. Root-level agent-turn files remain evidence-only.
  Nested node paths join by node id and may attach to multiple known runtimes or
  operations for that node.
