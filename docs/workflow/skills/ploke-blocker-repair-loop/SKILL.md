---
name: ploke-blocker-repair-loop
description: Use this skill when a Prototype 1 or ploke-eval diagnostic step, run review, replay, doctor check, or protocol run finds a blocker that should stop the loop and be turned into a reproducing step/replay/protocol/doctor regression before fixing and resuming.
---

# Ploke Blocker Repair Loop

Use this skill when the loop hits a blocker. It is the stop-and-repair branch of
the diagnostic workflow: preserve the evidence, reproduce the broken contract,
fix the cause, verify the reproduction, then resume the run.

## Decision Gate

After a bounded `prototype1-step`, diagnostic review, or run review, classify the
issue before doing more loop work.

- Non-blocker: file or update an alive bug, note the residual risk, and continue
  the loop if the next step can still produce trustworthy evidence.
- Blocker: stop advancing the campaign. Do not keep producing run artifacts until
  there is a reproducing test or replay and the fix has been verified.

Treat an issue as a blocker when any of these are true:

- Continuing would create misleading evidence about model, tool, or protocol
  behavior.
- The controller cannot advance the required phase.
- Required oracle, patch, MBE, protocol, or adjudication evidence is missing,
  invalid, or internally contradictory.
- A provider, route, request-shape, tool-contract, or artifact-accounting failure
  blocks the current run target.
- A reported success does not prove the intended semantic operation happened.

If the blocker is external environment state only, record that directly and stop
or hand off to the operator. Do not invent a code regression test for a condition
that is only a missing credential, quota, or cloud permission.

## Evidence Freeze

Before editing code, record the exact evidence surface.

- Identify the campaign, worktree, current phase, transition journal entries,
  run artifacts, and command that exposed the blocker.
- Preserve the model-facing and tool-facing trace needed to reconstruct what
  happened, but do not print API keys, bearer tokens, raw auth headers, or full
  provider payloads containing secrets.
- Prefer durable notes under `docs/active/bugs/` for blocker reports. Keep them
  compact and evidence-backed.
- Do not mutate active run artifacts unless the user explicitly asks for repair
  of the artifact store itself.

## Reproduction Ladder

Choose the narrowest reproduction that exercises the same contract the loop
needed.

1. Step-level regression: use when the blocker is in phase advancement,
   scheduling, closure state, transition recording, route admission, or artifact
   accounting.
2. Replay-level regression: use when recorded model output, tool calls, or
   headless TUI behavior must pass through the live session/tool loop again.
   Recorded-provider replay is preferred over direct tool-result injection.
3. Protocol-level regression: use when the blocker is in segmentation,
   adjudication, oracle evidence, MBE integration, request shaping, or prompt
   contract behavior.
4. Doctor or preflight check: add this when the failure is detectable before a
   costly run, such as an invalid route/model configuration or required live API
   setting.

Live provider calls are allowed only when the relevant test or command is gated
behind `live_api_tests` or an explicit live-test opt-in, and the blocker cannot be
validated with local replay alone.

## Repair Loop

1. Name the broken contract in one sentence.
2. Add or update a bug report under `docs/active/bugs/` with the run evidence and
   intended reproduction surface.
3. Write the failing reproduction first, using the step, replay, protocol, or
   doctor surface selected above.
4. Run the reproduction and capture the failure signal.
5. Implement the smallest fix at the authority-bearing layer.
6. Re-run the reproduction, then the focused tests for the touched crate or
   workflow.
7. Update the bug report and any run-review note with the fix status, remaining
   risk, and exact resume command.

## Resume Gate

Resume the loop only after all applicable checks are true:

- The reproducing test or replay now passes.
- The admitted route, model, and protocol configuration match the intended run.
- Doctor or preflight checks cover the blocker if it can recur at setup time.
- The worktree and campaign artifacts still point to the intended run.
- Any non-blocking residual issue is filed separately as an alive bug.

## Report Shape

Use this shape when reporting back:

```text
Blocker:
Broken contract:
Evidence:
Reproduction:
Fix:
Verification:
Resume:
```
