---
name: diagnostic-loop-run
description: Use this skill when running or diagnosing a bounded Prototype 1 `ploke-eval loop` advance, especially when the user wants to take one `prototype1-step`, classify why a loop run did or did not progress, collect evidence from doctor/closure/run artifacts, decide whether a blocker is provider/env/config/model/tooling, or turn findings into a run review, bug report, doctor check, or regression test.
---

# Ploke Diagnostic Loop Run

## Overview

Use this skill to operate a Prototype 1 loop as a diagnostic cycle:
advance one bounded phase, read the durable artifacts, classify the result, and
leave a clear next action. Pair it with `ploke-prototype1-run-setup` before a
fresh campaign exists and with `ploke-run-review` once a run root needs deeper
trace reconstruction. If the diagnostic result is loop-blocking, switch to
`ploke-blocker-repair-loop` before advancing the campaign again.

For state-transition, History, Crown, surface-digest, or successor-handoff
questions, first read
`docs/workflow/skills/ploke-diagnostic-loop-run/references/prototype1-history-crown.md`.
That reference explains why strict transition gates are part of the
self-editing safety model rather than ordinary defensive checks.

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
- If a bounded step reveals a blocker, stop advancing the campaign and use
  `ploke-blocker-repair-loop` to capture evidence, reproduce the broken
  contract when appropriate, and decide whether the run can resume or must be
  abandoned.
- Correctness beats API-cost minimization. If live API behavior is required to
  verify a fix, blocker, route, or provider request shape, do the live call
  instead of substituting a local mock. Put any Rust test that performs the call
  behind the `live_api_tests` feature, and run that gated test explicitly.

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

Record the current phase and allowed actions. If doctor reports blockers,
classify them before advancing. Continue only for non-blockers or explicit
operator-owned environment exceptions; true loop blockers hand off to
`ploke-blocker-repair-loop`.

If the current run already contains invalid, malformed, contradictory, or
unverifiable required protocol/eval/oracle evidence, do not keep stepping it.
Treat that as a stop-and-abandon condition for the current campaign unless the
user explicitly asks to inspect or repair the artifact store. The fresh-run
goal is for the same transition to succeed on the first attempt under the
current source/workflow, not for a later reader change to reinterpret the old
run as successful.

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
surfaces before rerunning live work:

```bash
./target/debug/ploke-eval model current
./target/debug/ploke-eval model parent-patcher current
./target/debug/ploke-eval model provider current --model-id <model-id>
./target/debug/ploke-eval model providers <model-id>
```

If the suspected blocker is the protocol model/provider/request-body tuple, run
the explicit doctor preflight before spending a full protocol advance:

```bash
./target/debug/ploke-eval loop prototype1-doctor --repo-root . --live-protocol-preflight --format json
```

Treat this as live-provider work. It should report the model, provider, route
source, reasoning policy, bounded outcome, and an error class without printing
credentials. Do not skip it solely to save tokens or API spend when it is the
only faithful verification surface.

When the live behavior belongs in test coverage, use the existing live-test
pattern:

```rust
#[tokio::test]
#[cfg(feature = "live_api_tests")]
#[ignore = "live provider test"]
async fn live_provider_case() {
    // perform the real provider request
}
```

Run the live test directly with the feature enabled:

```bash
cargo test -p <crate> --features live_api_tests <test_name> -- --ignored --nocapture
```

If sandboxing or network policy blocks the command, request approval and rerun
the same gated live test.

### 3. Take One Bounded Advance

Run a single phase advance from the campaign worktree:

```bash
./target/debug/ploke-eval loop prototype1-step --repo-root . --format json
```

If sandboxing blocks log creation or live provider calls, rerun with approval
rather than changing the command shape. Keep the running session open and avoid
starting another loop command against the same campaign.

