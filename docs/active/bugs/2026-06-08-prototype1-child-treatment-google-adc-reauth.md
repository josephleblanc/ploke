# Prototype 1 Child Treatment Direct Google ADC Reauth Blocker

Status: open; external auth blocker; current campaign is stop-use for further
loop progress until direct-Google ADC auth is refreshed.

## Broken Contract

A live Prototype 1 child treatment using the admitted `direct_google` eval route
must have non-interactive Google ADC credentials available before spawning paid
child eval turns.

## Evidence

Campaign:

```text
p1-g35f-direct-protocol-2target-g0g2-1x3-20260608-204113
```

Worktree:

```text
/home/brasides/.ploke-eval/worktrees/p1-g35f-direct-protocol-2target-g0g2-1x3-20260608-204113
```

Child:

```text
node-54699de04880671e
branch-8519321c285af142
```

Runner sidecar:

```text
/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-20260608-204113/prototype1/nodes/node-54699de04880671e/runner-result.json
```

Treatment campaign:

```text
/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-20260608-204113-treatment-branch-8519321c285af142-1780980943282
```

Treatment closure state:

```text
eval.status = partial
eval.expected_total = 2
eval.complete_total = 0
eval.failed_total = 2
protocol.status = missing
```

Treatment run artifacts:

```text
/home/brasides/.ploke-eval/instances/prototype1/p1-g35f-direct-protocol-2target-g0g2-1x3-20260608-204113/treatments/branch-8519321c285af142/instances/BurntSushi__ripgrep-2209/runs/run-1780981022300-structured-current-policy-2cf64369/agent-turn-summary.json
/home/brasides/.ploke-eval/instances/prototype1/p1-g35f-direct-protocol-2target-g0g2-1x3-20260608-204113/treatments/branch-8519321c285af142/instances/BurntSushi__ripgrep-2295/runs/run-1780980944092-structured-current-policy-9f58cb97/agent-turn-summary.json
```

Both target runs aborted on their first provider request:

```text
terminal_record.outcome = aborted
terminal_record.attempts = 1
final_assistant_message = absent
code = HTTP_SEND_FAILED
kind = transport
transport detail = failed to resolve bearer token: failed to resolve Google application default credentials
```

Current command-environment check from `/home/brasides/code/ploke`:

```text
GOOGLE_PROJECT_ID = present
GOOGLE_REGION = present
GOOGLE_APPLICATION_CREDENTIALS = absent
OPENROUTER_API_KEY = present
```

`gcloud auth application-default print-access-token` fails non-interactively:

```text
Reauthentication failed. cannot prompt during non-interactive execution.
Please run:
  gcloud auth application-default login
```

The retry from the unrestricted main checkout command runner produced the same
result. The loop's own live preflight also failed on the direct-Google route:

```text
protocol_preflight.outcome = failed
model_id = google/gemini-3.5-flash
provider = google
route_source = direct_google
detail = provider_request: Failed while sending request to LLM provider.
phase = blocked
```

An active `gcloud` account was present, but ADC token minting still failed
non-interactively.

## Source Trace

```text
prototype1-step
-> child runner spawn for node-54699de04880671e
-> treatment eval campaign for branch-8519321c285af142
-> benchmark agent turn for BurntSushi__ripgrep-2209 and BurntSushi__ripgrep-2295
-> direct_google request for google/gemini-3.5-flash
-> Google bearer token resolution through ADC
-> ADC refresh fails
-> terminal turn aborts with HTTP_SEND_FAILED
-> treatment closure records eval.partial with failed_total = 2
-> protocol remains missing because no complete eval evidence exists
```

## Docs/Policy Expectation

The admitted run profile selected `direct_google` for eval/protocol calls. The
operator expectation was that `gauth` had provided direct-Google auth for the
rest of the day. The persisted agent-turn summaries instead prove that the child
treatment process could not resolve a Google bearer token from ADC.

This is distinct from the earlier OpenRouter embedding env miss in the same
campaign: child `node-54699de04880671e` reached the direct-Google request path
only after the controller was relaunched from the env-bearing source checkout.

## Current Repro Coverage

Current local proof is operational, not a Rust regression:

```text
gcloud auth application-default print-access-token
```

returns a non-interactive reauthentication failure. No source regression should
be invented for an external missing credential condition.

## Missing Repro / Validation

After operator auth is refreshed, rerun the direct-Google doctor/preflight or a
bounded same-campaign step from the env-bearing source checkout. The validation
must prove that the same route can resolve a bearer token before a fresh paid
child treatment is considered reliable.

## Fix Direction

- Refresh the actual ADC credential surface used by `direct_google`, for
  example with `gcloud auth application-default login` or the equivalent
  project-local `gauth` flow.
- Prefer a doctor/setup preflight that checks direct-Google ADC availability
  before child treatment spawn, so this fails before paid benchmark attempts.
- Keep the current campaign as stop-use evidence for loop progress; do not
  reinterpret aborted treatment evals as semantic child failures.

## Related Bugs

- [`2026-05-25-prototype1-step-env-cwd-preflight-reporting.md`](./2026-05-25-prototype1-step-env-cwd-preflight-reporting.md)
  covers the cwd/env split that caused the first two child treatments in this
  campaign to fail during OpenRouter embedding preflight.
- [`2026-06-08-prototype1-direct-google-g35flash-quota-empty-baseline.md`](./2026-06-08-prototype1-direct-google-g35flash-quota-empty-baseline.md)
  covers a separate direct-Google HTTP 429 provider exhaustion blocker. This
  report is not a quota finding; the current evidence is ADC token resolution.
