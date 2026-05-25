# Prototype 1 Continue Loops On Protocol Quota With No Progress

Status: fixed in code; step path live-verified, continue path pending live verification with safer review fanout
Discovered: 2026-05-22

## Summary

`prototype1-continue` can spin through the same `baseline_protocol` phase after
live protocol adjudication hits provider quota/rate limits. The controller
keeps attempting protocol advancement even when the previous attempt produced
no new required protocol artifacts, until the hard 256-advance guard fires:

```text
batch selection is invalid: prototype1-continue exceeded 256 phase advances without reaching a terminal state
```

Observed during the Google live run campaign
`p1-google-live-run-20260521-1`.

## Observed Failure

The live command was run from the parent worktree:

```text
/home/brasides/.ploke-eval/worktrees/p1-google-live-run-20260521-1
./target/debug/ploke-eval loop prototype1-continue --repo-root .
```

The provider errors were Google Vertex OpenAI-compatible endpoint responses,
not OpenRouter:

```text
WARN chat_http ... url="https://aiplatform.googleapis.com/v1/projects/<redacted>/locations/us-central1/endpoints/openapi/chat/completions" status=429
```

After the run, `prototype1-doctor` still reported:

```text
phase: baseline_protocol
allowed_actions: doctor, continue, step
```

The campaign closure state was partial:

```text
protocol.status = partial
tool-call-intent-segments = complete
tool-call-review = missing
tool-call-segment-review = missing
```

Only one protocol artifact existed:

```text
.../1779409231071_tool_call_intent_segmentation_BurntSushi__ripgrep-2209.json
```

## Affected Surface

- `crates/ploke-eval/src/cli/prototype1_state/run/core.rs`
  - `resume` loops until `Complete`, `Blocked`, or the numeric 256 guard.
  - `advance_baseline_protocol` calls `advance_protocol_closure` and discards
    the returned progress/failure report.
- `crates/ploke-eval/src/cli.rs`
  - `execute_protocol_run_tasks` records worker failures when
    `stop_on_error = false`.
  - `advance_protocol_closure` returns those failures in
    `ClosureAdvanceProtocolReport`, but the live continue path does not use
    them to decide whether progress occurred.
- Google direct route protocol adjudication through `ploke-protocol`.

## Failure Shape

The control loop currently treats this sequence as "try again":

```text
diagnose -> BaselineProtocol
advance_baseline_protocol
  -> advance_protocol_closure
     -> execute_protocol_run_tasks
        -> provider returns 429 for remaining protocol work
        -> failures are collected, not terminal, because stop_on_error=false
  -> returns Ok(report)
diagnose -> BaselineProtocol again
repeat until guard > 256
```

The loop has no progress proof for the phase advance. If protocol artifacts are
not created and the phase remains `baseline_protocol`, `prototype1-continue`
should not blindly retry the same provider work.

## Root Cause Analysis

External trigger: Google returned HTTP 429 from the Vertex OpenAI-compatible
chat/completions endpoint during protocol adjudication. The campaign was
actually using the direct Google route: the campaign model was
`google/gemini-2.5-flash`, the run execution log recorded
`selected_provider = google`, and the stale OpenRouter registry warning was not
the chat/protocol route.

Primary internal bug: the Prototype 1 controller treats a protocol closure
attempt as a completed phase advance whenever `advance_protocol_closure`
returns `Ok(report)`. That helper can return `Ok` with nonempty `failures`
when `stop_on_error = false`; the command renderer prints those failures, but
the `prototype1-continue` path discards the report and immediately re-diagnoses
the same `baseline_protocol` phase.

Progress-accounting wrinkle: a single protocol run can create an early
artifact, then fail later in the same run. In the observed campaign the intent
segmentation artifact exists, while call review and segment review remain
missing. Because `execute_protocol_run_task` returns `Err` on the later failure,
its local creation counters do not necessarily describe all artifacts created
before the error. A robust controller check must therefore inspect the
recomputed closure `before`/`after` summary as well as the report counters.

Current live state confirms the no-progress loop condition:

```text
phase: baseline_protocol
eval.status = complete
protocol.status = partial
tool-call-intent-segments = complete
tool-call-review = missing
tool-call-segment-review = missing
selected_runs[0].segmentation_needed = false
selected_runs[0].missing_call_indices = [0, 1, 2, 3, 4]
selected_runs[0].missing_segment_indices = [0, 1, 2]
```

Secondary contributing factor: protocol review fanout is not provider-aware.
The current campaign protocol policy had `max_concurrency = 100`; run-level
protocol work is capped by `TOOL_REVIEW_CALL_LIMIT = 8`, and Google JSON
protocol requests currently use `PROTOCOL_HTTP_MAX_ATTEMPTS = 1`. That makes
rate/quota failures more likely to surface as immediate run-level failures,
which is acceptable only if the controller stops cleanly instead of retrying
the same no-progress phase.

## Triage

Severity: high for full Prototype 1 live-loop reliability. This blocks Google
full-loop validation and can repeatedly hit the paid live provider endpoint
until the 256-advance guard fires.

Not a Google `Router` correctness bug: the Google route is selected and reaches
the Google endpoint. The bug is the Prototype 1 controller's missing
progress/no-progress transition for protocol closure failures.

Affected commands:

- `prototype1-continue`: must stop after the first no-progress protocol advance
  and report the failure details.
- `prototype1-step`: may perform one bounded attempt, but should still surface a
  no-progress protocol failure instead of silently returning success with the
  phase unchanged.
- `closure advance protocol`: can continue returning a report with failures
  when `stop_on_error = false`, because that command is a batch/report surface;
  the controller must interpret the report before deciding whether to loop.

