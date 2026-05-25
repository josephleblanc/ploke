---
name: blocker-repair-loop
description: Use this skill when a Prototype 1 or ploke-eval diagnostic step, run review, replay, doctor check, or protocol run finds a blocker that should stop the loop, preserve evidence, and either become a reproducing regression before a fresh run or mark the current run abandoned when its persisted state is invalid.
---

# Ploke Blocker Repair Loop

Use this skill when the loop hits a blocker. It is the stop branch of the
diagnostic workflow: preserve the evidence, classify whether the current run can
be trusted, then either repair the source/workflow before a fresh run or resume
only when the persisted run state is still valid.

## Decision Gate

After a bounded `prototype1-step`, diagnostic review, or run review, classify the
issue before doing more loop work.

- Non-blocker: file or update an alive bug, note the residual risk, and continue
  the loop if the next step can still produce trustworthy evidence.
- Blocker: stop advancing the campaign. Do not keep producing run artifacts until
  the disposition is clear.

Treat an issue as a blocker when any of these are true:

- Continuing would create misleading evidence about model, tool, or protocol
  behavior.
- The controller cannot advance the required phase.
- Required oracle, patch, MBE, protocol, or adjudication evidence is missing,
  invalid, or internally contradictory.
- A provider, route, request-shape, tool-contract, or artifact-accounting failure
  blocks the current run target.
- A reported success does not prove the intended semantic operation happened.

There are two blocker dispositions:

- Repair-and-resume: use only when the persisted campaign state is still
  trustworthy and the broken contract is in code, configuration, environment, or
  an idempotent preflight. Add a reproduction, fix the source/workflow, verify,
  then resume the same campaign.
- Abandon-and-restart: use when the run has already admitted or persisted
  invalid, malformed, contradictory, or unverifiable evidence for a required
  loop transition. Do not patch readers to reinterpret that run into success.
  Keep the evidence, mark the run/worktree as abandoned for loop purposes, fix
  any source/workflow guardrail if one is needed, and start a fresh worktree and
  campaign.

Default to abandon-and-restart for invalid required protocol/eval/oracle
artifacts in a self-editing Prototype 1 run. This follows the History model in
`crates/ploke-eval/src/cli/prototype1_state/history.rs`: the loop is a
state-transition system over self-editing artifacts, and invalid transition
evidence must stop admission instead of being salvaged by a later reader change.
The goal is to make reward-hacking paths hard to admit: a candidate should not
benefit from corrupt or ambiguous evidence that only becomes acceptable after
the controller changes its interpretation.

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
- For abandon-and-restart blockers, record that the old campaign/worktree is a
  stop-use evidence source. Do not delete it unless the user explicitly asks.

## Reproduction Ladder

Choose the narrowest reproduction that exercises the same contract the loop
needed.

1. Step-level regression: use when the blocker is in phase advancement,
   scheduling, closure state, transition recording, route admission, or artifact
   accounting.
   For parent/child observe failures such as `MissingTreatmentEvidence`, the
   contract is the transition/evidence reader, not the model/tool loop. Recreate
   the historical sidecar/channel/journal shape and prove the current
   controller waits for the treatment-bearing terminal channel result before
   completing observation.
2. Replay-level regression: use when recorded model output, tool calls, or
   headless TUI behavior must pass through the live session/tool loop again.
   Recorded-provider replay is preferred over direct tool-result injection.
   For Prototype 1 blockers found in a live self-edit or benchmark-eval run,
   local recorded replay is necessary but not sufficient: after the local replay
   passes, run a same-target live replay/probe before declaring the blocker
   cleared for fresh loop work.
3. Protocol-level regression: use when the blocker is in segmentation,
   adjudication, oracle evidence, MBE integration, request shaping, or prompt
   contract behavior.
4. Doctor or preflight check: add this when the failure is detectable before a
   costly run, such as an invalid route/model configuration or required live API
   setting.

Correctness beats API-cost minimization. If the blocker depends on live provider
behavior, request shape, routing, quota handling, or provider-auth semantics,
verify it with the real provider rather than a mock. Gate Rust tests that make
live calls behind `live_api_tests` or an explicit live-test opt-in, and do not
claim the blocker is fixed from local replay alone when live behavior is the
contract under test.

Same-target live replay/probe means rerunning the historical target through the
current broad headless TUI path, not starting an unrelated fresh campaign. Use
the existing replay surfaces when available:

```bash
./target/debug/ploke-eval run replay self-edit-live \
  --request <published-broad-harness-request.json> \
  --result <historical.headless-tui.json> \
  --workspace <isolated-target-checkout> \
  --tail live-step \
  --max-attempts 1 \
  --timeout-secs <bounded> \
  --format json
```

