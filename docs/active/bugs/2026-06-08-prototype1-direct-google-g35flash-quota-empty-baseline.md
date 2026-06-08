# Prototype 1 Direct Google Quota Produces Empty Baseline

Status: external provider blocker; current campaign stop-use for loop progress.

Campaign:

```text
p1-handofffix2-g35flash-p25flash-5g1x2-a2-20260608-102457
```

Worktree:

```text
/home/brasides/.ploke-eval/worktrees/p1-handofffix2-g35flash-p25flash-5g1x2-a2-20260608-102457
```

Run root:

```text
/home/brasides/.ploke-eval/instances/prototype1/p1-handofffix2-g35flash-p25flash-5g1x2-a2-20260608-102457/BurntSushi__ripgrep-2209/runs/run-1780939791539-structured-current-policy-82ab7d13
```

## Broken Contract

A fresh Prototype 1 validation run needs a successful baseline patch-producing
eval before protocol and successor evidence can be trusted. This campaign's
baseline turn aborted on a provider quota error and persisted an empty patch.

## Evidence

The campaign was created from fixed source commit `bd894a8a` and admitted the
same policy as the stopped handoff campaign, except for the new profile name:

```text
model.id = "google/gemini-3.5-flash"
model.route_source = "direct-google"
model.provider = "google"

protocol.model.id = "google/gemini-2.5-flash"
protocol.model.route_source = "direct-google"
protocol.model.provider = "google"
```

Setup and preflight were healthy before the first step:

```text
prototype1-doctor phase = baseline_eval
blockers = []
headless_tui_setup_preflight.outcome = passed
protocol_preflight.outcome = passed
protocol_preflight.model_id = google/gemini-2.5-flash
protocol_preflight.route_source = direct_google
```

The bounded step command was:

```text
./target/debug/ploke-eval loop prototype1-step \
  --repo-root /home/brasides/.ploke-eval/worktrees/p1-handofffix2-g35flash-p25flash-5g1x2-a2-20260608-102457 \
  --format json
```

The model-facing turn then aborted:

```text
agent-turn-summary.json
TurnFinished.outcome = aborted
TurnFinished.error_id = a7419a81-2e2c-4b27-833a-311b576d4e77
TurnFinished.summary contains code=HTTP_429 kind=http_status
TurnFinished.summary contains status=RESOURCE_EXHAUSTED
TurnFinished.summary contains "Resource exhausted. Please try again later."
TurnFinished.attempts = 27
```

The downstream state mechanically advanced:

```text
prototype1-doctor phase = baseline_protocol
blockers = []
closure-state eval_status = complete
closure-state protocol_status = missing
```

But the patch evidence is empty:

```text
multi-swe-bench-submission.jsonl
{"org":"BurntSushi","repo":"ripgrep","number":2209,"fix_patch":""}

benchmark-patch-projection.json
submission.sha256 = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
submission.byte_len = 0
submission.line_count = 0
check.status = passed
```

## Source Trace

No repo source fix is claimed here. The upstream cause is an external direct
Google quota/resource-exhaustion response for `google/gemini-3.5-flash`.

The downstream artifact path that made the run unsafe to continue is the
baseline eval export:

```text
agent turn abort -> empty multi-swe-bench submission -> closure eval complete -> doctor baseline_protocol
```

## Docs/Policy Expectation

The admitted profile intentionally uses direct Google for the parent patcher and
protocol model. Operator setup policy says provider/auth failures are external
environment blockers and should not become source regressions unless a repo
guard or replay surface is explicitly being changed.

## Current Repro Coverage

The live run itself is the evidence. It proves current direct-Google auth reached
the Vertex endpoint, but quota for the patch-generation route was exhausted
during the benchmark turn.

Existing source regressions for the prior observe/selection and successor
identity bugs still pass, and the fixed binary built successfully in both the
main checkout and the fresh campaign worktree.

## Missing Repro / Validation

Fresh handoff validation remains missing. A useful replacement run needs either:

1. the same direct-Google route after quota recovers; or
2. an explicit operator-approved profile/model/provider change.

Do not continue this campaign into protocol as handoff-validation evidence,
because protocol would review an empty patch caused by provider quota failure.

## Fix Direction

For this campaign, abandon-and-restart after provider quota recovers or after an
approved route/profile change. Do not patch readers or protocol admission to
treat the empty baseline as success.

Separately, it may be worth adding a future guard that distinguishes mechanical
baseline eval completion from a patch-producing, non-aborted benchmark turn, but
that is not the source fix for this provider blocker.

## Related Bugs

- [`2026-06-08-prototype1-successor-handoff-stale-parent-identity.md`](./2026-06-08-prototype1-successor-handoff-stale-parent-identity.md)
  Fixed source bug that this fresh campaign was meant to validate.
- [`2026-05-26-prototype1-foreground-child-recovery-misses-parent-comparison.md`](./2026-05-26-prototype1-foreground-child-recovery-misses-parent-comparison.md)
  Fixed source bug from the earlier campaign replay.
