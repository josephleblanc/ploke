# Parent Baseline Run Review: p1-gemini35-flash-direct-2g1x3-20260525-073410

Date: 2026-05-25

Scope: parent baseline eval/protocol record only. This review covers the
single eval run and protocol artifact for `BurntSushi__ripgrep-2209`; it does
not review child attempts, selection, or later loop progress.

## Verdict

The parent eval record is mechanically complete and produced a non-empty,
issue-relevant ripgrep patch. The tool lifecycle is well recorded: the provider
emitted 92 tool calls and the run recorded the same 92 call ids with no missing
provider calls. The patch changes `crates/printer/src/util.rs` to limit
replacement output to the real match range in multiline mode and adds
regression tests in `crates/printer/src/standard.rs`.

The protocol record is only partial. The protocol root contains the
`tool_call_intent_segmentation` artifact, but the required
`tool-call-review` and `tool-call-segment-review` artifacts are absent for this
run. Closure therefore reports eval `complete` and protocol `partial`, even
though the run registration says a protocol aggregate is available.

This is not a case where the entire run record is missing. The run has
`record.json.gz`, `agent-turn-trace.json`, `agent-turn-summary.json`,
`llm-full-responses.jsonl`, `validation-audit.json`,
`multi-swe-bench-submission.jsonl`, `benchmark-patch-projection.json`, and one
protocol artifact. The current gap is narrower: procedure-specific protocol
review records are absent, and several present records still require manual
joins or are not first-class playback iterators.

## Evidence Roots