Immediate fix target: `advance_baseline_protocol` should consume the
`ClosureAdvanceProtocolReport`. If failures are present and the protocol
closure summary did not advance, return a blocking `PrepareError` that includes
campaign id, phase, selected runs, remaining procedures, created counts, and a
bounded failure summary. The `resume` loop should no longer need the 256 guard
to protect this phase.

Follow-up after the controller fix: consider Google-aware protocol throttling or
retry/backoff for JSON adjudication. That is a reliability improvement, not the
root fix for this bug.

## Expected Behavior

When a protocol advancement attempt produces failures and no new required
protocol artifacts, the live controller should stop with an actionable blocking
diagnostic instead of looping.

The operator-facing error should include:

- current phase: `baseline_protocol`;
- campaign id and instance id;
- remaining procedures;
- provider/model route, e.g. direct Google model `google/gemini-2.5-flash`;
- the relevant provider status, e.g. HTTP 429 quota/rate limit;
- whether any new protocol artifacts were created during the attempted advance.

`prototype1-step` may perform one bounded attempt, but `prototype1-continue`
must not hammer the same no-progress phase repeatedly.

## Fix Direction

Add a progress/no-progress decision to the Prototype 1 controller boundary.

For `BaselineProtocol`, inspect the `ClosureAdvanceProtocolReport` returned by
`advance_protocol_closure`. If it contains failures and all creation counters
are zero, or if recomputed closure state is unchanged for the selected
procedures, convert the phase to `Blocked` or return a `PrepareError` with the
failure details.

Longer term, replace the numeric 256 guard with explicit phase-transition
progress fingerprints:

```text
BeforeState + AdvanceResult -> Progress | Blocked(NoProgress) | Complete
```

No provider quota/rate-limit path should depend on an arbitrary loop-count
backstop for safety.

## Implementation Note

Implemented on 2026-05-22:

- `advance_protocol_closure` now preserves selected run plans even when a
  worker later fails, so controller diagnostics can name the remaining work.
- `advance_protocol_or_block` interprets the existing
  `ClosureAdvanceProtocolReport` for the Prototype 1 controller.
- `advance_baseline_protocol` now calls `advance_protocol_or_block` instead of
  discarding the closure report.
- A protocol report with incomplete status and unchanged closure summary now
  returns a blocking `PrepareError`; a report with failures but changed closure
  summary is still treated as progress.

## Live Verification: 2026-05-25 Step Path

Campaign:

```text
p1-gemini35-flash-direct-15g2x3-clean-20260525-120225
```

Run:

```text
run-1779710592442-structured-current-policy-8bcdf6e0
```

The first baseline protocol step made partial progress before a Google 429:

```text
protocol.status = partial
tool-call-intent-segments = complete
tool-call-review = missing
tool-call-segment-review = missing
protocol_counts.total_calls = 77
protocol_counts.total_segments = 13
```

The protocol artifact directory contained exactly one segmentation artifact and
no review artifacts:

```text
tool_call_intent_segmentation artifacts: 1
tool_call_review artifacts: 0
tool_call_segment_review artifacts: 0
```

A second bounded `prototype1-step` attempted the remaining review work, hit a
fresh direct-Google 429, created no new artifacts, and exited nonzero with the
expected blocking diagnostic:

```text
batch selection is invalid: baseline_protocol blocked: campaign
p1-gemini35-flash-direct-15g2x3-clean-20260525-120225 made no protocol progress;
model google/gemini-3.5-flash; route DirectGoogle;
remaining=tool-call-review(...), tool-call-segment-review(...);
created={segmentations:0, call_reviews:0, segment_reviews:0}
```

That verifies the bounded `prototype1-step` no-progress path. It does not yet
prove that `prototype1-continue` stops after the same condition without
re-entering the phase, because the live campaign was intentionally paused
instead of spending more provider calls under quota exhaustion.

## Mitigation: Configurable Tool Review Fanout

The same 2026-05-25 run also showed that the remaining protocol work was all in
tool-call review and segment review:

```text
selected_runs=BurntSushi__ripgrep-2209(segmentation_needed=false, missing_calls=77, missing_segments=13)
remaining=tool-call-review(...), tool-call-segment-review(...)
```

Before the mitigation, the per-run tool-call review fanout was hard-coded:

```text
TOOL_REVIEW_CALL_LIMIT = 8
```

That limit is separate from campaign `protocol.max_concurrency`; this campaign
had only one run, so run-level concurrency could not reduce the number of
simultaneous per-call review adjudications.

Source now exposes the per-run fanout as a run-profile knob:

```toml
[protocol]
tool_review_parallelism = 1
```

The default remains `8`, preserving the old behavior for existing profiles. New
direct-Google loop profiles can lower it to `1` or `2` to trade runtime for a
smaller burst against the provider quota. This is a mitigation, not a complete
retry/backoff policy for HTTP 429.

Verified non-live with:

```text
cargo test -p ploke-eval protocol_report --lib
cargo check -p ploke-eval
```

Rerun on 2026-05-22:

```text
cargo test -p ploke-eval protocol_report --lib -- --nocapture
```

Still pending: rerun the live Google `prototype1-continue` path and confirm the
current partial `baseline_protocol` campaign stops with the bounded blocking
diagnostic instead of the 256-advance guard.

## Verification Targets

- Unit test: a mocked `advance_protocol_closure` report with nonempty failures
  and zero created artifacts makes `prototype1-continue` stop after one
  attempted phase advance.
- State test: a `baseline_protocol` closure state with partial protocol
  artifacts and unchanged missing procedures is reported as blocked/no-progress
  after a failed advance.
- Live/manual check: a Google 429 during remaining protocol review work surfaces
  as a bounded blocking diagnostic, not a 256-iteration guard failure.
