# 2026-05-24 Prototype 1 Orchestrator Loop Notes

Status: active orchestration notes for the long-running Prototype 1 diagnostic loop.

These notes capture orchestration-level observations only. Run-scoped evidence
belongs in `docs/active/agents/run-reviews/`; code or workflow defects belong in
`docs/active/bugs/` when they are actionable.

## Note 1: Board Setup And First Dispatch

Time: 2026-05-24 10:20 UTC

- Main checkout: `feature/ploke-loop`, clean before board setup, ahead of origin.
- Recent source gate: `cargo fmt --all` and `cargo test --workspace` had passed
  after `a3d8630a test: correlate decision tree validation traces`.
- Initialized `.orchestrator/board.json` and created task set `prototype1-loop`.
- Added lanes:
  - `prototype1-loop` for setup, doctor, and one bounded step.
  - `run-review` for artifact-backed run reviews.
  - `blocker-repair` for true stop-and-repair blockers.
- Added workers:
  - `loop-operator`
  - `run-reviewer`
  - `blocker-repair`
  - `notes-bug-docs`
- Surprise: lane validation caught overlapping `.orchestrator/**` ownership
  between loop and review lanes. I tightened those surfaces to worker-specific
  packet/report paths and re-ran `xtask orchestrate check`; board health is now
  green.
- Dispatched loop-operator sub-agent `019e597f-d360-7693-94c0-7fc76f390a02`
  with task `loop-operator-step`.

Next expected evidence:

- Exact campaign/worktree/model/provider route, anchored by files or CLI.
- Doctor phase before any advancement.
- At most one bounded `prototype1-step` result, or a setup/readiness blocker.
- Artifact roots suitable for a separate run-review agent.

## Note 2: Blocker Recurred From Stale Worktree Binary

Time: 2026-05-24 10:35 UTC

Loop operator report:

- Campaign: `p1-gemini35-flash-multigen-2g3x3-20260523-223658`.
- Worktree:
  `/home/brasides/.ploke-eval/worktrees/p1-gemini35-flash-multigen-2g3x3-20260523-223658`.
- Phase before and after the attempted step: `baseline_protocol`.
- Step result: nonzero exit, no protocol progress, no protocol artifact
  directory for this campaign.
- Provider rejection: OpenRouter `google-ai-studio` returned HTTP 400 because
  reasoning was mandatory and could not be disabled.

Local verification:

- The request log
  `/home/brasides/.ploke-eval/logs/ploke_eval_20260524_033013_947798.log`
  shows `reasoning.effort = "none"` in the request body.
- The campaign worktree is at `9087831a`, and its
  `crates/ploke-protocol/src/llm.rs` still hard-codes
  `.with_reasoning(... ReasoningEffort::None)`.
- The main checkout has the newer `ProtocolReasoningPolicy` path and only
  applies request reasoning when the admitted policy asks for it.

Interpretation:

This is not evidence that the current source fix regressed. It is evidence that
our resume workflow can re-hit a fixed blocker when the operator uses a stale
worktree-local binary/source for an older campaign. I blocked
`loop-operator-step` on the board as `protocol-reasoning-stale-worktree`.

Next safe action:

- Run the original run through current-source `prototype1-doctor
  --live-protocol-preflight` before any more `prototype1-step`.
- If that passes, decide whether resuming the campaign with the current fixed
  control binary is an accepted workflow, or whether the campaign should be
  restarted from a parent that includes the infrastructure fix.

## Note 3: Current-Source Preflight Found A New Canary Budget Blocker

Time: 2026-05-24 10:38 UTC

Command:

```text
target/debug/ploke-eval loop prototype1-doctor \
  --repo-root /home/brasides/.ploke-eval/worktrees/p1-gemini35-flash-multigen-2g3x3-20260523-223658 \
  --live-protocol-preflight \
  --format json
```

Result:

- Doctor used current main-checkout source, not the stale worktree binary.
- `protocol_preflight.reasoning = omit`, so the old request-shape bug is fixed
  in current source.