- Campaign root:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-2g1x3-20260525-073410`
- Run root:
  `/home/brasides/.ploke-eval/instances/prototype1/p1-gemini35-flash-direct-2g1x3-20260525-073410/BurntSushi__ripgrep-2209/runs/run-1779695507168-structured-current-policy-58118bd8`
- Protocol root:
  `/home/brasides/.ploke-eval/protocol/prototype1/p1-gemini35-flash-direct-2g1x3-20260525-073410/BurntSushi__ripgrep-2209/runs/run-1779695507168-structured-current-policy-58118bd8`
- Registration:
  `/home/brasides/.ploke-eval/registries/runs/run-1779695507168-structured-current-policy-58118bd8.json`
- Runtime-playback inventory checked:
  `docs/workflow/evalnomicon/drafts/observability/runtime-playback/inventory/README.md`,
  `record-surface-map.md`, and `latest-run-emission-worksheet.md`

## Execution Path

`prototype1 parent eval setup -> run single agent -> agent-single-turn ->
runner.rs benchmark turn -> record.json.gz / agent-turn-* / llm-full-responses
-> benchmark submission and patch projection -> protocol intent segmentation`

The registration records this as `run_arm.id = structured-current-policy`,
`run_role = treatment`, model `google/gemini-3.5-flash`, provider `google`, and
budget `max_turns = 40`, `max_tool_calls = 200`, `wall_clock_secs = 1800`.
For Prototype 1 purposes this is the parent baseline eval/protocol record that
seeds later search; the artifact labels are the runner's arm labels.

## Closure State

`closure-state.json` at `2026-05-25T08:02:27.968973649+00:00` reports:

- Registry: `complete`, expected 1, mapped 1.
- Eval: `complete`, expected 1, complete 1.
- Protocol: `partial`, expected 1, full 0, partial 1.
- Required procedures:
  `tool-call-intent-segments`, `tool-call-review`,
  `tool-call-segment-review`.
- Procedure status:
  `tool-call-intent-segments = complete`,
  `tool-call-review = missing`,
  `tool-call-segment-review = missing`.
- Protocol counts:
  `total_calls = 92`, `reviewed_calls = 0`,
  `total_segments = 12`, `usable_segments = 0`,
  `missing_segments = 12`.

The registration lifecycle separately says protocol `completed` with detail
`protocol aggregate available`. In this review, closure is the better status
source for required-procedure completeness: the protocol aggregate exists, but
two required procedure families are absent.

## Eval And Patch Output

The eval run completed and exported a non-empty Multi-SWE-Bench patch. The
patch projection reports:

- Benchmark instance: `BurntSushi__ripgrep-2209`
- Base SHA: `4dc6c73c5a9203c5a8a89ce2161feca542329812`
- Checkout head at export: same SHA
- Submission line count: 173
- Submission byte length: 5580
- Projection check: `passed`
- Detail: `fix_patch exported from the recorded checkout cwd`

The checkout diff at the recorded repo root contains two changed files:

- `crates/printer/src/util.rs`
- `crates/printer/src/standard.rs`

The exported patch:

- computes a replacement `limit` as `range.end` for multiline mode and
  `subject.len()` for single-line mode;
- replaces direct `matcher.replace_with_captures_at(...)` use with an internal
  `replace_with_captures_at_limit(...)` helper;
- stops capture iteration when `m.start() >= limit`;
- copies the non-matching suffix only up to `limit`;
- adds two regression tests for multiline replacement output.

This is an issue-relevant patch, not just test churn. The validation audit
classifies two changed paths, with production edits in `util.rs` and test edits
in `standard.rs`, and no patch-quality red flags.

## Oracle And MBE State

The run profile has `[execution.mbe].enabled = false` and `[selection.oracle]`
set to `mode = "record-only"` with `require_evidence = true`. This parent
review therefore does not claim a Multi-SWE-Bench oracle pass. The benchmark
evidence here is exported patch shape plus agent-visible cargo validation, not
an MBE execution result.

## LLM And Tool Behavior

Trace audit result:

- Raw provider responses: 93
- Provider-emitted tool calls: 92
- Recorded tool calls: 92
- Missing recorded provider call ids: 0
- Extra recorded call ids: 0
- Finish reasons: 92 `tool_calls`, 1 `stop`
- Final response usage: 156,694 prompt tokens, 489 completion tokens,
  157,900 total tokens

Recorded call classifications:

- `context_results`: 28
- `completed`: 15
- `read_with_content`: 23
- `transport_failure`: 5
- `empty_completed_read`: 13
- `duplicate_request`: 8

The model found the relevant code path, but it spent many calls on repeated or
low-yield context retrieval. It repeatedly searched around
`replace_with_captures_at`, `replace_all`, `Replacer`, and `StandardSink`, and
some successful `read_file` calls returned empty content because the requested
line ranges were beyond the file range. Those are information-quality issues,
not lifecycle holes.

The model recovered from tool failures and a compile failure. `code_item_lookup`
failed on ambiguous/internal cases, two semantic edit attempts failed with
transport errors, and a later test insertion caused compilation failure. The
run then used direct patching and focused reads to recover, followed by passing
cargo calls.

## Validation Evidence

The validation audit records seven cargo calls. The final cargo call was:

- command: `check`
- manifest: `/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep/Cargo.toml`
- status: success
- resolved scope: workspace
- covers changed files: true

Other useful validation evidence:

- A package-targeted `grep-printer` test call appears in the trace audit before
  and after the regression-test repair; the final `grep-printer` test segment
  is classified as successful by the run trace.
- The validation audit also records successful cargo calls that cover changed
  files before and after the compile-failure recovery.
- No formatting check was recorded. The validation audit explicitly warns:
  do not claim `cargo fmt` or `rustfmt` passed.

The final assistant response claimed `cargo test --package grep-printer` and
`cargo check --package ripgrep` passed. The trace supports package arguments in
the model-visible tool requests, but the validation-audit projection stores the
manifest path and coverage more strongly than package names. For future
adjudication, prefer the validation audit's `manifest_path`, `resolved_scope`,
and `covers_changed_files` fields over the final prose claim.

## Positive Examples And Adjudication Candidates

Strong positive chain:

- Tool context around `Replacer::replace_all` and `Matcher` capture APIs led to
  a bounded helper in `util.rs`, rather than changing the public
  `grep-matcher` API.
- A failed semantic edit path did not end the run; the model switched to a
  non-semantic patch and applied the production change.
- A compile-failing regression test produced a recovery segment. The model
  inspected the bottom of `standard.rs`, repaired the test placement/import
  issue, and reran validation successfully.

Candidate adjudication fields:

- `cargo_failure_used_for_repair = true`
- `final_validation_covers_changed_files = true`
- `fmt_evidence_present = false`
- `protocol_review_complete = false`
- `tool_lifecycle_parity = true`
- `empty_successful_reads_count = 13`
- `duplicate_tool_request_count = 8`
- `semantic_tool_failure_recovered = true`

## Trace Reconstruction

The run begins with targeted search for multiline replacement terms and the
`Replacer` abstraction. It locates `crates/printer/src/util.rs` and reads the
main replacement implementation. It then searches for
`replace_with_captures_at` and inspects `crates/matcher/src/lib.rs` to
understand capture iteration.

The middle of the run shifts from matcher internals back to printer behavior.
The model searches for `replace_all`, `StandardSink`, and replacement-related
tests in `standard.rs`, then validates baseline state with cargo. Some of this
is inefficient: the trace includes duplicate searches and multiple empty reads
against out-of-range line spans. Still, the successful reads provide enough
context to identify the boundary bug.

The first production edit adds a helper and patches `util.rs`. Two semantic
edit attempts fail, but a direct patch succeeds. After that, the model searches
for the right test location, inserts tests, hits a compile failure, reads the
actual test module area, and repairs the test addition. The final segment runs
validation commands and ends with a non-empty patch export.

The trace therefore shows real progress: tool output informed the production
edit, validation failure changed the next action, and the run ended with an
issue-relevant patch plus passing recorded validation. It also shows waste:
the model repeatedly queried already-known symbols and trusted some empty
successful reads before switching to larger reads.

## Protocol Review And Blind Spots

The protocol root contains exactly one protocol artifact:

- `1779695958182_tool_call_intent_segmentation_BurntSushi__ripgrep-2209.json`

That artifact segments all 92 tool calls into 12 labeled segments with no
uncovered calls and no ambiguous calls. Its overall rationale is broadly
accurate: locate target, inspect candidate code, edit, validate, locate tests,
add tests, recover from compile failure, and run final validation.

The missing protocol review procedures matter. There is no
`tool_call_review` artifact and no `tool_call_segment_review` artifact for this
run. Because of that, protocol did not independently judge whether each tool
result was semantically useful, whether empty reads were harmless, whether the
final patch was behaviorally sufficient, or whether the final validation scope
matched the final answer.

Protocol blind spot: segmentation completed, but segmentation is not a semantic
success auditor. It can say every call belongs to a phase; it does not prove
the patch is correct or that the model used each result well.

## Runtime-Playback Inventory Classification

Checked inventory files before making missing-record claims:

- `runtime-playback/inventory/README.md`
- `runtime-playback/inventory/record-surface-map.md`
- `runtime-playback/inventory/latest-run-emission-worksheet.md`

Classification for this run:

- `record absent`: protocol-root `*_tool_call_review_*.json` and
  `*_tool_call_segment_review_*.json` are absent. Closure reports both required
  procedure families as missing.
- `record present, playback gap`: `llm-full-responses.jsonl` exists and has
  the raw provider responses, but the inventory says this is not yet loaded
  beside graph agent-turn records as a first-class playback family.
- `record present, playback gap`: `record.json.gz` exists, but the inventory
  says runtime playback still needs explicit iterators that choose between
  branch run records and agent-turn artifacts without duplicating facts.
- `record present, playback gap`: protocol intent segmentation exists, but
  protocol procedure internals are summarized evidence rather than ordered
  runtime playback steps.
- `record present, manual join needed`: `multi-swe-bench-submission.jsonl` and
  `benchmark-patch-projection.json` exist and join to the run by path/run root;
  the inventory says these are benchmark/evaluation witnesses, not first-class
  graph inputs by default today.
- `record present, manual join needed`: `config/ploke/proposals.json` exists
  with four applied proposals. It is useful for edit lifecycle review, but the
  inventory says proposal persistence is only reliable playback evidence when
  the run explicitly redirects or records the path and proposal ids.
- `operator/convenience record`: `closure-state.json` is a reduced campaign
  status projection/cache. It is the right status summary for this review, but
  it is not the underlying tool-call authority.
- `operator/convenience record`: run registration is the discovery/lifecycle
  authority for this run, but its protocol lifecycle detail should not override
  closure's per-procedure missing statuses.

Optional failure artifacts are absent but not evidence gaps for this successful
run: `parse-failure.json` and `indexing-failure.db` are referenced by
registration paths but were not emitted, while `indexing-status.json` reports
cached starting DB setup completed.

## What Is Working

- The parent eval path can produce a non-empty, issue-relevant patch through
  direct Google Gemini 3.5 Flash.
- Provider and recorded tool ledgers match exactly for this run.
- The run captures enough artifacts for independent review: raw provider
  responses, agent turn trace/summary, compressed run record, validation audit,
  submission export, patch projection, and protocol segmentation.
- The model had a useful recovery loop after a compile failure.
- The validation audit correctly refuses to overclaim formatting evidence.

## What Is Not Working Yet

- Required protocol review families did not run or did not persist:
  `tool-call-review` and `tool-call-segment-review` are absent.
- The run registration says protocol completed because an aggregate exists,
  while closure correctly reports partial protocol. That wording can mislead
  operators if they only read registration lifecycle.
- The agent wasted calls on duplicate searches and empty successful reads.
- Several present record families still need manual joins or playback iterators
  before a reviewer can reconstruct the run from one typed cursor.
- Final assistant prose is less authoritative than the validation audit for
  validation scope; adjudication should prefer structured validation fields.

## Action Items

1. Non-blocker for eval: keep this parent eval record as a usable baseline
   candidate. It has a non-empty patch, applied edit proposals, and final cargo
   evidence covering changed files.
2. Blocker for protocol completeness before treating the parent protocol as
   done: generate or repair the missing `tool-call-review` and
   `tool-call-segment-review` artifacts for this run, or explicitly classify
   why those required procedure families are intentionally skipped.
3. Improve operator status wording: registration protocol `completed` should
   distinguish "aggregate available" from "all required protocol procedures
   complete" so it does not conflict with closure's `partial` status.
4. Add adjudication fields for validation-scope authority, empty successful
   reads, duplicate requests, and validation-failure recovery. This run has
   clean examples of each.
5. Runtime playback should promote `llm-full-responses.jsonl`, protocol
   procedure artifacts, patch projection, MBE submission, and proposal records
   through typed joins instead of requiring manual path-based review.
