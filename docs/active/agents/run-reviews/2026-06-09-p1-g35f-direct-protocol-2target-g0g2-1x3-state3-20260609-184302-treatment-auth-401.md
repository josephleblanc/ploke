# Run review: p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-184302

Status: complete run review of a genuine failure. The campaign ran to a normal
terminal (`stop_after: Complete`, all 3 planned gen-1 children ran) but produced
`outcome: completed:Reject`, `selection=none`, `node_status: Failed`. This review
proves the cause and separates the environmental trigger from the Ploke-side
contract gap. This run used pre-PR1-4 `tui_adapter` code; no `Attempt` /
`SurfacePolicy` paths were exercised.

## Short verdict

The run FAILED to make real progress. It is NOT a legitimate "no useful child,
nothing to select" terminal: the three children were never adjudicated on merit.
Each child's treatment eval phase was killed by a provider authentication
expiry (`HTTP 401`, direct Google `aiplatform.googleapis.com`), so no child
produced complete run metrics, which made every child `treatment_failed`, which
left the selector with no valid candidate.

Single clearest piece of evidence — every treatment turn that ran after
~19:18 aborted on its first/next LLM request with:

```text
code=HTTP_401 kind=http_status
API error (status 401): "Request had invalid authentication credentials.
Expected OAuth 2 access token, login cookie or other valid authentication
credential." reason=ACCESS_TOKEN_TYPE_UNSUPPORTED
method=google.cloud.aiplatform.v1.PredictionService.ChatCompletions
```

(`treatments/branch-3d97a97a7b40f2c5/instances/BurntSushi__ripgrep-2209/runs/run-1781057869108-structured-current-policy-5a017390/agent-turn-summary.json`,
`TurnFinished.outcome = aborted`, `attempts = 1`.)

## Root cause (one paragraph) + classification