- HTTP status was 200.
- Doctor still reported `phase = blocked` because the model returned visible
  content `Here`, not sentinel JSON.

Log evidence:

- `/home/brasides/.ploke-eval/logs/ploke_eval_20260524_033510_949196.log`
  shows `max_tokens = 64`, `response_format = json_object`, status 200,
  finish reason `length` / `MAX_TOKENS`, and reasoning token use consuming most
  of the completion budget.

Interpretation:

The preflight is now too small for a reasoning-mandatory endpoint. This is a new
blocker because a live doctor preflight should distinguish request-shape failure
from response-budget failure instead of blocking a route that accepted the
request.

Action:

- Filed
  `docs/active/bugs/2026-05-24-prototype1-live-preflight-reasoning-budget-false-negative.md`.
- Dispatched blocker-repair sub-agent `019e598f-4837-74f2-b74f-47fa41d51247`
  on board task `repair-live-preflight-budget`.
- Loop advancement remains paused until focused tests pass and current-source
  live preflight is re-run successfully.

## Note 4: Live Preflight Repair Verified

Time: 2026-05-24 10:45 UTC

Repair:

- `run_protocol_live_preflight` now chooses canary budget through
  `protocol_live_preflight_max_tokens`.
- `reasoning = omit` and explicit reasoning-effort policies can use a bounded
  512-token canary.
- `reasoning = disabled` keeps the small 64-token canary.

Verification:

- `cargo test -p ploke-eval protocol_live_preflight_budget -- --nocapture`
  passed.
- `cargo run -p ploke-eval -- loop prototype1-doctor --repo-root
  /home/brasides/.ploke-eval/worktrees/p1-gemini35-flash-multigen-2g3x3-20260523-223658
  --live-protocol-preflight --format json` passed.

Observed live preflight result:

```text
protocol_preflight.outcome = passed
model_id = google/gemini-3.5-flash
provider = google-ai-studio
route_source = openrouter
reasoning = omit
max_tokens = 512
phase = baseline_protocol
blockers = []
```

Next action:

- Unblock the loop operator task and resume with the current main-checkout
  binary. Do not use the stale worktree-local
  `./target/debug/ploke-eval` from the older campaign worktree.

## Note 5: Loop Operator Resumed After Repair

Time: 2026-05-24 10:46 UTC

Board action:

- Resolved blocker `protocol-reasoning-stale-worktree` with evidence from the
  fixed live preflight.
- Reactivated `loop-operator-step` for worker `loop-operator`.
- Regenerated `.orchestrator/workers/loop-operator.md`.

Source checkpoint:

- Committed the live-preflight repair and notes as
  `be08adce Fix live protocol preflight canary budget`.

Dispatch:

- Spawned loop-operator sub-agent `019e5997-81ec-72d3-8905-097a83d92ba0`.
- Scope is one bounded Prototype 1 step against campaign
  `p1-gemini35-flash-multigen-2g3x3-20260523-223658`.
- The operator was explicitly instructed to use the current main-checkout
  binary through `cargo run -p ploke-eval -- ...`, not the stale worktree-local
  binary.

## Note 6: Protocol Segmentation Trailing-Characters Blocker

Time: 2026-05-24 10:51 UTC

Loop result:

- The operator ran doctor, one bounded step, and doctor again with the current
  main-checkout binary.
- Doctor before and after reported `phase = baseline_protocol` and
  `blockers = []`.
- The step exited nonzero with no protocol progress:
  `segmentations = 0`, `call_reviews = 0`, `segment_reviews = 0`.

Failure:

- `tool_call_intent_segmentation` failed with
  `failed to parse json response: trailing characters at line 62 column 1`.
- The provider response was HTTP 200 and contained a complete segmentation JSON
  object followed by an extra top-level closing brace.

Board action:

- Blocked `loop-operator-step` on
  `protocol-segmentation-json-trailing-characters`.
- Filed
  `docs/active/bugs/2026-05-24-prototype1-protocol-segmentation-json-trailing-characters.md`.

Next action:

- Hand off to blocker repair for a protocol-level regression and the smallest
  parser or retry-classification fix. Do not run another loop step until the
  reproduction passes and the Gemini campaign is rechecked.

## Note 7: Protocol Trailing-Brace Parser Repair Verified

Time: 2026-05-24 11:35 UTC

Repair:

- `parse_protocol_json_content` now recovers a complete root JSON object prefix
  only when the trailing suffix is non-empty and contains only redundant closing
  braces plus whitespace.
- The parser still rejects non-brace trailing text and non-object roots with
  trailing braces, so arbitrary malformed provider output is not accepted as
  success.

Verification:

- `cargo test -p ploke-protocol parse_protocol_json_content_ -- --nocapture`
- `cargo test -p ploke-protocol parse_protocol_json_content_ -- --nocapture`
  passed with 7 tests after the object-root narrowing.
- `cargo test -p ploke-protocol` passed with 19 tests plus doc tests.
- `cargo fmt --all` completed.

Resume command:

```text
cargo run -p ploke-eval -- loop prototype1-step \
  --repo-root /home/brasides/.ploke-eval/worktrees/p1-gemini35-flash-multigen-2g3x3-20260523-223658 \
  --format json
```

Use the current source checkout binary via `cargo run -p ploke-eval`; do not use
the stale campaign worktree-local `./target/debug/ploke-eval`.

## Note 8: Protocol Segmentation Persisted After Repair

Time: 2026-05-24 11:00 UTC

Loop result:

- The resumed loop operator used the current source checkout at `3949c88e`.
- Doctor before the step was clean: `phase = baseline_protocol`,
  `blockers = []`.
- The bounded step exited 0.
- Doctor after the step still reported `phase = baseline_protocol`, but closure
  state advanced from protocol `missing` to protocol `partial`.

Persisted evidence:

- `tool-call-intent-segments` is now complete.
- `tool-call-review` and `tool-call-segment-review` remain missing.
- New protocol artifact:
  `/home/brasides/.ploke-eval/protocol/prototype1/p1-gemini35-flash-multigen-2g3x3-20260523-223658/BurntSushi__ripgrep-2209/runs/run-1779602498508-structured-current-policy-ed4a409a/1779620303372_tool_call_intent_segmentation_BurntSushi__ripgrep-2209.json`

Interpretation:

- This validates that the parser repair unblocked the original protocol
  segmentation failure for the live Gemini campaign.
- The loop is not complete yet; the next safe action is another bounded
  protocol step to produce one of the remaining required procedure artifacts.

## Note 9: Stored Segmentation Not Aggregate-Usable

Time: 2026-05-24 11:08 UTC

Loop result:

- The next bounded protocol step exited 1 and created no new protocol artifacts.
- Closure remained `registry = complete`, `eval = complete`,
  `protocol = partial`.
- `tool-call-intent-segments` remained complete; `tool-call-review` and
  `tool-call-segment-review` remained missing.

Important status contradiction:

```text
artifact_count = 1
segmentation_present = true
aggregate_available = false
next_step = intent_segmentation
```

Interpretation:

- The loop should not have retried live intent segmentation while a stored
  segmentation artifact already existed.
- The failed retry produced another malformed segmentation response, but that is
  a symptom. The higher-signal blocker is that the aggregate/planner path either
  cannot use the stored segmentation anchor or fails to report why it cannot use
  it.

Action:

- Blocked `loop-operator-protocol-step-2` as
  `protocol-segmentation-anchor-skipped`.
- Filed
  `docs/active/bugs/2026-05-24-prototype1-protocol-segmentation-anchor-skipped.md`.
- Next action is a blocker-repair pass with a local regression around protocol
  artifact loading/aggregate planning. Do not run another live protocol step
  until this path is repaired or clearly diagnosed.

## Note 10: Abandon Current Run Instead Of Salvaging Invalid State

Time: 2026-05-24 11:20 UTC

Operator correction:

- The current segmentation-anchor issue should not be repaired as a way to
  continue the same worktree/campaign.
- The Prototype 1 loop is a self-editing harness, so required transition
  evidence must be cleanly admissible when produced.
