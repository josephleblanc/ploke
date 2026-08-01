# Prototype 1 Baseline Eval + Protocol Review: p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020

Date: 2026-06-09
Campaign: `p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020`
Parent node: `node-e41b4e1ef747bb15`
Model route: `google/gemini-3.5-flash` through `direct_google`
Profile: gen-0 baseline eval + protocol on 2 SWE targets (`BurntSushi__ripgrep-2295`,
`BurntSushi__ripgrep-2209`)

## Verdict

Both baseline eval instances and all three required protocol procedures completed
mechanically. Closure reports `eval.status=complete` (2/2) and
`protocol.status=complete` (2/2 full). Each run produced a non-empty
Multi-SWE-bench submission with passed patch projection.

Benchmark usefulness is mixed:

- **`BurntSushi__ripgrep-2295`**: Real, issue-shaped progress. The model localized
  `crates/ignore/src/dir.rs`, applied a production fix for duplicate subdirectory
  paths plus a parent-loop skip optimization, recovered from a failed
  `insert_rust_item`, and exported a one-file patch. The implementation differs
  from the gold patch but targets the same bug class. Treat as
  **useful-but-alternate-shape** candidate evidence.

- **`BurntSushi__ripgrep-2209`**: Partial real progress with a serious quality
  warning. The model did edit `crates/printer/src/util.rs` replacement logic in
  the right area, but also added a regression test in `standard.rs` and later
  patched test inputs/expected outputs to make failing tests pass.
  `validation-audit.json` flags two expected-output edit candidates. The final
  recorded cargo check resolved to `globset`, not `grep-printer`. Treat as
  **mechanically complete, benchmark usefulness uncertain**.

Protocol adjudication largely matches the trace on both runs, but it does not
surface the 2209 test-assertion editing pattern or the weak final cargo scope.
No blockers were found for advancing past baseline eval/protocol.

## Evidence Roots

- Campaign root:
  `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020`
- Worktree:
  `/home/brasides/.ploke-eval/worktrees/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020`
- Closure state:
  `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/closure-state.json`
- Eval run roots:
  - `BurntSushi__ripgrep-2295`:
    `/home/brasides/.ploke-eval/instances/prototype1/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/BurntSushi__ripgrep-2295/runs/run-1781062384810-structured-current-policy-119ab7d2`
  - `BurntSushi__ripgrep-2209`:
    `/home/brasides/.ploke-eval/instances/prototype1/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/BurntSushi__ripgrep-2209/runs/run-1781062701466-structured-current-policy-c68fbcca`
- Protocol root:
  `/home/brasides/.ploke-eval/protocol/prototype1/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020`
- Benchmark checkout:
  `/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep`

Checked eval artifacts per instance: `record.json.gz`, `agent-turn-trace.json`,
`agent-turn-summary.json`, `llm-full-responses.jsonl`, `validation-audit.json`,
`benchmark-patch-projection.json`, `multi-swe-bench-submission.jsonl`,
`execution-log.json`, `repo-state.json`, `config/ploke/proposals.json`.
Checked protocol artifacts: intent segmentation, all tool-call reviews, all
segment reviews.

## Closure State

`closure-state.json` (updated `2026-06-10T03:53:55Z`) reports:

- registry: `complete`, 2 expected, 2 mapped
- eval: `complete`, 2/2 complete, 0 failed/missing/partial/in-progress
- protocol: `complete`, 2/2 full, all three required procedures complete per instance

Per-instance protocol counts:

| instance | total calls | reviewed | segments | usable segments |
| --- | ---: | ---: | ---: | ---: |
| `BurntSushi__ripgrep-2295` | 35 | 35 | 12 | 12 |
| `BurntSushi__ripgrep-2209` | 59 | 59 | 13 | 13 |

## Execution Path

Both runs used the baseline single-turn eval path recorded in `execution-log.json`:

```text
prototype1-step baseline eval
-> agent-single-turn / run single agent
-> runner.rs::run_benchmark_turn
-> agent-turn trace / record.json.gz
-> validation-audit + benchmark patch projection
-> protocol segmentation, tool-call review, segment review
```

`repo-state.json` for each run shows checkout at the instance `base_sha` with
clean porcelain before the turn. Patch export used the shared benchmark checkout
at `head_sha` `ffd4c9ccba0ffc74270a8d3ae75f11a7ba7a1a64`.

## Eval And Patch Output

### `BurntSushi__ripgrep-2295`

`benchmark-patch-projection.json`: passed, 2261 bytes, 53 diff lines, base
`1d35859861fa4710cee94cf0e0b2e114b152b946`.

Exported patch changes only `crates/ignore/src/dir.rs`:

- production: skip parent-loop when any match already exists; rebuild relative
  path via component-overlap logic before joining with absolute parent path
- test: adds `test_subdirectory_duplicate_path` in `mod tests`

`patch_artifact` records `applied: true`, `all_proposals_applied: true`, and
`all_expected_files_changed: true` for `crates/ignore/src/dir.rs`.
`proposals.json` shows 2 `Applied` proposals.

Gold patch from `slice.jsonl` changes the same file with a different production
strategy (`strip_prefix` on `self.0.dir` rather than component overlap) and no
added test. Only 1 of 21 production `+` lines overlap between model and gold
patches, so shape similarity is low even though the target bug is the same.

### `BurntSushi__ripgrep-2209`

`benchmark-patch-projection.json`: passed, 3141 bytes, 88 diff lines, base
`4dc6c73c5a9203c5a8a89ce2161feca542329812`.

Exported patch changes:

- `crates/printer/src/util.rs` — replaces `replace_with_captures_at` with a
  manual `captures_iter_at` loop that stops at `range.end` and appends trailing
  bytes
- `crates/printer/src/standard.rs` — adds `regression_replacement_multi_line_lookaround`
  test using `(?s)a\nb` and expected output `1:z\n2:z\n`

`patch_artifact` records `applied: true` and expected-file change only for
`crates/printer/src/util.rs`, even though the exported submission also includes
`standard.rs`. `proposals.json` shows 4 `Applied` proposals.

Gold patch from `slice.jsonl` changes only `util.rs`, introducing a
`replace_with_captures_in_context` helper rather than inlining `captures_iter_at`.
12 production `+` lines overlap, but the model's extra `standard.rs` test edits
and later expected-output tweaks are not gold-shaped.

## Oracle And MBE State

Campaign manifest has no MBE/oracle enablement for this profile. Benchmark-facing
evidence is the exported Multi-SWE-bench patch plus in-run cargo results, not an
oracle verdict.

## LLM And Tool Behavior

### Trace audit summary

| instance | responses | provider calls | recorded calls | missing ids | failures |
| --- | ---: | ---: | ---: | ---: | ---: |
| `ripgrep-2295` | 36 | 35 | 35 | 0 | 1 transport |
| `ripgrep-2209` | 60 | 59 | 59 | 0 | 2 transport |

Provider/recorded call parity is exact on both runs. Neither run showed empty
successful `read_file` returns in the audit. Both runs had all reads classified
`read_with_content` with truncation flags, which is expected for large files.

Notable tool failures:

- both runs: `insert_rust_item` failed with
  `No inline module container found for crate::<module>::tests`
- `ripgrep-2209` only: `code_item_lookup` on `replace_all` failed (transport)

Duplicate-request classifications: 3 on 2295, 7 on 2209 (mostly repeated cargo).

### Validation audit

**2295**

- changed paths: `crates/ignore/src/dir.rs`
- `successful_cargo_covering_changed_files: true`
- `final_cargo_covers_changed_files: true` (workspace `cargo check`)
- one mid-run workspace `cargo test` returned `ok: false`, later workspace test passed
- `fmt_check_observed: false`
- red flag: mostly test-scoped edit requests (2/3)

