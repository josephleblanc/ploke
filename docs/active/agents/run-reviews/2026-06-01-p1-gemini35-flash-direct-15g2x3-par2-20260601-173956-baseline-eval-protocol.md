# Prototype 1 Baseline Eval + Protocol Review: p1-gemini35-flash-direct-15g2x3-par2-20260601-173956

Date: 2026-06-01
Campaign: `p1-gemini35-flash-direct-15g2x3-par2-20260601-173956`
Task: `BurntSushi__ripgrep-2209`
Run: `run-1780361227892-structured-current-policy-2a93bb07`
Parent node: `node-552c19a55f53dbe6`
Model route: `google/gemini-3.5-flash` through direct Google

## Verdict

The baseline eval and protocol completed mechanically. Closure reports one
complete eval instance and one full protocol instance, with all three required
procedures complete. The eval produced a non-empty Multi-SWE-bench patch, and
the model made real benchmark-shaped progress: it localized `grep-printer`
replacement logic, edited `crates/printer/src/util.rs`, added regression tests
for both linked issues, and saw successful cargo output after those edits.

The patch is not clean enough to treat as a high-confidence benchmark success.
The exported diff removes the doc comment on `Replacer::replace_all`, leaves
the `pub fn` line over-indented, and has no recorded formatting check. Protocol
credited the edit and validation chain, but it missed the formatting/doc-comment
quality issue and a read-tool defect where successful reads returned empty
content for line ranges that exist in the checkout.

After this baseline run, doctor reports phase `child_plan` with no blockers.
The run is ready for continued child-plan work operationally, but the baseline
patch should be considered useful-but-sloppy evidence rather than a clean
candidate patch.

## Evidence Roots

- Campaign root:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-par2-20260601-173956`
- Eval run root:
  `/home/brasides/.ploke-eval/instances/prototype1/p1-gemini35-flash-direct-15g2x3-par2-20260601-173956/BurntSushi__ripgrep-2209/runs/run-1780361227892-structured-current-policy-2a93bb07`
- Protocol root:
  `/home/brasides/.ploke-eval/protocol/prototype1/p1-gemini35-flash-direct-15g2x3-par2-20260601-173956/BurntSushi__ripgrep-2209/runs/run-1780361227892-structured-current-policy-2a93bb07`
- Benchmark checkout:
  `/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep`
- Closure state:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-par2-20260601-173956/closure-state.json`

Checked eval artifacts included `record.json.gz`, `agent-turn-trace.json`,
`agent-turn-summary.json`, `llm-full-responses.jsonl`,
`validation-audit.json`, `benchmark-patch-projection.json`,
`multi-swe-bench-submission.jsonl`, `indexing-status.json`,
`snapshot-status.json`, and `repo-state.json`. Checked protocol artifacts
included the intent segmentation artifact, all 52 tool-call review artifacts,
and all 8 segment-review artifacts.

## Closure State

`closure status --campaign p1-gemini35-flash-direct-15g2x3-par2-20260601-173956
--format json` reports:

- registry: `complete`, 1 expected, 1 mapped
- eval: `complete`, 1 expected, 1 complete, 0 failed/missing/partial/in-progress
- protocol: `complete`, 1 expected, 1 full, 0 failed/missing/partial/in-progress
- protocol procedures: `tool-call-intent-segments`, `tool-call-review`, and
  `tool-call-segment-review` all complete
- protocol counts: 52 total calls, 52 reviewed calls, 8 usable segments, 0
  mismatched or missing segments

`prototype1-doctor --repo-root . --format json` from the campaign worktree
reports phase `child_plan`, no blockers, and allowed actions `doctor`,
`continue`, and `step`. Prompt preflight passed, but it also notes that several
published broad-harness prompt files (`r3` through `r9`) do not yet have child
plans or materialized candidate workspaces. I treated those as current-phase
child-plan state, not baseline eval evidence.

## Eval And Patch Output

`benchmark-patch-projection.json` reports a passed patch projection:

- submission path: `multi-swe-bench-submission.jsonl`
- submission SHA-256:
  `f9a66419d5ad82f2393a3cda66218b3a13c3ce9d1ee28a8f395ae41b3e8913e4`
- byte length: 3719
- line count: 111
- diff base: `4dc6c73c5a9203c5a8a89ce2161feca542329812`
- detail: `fix_patch exported from the recorded checkout cwd`

The exported patch changes:

