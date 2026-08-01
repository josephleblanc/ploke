Findings:
- High: `ploke-tree` currently depends on `ploke-records/tool-contracts`, which
  pulls in `ploke-tui` through `ploke-records`.
- High: agent-turn discovery recursively scans the whole run root and can enter
  worktrees, target directories, caches, logs, or unrelated files.
- Existing unrelated `ploke-eval` dirty files are present in the worktree. They
  are outside this task and must not be staged with this wave.

Positive checks:
- Store deserialization goes through named `ploke_records::agent_turn` types.
- No graph authority is introduced in the store loader.

Decision:
- Review is not accepted until dependency and discovery-scope repairs land.
