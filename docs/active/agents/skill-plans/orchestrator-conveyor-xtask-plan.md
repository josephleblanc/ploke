# Orchestrator Conveyor Xtask Plan

This tracks improvements to the `orchestrator-conveyor` skill and the
`target/debug/xtask orchestrate` board helper.

## Current Evidence

- Skill file: `.codex/skills/orchestrator-conveyor/SKILL.md`.
- CLI entry point: `xtask/src/commands/orchestrate.rs`.
- Lane validation module: `xtask/src/commands/orchestrate/lanes.rs`.
- Focused CLI tests: `xtask/src/commands/orchestrate/tests.rs`.
- Current ignored local state roots: `.orchestrator/` and `.orchestrator-archive/`.

Observed review points:

- `complete --summary` already exists and has a focused test, so the skill's Known Tool Gaps are stale.
- Default human output for `orchestrate` falls back to pretty JSON.
- `--format compact` emits the whole board as one long JSON line.
- `--format table` currently returns "Table formatting not yet implemented".
- `xtask/src/commands/orchestrate.rs` is too broad for a CLI command module: it owns args, board schema, state mutations, locking, packet rendering, and report writing.
- 2026-05-21 `rust-review-discipline` follow-up found that usage telemetry must be best-effort, task lifecycle transitions still need a board-owned API, `status --set` must not make busy workers look idle, and usage counts should distinguish `status --set` from plain `status`.

## Borrowed Patterns

These ideas are adapted from lightweight planning systems, but the goal here is
still a local agent board helper rather than a full issue tracker.

- Kanban: WIP limits and flow warnings are useful as local guardrails, but
  should warn before they block work.
- Taskwarrior: tags and virtual tags map well to task sets and saved dynamic
  views.
- GitHub Projects: simple field filters are useful when they remain bounded and
  predictable.
- Linear: active, backlog, and archive are separate concepts. Backlog and
  parked work should stay live board state; archive should be historical.

Reference docs:

- Kanban Guide: https://resources.kanban.university/wp-content/uploads/2021/06/The-Official-Kanban-Guide_US.pdf
- Taskwarrior tags and virtual tags: https://taskwarrior.org/docs/tags/
- GitHub Projects filtering: https://docs.github.com/en/issues/planning-and-tracking-with-projects/customizing-views-in-your-project/filtering-projects
- Linear team pages: https://linear.app/docs/default-team-pages

## CLI Surface Frame

- Source facts: the persisted board file, lane definitions, workers, tasks, blockers, events, generated packet paths, report paths, task sets, saved views, usage metadata, timestamps, and policy settings.
- Semantic object: the orchestration board, including worker slots, lane ownership, task lifecycle, blocker lifecycle, task-set membership, saved views, health checks, WIP policy, notes, and command usage counters.
- Projection: bounded status, worker packet, lane validation, blocker list, task-set view, saved view, board health, archive/reset summary, usage summary, and policy warning summary.
- Renderer: human text, compact JSON, and table-like text. CLI code should dispatch and render, not become the semantic owner.

## Improvement Inventory

This is the feature inventory. The execution order is the slice list below,
because several features depend on fields or projections introduced by earlier
slices.

1. Sync the skill documentation.
   - Remove `complete --summary` from Known Tool Gaps.
   - Add `complete --summary` to the normal workflow examples.
   - Clarify that routine status checks should use bounded projections.

2. Split the command implementation into smaller modules.
   - Keep clap args and dispatch thin.
   - Move board schema and transitions behind a board module.
   - Keep lane validation in its own module.
   - Move packet rendering and report/archive rendering out of the command module.

3. Add bounded status output.
   - Add a routine status view with counts by task state, active workers, queue lengths, blockers, and lane ownership.
   - Add stale retainers later once retainer refresh tracking exists.
   - Avoid requiring `jq` for ordinary check-ins.
   - Keep full board JSON available for explicit machine-readable inspection.

4. Add a blocker lifecycle.
   - Add `unblock` or `block resolve`.
   - Record resolution summary/evidence.
   - Restore the task to an explicit next state rather than leaving stale blockers current.

5. Add task-set activation and filtering.
   - A task can belong to one or more named sets, such as `current-thread`, `other-thread:<name>`, `backlog`, `parked`, or `review-wave`.
   - A current orchestrator thread can choose one set, or a small declared set list, as its routine active view.
   - Tasks outside the active set are not archived or forgotten; they are still board state, but routine status and packet generation can ignore them unless requested.
   - Add CLI filtering, for example `orchestrate status --set <set>`, `orchestrate packet <worker> --set <set>`, or a broader `--task-set <set>` argument.
   - Add commands to manage membership, for example `task-set create`, `task-set activate`, `task-set add`, and `task-set remove`.
   - Keep this as board semantics, not only renderer filtering, so another thread can own an active set without colliding with the current thread's default view.
   - Avoid one mutable global active set. Prefer explicit `--set` selection first, then a scoped local default such as a thread/profile setting if repeated use proves it is worth the state.

6. Add lightweight usage tracking.
   - Store local-only counts under `.orchestrator/`, for example `.orchestrator/usage.json`.
   - Count command paths such as `status`, `status --set`, `packet`, `complete --summary`, `task-set activate`, and `block resolve`.
   - Track only coarse usage counts and optional `last_used_at`; do not store prompts, worker packet contents, user text, or full argument values.
   - Keep the persisted shape owned by a named Rust type, not `serde_json::Value` field walking.
   - Add a small usage projection, for example `orchestrate usage`, so we can see which planned helpers are actually used over time.
   - Treat usage counts as planning feedback only. They are not orchestration authority and should not affect task lifecycle decisions.