- `crates/printer/src/util.rs`
- `tests/regression.rs`

Direct checkout verification at the benchmark repo showed the same two modified
paths. The production edit replaces `Matcher::replace_with_captures_at` inside
`Replacer::replace_all` with a manual `try_captures_iter_at` loop and stops
replacement when `m.start() >= limit_end`. The tests add `r2208` and `r2095`
regressions under `tests/regression.rs`, both gated on PCRE2.

The patch-quality problem is visible in the exported diff and direct file
inspection:

- the method doc comment for `replace_all` is deleted
- line 50 in `crates/printer/src/util.rs` is over-indented as
  `        pub fn replace_all<'a>(`
- `validation-audit.json` records `fmt_check_observed: false`

Cargo accepted this code, but no recorded rustfmt or `cargo fmt` evidence exists.

## Oracle And MBE State

The admitted profile has `[execution.mbe] enabled = false`, and I found no MBE
or oracle artifact for this baseline review. That is expected for this profile.
The benchmark-facing evidence is the exported Multi-SWE-bench patch, not an
oracle verdict.

## LLM And Tool Behavior

The bundled trace audit reported:

- responses: 53
- provider-emitted tool calls: 52
- recorded tool calls: 52
- missing provider call ids: 0
- extra recorded call ids: 0
- finish reasons: 52 `tool_calls`, 1 `stop`
- recorded classifications: 15 completed, 3 transport failures,
  21 reads with content, 10 empty completed reads, 3 duplicate requests

The three real tool failures were not terminal:

- `request_code_context` failed on `replacement` with missing fixed rule
  `ploke.TypeTargetPaths`
- `code_item_lookup` on `replace_with_captures_at` failed because multiple
  items matched despite the file/module/name query
- `code_item_lookup` on `StandardSink::replace` failed because no such method
  was found at the requested path

The model recovered by falling back to directory listings and manual reads.

The confirmed bad tool-output case is more serious. Ten successful `read_file`
calls returned `ok:true`, `exists:true`, nonzero file byte length, and
`content:""` for ranges that exist. Direct `wc -l` verification showed:

- `crates/matcher/src/lib.rs`: 1322 lines
- `crates/printer/src/standard.rs`: 3678 lines

Examples of empty successful reads inside existing ranges:

- `crates/matcher/src/lib.rs`, lines 1100-1200
- `crates/matcher/src/lib.rs`, lines 1000-1100
- `crates/printer/src/standard.rs`, lines 900-1100
- `crates/printer/src/standard.rs`, lines 1100-1250

The model worked around this with overlapping reads, but these calls should not
be counted as information success.

## Positive Examples And Adjudication Candidates

Strong positive chain:

```text
read util.rs and matcher/printer APIs
-> interpret over-extended multiline replacement path
-> apply_code_edit to Replacer::replace_all
-> cargo check/test succeeds
-> add r2208 and r2095 regression tests
-> targeted tests pass
-> final ripgrep and grep-printer tests pass
```

The validation-audit cargo ledger after the production edit is stronger than
several prior runs:

- workspace-resolved `cargo check` at the ripgrep root covered changed files
- workspace-resolved `cargo test` at the ripgrep root covered changed files
- targeted `cargo test --package ripgrep r2208` passed
- targeted `cargo test --package ripgrep r2095` passed
- final `cargo test --package grep-printer` passed

Candidate adjudication signals:

- cargo/test output was visible and materially used
- issue-linked regression tests were added and directly run
- no formatting check was recorded despite a visible formatting defect
- successful read calls with empty content should be a negative information
  signal even when transport says `ok:true`
- protocol should preserve applied-edit state in the sequence summary

## Trace Reconstruction

The active execution path for the baseline was:

```text
prototype1-step
-> baseline eval
-> runner.rs::run_benchmark_turn
-> agent-turn trace / record.json.gz
-> benchmark patch projection
-> protocol segmentation, tool-call review, segment review
```

Concrete trace chain:

1. The model began with repository layout discovery, then `request_code_context`
   failed on `replacement`.
2. It recovered by listing `crates/printer/src` and reading
   `crates/printer/src/util.rs`, where `Replacer::replace_all` lives.
3. It inspected `crates/matcher/src/lib.rs` and `crates/printer/src/standard.rs`
   to understand matcher replacement semantics and caller context. This phase
   included useful reads, empty successful reads, and overlapping read windows.