When a child-plan broad-harness batch emits a completed child attempt while the
bounded step is still running, treat that as a review handoff point. Dispatch a
separate run-review worker for that completed attempt and keep monitoring the
live step locally. Scope the review to the single attempt/slot that just
produced a result, including its branch, workspace, commit, terminal record,
tool failures, and edit lifecycle. Do not wait for the whole fanout batch before
starting reviews when earlier child artifacts already exist.

If operating as the loop orchestrator, this handoff is mandatory: the
orchestrator keeps the loop session and board current, while the review
sub-agent uses `ploke-run-review` on the completed child attempt. When that
review returns high-priority non-blocking findings, dispatch a separate repair
worker or file an alive bug without blocking the still-running fanout unless the
finding is a true transition blocker.

When the operator says to clean up a discovered issue in the main codebase while
the loop continues, interpret that as a delegation instruction. Start a bounded
repair worker for the source fix and tests, give it explicit file ownership, and
keep the live loop session moving locally. Do local repair work only when it is
the immediate critical path or no sub-agent slot is available.

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

If the child channel has `evaluating` but no terminal `result`, and neither
`nodes/<node>/runner-result.json` nor `nodes/<node>/results/<runtime>.json`
exists, the child is probably inside
`run_prototype1_resolved_branch_treatment`. Refine that state by locating the
treatment campaign whose id starts with:

```text
<baseline-campaign>-treatment-<branch-id>-
```

The branch id is in `nodes/<node>/node.json`. Once the treatment campaign
exists, inspect:

```text
$HOME/.ploke-eval/campaigns/<treatment-campaign>/campaign.json
$HOME/.ploke-eval/campaigns/<treatment-campaign>/closure-state.json
```

The treatment `closure-state.json` is the best current intermediate record for
whether the child is still in eval closure, protocol closure, or post-closure
evidence assembly. If no treatment campaign exists yet, the child is still
before or inside treatment campaign creation/early eval preparation.

If a run root exists, collect:

- `execution-log.json`;
- `benchmark-patch-projection.json`;
- `multi-swe-bench-submission.jsonl`;
- `agent-turn-summary.json`, at least `terminal_record.outcome`,
  `terminal_record.summary`, and whether `final_assistant_message` is present;
- `final_report.json` or other MBE/oracle artifacts, if configured;
- `record.json.gz` and trace sidecars for later run review.

If the terminal record says the agent turn was aborted, or if there is no final
assistant message, do not treat a non-empty patch projection as a clean eval
success. Classify this as `artifact_accounting` or
`invalid_transition_evidence`, write the evidence down, and stop before
advancing protocol.

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
- `blocker_decision`: non-blocker filed-and-continue, repair-and-resume,
  abandon-and-restart, or environment/operator handoff
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
  terminal turn outcome, or protocol artifacts disagree.
- `invalid_transition_evidence`: required protocol, eval, oracle, History, or
  selection evidence was persisted but is malformed, contradictory,
  unverifiable, or not admissible for the next transition.
- `validation_surface`: the model or framework reports validation that is weak,
  unrelated, missing, or not model-visible.
- `repo_state`: dirty worktree, wrong branch, missing checkout, stale worktree,
  or path-sensitive fixture/workspace problem.

Avoid flattening these into "the run failed." The next action depends on the
failure class.

## What To Promote

Use the diagnostic result to improve the loop. Keep the branch decision explicit:
non-blockers can be filed as alive bugs while the loop continues. Blockers move
to `ploke-blocker-repair-loop` and stop campaign advancement. If required
persisted transition evidence is invalid, abandon the current run and start a
fresh campaign after any setup/source guardrail is corrected.

- Add or update a bug report when the same issue could recur.
- Add a doctor/preflight check when the blocker is detectable before paid loop
  work.
- Add a regression test when the blocker is local and reproducible.
- Update `ploke-run-review` when the artifact-reading workflow discovers a new
  high-signal review pattern.
- Update setup docs/skills when the fix is an operator workflow guardrail.
- Start a fresh worktree/campaign when the blocker invalidates the current
  transition evidence.

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
