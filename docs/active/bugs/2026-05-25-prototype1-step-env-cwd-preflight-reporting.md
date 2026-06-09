# Prototype 1 Step Env-Cwd Preflight Reporting

Status: open; failed-row doctor blocker fix in progress
Discovered: 2026-05-25
Updated: 2026-06-08

## Summary

Fresh campaign `p1-gemini35-flash-direct-fresh-20260524-190016` failed during
baseline eval setup because the live step was launched from a worktree shell
that did not expose provider credentials. The closure state recorded the real
failure, but `prototype1-step --format json` returned a doctor-shaped report
with `phase = baseline_eval` and `blockers = []`, so the operator had to read
closure artifacts to see that eval had failed.

## Evidence

Campaign:

```text
p1-gemini35-flash-direct-fresh-20260524-190016
```

Worktree:

```text
/home/brasides/.ploke-eval/worktrees/p1-gemini35-flash-direct-fresh-20260524-190016
```

The campaign was configured for the intended direct-Google route:

```text
model_id = google/gemini-3.5-flash
route_source = direct_google
provider = google
protocol_preflight.reasoning = omit
```

The live protocol preflight passed before the baseline step:

```text
protocol_preflight.outcome = passed
protocol_preflight.route_source = direct_google
protocol_preflight.provider = google
protocol_preflight.reasoning = omit
```

The bounded step command launched from the worktree:

```text
./target/debug/ploke-eval loop prototype1-step --repo-root . --format json
```

`prototype1-step` returned a doctor-shaped JSON report with:

```text
phase = baseline_eval
blockers = []
```

Post-step closure status showed that eval had actually failed:

```text
eval.status = partial
eval.failed_total = 1
eval_failure = database setup failed during 'embedding_model_preflight'
```

The failure detail was an embedding preflight provider-env miss for the
OpenRouter-backed eval embedding model:

```text
embedding preflight failed for 'mistralai/codestral-embed-2505':
Var error: Error from env variable, original: environment variable not found
```

Provider env presence differed by launch cwd. The source checkout command
environment reported Google and OpenRouter credentials present, while the
fresh worktree command environment reported both absent. No secret values were
printed.

## 2026-06-05 Recurrence

The same blocker reproduced during a fresh run setup after an auth-poisoned
attempt was abandoned.

Campaign:

```text
p1-admissionfix-g35flash-p25flash-20260605-050938
```

Worktree:

```text
/home/brasides/.ploke-eval/worktrees/p1-admissionfix-g35flash-p25flash-20260605-050938
```

Command:

```text
./target/debug/ploke-eval loop prototype1-step --repo-root . --format json
```

The worktree-local command environment reported:

```text
OPENROUTER_API_KEY=missing
GOOGLE_API_KEY=missing
```

The source checkout command environment reported:

```text
OPENROUTER_API_KEY=present
GOOGLE_API_KEY=present
```

The bounded step wrote only early baseline eval setup artifacts:

```text
/home/brasides/.ploke-eval/batches/prototype1/p1-admissionfix-g35flash-p25flash-20260605-050938/ripgrep-burntsushi-ripgrep-2209-eval-slice-20260605121349/batch-run-summary.json
/home/brasides/.ploke-eval/instances/prototype1/p1-admissionfix-g35flash-p25flash-20260605-050938/BurntSushi__ripgrep-2209/run.json
/home/brasides/.ploke-eval/campaigns/p1-admissionfix-g35flash-p25flash-20260605-050938/closure-state.json
```

The batch summary records the concrete failure:

```text
status = failed
error = database setup failed during 'embedding_model_preflight':
        embedding preflight failed for 'mistralai/codestral-embed-2505':
        Var error: Error from env variable, original: environment variable not found
```

The closure state records:

```text
eval.status = partial
eval.failed_total = 1
protocol.status = missing
```

But `prototype1-doctor --format json` still reported:

```text
phase = baseline_eval
blockers = []
allowed_actions = ["doctor", "continue", "step"]
```

Source trace:

```text
prototype1-step
-> run::step
-> advance_baseline_eval
-> advance_eval_closure
-> execute_batch_eval_for_manifest
-> embedding_model_preflight failure
-> closure-state eval.failed_total = 1
-> diagnose
-> extend_baseline_eval_registration_blockers
```

The classifier only blocked `row.eval_status == ClosureClass::Partial`, so a
failed instance row left doctor clean even though the aggregate eval summary was
partial because `failed_total = 1`.

## Impact

This is not evidence that direct Google chat/protocol routing is broken. It is
an operator/workflow failure plus a reporting gap:

- live commands must be launched from an env-bearing cwd or explicit env source;
- eval embeddings still use the OpenRouter-backed default unless overridden;
- `prototype1-step` should not make a failed eval preflight look like an
  unadvanced clean doctor result.

## Disposition

Abandon-and-restart for clean loop evidence.

The failed campaign should not be used as evidence that the baseline eval
transition succeeded first time. The worktree build artifacts were cleaned with:

```text
cargo clean
```

## Fix Direction

- Add or extend a doctor/setup preflight that checks the eval embedding route
  and required credential presence from the same launch environment that will
  run `prototype1-step`.
- Make `prototype1-step --format json` surface newly recorded closure failures
  in its own report instead of returning only a clean doctor-shaped phase
  summary.
- Continue the next fresh run by launching the main checkout binary from the
  env-bearing source checkout and passing `--repo-root <fresh-worktree>`.

## Current Source Fix

`crates/ploke-eval/src/cli/prototype1_state/run/core.rs` now treats
`ClosureClass::Failed` baseline eval rows as doctor blockers in
`extend_baseline_eval_registration_blockers`, matching the existing partial-row
guard.

Focused regression:

```text
failed_baseline_eval_closure_adds_doctor_blocker
```

The failed campaign remains abandon-and-restart evidence. The fix should be
verified with a fresh campaign launched from an env-bearing cwd.

## 2026-06-08 Child Treatment Recurrence

The same launch-cwd environment split recurred during child treatment
evaluation, after the parent campaign had already produced and admitted broad
harness child candidates.

Campaign:

```text
p1-g35f-direct-protocol-2target-g0g2-1x3-20260608-204113
```

Worktree:

```text
/home/brasides/.ploke-eval/worktrees/p1-g35f-direct-protocol-2target-g0g2-1x3-20260608-204113
```

Affected child runner sidecars:

```text
/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-20260608-204113/prototype1/nodes/node-7b23e180945c96ef/runner-result.json
/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-20260608-204113/prototype1/nodes/node-e7f2f7430e7ca64b/runner-result.json
```

Both children failed treatment before any direct-Google chat turn because their
treatment evals failed during `embedding_model_preflight`:

```text
embedding preflight failed for 'mistralai/codestral-embed-2505':
Var error: Error from env variable, original: environment variable not found
```

The source checkout command environment still had `OPENROUTER_API_KEY`
available, while the campaign worktree command environment did not. Running the
main checkout binary from `/home/brasides/code/ploke` and passing
`--repo-root <campaign-worktree>` allowed the next child to proceed past
embedding setup and reach the direct-Google request path.

Recovery command shape:

```text
/home/brasides/code/ploke/target/debug/ploke-eval loop prototype1-step \
  --repo-root /home/brasides/.ploke-eval/worktrees/p1-g35f-direct-protocol-2target-g0g2-1x3-20260608-204113 \
  --format json
```

This recurrence expands the original bug from baseline eval setup to child
treatment setup: every helper that launches eval work must preserve the
operator-approved credential environment or run an equivalent preflight from the
same process environment that will spawn paid work.
