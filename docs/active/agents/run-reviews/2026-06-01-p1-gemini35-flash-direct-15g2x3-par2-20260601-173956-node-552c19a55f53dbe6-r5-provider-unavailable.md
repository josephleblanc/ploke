# Prototype 1 Broad-Harness Run Review: node-552c19a55f53dbe6-r5 provider-unavailable

Date: 2026-06-02
Campaign: `p1-gemini35-flash-direct-15g2x3-par2-20260601-173956`
Task: `BurntSushi__ripgrep-2209`
Parent node: `node-552c19a55f53dbe6`
Assigned slot: `node-552c19a55f53dbe6-r5`
Model route: `google/gemini-3.5-flash` through direct Google, from campaign closure config

## Verdict

The r5 broad-harness attempt is mechanically recorded, but benchmark-useless. It
ran the Prototype 1 broad headless-TUI adapter against the r5 candidate
workspace, recorded 55 tool requests and 55 tool completions, then aborted with a
typed `provider_unavailable` terminal caused by Google/Vertex HTTP 429
`RESOURCE_EXHAUSTED`. No proposal event was recorded, no submitted broad-harness
result file exists, and direct git inspection of the candidate workspace showed
no modified files.

This is not evidence of a bad patch or a successful no-op. It is a provider
capacity failure after a long discovery trace. The model had started to converge
from general Prototype 1 surfaces toward parser/discovery internals and was last
asking how `run_discovery_phase` builds crate context and whether edition could
be captured there, but the trace ended before any edit, proposal, or final
validation after an edit.

The attempt also reproduces an important tool-quality problem: several
`read_file` calls returned `ok:true`, `exists:true`, and the correct byte length
but empty `content` for line ranges that exist in the checkout. That should not
be counted as information success.

## Evidence Roots

- Campaign root:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-par2-20260601-173956`
- Request JSON:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-par2-20260601-173956/prototype1/messages/edit-harness-request/node-552c19a55f53dbe6-r5.json`
- Request prompt:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-par2-20260601-173956/prototype1/messages/edit-harness-request/node-552c19a55f53dbe6-r5.md`
- Headless TUI trace:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-par2-20260601-173956/prototype1/messages/edit-harness-result/node-552c19a55f53dbe6-r5.headless-tui.json`
- Expected submitted result path, verified absent:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-par2-20260601-173956/prototype1/messages/edit-harness-result/node-552c19a55f53dbe6-r5.json`
- Candidate workspace, verified clean:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-par2-20260601-173956/prototype1/workspaces/edit-harness/node-552c19a55f53dbe6-r5`
- Parent node record:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-par2-20260601-173956/prototype1/nodes/node-552c19a55f53dbe6/node.json`
- Closure state:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-par2-20260601-173956/closure-state.json`
- Existing baseline eval/protocol review:
  `/home/brasides/code/ploke/docs/active/agents/run-reviews/2026-06-01-p1-gemini35-flash-direct-15g2x3-par2-20260601-173956-baseline-eval-protocol.md`