The `direct_google` OAuth2 access token used for `google/gemini-3.5-flash`
expired partway through the run, in the window **~19:17:36–19:18:16 local**.
Every LLM request after that boundary returned `HTTP 401`
(`ACCESS_TOKEN_TYPE_UNSUPPORTED`). Each gen-1 child's treatment campaign
evaluates two SWE-bench instances (`BurntSushi__ripgrep-2295` first, then
`BurntSushi__ripgrep-2209`). The `2295` instances ran 19:12–19:18 and mostly
succeeded (two of three applied real patches); the `2209` instances ran 19:17–19:19
and aborted immediately with `401` in all three branches. Because the treatment
batch requires complete metrics for both instances, each child failed with
`disposition: treatment_failed` ("did not produce complete run metrics
(status=failed)"). With all candidates failed, successor selection had nothing to
score (`selection=none`), and the parent loop honestly surfaced `Reject` /
`node_status: Failed`.

**Classification: provider / environment failure (credential/auth expiry).**
Not model behavior (the model never got to respond on the failing turns), not
candidate generation (broad-harness self-edits to
`crates/ploke-tui/src/tools/mod.rs` were generated, applied, and committed), not
protocol adjudication, not the successor-selection logic (it correctly found no
valid candidate). Secondary, Ploke-side contract gap: the eval harness has no
mid-run token refresh/retry and no distinct "infrastructure/provider auth"
disposition, so a transient provider outage is recorded as a permanent
`treatment_failed` and silently sinks an entire generation.

## Evidence roots

- Campaign root:
  `~/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-184302/`
- Worktree (pre-refactor checkout used by the run):
  `~/.ploke-eval/worktrees/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-184302/`
- Transition journal:
  `…/prototype1/transition-journal.jsonl` (42 entries, last `recorded_at`
  19:20:10 local)
- Parent node: `node-a3e9bd7e78a9a124` (gen0). Gen-1 children:
  `node-5743f8012fcf38c6` (branch-a2135367117c9024, broad-harness-g1-01),
  `node-669d264e924d5d20` (branch-cca64866f34186f3, broad-harness-g1-02),
  `node-6ef6d15c2256e3bd` (branch-3d97a97a7b40f2c5, broad-harness-g1-03;
  child_runtime `9723cb91-38ca-404f-947b-1bcc27b6214f`, the one named in the CLI
  summary).
- Treatment eval roots:
  `~/.ploke-eval/instances/prototype1/…/treatments/branch-*/instances/BurntSushi__ripgrep-{2209,2295}/runs/run-*/`
- Route/credential config: `campaign.json` and `closure-state.json` —
  `model_id=google/gemini-3.5-flash`, `route_source=direct_google`.

## Closure / mechanical state

- Baseline (gen0) eval+protocol closure is mechanically complete:
  `closure-state.json` `eval.status=complete` (expected=2, complete=2, failed=0),
  protocol baseline complete. Baseline ran 18:45–18:57 and is unaffected.
- Gen-1 children all `status: failed` (`nodes/*/node.json`), each via
  `runner-result.json` `disposition=treatment_failed`.
- Branch registry: not written (`History Selection Formula Rows: missing`),
  consistent with no sealed selection — selection never produced a candidate set
  because all upstream children failed.

## Per-child trace (3 children, same failure shape)

Each row: the child's broad-harness self-edit succeeded; the downstream
treatment eval failed because the second instance hit `401`.

| Child node | branch | broad self-edit (gen) | treatment `2295` | treatment `2209` | child disposition |
| --- | --- | --- | --- | --- | --- |
| node-5743f8012fcf38c6 | a2135367117c9024 | edit applied to `tools/mod.rs` | `run-1781057503842` completed, patch applied | `run-1781057834568` aborted `HTTP_401` (attempts=2) | treatment_failed |
| node-669d264e924d5d20 | cca64866f34186f3 | edit applied to `tools/mod.rs` | `run-1781057503842` aborted `HTTP_401` at attempt 44 (~19:18:16 last response) | `run-1781057936873` aborted `HTTP_401` (attempts=1) | treatment_failed |
| node-6ef6d15c2256e3bd | 3d97a97a7b40f2c5 | edit applied to `tools/mod.rs`, committed `0831614f…` | `run-1781057505015` completed (last response 19:17:36), patch applied | `run-1781057869108` aborted `HTTP_401` (attempts=1, ~19:18:33) | treatment_failed |

Concrete trace chain (representative child `node-6ef6d15c2256e3bd`):

```text
broad-harness-request:node-a3e9bd7e78a9a124:r3
  -> headless TUI self-edit applies crates/ploke-tui/src/tools/mod.rs
  -> child binary built (node-6ef6d15c2256e3bd/bin/ploke-eval), derived commit 0831614f
  -> treatment campaign launched (2 instances)
     -> 2295 turn: indexing OK -> model works -> patch applied (last LLM 19:17:36)
     -> 2209 turn: indexing OK -> first ChatCompletion -> TurnFinished aborted HTTP_401
        -> patch_artifact.applied=false, any_expected_file_changed=false
        -> validation-audit: changed_paths=[], cargo_calls=[]
  -> treatment batch lacks complete metrics for 2209
  -> observe_child(after): failed disposition=treatment_failed
     "batch selection is invalid: treatment '…-branch-3d97a97a7b40f2c5-…'
      instance 'BurntSushi__ripgrep-2209' did not produce complete run metrics (status=failed)"
  -> node.json status=failed
```

The `2295` treatment turns that completed earlier ended with
`TOOL_EXECUTION_FAILED kind=tool_execution` ("insert_rust_item: No inline…") yet
still `applied=true, any_expected_file_changed=true` — i.e. the model made real
edits on the calls that ran before the token died. This confirms the binary and
its credentials were working pre-expiry; the failure is temporal, not a
missing-credential-in-subprocess problem (same child binary served both `2295`
success and `2209` `401`).

## Successor selection

`selection=none` because there was no valid candidate to score, not because of a
selection-logic bug or a below-threshold reject. All three children were
`treatment_failed` before any branch evaluation / metric-set / History selection
block was produced. No `successor_runtime` / `successor_pid`, so there is no
successor stderr to inspect. The state machine's terminal `Reject` is the honest
projection of "every input child failed."

## Required distinctions (run-review gate)

- mechanical completion: baseline closure complete; gen-1 nodes terminal-failed.
- benchmark outcome: zero benchmark progress from the children's treatment evals
  (no `2209` metrics in any branch); self-edits were generated but never scored.
- provider trace: clean `HTTP_401 ACCESS_TOKEN_TYPE_UNSUPPORTED` on the failing
  turns; healthy `tool_calls`/`stop` finish reasons before the expiry boundary.
- recorded tool lifecycle vs information success: the failing turns made no tool
  calls at all (aborted on the first request).
- protocol judgment: protocol only produced artifacts for the baseline run; it
  did not (and could not) adjudicate the aborted treatment turns. Do not read the
  baseline protocol completeness as child progress.

## Broken contract (bug-report-discipline)

Expected invariant: a transient provider authentication failure during the
treatment phase should not be silently converted into a permanent
`treatment_failed` disposition that discards an entire generation; and when it
is, the terminal record should classify it as an infrastructure/provider failure
distinct from a merit-based child reject.

What happened instead: a mid-run `direct_google` token expiry produced `HTTP 401`
on all post-expiry turns; the harness recorded each child as `treatment_failed`
("did not produce complete run metrics"), the parent surfaced
`completed:Reject` / `node_status: Failed`, and nothing in the durable terminal
summary distinguishes "provider auth died" from "the model's children were bad."

Source boundaries to inspect (named, not yet patched):

- Treatment batch validity / disposition mapping (the producer of
  `disposition=treatment_failed` "batch selection is invalid … did not produce
  complete run metrics"). This is where an aborted-on-`HTTP_401` turn should be
  classified as provider/infrastructure rather than merit failure.
- The benchmark turn driver that records `TurnFinished{outcome:aborted, code:HTTP_401}`
  but still writes a "completed steps" `execution-log.json` and an empty
  `patch_artifact`/`validation-audit` — the abort cause is not propagated into
  the treatment disposition.
- `direct_google` credential acquisition/refresh for long multi-phase runs (no
  token refresh or retry-on-401 across the >30 min run).

## Record-surface status

| Surface | Label | Notes |
| --- | --- | --- |
| campaign/closure/profile | present | `direct_google`, baseline closure complete. |
| transition journal | present | 9 spawn/9 child/6 observe/3 committed; all observe_child(after) = treatment_failed. |
| node records (gen1) | present | all `status=failed`, `runner-result.json` disposition=treatment_failed. |
| broad-harness request/result + headless sidecars | present, manual join needed | self-edits to `tools/mod.rs` applied + committed for all three. |
| treatment run roots | present | execution-log "complete", but `agent-turn-summary` shows aborted HTTP_401 turns and empty patch/validation. |
| treatment `llm-full-responses.jsonl` | present, manual join needed | aborted turns carry the raw 401 body. |
| branch evaluation / metric set / History selection | record absent | never produced; selection=none is consistent. |

## Current repro coverage vs missing repro

- Current coverage: none specific to "mid-run provider auth expiry cascades to a
  whole-generation `treatment_failed` with no distinct classification." Related
  provider-error reviews exist (HTTP 429) but cover different lifecycle bugs.
- Missing minimal repro: a replay/unit test that feeds an aborted-on-`HTTP_401`
  treatment turn (or a treatment campaign with one instance missing metrics due
  to provider abort) through the treatment-disposition mapping and asserts it is
  classified as provider/infrastructure failure (and, ideally, retried or paused)
  rather than `treatment_failed`. The persisted artifacts here
  (`run-1781057869108…/agent-turn-summary.json`,
  `nodes/node-6ef6d15c2256e3bd/runner-result.json`,
  `transition-journal.jsonl`) are sufficient inputs for a historical-replay test.

## Action items (prioritized)

1. Operational (immediate): re-run with a non-expiring credential path for the
   full expected wall-clock (refresh the `direct_google` token, or use a
   provider/route whose key does not expire mid-run, e.g. an OpenRouter or
   long-lived key). This run's failure is an environment artifact; the children's
   self-edits and `2295` evals were progressing.
2. Resilience (Ploke): add token refresh / bounded retry-on-401 for `direct_google`
   long runs, and/or pause-and-resume the treatment phase on a provider-auth
   abort instead of finalizing the child as `treatment_failed`.
3. Classification/observability (Ploke): introduce a provider/infrastructure
   disposition distinct from merit `treatment_failed`, and surface it in the
   terminal loop summary so `node_status: Failed` is not conflated with
   "no useful child."
4. Regression: add the historical-replay test described above before treating
   this class as fixed (see `.codex/skills/historical-replay-test`).

## PR1–4 / PR5+ relevance

This run predates and does not exercise the PR1–4 `tui_adapter` refactor
(`Attempt`/`SurfacePolicy`); the broad self-edit ran on the pre-refactor headless
path. The failure is in the treatment-eval / disposition layer, not the broad
edit-surface adapter, so PR1–4 would not have changed this outcome. PR5+ is only
relevant if it touches treatment disposition classification or provider/credential
handling — it does not address mid-run auth expiry as described here.

## Related

- Scout fan-in for the sibling direct-Google two-target campaign (different
  incident: HTTP 429 + rejected-branch selection + gen2 stale-observe):
  [`2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-auth3-scout-fanin.md`](2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-auth3-scout-fanin.md)
- Prior provider-unavailable child review (same provider-fragility class):
  [`2026-06-01-p1-gemini35-flash-direct-15g2x3-par2-20260601-173956-node-552c19a55f53dbe6-r5-provider-unavailable.md`](2026-06-01-p1-gemini35-flash-direct-15g2x3-par2-20260601-173956-node-552c19a55f53dbe6-r5-provider-unavailable.md)