If the headless result is missing but a raw provider sidecar exists, use
`--raw-full-response <llm_full_response.jsonl>` with `--through-response-index`
as the breakpoint. If the right surface is a benchmark eval turn rather than a
broad self-edit request, use `run replay turn-live`. If replay artifacts are
insufficient, use `loop prototype1-harness attempt` only against an isolated
checkout/request pair whose `.git` does not point back at the primary repo.

Do not use live self-edit replay as proof for parent/child handoff blockers.
For observe-child or successor-handoff blockers, read the actual transition
evidence first:

- `prototype1/transition-journal.jsonl`
- `prototype1/nodes/<node>/node.json`
- `prototype1/nodes/<node>/invocations/<runtime>.json`
- `prototype1/nodes/<node>/runner-result.json`
- `prototype1/nodes/<node>/results/<runtime>.json`
- `prototype1/nodes/<node>/channels/<runtime>/child-to-parent.jsonl`
- `prototype1/evaluations/<branch>.json`, when present
- `prototype1/history/`, `successor-ready/`, and `successor-completion/` for
  successor handoff failures

Then choose a step-level or journal/transition replay that exercises the same
reader/admission path. For a successful child, a minimal success sidecar is not
complete treatment evidence unless it points to or embeds the treatment result
the parent needs to compare against baseline. The regression should fail if the
parent can complete observation from the success sidecar alone while the
treatment-bearing channel `Result` is still pending or unread.

Do not write a regression whose effect is to make invalid persisted transition
evidence acceptable for the same run. If a regression is needed after an
abandon-and-restart decision, it should prove that the next fresh run succeeds,
that invalid state is blocked with a clear diagnostic, or that setup/doctor
detects the bad condition before paid loop work.

## Repair Loop

1. Name the broken contract in one sentence.
2. Add or update a bug report under `docs/active/bugs/` with the run evidence and
   intended disposition: repair-and-resume or abandon-and-restart.
3. If operating as orchestrator and the operator wants the live loop to keep
   moving, delegate the source repair to a bounded worker with explicit file
   ownership while the orchestrator continues monitoring or stepping the loop.
4. For repair-and-resume, write the failing reproduction first, using the step,
   replay, protocol, or doctor surface selected above.
5. Run the reproduction and capture the failure signal.
6. Implement the smallest fix at the authority-bearing layer.
7. Re-run the reproduction, then the focused tests for the touched crate or
   workflow.
8. For Prototype 1 blockers exposed by live model/tool behavior, run the
   same-target live replay/probe after local replay passes. If it cannot run due
   to quota, auth, or missing artifacts, record that explicitly; do not treat
   the blocker as fully cleared.
9. For parent observe, selection, or successor handoff blockers, replay or step
   the historical transition/evidence shape after the local regression passes.
   The acceptance condition is that the controller reaches the next intended
   phase using the same evidence class that failed in the live run.
10. Update the bug report and any run-review note with the fix status, remaining
   risk, and exact resume command.

For abandon-and-restart, skip current-run source salvage unless there is a
separate local bug that will affect the fresh run. Update the bug report and
orchestrator notes, then create a new worktree/campaign using
`ploke-prototype1-run-setup`.

## Resume Gate

Resume the same campaign only after all applicable checks are true:

- The reproducing test or replay now passes.
- If the blocker was in parent observation, selection, or successor handoff, the
  historical transition/evidence replay or equivalent step proof now reaches the
  next intended phase.
- The admitted route, model, and protocol configuration match the intended run.
- Doctor or preflight checks cover the blocker if it can recur at setup time.
- The worktree and campaign artifacts still point to the intended run.
- Any non-blocking residual issue is filed separately as an alive bug.
- If the repair changed code used by `prototype1-step`, remove stale build
  artifacts in the campaign worktree before resuming, or rebuild the campaign
  worktree binary. A safe pattern is:

  ```bash
  cargo clean
  /home/brasides/code/ploke/target/debug/ploke-eval loop prototype1-step --repo-root <campaign-worktree> --format json
  ```

  Run `cargo clean` from the stale campaign worktree, not from the source
  checkout with the fresh fix. This deletes only build artifacts; do not delete
  run evidence or campaign artifacts unless the user explicitly asks.

Do not resume a campaign whose required persisted protocol/eval/oracle evidence
has been classified invalid. Start a fresh campaign instead.

## Report Shape

Use this shape when reporting back:

```text
Blocker:
Broken contract:
Evidence:
Disposition:
Reproduction:
Fix:
Verification:
Resume:
```
