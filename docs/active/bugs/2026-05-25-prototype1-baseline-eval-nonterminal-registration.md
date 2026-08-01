# 2026-05-25 Prototype 1 Baseline Eval Nonterminal Registration

Status: fixed in source with regression coverage. The exposing campaign should
not be advanced for loop evidence.

## Symptom

Campaign `p1-gemini35-flash-direct-15g2x3-fixed2-20260525-073113` was in
generation 0 baseline eval. Its closure state still reported baseline eval as
missing, so `prototype1-doctor` originally reported `phase=baseline_eval` with
no blockers and allowed `continue` / `step`.

However, the run registry already had a nonterminal baseline attempt:

```text
run-1779719499688-structured-current-policy-e82645cb
execution_status=Running
updated_at=2026-05-25T14:31:40.447760002+00:00
```

The run root had setup/indexing artifacts and an `agent-turn-trace.json`, but
the terminal evidence was absent:

```text
record.json.gz missing
agent-turn-summary.json missing
llm-full-responses.jsonl missing
validation-audit.json missing
benchmark-patch-projection.json missing
```

## Broken Contract

Doctor treated the stale `closure-state.json` as authoritative for whether
baseline eval could be retried. That allowed a fresh baseline eval step even
though a prior registered attempt had already started and produced partial
durable evidence.

For Prototype 1 this is unsafe: a half-written registered attempt is not the
same state as "no baseline eval has happened." Re-running over it can blur the
ordering of evidence, make closure state disagree with the run registry, and
pollute later protocol or selection interpretation.

## Fix

When generation 0 baseline eval is incomplete, doctor now inspects registered
runs for each closure instance. If any registration is still `Registered` or
`Running`, doctor reports `phase=blocked`, allows only `doctor`, and names the
run id, instance id, execution status, update time, and whether the terminal
record and turn summary exist.

Doctor also blocks when closure state explicitly records partial baseline eval
evidence, even if no nonterminal run registration is found.

After rebuilding source, the exposing campaign now reports:

```text
phase=blocked
allowed_actions=["doctor"]
blocker: baseline eval has nonterminal registered attempt
'run-1779719499688-structured-current-policy-e82645cb' for instance
'BurntSushi__ripgrep-2209' with execution_status=Running,
updated_at=2026-05-25T14:31:40.447760002+00:00, record missing,
turn summary missing; refusing to start another baseline eval until this
attempt is classified or abandoned
```

## Disposition

Stop advancing `p1-gemini35-flash-direct-15g2x3-fixed2-20260525-073113` for
loop evidence. It has a nonterminal baseline attempt with partial persisted
artifacts and no terminal run record.

The correct next operator action is to abandon the campaign and start a fresh
run after the source fix is present.

## Verification

Focused tests:

```text
cargo test -p ploke-eval replay_observe_child_uses_default_stale_threshold -- --nocapture
RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval nonterminal_baseline_eval_registration_adds_doctor_blocker -- --nocapture
RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval partial_baseline_eval_closure_adds_doctor_blocker -- --nocapture
RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval run_profile_execution_defaults_observe_child_stale_after -- --nocapture
```

Live doctor check against the exposing campaign:

```text
/home/brasides/code/ploke/target/debug/ploke-eval loop prototype1-doctor \
  --repo-root /home/brasides/.ploke-eval/worktrees/p1-gemini35-flash-direct-15g2x3-fixed2-20260525-073113 \
  --format json
```
