# Prototype 1 Baseline Protocol Review: Fixed 15g2x3, ripgrep-2209

Date: 2026-05-25

Campaign: `p1-gemini35-flash-direct-15g2x3-fixed-20260525-123824`

Parent node: `node-57e8487f70ce4abc`

Run: `run-1779712736739-structured-current-policy-a34790bb`

Task: `BurntSushi__ripgrep-2209`

Review status: complete. This review is intentionally protocol-focused. It
uses the prior baseline eval review only for context and checks the completed
protocol artifacts, closure state, doctor state, and strict JSON parse behavior
directly.

## Short Verdict

Protocol completed all required stages for the baseline run. The protocol
directory contains one intent segmentation artifact, 71 tool-call review
artifacts, and 7 segment-review artifacts. `closure-state.json` agrees:
protocol status is `complete`, all three required procedures are `complete`,
and protocol counts are 71 reviewed calls, 7 usable segments, 0 mismatched
segments, and 0 missing segments.

The malformed JSON issue did not block this run. I found 4 persisted
`raw_content` adjudicator outputs that fail strict `serde_json` parsing, all in
otherwise complete artifacts. All 4 have the same shape: a redundant final
punctuation fragment, specifically a separate `"."` before the closing object.
The artifacts do not show multiple malformed patterns and do not show arbitrary
trailing natural-language prose after a complete JSON object.

The non-blocking issue to file later is observability, not immediate protocol
correctness. The persisted artifact keeps malformed `raw_content` and a
normalized usable output, but it does not record whether this was a retry,
local repair, retry attempt count, or repair kind. That makes the live terminal
"malformed JSON retry" messages hard to audit after the fact.

## Evidence Roots

- Baseline run root:
  `/home/brasides/.ploke-eval/instances/prototype1/p1-gemini35-flash-direct-15g2x3-fixed-20260525-123824/BurntSushi__ripgrep-2209/runs/run-1779712736739-structured-current-policy-a34790bb`
- Protocol artifact root:
  `/home/brasides/.ploke-eval/protocol/prototype1/p1-gemini35-flash-direct-15g2x3-fixed-20260525-123824/BurntSushi__ripgrep-2209/runs/run-1779712736739-structured-current-policy-a34790bb`
