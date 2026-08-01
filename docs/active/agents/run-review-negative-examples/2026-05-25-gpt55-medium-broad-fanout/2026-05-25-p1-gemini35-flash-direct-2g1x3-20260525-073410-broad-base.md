# p1-gemini35-flash-direct-2g1x3 Broad Harness Base Attempt Review

Date: 2026-05-25
Campaign: `p1-gemini35-flash-direct-2g1x3-20260525-073410`
Attempt: `broad-harness-request:node-b2a63be3e50c7ee8`
Instance: `BurntSushi__ripgrep-2209`

## Verdict

Mechanically, the base broad-harness attempt produced a headless TUI trace and applied two edit proposals. Benchmark-wise, it is unusable: both edits appended duplicate, deliberately failing diagnostic tests to `crates/ploke-tree/src/tests.rs`, no submitted-result JSON was written at the request's `submitted_result_path`, and the terminal state is `tool_failed` after the headless event stream closed.

This is not a missing-record campaign. The campaign has closure, a full eval run root, `record.json.gz`, agent-turn records, patch projection, an MBE submission artifact, and one protocol segmentation artifact. For this specific broad-harness attempt, however, the requested submitted-result record is absent and the durable per-attempt evidence is the `.headless-tui.json` trace plus the dirty candidate checkout.

## Evidence Roots

