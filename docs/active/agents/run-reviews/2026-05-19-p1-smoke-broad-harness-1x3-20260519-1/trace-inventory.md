# Trace Inventory

Scope: `/home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1` and `/home/brasides/.ploke-eval/worktrees/p1-smoke-broad-harness-1x3-20260519-1`

## Verdict

The campaign has generated trace-bearing files. The worktree checkout itself has no live `prototype1-state` trace files; it only contains copied/archive notes and one old `summary.json` fixture.

## Generated run artifacts

### `/home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/messages/edit-harness-result`

- `node-a87394840086768d.headless-tui.json`
  - records: `attempts=6`, `events=139`
  - event kinds: `proposal=2`, `tool_request=66`, `tool_completed=66`, `tool_failed=4`, `turn=1`
  - terminal: `applied`
  - timestamps: no embedded timestamp-like keys found; file mtime `2026-05-19 02:23:55 -0700`

- `node-a87394840086768d-r2.headless-tui.json`
  - records: `attempts=11`, `events=189`
  - event kinds: `proposal=7`, `tool_request=87`, `tool_completed=88`, `tool_failed=6`, `turn=1`
  - terminal: `applied`
  - timestamps: no embedded timestamp-like keys found; file mtime `2026-05-19 02:37:58 -0700`

### `/home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1`

- `transition-journal.jsonl`
  - records: `2`
  - kinds: `parent_started=1`, `resource=1`
  - recorded_at range: `1779181859381..1779181859398`
  - terminal state: none exposed; last record is `resource`
  - file mtime: `2026-05-19 02:10:59 -0700`

### `/home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1`

- `slice.jsonl`
  - records: `1`
  - shape: single JSON payload object, not an event journal (`kind`/`recorded_at` absent)
  - terminal state: not inferable
  - file mtime: `2026-05-19 02:01:08 -0700`

## Worktree-only copied/archive material

No live trace files were found under `/home/brasides/.ploke-eval/worktrees/p1-smoke-broad-harness-1x3-20260519-1` with the trace-file filters used here.

What did match there was copied/archive material, not current run traces:

- `crates/ploke-tui/ai_temp_data/openrouter_matrix/run-20250831-230037/summary.json`
- `.orchestrator.archive.2026-05-12-egui-task-readability/.../*.report.md`
- `docs/active/agents/.../*.md`
- `docs/active/plans/self-improvement-loop/typed-persistence-spine/traceability-matrix.md`

Treat those as noisy background docs/fixtures for this review.

## Small verification commands

```bash
find /home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1 -type f | rg 'agent-turn|trace.*jsonl|summary.*jsonl|headless-tui|transition-journal|slice\\.jsonl'
wc -l /home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/transition-journal.jsonl /home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/slice.jsonl
jq -s '{recorded_at_min: (map(.recorded_at) | min), recorded_at_max: (map(.recorded_at) | max), kinds: (map(.kind) | group_by(.) | map({kind:.[0], count:length}))}' /home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/transition-journal.jsonl
jq '{event_kinds: ([.events[].kind] | group_by(.) | map({kind:.[0], count:length})), terminal: .terminal}' /home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/messages/edit-harness-result/node-a87394840086768d.headless-tui.json
```
