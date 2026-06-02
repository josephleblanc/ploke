# Prototype 1 Step Env-Cwd Preflight Reporting

Status: open
Discovered: 2026-05-25

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