- Campaign state:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-fixed-20260525-123824`
- Worktree:
  `/home/brasides/.ploke-eval/worktrees/p1-gemini35-flash-direct-15g2x3-fixed-20260525-123824`

The bundled trace audit was run read-only:

```text
python3 docs/workflow/skills/ploke-run-review/scripts/run_trace_audit.py /home/brasides/.ploke-eval/instances/prototype1/p1-gemini35-flash-direct-15g2x3-fixed-20260525-123824/BurntSushi__ripgrep-2209/runs/run-1779712736739-structured-current-policy-a34790bb --markdown
```

Audit summary: 72 provider responses, 71 provider-emitted tool calls, 71
recorded tool calls, 0 missing provider call ids, 0 extra recorded call ids, 71
`tool_calls` finishes, and one final `stop`.

## Procedure Completion

The protocol artifact directory contains exactly 79 JSON files:

| Procedure | Artifact count | Evidence |
| --- | ---: | --- |
| `tool_call_intent_segmentation` | 1 | `1779713106220_tool_call_intent_segmentation_BurntSushi__ripgrep-2209.json` |
| `tool_call_review` | 71 | one artifact for each of the 71 recorded calls |
| `tool_call_segment_review` | 7 | one artifact for each of the 7 labeled segments |

The segmentation output reports full coverage:

- `total_calls`: 71
- `labeled_segments`: 7
- `ambiguous_segments`: 0
- `labeled_calls`: 71
- `ambiguous_calls`: 0
- `uncovered_calls`: 0

The seven segment-review artifacts cover:

| Segment | Calls | Label | Segment review |
| --- | ---: | --- | --- |
| `segment:0` | 0-4 | `LocateTarget` | `focused_progress`, high |
| `segment:1` | 5-37 | `InspectCandidate` | `mixed`, high |
| `segment:2` | 38-43 | `EditAttempt` | `focused_progress`, high |
| `segment:3` | 44-45 | `ValidateHypothesis` | `focused_progress`, high |
| `segment:4` | 46-59 | `InspectCandidate` | `focused_progress`, high |
| `segment:5` | 60-60 | `EditAttempt` | `focused_progress`, medium |
| `segment:6` | 61-70 | `ValidateHypothesis` | `mixed`, high |

## Closure And Doctor State

`closure-state.json` reflects protocol completion correctly:

- registry: expected 1, mapped 1, status `complete`
- eval: expected 1, complete 1, status `complete`
- protocol: expected 1, full 1, failed 0, missing 0, status `complete`
- required procedures: `tool-call-intent-segments`, `tool-call-review`, and
  `tool-call-segment-review`
- status by procedure: all three complete with 0 failed, 0 missing, and 0
  partial
- instance protocol counts: 71 total calls, 71 reviewed calls, 7 total
  segments, 7 usable segments, 0 mismatched segments, 0 missing segments

`prototype1-doctor --format json` on the named worktree now reports
`phase: "child_plan"` and `blockers: []`. It also reports the same parent
identity, campaign id, and admitted run profile commitment.

There is still a stale projection caveat outside protocol closure: the
campaign's `prototype1/scheduler.json` still lists the root parent in
`frontier_node_ids` with node status `planned`, while
`prototype1/nodes/node-57e8487f70ce4abc/node.json` says `status: "running"`.
That does not contradict protocol completion because closure and doctor have
advanced, but it is another operator-status projection mismatch to track.

## Malformed JSON

I checked every persisted protocol `raw_content` string under the protocol
artifact root with strict `json.loads`. Results:

- protocol files checked: 79
- persisted LLM `raw_content` strings: 235
- strict JSON failures: 4
- strict JSON failures with final usable normalized output: 4

The 4 malformed raw strings were:

| Artifact | Target | Branch | Pattern |
| --- | --- | --- | --- |
| `1779713261056_tool_call_review_BurntSushi__ripgrep-2209.json` | `call:25` | usefulness | redundant final `"."` fragment |
| `1779713263614_tool_call_review_BurntSushi__ripgrep-2209.json` | `call:54` | redundancy | redundant final `"."` fragment |
| `1779713322897_tool_call_segment_review_BurntSushi__ripgrep-2209.json` | `segment:1` | usefulness | redundant final `"."` fragment |
| `1779713351234_tool_call_segment_review_BurntSushi__ripgrep-2209.json` | `segment:3` | redundancy | redundant final `"."` fragment |

Each malformed raw string ends like this pattern:

```text
... rationale text."
"."
}
```

That is a single pattern: redundant final punctuation fragment before the final
object close. It is not multiple malformed patterns, and the persisted examples
do not show extra trailing natural-language commentary after JSON.

Current code explains why the protocol completed. `ploke-protocol` first tries
strict JSON parsing, then applies local repair paths, including
`repair_redundant_final_punctuation_fragment`, before returning a parse error.
The final artifacts retain the original malformed `raw_content` but expose
valid normalized branch outputs.

What can be said from durable artifacts:

- 4 persisted adjudicator raw outputs required repair to be strict JSON.
- None caused a failed, missing, or partial protocol procedure.
- None exceeded a durable budget in the sense of leaving an exhausted or failed
  protocol record.

What cannot be said from durable artifacts:

- The exact live terminal retry count.
- Whether each terminal "retry" was a provider retry, parser repair, or
  controller-level retry.
- Any per-branch retry budget consumption.

The artifact gap is therefore `record present, manual join needed`: the raw
malformed output and final normalized judgment are present, but retry/repair
provenance is not first-class.

## Useful Adjudication Examples

The protocol artifacts do contain positive examples and candidate fields for
future LLM adjudication:

- Call `39`, `apply_code_edit`: `overall = recoverable_detour`,
  `usefulness = no_value`, `redundancy = search_thrash`,
  `recoverability = clear_next_step`. This is a good negative example for
  failed edit attempts that only provide tool-state information.
- Call `41`, `apply_code_edit`: `overall = mixed`, `usefulness = key_progress`,
  `redundancy = redundant_repeat`, `recoverability = clear_next_step`. This is
  a useful example of a failed same-file edit that becomes useful only because
  the following `non_semantic_patch` recovers.
- Call `43`, `non_semantic_patch`: `overall = focused_progress`,
  `usefulness = key_progress`, `recoverability = no_recovery_needed`. This is
  the positive recovery example after the stale edit failures.
- Call `67`, `cargo test --all-features -p grep-printer`: `overall = mixed`,
  `usefulness = key_progress`, `recoverability = clear_next_step`. The protocol
  correctly treats a failing validation command as useful signal rather than
  transport failure.
- Segment `1`: `overall = mixed`, `redundancy = redundant_repeat`,
  `recoverability = partial_next_step`. This is a compact segment-level
  example of useful investigation polluted by repeated identical
  `replace_with_captures_at` searches and overlapping reads.
- Segment `6`: `overall = mixed`, `recoverability = clear_next_step`. This is
  a useful final-validation example because it includes a failed all-features
  validation amid later package checks.

Useful fields already present:

- `packet.target_id`
- `packet.scope_summary`
- `signals.*`
- `usefulness.verdict`, `confidence`, `rationale`
- `redundancy.verdict`, `confidence`, `rationale`
- `recoverability.verdict`, `confidence`, `rationale`
- `overall`, `overall_confidence`, `synthesis_rationale`
- segmentation `label`, `status`, `start_index`, `end_index`, and coverage
  counts

Candidate fields to add later:

- `raw_content_strict_json_valid`
- `json_repair_applied`
- `json_repair_kind`
- `adjudicator_attempt_count`
- `adjudicator_retry_budget`
- `adjudicator_retry_exhausted`

## Non-Blocking Issues

1. Persist protocol JSON repair/retry provenance. The current artifacts are
   usable, but they require manual strict-parse auditing to know that 4 raw
   adjudicator outputs were malformed and locally repaired.

2. Keep closure/doctor status and Prototype 1 node/scheduler projections in
   sync. Closure and doctor say protocol is complete and the next phase is
   `child_plan`, while scheduler/node projections still look pre-completion.

3. Keep final-validation semantics visible in protocol summaries. The call and
   segment artifacts preserve the failed all-features validation as useful
   signal, but downstream summaries should not collapse this to simple success.
