---
name: ploke-diagnostic-loop-run
description: Use this skill when running or diagnosing a bounded Prototype 1 `ploke-eval loop` advance, especially when the user wants to take one `prototype1-step`, classify why a loop run did or did not progress, collect evidence from doctor/closure/run artifacts, decide whether a blocker is provider/env/config/model/tooling, or turn findings into a run review, bug report, doctor check, or regression test.
---

# Ploke Diagnostic Loop Run

## Overview

Use this skill to operate a Prototype 1 loop as a diagnostic cycle:
advance one bounded phase, read the durable artifacts, classify the result, and
leave a clear next action. Pair it with `ploke-prototype1-run-setup` before a
fresh campaign exists and with `ploke-run-review` once a run root needs deeper
trace reconstruction.

## Boundaries

- Prefer `prototype1-step` for diagnostic work. Use `prototype1-continue` only
  when the user explicitly asks for continuous execution.
- Do not run overlapping loop/eval commands against the same campaign or
  worktree.
- Do not treat terminal summaries as authoritative when artifacts disagree.
- Do not print raw environment, API keys, bearer tokens, credential file
  contents, or unnecessary provider metadata. Report only presence/absence,
  route names, and bounded error classes.
- Do not mutate run artifacts while diagnosing. Read them, then decide whether
  code/docs/skill/bug-report work is needed in the source checkout.
- If a run root exists and the user asks "what actually happened?", switch to
  `ploke-run-review` for detailed trace work.

## Diagnostic Cycle

### 1. Anchor The Run

Resolve the exact campaign, worktree, parent branch, parent node, profile, and
model/provider route before advancing anything.

Useful probes:

```bash
git status --short --branch
git worktree list
./target/debug/ploke-eval loop prototype1-doctor --repo-root . --format json
./target/debug/ploke-eval closure status --campaign <campaign> --format json
```

Record the current phase and allowed actions. If doctor reports blockers, do not
advance until the blockers are understood or the user explicitly accepts them.

### 2. Inspect The Admitted Configuration

Read configuration from the campaign and admitted profile, not from memory or
path names:

```bash
cat ~/.ploke-eval/campaigns/<campaign>/campaign.json
cat ~/.ploke-eval/campaigns/<campaign>/prototype1/run-profile.toml
cat ~/.ploke-eval/campaigns/<campaign>/closure-state.json
```

Check at least:

- active/eval model and provider;
- protocol model and provider, if split;
- protocol policy such as max tokens, concurrency, required procedures, and
  any model-specific knobs;
- MBE/oracle settings;
- run bounds such as max generations, total nodes, and child fanout.

When model/provider behavior is the suspected blocker, inspect the current model
surfaces before rerunning paid work:

```bash
./target/debug/ploke-eval model current
./target/debug/ploke-eval model parent-patcher current
./target/debug/ploke-eval model provider current --model-id <model-id>
./target/debug/ploke-eval model providers <model-id>
```

### 3. Take One Bounded Advance

Run a single phase advance from the campaign worktree:

```bash
./target/debug/ploke-eval loop prototype1-step --repo-root . --format json
```

If sandboxing blocks log creation or live provider calls, rerun with approval
rather than changing the command shape. Keep the running session open and avoid
starting another loop command against the same campaign.

### 4. Re-read State After The Advance

Always check state after the step, even when the command failed:

```bash
./target/debug/ploke-eval loop prototype1-doctor --repo-root . --format json
./target/debug/ploke-eval closure status --campaign <campaign> --format json
git status --short --branch
```

Then inspect newly written artifacts:

```bash
find ~/.ploke-eval/instances/prototype1/<campaign> -maxdepth 5 -type f
find ~/.ploke-eval/protocol/prototype1/<campaign> -maxdepth 5 -type f
find ~/.ploke-eval/campaigns/<campaign>/prototype1 -maxdepth 3 -type f
```

If a run root exists, collect:

- `execution-log.json`;
- `benchmark-patch-projection.json`;
- `multi-swe-bench-submission.jsonl`;
- `final_report.json` or other MBE/oracle artifacts, if configured;
- `record.json.gz` and trace sidecars for later run review.

## Evidence Receipt

Before reporting a verdict, produce or preserve a compact evidence receipt:

- `campaign`: campaign id
- `worktree`: parent worktree and branch
- `phase_before`: doctor phase before the step
- `phase_after`: doctor phase after the step
- `command`: exact bounded command run
- `model_route`: eval/protocol model, provider, and route source
- `closure_delta`: registry/eval/protocol status before and after
- `artifacts_created`: eval runs, protocol artifacts, history/scheduler updates
- `patch_state`: patch/submission present, empty, missing, or contradictory
- `oracle_mbe_state`: configured, ran, skipped, missing, or failed
- `tool_visibility`: whether model-facing tool output is known to be visible
- `worktree_state`: clean/dirty and whether changes are expected
- `failure_class`: one primary class from the taxonomy below
- `next_action`: exact safe next command or implementation/reporting task

The receipt can live in chat for small checks. For durable discoveries, write or
update a run review, bug report, skill, or regression-test note.

## Failure Taxonomy

Classify blockers precisely:

- `provider_env`: missing auth, quota, region/project config, network, or
  provider outage.
- `provider_request_shape`: the provider rejects the request body or route, such
  as unsupported reasoning controls or endpoint mismatch.
- `protocol_config`: admitted protocol policy is too small, missing, stale, or
  not threaded to the request builder.
- `loop_progress`: the controller retries or remains in the same phase without
  creating required artifacts.
- `model_behavior`: the model ran but failed to localize, edit, validate, or use
  available evidence.
- `tool_contract`: tool calls completed mechanically but returned useless,
  truncated, stale, misleading, or incorrectly summarized payloads.
- `artifact_accounting`: closure, trace summary, submission, patch projection,
  or protocol artifacts disagree.
- `validation_surface`: the model or framework reports validation that is weak,
  unrelated, missing, or not model-visible.
- `repo_state`: dirty worktree, wrong branch, missing checkout, stale worktree,
  or path-sensitive fixture/workspace problem.

Avoid flattening these into "the run failed." The next action depends on the
failure class.

## What To Promote

Use the diagnostic result to improve the loop:

- Add or update a bug report when the same blocker could recur.
- Add a doctor/preflight check when the blocker is detectable before paid loop
  work.
- Add a regression test when the failure is local and reproducible.
- Update `ploke-run-review` when the artifact-reading workflow discovers a new
  high-signal review pattern.
- Update setup docs/skills when the fix is an operator workflow guardrail.

Strong doctor-check candidates include provider auth presence, route/model
compatibility, protocol reasoning policy, protocol max-token budget, artifact
writability, and a tiny live JSON adjudication preflight when live calls are
explicitly allowed.

## Reporting Shape

Lead with the phase result and blocker:

```text
Step attempted: baseline_protocol
Result: no protocol progress
Blocker: provider_request_shape
Evidence: OpenRouter/google-ai-studio rejected reasoning.effort=none for google/gemini-3.5-flash
Artifacts: eval complete, protocol missing, no protocol files written, worktree clean
Next: make protocol reasoning configurable and add doctor preflight
```

Then give only the details needed for the user's next decision. Full trace
narratives belong in `ploke-run-review`.