- If required protocol evidence becomes invalid, an attempt to advance should
  block the current run. The next operational move is a fresh worktree/campaign,
  not changing readers so the old run can continue.

Action:

- Closed the blocker-repair sub-agent and removed its partial local source
  edit.
- Updated the diagnostic/setup/blocker skills to distinguish
  repair-and-resume from abandon-and-restart.
- Reclassified
  `docs/active/bugs/2026-05-24-prototype1-protocol-segmentation-anchor-skipped.md`
  as an abandoned-run blocker.

Next action:

- Do not run another `prototype1-step` against
  `p1-gemini35-flash-multigen-2g3x3-20260523-223658`.
- Start a fresh Gemini 3.5 Flash worktree/campaign after re-reading the current
  profile/model/provider files.

## Note 11: First Fresh Attempt Abandoned For Wrong Route And Aborted Eval

Time: 2026-05-24 21:02 UTC

Fresh attempt:

- Campaign: `p1-gemini35-flash-multigen-fresh-20260524-112232`.
- Worktree:
  `/home/brasides/.ploke-eval/worktrees/p1-gemini35-flash-multigen-fresh-20260524-112232`.
- Model: `google/gemini-3.5-flash`.

What happened:

- Workspace gate passed before setup.
- Setup and doctor succeeded mechanically.
- A bounded `prototype1-step` moved doctor from `baseline_eval` to
  `baseline_protocol`, and closure marked eval complete.
- The campaign was actually admitted with `route_source = open_router`, not the
  intended direct-Google route.
- The recorded agent turn ended `aborted` after an OpenRouter HTTP 402
  credit/token-limit error; the raw provider body is sensitive and should not be
  quoted.
- The exported patch is non-empty but only adds an unused helper in
  `crates/printer/src/util.rs`; the original callsite remains unchanged.
- Focused target checks pass only because the helper is unused:
  `cargo check -p grep-printer`, `cargo test -p grep-printer`, and
  `cargo fmt -- --check`.

Interpretation:

- This worktree/campaign is abandoned as setup-invalid for the direct-Google
  lane and invalid as clean eval evidence.
- Filed
  `docs/active/bugs/2026-05-24-prototype1-eval-complete-after-aborted-turn.md`.
- Updated setup/diagnostic skills so future operators refresh/verify model
  route provenance and check terminal turn outcome before treating a patch as
  clean eval completion.

Next action:

- Start another fresh campaign with explicit `--route-source direct-google`
  after the refreshed registry shows `google/gemini-3.5-flash` as
  `direct_google`.

## Note 12: Fresh Direct-Google Setup Blocked By Worktree Env Launch

Time: 2026-05-25 02:10 UTC

Fresh attempt:

- Campaign: `p1-gemini35-flash-direct-fresh-20260524-190016`.
- Worktree:
  `/home/brasides/.ploke-eval/worktrees/p1-gemini35-flash-direct-fresh-20260524-190016`.
- Parent node: `node-d7757d356141f9f1`.
- Route: `google/gemini-3.5-flash` via `direct_google`.

What happened:

- Workspace gate passed before setup.
- Setup and doctor succeeded.
- `prototype1-doctor --live-protocol-preflight` passed with
  `provider = google`, `route_source = direct_google`, and `reasoning = omit`.
- The bounded step was launched from the new worktree using its local binary.
- That worktree launch environment did not expose the provider credentials that
  are present from the source checkout launch environment.
- Baseline eval failed before a model turn during OpenRouter-backed eval
  embedding preflight for `mistralai/codestral-embed-2505`.
- `prototype1-step --format json` returned a doctor-shaped report with
  `phase = baseline_eval` and `blockers = []`; the eval failure was visible
  only after reading closure status.
- The abandoned worktree was cleaned with `cargo clean`, removing 7.6 GiB of
  build artifacts.

Interpretation:

- This does not invalidate the direct-Google chat/protocol route; the live
  protocol canary passed.
- The campaign is abandoned as clean evidence because the first baseline eval
  transition failed for operator launch-env reasons.