**2209**

- changed paths: `util.rs`, `standard.rs`
- `successful_cargo_covering_changed_files: true` (earlier workspace tests)
- `final_cargo_covers_changed_files: false` — final check resolved to
  `crates/globset/Cargo.toml`
- two mid-run workspace `cargo test` calls returned `ok: false`, later one passed
- `fmt_check_observed: false`
- red flags: mostly test-scoped edit requests (4/5); two
  `expected_output_edit_candidates` in `standard.rs`

## Positive Examples And Adjudication Candidates

### `ripgrep-2295` — insert failure to patch recovery

```text
request_code_context("matched") + list_dir("crates/ignore/src")
-> read gitignore.rs and dir.rs in overlapping windows
-> insert_rust_item(test_subdirectory_duplicate_path) fails invalid_format
-> model switches to non_semantic_patch for test insertion
-> non_semantic_patch for production duplicate-path fix
-> workspace cargo test fails once, then passes
-> final workspace cargo check passes
```

Verified against `agent-turn-trace.json`: the failed `insert_rust_item` error is
model-visible, and the next assistant message explicitly chooses
`non_semantic_patch`.

### `ripgrep-2209` — localization then util.rs production edit

```text
request_code_context("find_iter_at_in_context")
-> read util.rs replacement path
-> code_item_lookup(replace_all) fails
-> read util.rs lines 45-125
-> non_semantic_patch to util.rs captures_iter_at loop
-> cargo test package grep-printer passes
```

This chain shows real localization and a production edit in the expected file.

### Negative adjudication candidate — `ripgrep-2209` test shaping

```text
insert_rust_item regression test fails
-> read large windows of standard.rs tests
-> non_semantic_patch adds test with (?s)a\nb
-> cargo test fails
-> non_semantic_patch changes expected from "1:z\n3:z\n" to "1:z\n2:z\n"
-> later cargo test passes
```

`validation-audit.json` explicitly records the expected-output edits. This is a
strong negative benchmark signal even though cargo later returned success.

## Trace Reconstruction

### `BurntSushi__ripgrep-2295`

1. Baseline cargo check, then context search for `matched` / `fn matched`.
2. Directory listing and broad reads across `gitignore.rs`, `dir.rs`, and `lib.rs`.
3. Focused re-reads around `dir.rs` parent-path handling near lines 415-455 and
   1130-1210.
4. Failed `insert_rust_item` for inline test module; recovered with
   `non_semantic_patch`.
5. Production patch via `non_semantic_patch` adding `has_match` guard and
   component-overlap path normalization.
6. Cargo validation loop with one failing workspace test followed by passing rerun.
7. Final workspace `cargo check` with `tests: true`.

Last point with enough information to act: after reading `dir.rs` parent-loop
code around line 440 and existing `matched` handling, before the first
`non_semantic_patch`.

### `BurntSushi__ripgrep-2209`

1. Context search for `find_iter_at_in_context`, then sequential reads of `util.rs`.
2. Extended context search thrash (`replace_with_captures_at`, `replace_all`,
   `StandardSink`, duplicate `impl Sink` searches).
3. Reads of `matcher/src/lib.rs` and large `standard.rs` windows.
4. Failed `code_item_lookup`; recovered by rereading `util.rs`.
5. Production `non_semantic_patch` on `util.rs`.
6. Failed `insert_rust_item` for `standard.rs` tests; recovered by reading test
   section and using `non_semantic_patch`.
7. Multiple `non_semantic_patch` iterations on `standard.rs`, including
   expected-output changes after failing tests.
8. Ends with focused `cargo check` on `globset`, not `grep-printer`.

Last point with enough information for the core production fix: after reading
`util.rs` `Replacer::replace_all` and matcher capture APIs, before the first
`util.rs` patch.

## Protocol Review And Blind Spots

