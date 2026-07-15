# Prototype 1 Parent-Patcher Role Is Not Admitted Or Preflighted

Status: config mitigated and doctor visibility repaired; fresh live route verification pending
Discovered: 2026-07-14

## Summary

A fresh profile-backed Prototype 1 campaign completed its baseline evaluation,
protocol adjudication, closure, and R6-to-R7 policy/budget transition using
direct Google. Its R7-to-R8 child-plan transition nevertheless sent all three
broad headless-TUI calls through OpenRouter with
`openai/gpt-5.1-codex`. OpenRouter rejected each call with HTTP 402 before tool
use because the key could not fund the requested 65,536-token response budget.

This was not route drift inside the admitted eval/protocol profile. Broad child
generation deliberately uses the separate machine-global parent-patcher model
selection. The setup receipt, admitted run profile, doctor checks, and walk
phase display did not surface or preflight that effective role, so the campaign
appeared ready for a direct-Google live loop until the first broad patch call.

## Evidence

Campaign:

```text
p1-stage5-observe-g35f-orembed-3g1x3-p3-20260714-213222
```

Walk session and failed operation:

```text
session_id = abe149b1-07fe-42bf-bfe1-d5d43e1b9a37
operation_id = 77d4c71c-3b73-436f-80de-9737a826a3c0
transition_id = 8b99e673-8bec-5e58-853b-d4ef92803bb5
attempted edge = R7 -> R8
```

The campaign-local profile and campaign manifest select direct Google
`google/gemini-3.5-flash` for eval and protocol. Those phases completed. The
three persisted broad-headless diagnostics instead record:

```text
model_route.route_source = openrouter
model_route.router = openrouter
model_route.provider_slug = openai
model_route.endpoint_host = openrouter.ai
terminal = exhausted after 1 attempt
turn outcome = aborted
error code = HTTP_402
```

Artifacts:

```text
~/.ploke-eval/campaigns/p1-stage5-observe-g35f-orembed-3g1x3-p3-20260714-213222/prototype1/messages/edit-harness-result/node-a0298efef9df3f01.headless-tui.json
~/.ploke-eval/campaigns/p1-stage5-observe-g35f-orembed-3g1x3-p3-20260714-213222/prototype1/messages/edit-harness-result/node-a0298efef9df3f01-r2.headless-tui.json
~/.ploke-eval/campaigns/p1-stage5-observe-g35f-orembed-3g1x3-p3-20260714-213222/prototype1/messages/edit-harness-result/node-a0298efef9df3f01-r3.headless-tui.json
```

The failed batch was persisted correctly as a child plan with zero admitted
children and three rejected surface attempts:

```text
~/.ploke-eval/campaigns/p1-stage5-observe-g35f-orembed-3g1x3-p3-20260714-213222/prototype1/messages/child-plan/node-a0298efef9df3f01.json
```

The provider reported that the request asked for up to 65,536 output tokens
while the key could fund only 3,139. No edit was staged and no validation ran.

## Source Boundary

`run_broad_headless_tui_attempt` constructs
`BroadTuiAttemptOptions::for_parent_patcher_defaults`. That constructor calls
`load_parent_patcher_model_selection()` rather than resolving the admitted
campaign/profile eval model. Source comments describe this as a temporary split
configuration, and the operator walkthrough documents the broad parent patcher
as a separate live model surface.

Keeping separate role models is not itself the bug. The missing contract is
that setup and readiness checks do not expose the full effective live model
surface they are admitting the operator to run.

## Broken Contract

Before setup reports a fresh campaign ready for a full live loop, the operator
must be able to see and validate every live model role that the requested run
can reach. At minimum, the effective broad parent-patcher model, provider, and
route source must be visible beside the admitted eval/protocol settings and
must participate in an appropriate readiness preflight.

The parent-patcher role must remain separately configured unless the operator
explicitly chooses to unify it with the campaign profile. The repair must not
silently reinterpret the admitted profile as authority over a distinct role.

## Immediate Mitigation

The machine-global role was changed before any retry:

```text
ploke-eval model parent-patcher set google/gemini-3.5-flash
ploke-eval model parent-patcher current
google/gemini-3.5-flash    gemini-3.5-flash
```

The current campaign must not be retried as fresh evidence. Its authoritative
R7-to-R8 attempt is indeterminate and its persisted child plan validly records
three rejected attempts. Preserve and abandon the session, then validate the
corrected role in a new campaign.

## Missing Regression Coverage

Add production-entrypoint coverage proving that:

1. setup/doctor output reports the effective broad parent-patcher model and
   route independently from eval and protocol;
2. the readiness path fails before R7 when that role cannot make its required
   live call;
3. a direct-Google eval/protocol profile plus an OpenRouter parent patcher is
   represented as three deliberate route choices, not one misleading campaign
   route;
4. the Stage 5 rejected-only child plan remains readable and does not mint new
   broad slots on retry.

## Source Repair

`prototype1-doctor --headless-tui-setup-preflight` now resolves the effective
parent-patcher selection through the same production loader used by broad child
generation. Its typed report includes the model and `ModelRouteRecord`, and
table output shows route source, router, provider, and endpoint. Missing or
invalid selection fails locally at `parent_patcher_model` before sparse/BM25
runtime setup.

This remains a non-network preflight, as documented by the existing flag. It
does not alter the setup-plan digest, admit machine-local role configuration
into the campaign profile, or silently unify parent-patcher authority with the
eval/protocol model. Focused regressions cover direct Google with a stale
OpenRouter preference, missing selection, JSON visibility, and the non-broad
early skip. A separately named live parent-patcher canary remains future work.