- Filed
  `docs/active/bugs/2026-05-25-prototype1-step-env-cwd-preflight-reporting.md`.

Next action:

- Start another fresh campaign/worktree.
- Launch live setup/doctor/step commands from `/home/brasides/code/ploke`, using
  `/home/brasides/code/ploke/target/debug/ploke-eval --repo-root <fresh-worktree>`
  where applicable so the command inherits the env-bearing source checkout.

## Note 13: Fresh Direct-Google Eval And Protocol Completed

Time: 2026-05-25 02:25 UTC

Fresh attempt:

- Campaign: `p1-gemini35-flash-direct-fresh-20260524-190632`.
- Worktree:
  `/home/brasides/.ploke-eval/worktrees/p1-gemini35-flash-direct-fresh-20260524-190632`.
- Parent node: `node-ea38dcdf301d7d63`.
- Route: `google/gemini-3.5-flash` via `direct_google`.

What happened:

- Setup and doctor succeeded.
- Live protocol preflight passed with `provider = google`,
  `route_source = direct_google`, `reasoning = omit`, and `max_tokens = 512`.
- Baseline eval completed from the source-checkout launch environment.
- The eval produced a non-empty Multi-SWE-bench patch touching
  `crates/printer/src/util.rs` and `crates/printer/src/standard.rs`.
- Direct target verification showed `cargo test -p grep-printer` passed, while
  `cargo fmt -- --check` failed on indentation in
  `crates/printer/src/util.rs`.
- Protocol initially became partial after segmentation and a direct-Google
  timeout before call review.
- A second bounded protocol step resumed from the partial state and completed
  all required protocol procedures.

Protocol evidence:

- `protocol.status = complete`.
- `tool_call_intent_segmentation = complete`.
- `call_review_count = 56`.
- `segment_review_count = 4`.
- `missing_call_indices = []`.
- `missing_segment_indices = []`.
- Doctor phase advanced to `child_plan`.

Interpretation:

- This is the cleanest evidence so far that the route/profile/doctor/step
  workflow can move through eval and protocol on the direct-Google lane.
- The retry policy is useful: malformed direct-Google adjudication responses
  with trailing sentence fragments were retried per item rather than admitted
  as invalid protocol evidence.
- The run is still not benchmark-clean because the patch fails formatting, and
  eval validation remains too weak when the model does not run a format check.

Next action:

- Continue with one bounded step from `child_plan`.
- Preserve this run review as positive protocol evidence and keep the cargo
  validation issue as a non-blocking bug.

## Note 14: Child Plan Blocked By Parent-Patcher Route Preference

Time: 2026-05-25 02:27 UTC

Attempted action:

- Ran one bounded `prototype1-step` from `child_plan` for
  `p1-gemini35-flash-direct-fresh-20260524-190632`.

What happened:

- The step published nine broad headless-TUI request slots.
- All nine slots failed before an admissible edit.
- The current log shows the same route-source split in a new path:
  `headless_model_route` paired direct-Google model
  `google/gemini-3.5-flash` with persisted OpenRouter provider preference
  `google-ai-studio`.
- The batch guard rejected admission with `0` children where the minimum was
  `3`.
- Post-failure doctor reports `phase = blocked` because published prompt files
  reference candidate workspaces that were never materialized.

Fix:

- Patched the parent-patcher default model-selection path so persisted provider
  preferences are ignored for direct-Google registry rows.
- Explicit provider pins still validate and reject incompatible OpenRouter
  providers for direct-Google routes.
- Added focused regressions:
  - `parent_patcher_selection_ignores_openrouter_preference_for_direct_google_registry_row`
  - `headless_model_selection_explicit_direct_google_rejects_openrouter_provider_pin`

Verification:

- Both focused tests pass.
- Filed
  `docs/active/bugs/2026-05-25-parent-patcher-direct-google-provider-preference.md`.

Disposition:

- Treat the current campaign as stop-use for clean child-plan evidence because
  it already contains failed published request-slot artifacts and prompt
  preflight blockers.
- Start the next trustworthy loop evidence from a fresh campaign after the fix
  is committed.