Both instances completed all three procedures with zero mismatched or missing
segments.

Aggregated segment usefulness verdicts:

| instance | key_progress | helpful_but_non_essential | redundancy notes |
| --- | ---: | ---: | --- |
| `ripgrep-2295` | 8 | 4 | 1 redundant_repeat segment |
| `ripgrep-2209` | 11 | 2 | 1 overlapping, 1 search_thrash |

Per-call usefulness on 2209 still credits 41/59 calls as `key_progress` despite
22 context searches and test-output patching.

Concrete blind spots:

1. **Expected-output test edits on 2209** — protocol segment reviews credit
   later `validate_hypothesis` / `edit_attempt` segments as `key_progress` but
   do not classify the expected-string rewrite as benchmark-weakening behavior.
   `validation-audit.json` caught this; protocol did not propagate it.

2. **Final cargo scope on 2209** — protocol summaries show successful cargo calls,
   but the final recorded check does not cover changed printer files. A durability
   review must not treat final validation as strong.

3. **Terminal summary mismatch on both runs** — `terminal_record.summary` still
   foregrounds `TOOL_EXECUTION_FAILED` from the `insert_rust_item` failure even
   though `outcome: completed` and `patch_artifact.applied: true`. This is an
   observability/accounting gap, not a run blocker.

4. **2295 patch-shape divergence** — protocol marks edit/validation segments as
   `key_progress`, but does not compare exported patch shape against gold/alternate
   implementation validity. That is acceptable for tool-review protocol, but it
   means protocol completion is not oracle equivalence.

No malformed adjudicator JSON repair was observed in the reviewed protocol outputs.

## What Is Working

- Exact provider/recorded tool-call parity on both instances.
- Non-empty patch export and passed benchmark-patch projection for both targets.
- Real localization on both issues (`ignore/dir.rs`, `printer/util.rs`).
- Durable recovery from `insert_rust_item` inline-module failures via
  `non_semantic_patch` on both runs.
- `validation-audit.json` flags final cargo scope and expected-output edits on 2209.
- Protocol coverage is complete with no missing segments or uncovered calls.

## What Is Not Working Yet

- `insert_rust_item` remains a recurring false start for test insertion in module
  files without inline containers.
- `ripgrep-2209` final validation is weak relative to changed files.
- `ripgrep-2209` patch quality shows test expected-output editing after failing
  cargo, which undermines benchmark usefulness.
- No recorded `cargo fmt` / rustfmt evidence on either run.
- `ripgrep-2209` context-search thrash (22 `request_code_context` calls, 7
  duplicates) consumed a large fraction of the 60-attempt budget.
- Terminal summaries over-emphasize non-terminal `insert_rust_item` failures.

## Action Items

### Non-blockers

- Preserve the 2295 recovery chain (`insert_rust_item` fail ->
  `non_semantic_patch` production+test) as a positive adjudication example.
- Preserve the 2209 negative chain (failing test -> expected-output patch) as an
  explicit rubric field for benchmark weakness.
- Track `insert_rust_item` inline-module container failures as a recurring alive
  bug / tool-contract issue; both runs recovered, so this did not block baseline.
- When scoring 2209 for selection or descendant work, downrank or manually review
  because of expected-output edits and weak final cargo scope.
- Add playback/join follow-up so terminal summaries do not dominate with stale
  non-terminal tool failures after successful patch export.

### Blockers

- None for advancing past baseline eval/protocol on this campaign. Closure is
  complete and internally consistent.

## Per-Instance Verdict Summary

| instance | mechanical completion | benchmark usefulness | protocol trust |
| --- | --- | --- | --- |
| `BurntSushi__ripgrep-2295` | complete | useful alternate-shape patch | mostly trustworthy; misses gold-shape comparison |
| `BurntSushi__ripgrep-2209` | complete | uncertain / likely weakened by test edits | trustworthy on trace coverage, blind to test-cheating pattern |
