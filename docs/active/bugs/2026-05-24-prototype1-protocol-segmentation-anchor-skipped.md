# Prototype 1 Protocol Segmentation Anchor Skipped

Status: abandoned-run blocker; do not continue this campaign for loop progress.

## Summary

The Gemini 3.5 Flash Prototype 1 campaign
`p1-gemini35-flash-multigen-2g3x3-20260523-223658` has one persisted
`tool_call_intent_segmentation` artifact, but `protocol status` cannot build an
aggregate from it and plans another intent-segmentation call instead of
advancing to follow-up review.

This is a loop blocker because repeating intent segmentation spent another live
provider call, created no new protocol artifact, and received another malformed
segmentation response. The malformed response is a symptom of the bad retry; the
primary contract failure is that a stored segmentation anchor is either not
aggregate-usable or not reported as an actionable diagnostic.

Operator decision recorded 2026-05-24: do not repair this run into success by
changing the reader/aggregate path after invalid protocol state exists. This is
related to the state-transition and History model in
`crates/ploke-eval/src/cli/prototype1_state/history.rs`: invalid required
transition evidence must block admission. The correct loop action is to abandon
this worktree/campaign as a progress source and start a fresh run where
segmentation succeeds on the first attempt.

## Evidence

Campaign:

```text
p1-gemini35-flash-multigen-2g3x3-20260523-223658
```

Run record:

```text
/home/brasides/.ploke-eval/instances/prototype1/p1-gemini35-flash-multigen-2g3x3-20260523-223658/BurntSushi__ripgrep-2209/runs/run-1779602498508-structured-current-policy-ed4a409a/record.json.gz
```

Existing protocol artifact:

```text
/home/brasides/.ploke-eval/protocol/prototype1/p1-gemini35-flash-multigen-2g3x3-20260523-223658/BurntSushi__ripgrep-2209/runs/run-1779602498508-structured-current-policy-ed4a409a/1779620303372_tool_call_intent_segmentation_BurntSushi__ripgrep-2209.json
```

Operator report:

```text
.orchestrator/reports/loop-operator/2026-05-24T11-03-30Z-protocol-step-2.md
```

Current status command:

```text
./target/debug/ploke-eval protocol status --record /home/brasides/.ploke-eval/instances/prototype1/p1-gemini35-flash-multigen-2g3x3-20260523-223658/BurntSushi__ripgrep-2209/runs/run-1779602498508-structured-current-policy-ed4a409a/record.json.gz --format json
```

Observed status:

```json
{
  "tool_calls_total": 73,
  "protocol_eligible": true,
  "artifact_count": 1,
  "segmentation_present": true,
  "call_review_count": 0,
  "segment_review_count": 0,
  "aggregate_available": false,
  "next_step": {
    "kind": "intent_segmentation"
  }
}
```

The failed retry left closure unchanged:

```text
registry = complete
eval = complete
protocol = partial
tool-call-intent-segments = complete
tool-call-review = missing
tool-call-segment-review = missing
```

The retry response in
`/home/brasides/.ploke-eval/logs/ploke_eval_20260524_040239_963758.log`
contained structured segmentation content followed by a duplicated trailing
fragment before the final object close. The parser failed with:

```text
expected ',' or '}' at line 45 column 1
```

Do not treat that provider output as the root issue. If the stored segmentation
artifact had been accepted by the aggregate path, the next step should have been
`tool-call-review` or `tool-call-segment-review`, not another segmentation
request.

## Broken Contract

`protocol status` and `protocol_run_plan` must not silently disagree in a way
that causes repeat live segmentation calls:

- if a stored segmentation artifact is valid, the aggregate loader should use it
  as the anchor and plan follow-up review;
- if the stored artifact is invalid, identity-mismatched, or shape-skipped, the
  status/plan output should report that diagnostic and stop the current run
  instead of presenting it as ready to re-segment.

The current split is misleading:

- `protocol_state_for_run` counts raw stored artifacts and reports
  `segmentation_present = true`;
- `protocol_run_plan` treats `ProtocolAggregateError::MissingAnchor` as
  `segmentation_needed = true`;
- the operator sees no explicit reason why the existing segmentation was not
  aggregate-usable.

## Likely Code Surface

The relevant paths are:

- `crates/ploke-eval/src/protocol_artifacts.rs`
  - `list_protocol_artifact_load_results`
  - `load_protocol_artifact_tolerant`
  - `validate_decoded_protocol_artifact_identity`
- `crates/ploke-eval/src/protocol_aggregate.rs`
  - `load_protocol_aggregate_from_artifacts`
  - `classify_protocol_artifact_load_results`
  - `tool_call_artifact_from_decoded`
  - `build_protocol_aggregate`
- `crates/ploke-eval/src/cli.rs`
  - `protocol_run_plan`
  - `protocol_state_for_run`

## Future Guardrail Direction

If this becomes source work, add a local regression that demonstrates the
planner does not re-run segmentation when an existing stored segmentation is
present but cannot be used as the aggregate anchor.

Good future guardrail tests would include one of:

- a stored segmentation artifact that decodes and validates, proving
  `load_protocol_aggregate_from_artifacts` can use the current artifact shape as
  the anchor;
- a stored segmentation artifact that is unloaded, identity-mismatched, or
  skipped, proving `protocol_run_plan` surfaces a diagnostic stop condition
  instead of returning `segmentation_needed = true`.

Any such guardrail should avoid another live provider call as part of the
reproduction, and should not make this old run admissible after the fact.

## Workflow Direction

For this concrete run:

- keep the old artifacts as evidence;
- do not run another `prototype1-step` against this campaign;
- do not patch aggregate loading to reinterpret this run as success;
- start a fresh worktree/campaign using the same intended model/profile policy
  after re-reading the current files.

Do not paper over this by only broadening malformed JSON recovery. JSON recovery
can be a secondary resilience improvement for future runs, but it does not make
this persisted transition admissible.