Checked implementation path in the checkout:

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1419-1441`
  dispatches `run_broad_headless_tui_attempt` to
  `run_broad_headless_tui_attempt_with_options`.
- `cli_facing.rs:1528-1604` prepares the broad harness workspace, reads the
  published prompt, builds the TUI budget, and calls
  `tui_adapter::run_headless_with_model`.
- `cli_facing.rs:1725-1729` maps `HeadlessTerminal::ProviderUnavailable` to a
  `PrepareError::ProviderUnavailable` with phase `broad_headless_tui_attempt`.

## Closure State

The campaign closure state is mechanically complete for the baseline eval and
protocol run: registry complete, eval complete, protocol complete, and all three
required protocol procedures complete. That closure state is not r5-specific.

For the Prototype 1 child-plan surface, the parent node record reports
`node-552c19a55f53dbe6` with status `running`, generation `0`, instance
`BurntSushi__ripgrep-2209`, and runner args:

```text
loop prototype1-state --repo-root /home/brasides/.ploke-eval/worktrees/p1-gemini35-flash-direct-15g2x3-par2-20260601-173956
```

The r5 request is a published broad-harness request under that parent node, not a
separate eval run root. It points to a candidate workspace and an expected
submitted result output box.

## Eval And Patch Output

There is no r5 patch output.

The request declared this submitted result path:

```text
.../prototype1/messages/edit-harness-result/node-552c19a55f53dbe6-r5.json
```

Direct file existence verification found it absent. The only r5 result artifact
present is `node-552c19a55f53dbe6-r5.headless-tui.json`.

The headless trace records:

- top-level `attempts`: `[]`
- terminal: `provider_unavailable`
- events: 111 total
- event counts: 55 `tool_request`, 55 `tool_completed`, 1 `turn`
- tool counts: 27 `read_file`, 14 `list_dir`, 9 `request_code_context`,
  3 `cargo`, 2 `code_item_lookup`
- proposal events: none

Direct git verification in the r5 candidate workspace showed an empty
`git status --short`, empty `git diff --stat`, and empty `git diff --name-only`.
So this slot has no changed-path evidence, no candidate commit evidence, and no
benchmark submission evidence.

The three recorded validations all passed, but they were not final validation of
a proposed edit:

- `cargo check`, focused manifest `xtask/Cargo.toml`, 0 errors, 0 warnings
- `cargo test -- -- --list`, focused manifest `xtask/Cargo.toml`, 0 errors,
  0 warnings
- `cargo test -p ploke-tree`, workspace manifest `Cargo.toml`, 0 errors,
  0 warnings

Because no edit was staged or applied, these cargo results should be classified
as pre-edit environment/discovery evidence, not patch-quality evidence.

## Oracle And MBE State

No r5 oracle, MBE, Multi-SWE-bench submission, or benchmark patch projection was
found or expected from this provider-unavailable broad-harness attempt. The
baseline eval had benchmark artifacts, but r5 did not reach a submitted result.

## LLM And Tool Behavior

The active execution path was:

```text
Prototype 1 child-plan broad harness
-> published edit-harness request node-552c19a55f53dbe6-r5
-> run_broad_headless_tui_attempt
-> GitWorktreeBackend::prepare_broad_harness_workspace
-> tui_adapter::run_headless_with_model
-> write_broad_headless_tui_diagnostics
-> finish_broad_headless_tui_attempt
-> HeadlessTerminal::ProviderUnavailable / PrepareError::ProviderUnavailable
```

The prompt diagnostics show the workspace loaded at the r5 candidate path, BM25
ready with 6885 docs, context mode off, and the user prompt asking for a change
to improve `Prototype 1 descendant performance` outside protected core.

Concrete observed behavior:

1. The model began with broad repository discovery: `list_dir .`, then
   `request_code_context` for `descendant` and `descendants`, both of which
   returned zero snippets.
2. It inspected the campaign node records and the parent runner request, then ran
   `cargo check` before any edit.
3. It searched for `EVAL_CORE_SURFACE_ROOT`, listed `crates/ploke-eval`, and read
   the start of `crates/ploke-eval/src/cli/prototype1_state/backend.rs` to
   understand the protected core boundary.
4. It searched `benchmark`, listed and read `ploke-tree` surfaces, searched
   `successor`, read `successor.rs`, and ran `cargo test -- -- --list` plus
   `cargo test -p ploke-tree`.
5. It then read `crates/ploke-eval/src/cli/prototype1_state/mod.rs` repeatedly.
   Several successful reads returned empty content for valid ranges, causing
   overlapping re-reads.
6. It shifted toward ingestion/parser performance: listed
   `crates/ingest/syn_parser/src`, inspected `parser/visitor`, used
   `code_item_lookup` for `analyze_files_parallel` and `analyze_file_phase2`,
   searched for `CrateContext`, and read `discovery/single_crate.rs`.
7. The last retained assistant statement before terminal failure was: search for
   `run_discovery_phase` to see how crate context is built and whether edition
   can be captured during discovery. The following `request_code_context` for
   `run_discovery_phase` completed with snippets from
   `crates/ingest/syn_parser/src/discovery/mod.rs`.
8. The next event was a `turn` with outcome `aborted` and an HTTP 429
   `RESOURCE_EXHAUSTED` provider error from the Vertex/OpenAI-compatible chat
   completions endpoint.

This is a discovery-heavy trace with a plausible late localization direction,
but not enough to claim a useful edit. The trace never reached a staged proposal
or a final model answer.

## Positive Examples And Adjudication Candidates

There is no positive patch chain for r5 because there was no edit. The useful
signals are diagnostic rather than benchmark-success signals:

- The typed terminal classification `provider_unavailable` is valuable. It keeps
  provider quota failure separate from no-edit, timeout, tool failure, and
  rejected-surface outcomes.
- The model responded to no-hit searches by switching to direct directory/file
  inspection instead of stopping immediately.
- The model explicitly inspected protected-core context before attempting a
  broad-surface change. That is useful behavior for child-plan safety, even
  though no edit followed.
- Validation commands were visible and successful, but should be adjudicated as
  pre-edit environment checks only.
- Empty successful reads should become a negative information-success signal.

Candidate adjudication fields:

- provider-terminal category: provider unavailable vs timeout vs no edit vs
  applied edit
- validation timing: pre-edit, post-edit, or no-edit validation
- successful read with empty content inside an existing line range
- discovery-to-edit conversion: whether late useful context was followed by an
  edit before terminal failure
- full model transcript availability for headless broad-harness review

## Trace Reconstruction

Concrete trace chain:

```text
request_code_context("descendant")
-> no snippets; model switches to manual node/workspace/protected-core inspection
-> reads parent node and runner request, runs cargo check, inspects protected-core constant
-> searches successor and tree surfaces, runs ploke-tree tests
-> repeated mod.rs reads include empty successful read results for existing ranges
-> model changes direction to parser/discovery internals and asks about run_discovery_phase / CrateContext
-> provider returns HTTP_429 RESOURCE_EXHAUSTED before any proposal or submitted result
```

The last point where the model had a plausible direction was after it looked up
`run_discovery_phase`. The retained assistant statement says it was checking how
crate context is built and whether edition could be captured during discovery.
However, the trace does not contain the next reasoning step, an edit request, or
a proposed patch. Therefore this review should not infer that the model would
have edited discovery context or that such an edit was correct.

Important verification against the checkout:

- Trace calls `function-call-ff6858b0-cdff-42a2-9097-818df29fc917`,
  `function-call-0cc4f2ea-5a7f-46c8-8bbc-25b5af3aeb97`, and
  `function-call-1690ae5b-8f72-4587-805f-b90a1a0564f7` reported `ok:true`,
  `exists:true`, byte length `41794`, and empty `content` for ranges inside
  `crates/ploke-eval/src/cli/prototype1_state/mod.rs`.
- Direct read verification of the same checkout showed
  `prototype1_state/mod.rs` has 804 lines and non-empty content at lines
  701-730, including the "Intended, not implemented" and "Current persistence
  gaps" documentation block.
- This confirms the suspicious tool result was defective or under-informative;
  it was not an absent file or out-of-range request.

## Protocol Review And Protocol Blind Spots

There are no r5 protocol artifacts to review. Protocol completion in the closure
state refers to the baseline eval run, not this broad-harness r5 attempt.

For r5, the review surface is the broad-harness request, headless TUI trace,
workspace git state, and expected submitted-result path. The current playback
surface is enough to classify the terminal outcome, but weak for semantic review:

- `record present, manual join needed`: request JSON/prompt, headless trace,
  parent node record, closure state, and candidate workspace git state all have
  to be joined manually by path and slot id.
- `record absent`: submitted broad-harness result JSON for r5.
- `record absent`: proposal/admitted-change artifact for r5.
- `record present, playback gap`: headless trace includes tool-event previews,
  validation summaries, and a debug relay, but not a complete durable model
  transcript suitable for reconstructing every reasoning step. The debug relay
  reports `dropped: 44` and `truncated: 38`.
- `not applicable`: baseline `record.json.gz`, benchmark patch projection,
  Multi-SWE-bench submission, and protocol segment-review artifacts are not
  r5-specific because this attempt never submitted a patch.

The main protocol/adjudication blind spot is that a broad-harness attempt can run
substantial discovery and validations but still have no benchmark-useful output.
Fan-in should classify this slot by terminal outcome and artifact absence, not by
tool-call volume or passing pre-edit cargo commands.

## What Is Working

- The broad-harness request and prompt were published with a clear output box,
  candidate workspace, edit policy, validation contract, and evidence roots.
- The headless TUI diagnostics were persisted despite provider failure.
- The terminal outcome is typed as `provider_unavailable` and carries the
  provider error details: HTTP 429 and `RESOURCE_EXHAUSTED`.
- Tool lifecycle counts are internally balanced: 55 requests and 55 completions.
- The workspace remained clean, so there is no hidden partial patch to adjudicate.
- The adapter path refuses to publish a submitted broad-harness result when the
  terminal is provider-unavailable.

## What Is Not Working Yet

- The r5 slot produced no patch, no submitted result, and no changed-path
  evidence. It is benchmark-useless.
- Provider capacity failure ended the run after 111 events and a large retained
  context, so this attempt spent reviewer/model budget without yielding a
  candidate.
- `read_file` returned empty content for valid line ranges while reporting
  success and correct file size. That can cause search/read thrash and can be
  over-credited if only `ok:true` is considered.
- The trace has enough evidence to classify the failure but not enough full
  transcript detail to recover every model interpretation. Debug relay retention
  was partial.
- Passing cargo commands were not tied to an edit and should not be interpreted
  as validation of an improvement.

## Action Items

1. Fan-in classification: mark r5 as `provider_unavailable / no submitted
   result / no workspace diff`, not as a failed patch and not as a useful
   descendant candidate.
2. Operational retry policy: if another r5-equivalent candidate is needed, retry
   only after provider capacity recovers or with a different route/model; do not
   reuse this slot as benchmark evidence.
3. Non-blocker: add or strengthen adjudication for `ok:true` reads with empty
   content when the requested range exists. These should count as
   low-information or defective tool results.
4. Non-blocker: persist a complete headless-TUI model transcript or raw provider
   exchange for broad-harness attempts. The current debug relay is useful but
   partial (`dropped`/`truncated`), making semantic reconstruction manual and
   incomplete.
5. Non-blocker: label cargo validations by timing relative to edits. The r5
   validations were pre-edit checks and should not increase patch-confidence.
