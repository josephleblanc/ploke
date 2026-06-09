# Prototype 1 Broad Child Direct-Google 429 Zero Admission

Status: blocked external-provider evidence

## Broken Contract

A fresh Prototype 1 `prototype1-state` run with `min_children = 1`, `max_children = 3`,
direct-Google routing, and complete baseline/protocol evidence should either admit at
least one child or stop with a clear provider-capacity blocker before the campaign is
treated as useful loop-progress evidence.

## Evidence

- Campaign: `p1-g35f-direct-protocol-2target-g0g2-1x3-state2-20260609-040218`
- Source checkpoint: `750625fc` (`Fix post-apply refresh replay`)
- Worktree: `/home/brasides/.ploke-eval/worktrees/p1-g35f-direct-protocol-2target-g0g2-1x3-state2-20260609-040218`
- Command:
  `./target/debug/ploke-eval loop prototype1-state --repo-root /home/brasides/.ploke-eval/worktrees/p1-g35f-direct-protocol-2target-g0g2-1x3-state2-20260609-040218 --campaign p1-g35f-direct-protocol-2target-g0g2-1x3-state2-20260609-040218 --format json`
- Run log:
  `/tmp/p1-state2-prototype1-state.1781003120.log`
- Post-run doctor:
  `/tmp/p1-state2-post-doctor.1781004670.json`
- Closure status:
  `/tmp/p1-state2-closure-status.1781004670.json`
- Child result:
  `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state2-20260609-040218/prototype1/messages/edit-harness-result/node-c74417c118d8ab3a.headless-tui.json`
- Child plan:
  `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state2-20260609-040218/prototype1/messages/child-plan/node-c74417c118d8ab3a.json`

Observed structured fields:

- `closure-state.json`: registry `complete`, eval `complete`, protocol `complete`.
  Both target instances had complete eval and complete protocol.
- `child-plan/node-c74417c118d8ab3a.json`: `children` was empty and
  `rejected_surface_attempts` had one rejected provider-unavailable attempt.
- `node-c74417c118d8ab3a.headless-tui.json`: `terminal.terminal` was
  `provider_unavailable`; `model_route.route_source` was `direct_google`,
  `router` was `google`, and `endpoint_host` was `aiplatform.googleapis.com`.
- The provider body classified the upstream failure as HTTP 429
  `RESOURCE_EXHAUSTED`.
- Post-run doctor reported `phase = "complete"`, no blockers, and note:
  `admitted successor selection resolved to none`.

## Source Trace

The loop reached baseline eval and protocol successfully, then entered broad
headless-TUI child generation. The child generation request produced a
headless-TUI terminal provider error, which was converted into a rejected
surface attempt. Selection then had no children to admit.

Trace chain:

`direct_google child model request -> HTTP 429 after retry -> headless terminal
provider_unavailable -> child-plan rejected_surface_attempts[0] -> children = []
-> successor selection none`.

## Docs/Policy Expectation

The admitted profile requested a short run:

- gen0-gen2
- min children 1
- max children 3
- full-batch child scheduling
- direct-Google model and protocol routes
- two benchmark targets

The run did satisfy the two-target baseline/protocol prerequisite. It did not
produce a trustworthy generation-1 child because the provider failed during the
broad child edit harness.

## Current Repro Coverage

This is live-provider evidence, not a local deterministic repro. The current
artifact set proves:

- the route was direct Google, not OpenRouter;
- Google auth was sufficient for baseline/protocol and earlier child requests;
- the stop was an upstream provider-capacity response during child generation;
- the run should not be counted as generation-progress evidence.

The existing historical replay for the stale-anchor post-apply issue does not
cover this provider-capacity path, and should not be stretched to do so.

## Missing Repro / Validation

No source regression is currently justified because this is a quota/capacity
condition. The next validation surface is another same-profile live run after
provider capacity recovers, or a lower-concurrency profile if the operator wants
to reduce direct-Google burst pressure.

If this recurs under low concurrency, the missing code-level repro is a
doctor/preflight or provider-admission test that proves broad child generation
stops before publishing zero-useful child progress when direct-Google quota is
exhausted.

## Fix Direction

- Do not patch readers or selection to reinterpret this campaign as successful.
- Treat this campaign as evidence-only for child generation after the 429.
- Start a fresh campaign when provider capacity is available.
- If recurrences continue, improve the authority-bearing provider/admission
  layer: classify direct-Google `RESOURCE_EXHAUSTED` as a resumable external
  blocker with clearer operator guidance, or add a bounded child-generation
  retry/backoff policy that does not publish misleading zero-child progress.

## Related Bugs

- [`2026-06-08-prototype1-direct-google-g35flash-quota-empty-baseline.md`](./2026-06-08-prototype1-direct-google-g35flash-quota-empty-baseline.md)
- [`2026-06-08-prototype1-child-treatment-google-adc-reauth.md`](./2026-06-08-prototype1-child-treatment-google-adc-reauth.md)
- [`2026-05-25-prototype1-broad-headless-google-401-slot-thrash.md`](./2026-05-25-prototype1-broad-headless-google-401-slot-thrash.md)