4. It applied one production edit to `crates/printer/src/util.rs` via
   `apply_code_edit`; the patch artifact records that proposal as `Applied`.
5. It ran cargo check/test after the production edit and saw success.
6. It added `r2208` and `r2095` regression tests through two
   `insert_rust_item` calls; both proposals are recorded as `Applied`.
7. It ran targeted and broader tests successfully, then produced a final answer
   claiming the bug was resolved.

The last point where the model had enough information to act was before call
41, after reading `util.rs`, matcher replacement helpers, and standard printer
call sites. The production edit followed immediately and was not blocked by the
earlier tool failures.

## Protocol Review And Blind Spots

Protocol completed mechanically:

- 1 intent segmentation artifact
- 52 tool-call review artifacts
- 8 segment-review artifacts

Segment review mostly matches the trace:

- segment 0 credited recovery from failed `request_code_context`
- segment 3 marked matcher inspection as `mixed` with search thrash
- segment 4 marked printer inspection as overlapping but useful
- segment 5 credited the production edit as key progress
- segments 6 and 7 credited validation and regression-test work as key progress

The protocol blind spots are concrete:

1. The intent segmentation turn summary says `patch_proposed: false` and
   `patch_applied: false`, even though `agent-turn-summary.json` and
   `patch_artifact.edit_proposals` show three applied edit proposals. The
   per-call edit records were present, but the turn-level projection was wrong.
2. Segment review did not flag empty successful reads as low-information or
   defective. It credited surrounding inspection as useful, which is fair, but
   the tool lifecycle itself hid a bad result shape.
3. Protocol did not catch the patch-quality issue: deleted method docs,
   over-indentation, and missing fmt evidence.

## Record Inventory

- campaign state: `present` for `campaign.json`, `closure-state.json`,
  `scheduler.json`, and `prototype1/transition-journal.jsonl`
- parent identity: `present` in the active worktree and matched doctor output
- run profile and commitment: `present`
- scheduler projection: `present, manual join needed`; it still lists the root
  node but doctor is the clearer current phase surface
- node record and runner request: `present`
- runner result, invocation records, runtime result records, successor records,
  and runtime channels for the current `child_plan` phase: `record absent` in
  the checked node directory
- broad-harness requests: `present` for root and `r2` through `r9`
- broad-harness results and headless TUI traces: `record absent`; no child
  attempt has completed in the checked surface
- run registration, `record.json.gz`, agent-turn trace/summary,
  `llm-full-responses.jsonl`, validation audit, patch projection, and
  Multi-SWE-bench submission: `present`
- protocol artifacts: `present`
- indexing status, checkpoint DB, snapshot status, final snapshot DB:
  `present`
- MBE/oracle: `not applicable` for this profile

## What Is Working

- The direct-Google route produced a full eval turn and full protocol pass.
- Provider and recorded tool-call ledgers match exactly.
- The model recovered from tool failures instead of stopping.
- The model made a relevant production edit and added issue-linked tests.
- Cargo evidence covered changed files after the production edit.
- Protocol segment review correctly identified several recovery and redundancy
  patterns, including overlapping read/search behavior.

## What Is Not Working Yet

- `read_file` can return `ok:true` with empty content for valid line ranges,
  which encourages overlapping rereads and can be over-credited by protocol.
- Protocol's turn-level edit projection did not preserve the applied patch state.
- Protocol did not flag missing formatting evidence or obvious formatting/doc
  regressions in the exported patch.
- The final patch is benchmark-useful but not polished: it compiles and tests
  pass, but it deletes the method docs and leaves an over-indented method
  signature.
- Current child-plan state has published prompts without completed child-result
  records; that is not a baseline blocker, but it should be reviewed per child
  once attempts produce results.

## Action Items

1. Non-blocker: add a playback/protocol signal for successful reads with empty
   content when the requested line range is inside the file. This should be
   treated as low-information or defective, not merely `ok:true`.
2. Non-blocker: repair the protocol sequence projection so turn-level
   `patch_proposed` and `patch_applied` reflect applied edit proposals.
3. Non-blocker: include formatting evidence and doc/comment regressions in
   patch-quality adjudication. At minimum, protocol should surface
   `fmt_check_observed=false` beside visible formatting anomalies.
4. Follow-up review: when child attempts for `node-552c19a55f53dbe6-r2` and
   later requests complete, review each broad-harness child independently using
   its request, workspace, terminal trace, submitted result, and candidate git
   state.