7. Apply rust-review corrective slice before new features.
   - Record usage only as best-effort telemetry; bad or future usage metadata must not block board commands.
   - Count successful command paths with enough granularity to tell whether `status --set` is being used.
   - Move lifecycle mutations for assign, complete, review, block, and unblock behind board-owned transition helpers.
   - Preserve real worker occupancy in filtered status views so a worker active outside the selected set does not appear idle.
   - Add focused tests for malformed usage metadata, failed command counting, filtered worker occupancy, and invalid lifecycle transitions.

8. Add saved views.
   - Keep task sets as explicit metadata and views as dynamic projections over board state.
   - Start with built-in views such as `active`, `ready`, `review`, `blocked`, `stale`, and `all`.
   - Allow saved named views later if the built-ins prove useful.
   - Ensure views are read-only projections; changing a view must not mutate task lifecycle state.

9. Add a small filter language.
   - Keep it intentionally narrow: `set:<name>`, `lane:<name>`, `state:<state>`, `worker:<id>`, `stale:true`, `blocked:true`, `no:worker`, and negation such as `-state:reviewed`.
   - Let `status --set` start as direct task-set lookup, then make it one case of the filter/view machinery after filters exist.
   - Use filters to power `status --view` and future saved views.
   - Avoid generic expression parsing until real usage proves it is needed.

10. Add board health checks.
   - Add `orchestrate check` as a non-mutating command.
   - Start with integrity checks: lane validation, missing task/blocker references, blocked tasks without unblock actions, worker active tasks that no longer exist, missing allowed edit surfaces for writer tasks, and conflicting active assignments.
   - Add stale packet and stale task checks only after timestamp fields exist.
   - Keep health findings as warnings or validation output; do not mutate the board.

11. Add task aging and packet staleness.
   - Track timestamps for task creation, assignment, activation, completion, review, block, unblock, and packet generation.
   - Surface stale active tasks, stale packets, and long-waiting review items in bounded status and health checks.
   - Keep thresholds configurable but start with conservative defaults.

12. Add WIP policy warnings.
   - Track simple local limits such as active tasks per lane, active implementation workers, blocked task count, and unreviewed completion count.
   - Report WIP violations as warnings in `status --brief` and `orchestrate check`.
   - Do not prevent command execution until warning-only behavior proves insufficient.

13. Add decision notes.
   - Add a small `note` command for task-scoped or board-scoped decisions.
   - Use notes for why a task moved to another thread, why a helper idea was retired, or why a blocker was accepted.
   - Keep notes short and structured enough to render counts and latest summaries without reading full reports.

14. Track planned-feature lifecycle.
   - Track helper ideas as `planned`, `implemented`, `used`, `promoted`, or `retired`.
   - Use usage counts to identify commands or views that were implemented but never used.
   - Treat retirement as cleanup guidance, not an automatic deletion rule.

15. Add archive/reset ergonomics.
   - Add `archive` or `reset --archive` for a clean wave.
   - Prefer timestamped archive moves into `.orchestrator-archive/` over destructive deletion.
   - Keep task sets as the non-archive alternative for still-live work.

16. Make retainer refresh actionable.
   - Add a command to record answered retainer questions.
   - Surface stale retainers in bounded status.

17. Improve worker packets.
   - Include lane docs and notes.
   - Include relevant open blockers and proposed unblock actions.
   - Include the current task-set context.
   - Include the rule that workers do not mutate the board.

## Suggested Slice Order

1. Documentation sync and this plan.
2. Module split with no behavior change.
3. Lightweight usage counters and `orchestrate usage`.
4. Bounded status projection and renderer, without future-only stale claims.
5. Task-set model and direct `status --set`.
6. Blocker resolution.
7. Rust-review corrective slice: best-effort usage, board-owned lifecycle transitions, filtered worker occupancy, and `status --set` usage counts.
8. Timestamp/staleness fields for tasks and packets.
9. Board health check.
10. Built-in views and narrow filters.
11. Packet filtering and task-set-aware packet rendering.
12. Retainer refresh tracking.
13. WIP warning policy.
14. Decision notes.
15. Planned-feature lifecycle tracking.
16. Archive/reset command.

## Progress

- [x] Create durable plan under `docs/active/agents/skill-plans/`.
- [x] Sync `orchestrator-conveyor` skill documentation.
- [x] Split `xtask/src/commands/orchestrate.rs` into smaller modules.
- [x] Add local usage counters under `.orchestrator/`.
- [x] Add bounded status projection.
- [x] Add task-set model and direct `status --set`.
- [x] Add blocker resolution.
- [x] Apply rust-review corrective slice for usage, lifecycle transitions, and filtered worker occupancy.
- [x] Add task aging and packet staleness.
- [x] Add board health check.
- [ ] Add saved views and narrow filters.
- [ ] Improve worker packet rendering.
- [ ] Add retainer refresh tracking.
- [ ] Add WIP policy warnings.
- [ ] Add decision notes.
- [ ] Add planned-feature lifecycle tracking.
- [ ] Add archive/reset command.