- Campaign root: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-2g1x3-20260525-073410`
- Request: `prototype1/messages/edit-harness-request/node-b2a63be3e50c7ee8.json`
- Headless result / TUI trace: `prototype1/messages/edit-harness-result/node-b2a63be3e50c7ee8.headless-tui.json`
- Submitted result expected by request: `prototype1/messages/edit-harness-result/node-b2a63be3e50c7ee8.json`
- Candidate checkout: `prototype1/workspaces/edit-harness/node-b2a63be3e50c7ee8`
- Campaign run root checked for inventory context: `/home/brasides/.ploke-eval/instances/prototype1/p1-gemini35-flash-direct-2g1x3-20260525-073410/BurntSushi__ripgrep-2209/runs/run-1779695507168-structured-current-policy-58118bd8`

## Execution Path

This artifact came from the published broad headless-TUI request path:

```text
prototype1-harness attempt -> tui_adapter::run_headless_with_model -> headless ploke-tui event stream -> edit-harness-result/*.headless-tui.json
```

It should not be read as the same surface as the later campaign eval run root. The run root contains `record.json.gz`, `agent-turn-trace.json`, `llm-full-responses.jsonl`, `benchmark-patch-projection.json`, and `multi-swe-bench-submission.jsonl`, but those are campaign/eval evidence and not the per-attempt submitted-result file for this broad-harness attempt.

## Request Slot, Workspace, Branch, Commit

- Request id: `broad-harness-request:node-b2a63be3e50c7ee8`
- Admission runtime id: `7f522409-04d2-4095-95c2-1f8deed936f9`
- Target artifact: `artifact:git-commit:bf8269e1511dfcb27c7b91dfca56ea290464413a`
- Policy: `workspace except ploke-eval`
- Workspace loaded by TUI: `.../prototype1/workspaces/edit-harness/node-b2a63be3e50c7ee8`
- Focused root: `.../node-b2a63be3e50c7ee8/crates/ploke-tree`
- Git branch checked in the workspace: `prototype1-broad-broad-harness-request-node-b2a63be3e50c7ee8`
- Git HEAD: `bf8269e1511dfcb27c7b91dfca56ea290464413a`
- Checkout status: dirty, with only `crates/ploke-tree/src/tests.rs` modified.

## Closure And Campaign State

The campaign manifest records model `google/gemini-3.5-flash`, route source `direct_google`, benchmark family `multi_swe_bench_rust`, and required protocol procedures `tool-call-intent-segments`, `tool-call-review`, and `tool-call-segment-review`.

`closure-state.json` reports the instance eval as complete and protocol as partial. It points at run `run-1779695507168-structured-current-policy-58118bd8`, with `protocol_counts.total_calls = 92`, `reviewed_calls = 0`, `total_segments = 12`, and `missing_segments = 12`.

## Record Inventory Labels

- `operator/convenience record`: `node-b2a63be3e50c7ee8.headless-tui.json` is compact broad-harness diagnostic evidence and the per-attempt TUI trace.
- `record absent`: `node-b2a63be3e50c7ee8.json`, the request's submitted-result path, does not exist.
- `record present, manual join needed`: the campaign run root has `record.json.gz`, `agent-turn-trace.json`, `agent-turn-summary.json`, `llm-full-responses.jsonl`, `validation-audit.json`, `benchmark-patch-projection.json`, and `multi-swe-bench-submission.jsonl`, but these were manually joined by campaign/instance/run path rather than exposed as a per-attempt broad-harness playback view.
- `record present, playback gap`: the protocol root has one `tool_call_intent_segmentation` artifact, but no reviewed call artifacts for this campaign state.

## LLM And Tool Behavior

The headless trace contains 152 events: 74 `tool_request`, 74 `tool_completed`, 2 `tool_failed`, and 2 `proposal` events.

The model first explored the local workspace and `ploke-tree`, then used `request_code_context` searches such as `descendant`, `performance`, `RunExecutionGraph`, `closure`, and `prototype1_state`. It repeatedly read graph and artifact-tree code, then drifted into using failing tests as a code-search mechanism instead of making a benchmark-improving source change.

The two explicit tool failures were absolute-path `list_dir` requests outside configured roots:

- `/home/brasides/.ploke-eval/campaigns`
- `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-2g1x3-20260525-073410`

Those failures were recoverable in the model trace. They were not the main reason the attempt is benchmark-useless.

## Edit And Validation Trace

Two edit proposals were applied:

- `25e3a388-0204-5878-a8dc-f19c267fb19a`
- `e1e38d4d-614b-529b-93eb-267040a7e2f1`

Both touched `crates/ploke-tree/src/tests.rs`. The checkout diff appends two `#[test] fn temp_search_test()` definitions. Both tests are diagnostic probes that read `../ploke-eval/src/cli/prototype1_state/*.rs` and call `panic!("forced failure to show output")`. They are duplicate function names in the same module, and they are intentionally failing tests.

Validation evidence:

- `cargo check`: passed.
- `cargo test -p ploke-tree`: passed before the diagnostic tests were inserted.
- `cargo test -p ploke-eval`: failed with exit code 101 and 486 warnings.
- `cargo test -p ploke-tree -- temp_search_test`: failed by design, printing matches and panicking.

The final checkout state does not contain a candidate performance fix. It contains diagnostic test debris.

## Positive Examples And Adjudication Candidates

There is one useful adjudication signal: the model used a failed diagnostic test to extract concrete references in `prototype1_state/history.rs` and `backend.rs`. The trace shows tool output causing follow-up reads around `RealizeRequest` and `WorkspaceBackend`.

That signal should not score as benchmark progress. It should be classified as `useful context discovery without cleanup or patch`.

## Protocol Review And Blind Spots

The campaign protocol state is partial: segmentation completed, but call review and segment review are missing in closure. The run trace audit over the campaign run root reported 93 provider responses, 92 provider-emitted tool calls, 92 recorded tool calls, and 0 missing recorded provider call ids. That audit belongs to the campaign run root, not directly to this broad-harness attempt.

The blind spot is that the per-attempt headless result can say proposals were applied while the request's submitted-result artifact remains absent. A playback/reporting surface should make that distinction visible without requiring a manual path join.

## What Is Working

- The request captured the admission binding, policy, target artifact, prompt path, request hash, and submitted-result path.
- The headless TUI trace preserved tool requests, failures, applied proposals, validations, prompt diagnostics, and terminal state.
- The candidate checkout remained inspectable; branch, commit, and diff were recoverable.

## What Is Not Working Yet

- The attempt did not write the expected submitted-result JSON.
- The terminal state is `tool_failed` because the scan barrier saw the channel close after activity.
- The applied edits are not benchmark-improving and leave the checkout with intentionally failing duplicate tests.
- The model spent most of the attempt on repository archaeology and ad hoc search tests, not a scoped performance change.

## Action Items

1. Treat this attempt as a failed broad-harness candidate, not as a submitted patch.
2. Add a live bug or blocker-repair item for `tool_failed` plus absent submitted-result when proposals applied.
3. Add an adjudication field for `diagnostic edits left in checkout` and score it as negative unless reverted before submission.
4. Improve the broad-harness prompt or tool contract so prior-attempt/campaign evidence paths are usable from the configured workspace root, or so inaccessible absolute paths are clearly not requested from the model.
