# Design branch comparison (Phase 2)

Fork point: `feature/better-egui` @ `267b7b51` (theme foundation).

Review each branch in its worktree with trunk on a distinct port, then cherry-pick or merge winners onto `feature/better-egui`.

| Branch | Worktree | Commit | Trunk port | Focus |
|--------|----------|--------|------------|-------|
| `design/analyst-dashboard` | `../ploke-design-analyst` | `50ce11fb` | 8081 | Run Health card, promoted Analyst Snapshot, VerdictMixPanel ×3 |
| `design/trace-grid` | `../ploke-design-trace` | `84d82784` | 8082 | Shared `scan_grid.rs`; Agent Trace grid when tools > 8 |
| `design/density-polish` | `../ploke-design-density` | `5585adb2` | 8080 | `Frame::NONE` timeline, compact chips/cards, full-width tracks |

## Quick serve

```bash
# From each worktree root
env -u NO_COLOR trunk serve --config crates/ploke-egui/Trunk.toml --address 127.0.0.1 --port <PORT>
```

Load: `/?graph=/benchmark-fixtures/protocol-graph.json` (symlink or copy fixture into `benchmark-fixtures/` first).

## Screenshot checklist (all branches)

1. Full window after graph load
2. Eval & Protocol center pane
3. Agent Trace inspector (57-tool turn on protocol graph)
4. Bottom timeline
5. One dark + one light palette via theme selector

Per-branch notes live in each worktree under `docs/active/agents/ploke-egui-design-autonomy/captures/<branch-name>/README.md`.

## Gate status

All three branches passed `cargo test -p ploke-egui --features "dev,native-benchmark" benchmark_regression` (baseline self-compare). Run `benchmark_regression_against_baseline -- --ignored` on the main repo before merging if you need live median/p95 confirmation.

## Suggested merge order

1. **density-polish** — layout rhythm (low conflict)
2. **analyst-dashboard** — Eval & Protocol surface
3. **trace-grid** — may touch `call_review.rs` / `scan_grid.rs`; merge after analyst or resolve shared `scan_grid` first
