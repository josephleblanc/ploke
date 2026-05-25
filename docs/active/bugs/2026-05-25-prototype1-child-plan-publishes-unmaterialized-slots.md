# Prototype 1 Child Plan Publishes Unmaterialized Slots

Status: source fixed
Discovered: 2026-05-25
Fixed: 2026-05-25

## Summary

The `p1-gemini35-flash-direct-fresh-20260524-193515` loop reached
`child_plan` and produced three broad-harness child attempts, but the same
bounded `prototype1-step` also published prompt files for slots `r4` through
`r9` without materializing the corresponding candidate workspaces. The next
doctor check reports `phase = blocked` because prompt preflight treats those
published prompt files as live obligations and cannot find their workspaces or
protected-core files.

This is a controller/artifact-accounting blocker. The campaign now contains
durable prompt artifacts that describe nonexistent child workspaces, so the
current run should not be advanced for clean loop evidence until the transition
model is fixed.

## Evidence

Campaign:

```text
p1-gemini35-flash-direct-fresh-20260524-193515
```

Bounded command:

```text
/home/brasides/code/ploke/target/debug/ploke-eval loop prototype1-step \
  --repo-root /home/brasides/.ploke-eval/worktrees/p1-gemini35-flash-direct-fresh-20260524-193515 \
  --format json
```

Scheduler policy says the child budget is exactly three:

```text
child_budget.min = 3
child_budget.max = 3
child_schedule_mode = full-batch
```

The step produced three child artifacts:

```text
node-b19077fc35c373b5     -> commit f5c5ba3f
node-b19077fc35c373b5-r2  -> commit fdc7d55c
node-b19077fc35c373b5-r3  -> commit c1cd3143
```

The campaign nevertheless has published prompt files for r4-r9:

```text
prototype1/messages/edit-harness-request/node-b19077fc35c373b5-r4.md
prototype1/messages/edit-harness-request/node-b19077fc35c373b5-r5.md
prototype1/messages/edit-harness-request/node-b19077fc35c373b5-r6.md
prototype1/messages/edit-harness-request/node-b19077fc35c373b5-r7.md
prototype1/messages/edit-harness-request/node-b19077fc35c373b5-r8.md
prototype1/messages/edit-harness-request/node-b19077fc35c373b5-r9.md
```

No corresponding workspace directories exist for those slots. Doctor reports:

```text
phase = blocked
allowed_actions = ["doctor"]
prompt preflight missing dir for candidate workspace referenced by prompt: .../node-b19077fc35c373b5-r4
prompt preflight missing file for protected core file referenced by prompt: .../node-b19077fc35c373b5-r4/crates/ploke-eval/src/cli/prototype1_state/backend.rs
...
prompt preflight missing dir for candidate workspace referenced by prompt: .../node-b19077fc35c373b5-r9
prompt preflight missing file for protected core file referenced by prompt: .../node-b19077fc35c373b5-r9/crates/ploke-eval/src/cli/prototype1_state/backend.rs
```

The child-plan message admits three children, matching the child budget, not
nine.

## Expected Behavior

For a child budget of three, the controller should either:

- publish only the three prompt/workspace pairs it intends to execute; or
- materialize every published prompt's candidate workspace before the prompt is
  visible to preflight; or
- record unused slots as canceled/skipped so doctor does not treat them as
  missing live workspaces.

Published prompt artifacts must not outlive their corresponding workspace
materialization state as unclassified obligations.

## Impact

This blocks further loop progress from the campaign even though three child
attempts produced candidate commits. It also makes later run review noisy:
prompt preflight correctly sees missing directories, but the missing
directories are artifacts of controller over-publication rather than missing
operator setup.

## Disposition

Blocker. Stop advancing this campaign for clean loop evidence; the existing
campaign artifacts were not patched or reinterpreted.

Source fix: `prototype1-doctor` prompt preflight now derives live broad-harness
prompt obligations from request IDs carried in the child plan's request-bound
harness evidence. Published prompt files that are not referenced by the child
plan remain checked as prompt artifacts, but their candidate workspace paths are
classified as unused slots and do not block doctor.

Regression: `prompt_preflight_skips_unused_published_slot_workspaces_after_child_plan`
covers a published `r2` prompt without a materialized workspace after the child
plan live set contains only the first request.

Verification:

```text
cargo test -p ploke-eval prompt_preflight_skips_unused_published_slot_workspaces_after_child_plan
cargo test -p ploke-eval prompt_preflight
```
