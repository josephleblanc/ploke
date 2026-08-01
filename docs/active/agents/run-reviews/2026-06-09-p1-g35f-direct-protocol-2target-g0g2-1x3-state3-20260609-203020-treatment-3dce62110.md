# Prototype 1 Gen-1 Treatment Eval Review: branch-3dce62110fd6c20b

Date: 2026-06-09
Campaign: `p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020`
Child node: `node-03d21940fd657962` (generation 1, status `succeeded`)
Branch: `branch-3dce62110fd6c20b` / candidate `broad-harness-g1-02`
Broad slot: `broad-harness-request:node-e41b4e1ef747bb15:r2` (see
[`2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-broad-r2.md`](2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-broad-r2.md))
Model route: `google/gemini-3.5-flash` through `direct_google`

## Verdict

The gen-1 treatment child completed mechanically: node runtime `succeeded`,
treatment closure reports eval 2/2 and protocol 2/2 complete, both instances
exported non-empty Multi-SWE-bench submissions with passed patch projection, and
provider/recorded tool-call ledgers match on both runs (27/27 and 32/32).

Branch evaluation correctly **rejected** the branch overall because
`BurntSushi__ripgrep-2295` regressed `tool_calls_failed` (1 → 2) even though
`BurntSushi__ripgrep-2209` **kept** with improved operational metrics (fewer
calls, fewer failures, no same-file edit retries).

Benchmark usefulness is mixed and only partially correlated with branch metrics:

- **`BurntSushi__ripgrep-2209`**: Treatment produced a cleaner issue-shaped patch
  than the gen-0 baseline — production fix in `util.rs` plus a new regression
  test, without baseline’s expected-output test edits. Treat as
  **useful alternate candidate** on merit, independent of the branch reject.

- **`BurntSushi__ripgrep-2295`**: Real progress on the same bug class as baseline
  (`dir.rs` parent-path dedup), recovered from `insert_rust_item` via
  `non_semantic_patch`, but with an extra failed lookup and accidental doc-comment
  deletion in the exported patch. Treat as **useful-but-noisier** than baseline.

The broad-harness edit (`suppress_emission` across nine ploke-tui files) is not
directly exercised in these ripgrep benchmark checkouts; this review cannot prove
descendant benchmark benefit from the parent patch alone.

**No campaign blockers.** Rejection is expected operational disposition, not a
lifecycle or evidence defect.

## Evidence Roots

- Parent campaign node:
  `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/nodes/node-03d21940fd657962`
- Broad-harness workspace (derived artifact `677d409b`):
  `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/workspaces/edit-harness/node-e41b4e1ef747bb15-r2`
- Treatment campaign closure:
  `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-treatment-branch-3dce62110fd6c20b-1781064596901/closure-state.json`
- Branch evaluation:
  `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/evaluations/branch-3dce62110fd6c20b.json`
- Eval run roots:
  - `BurntSushi__ripgrep-2295`:
    `/home/brasides/.ploke-eval/instances/prototype1/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/treatments/branch-3dce62110fd6c20b/instances/BurntSushi__ripgrep-2295/runs/run-1781064597998-structured-current-policy-0564e627`
  - `BurntSushi__ripgrep-2209`:
    `/home/brasides/.ploke-eval/instances/prototype1/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/treatments/branch-3dce62110fd6c20b/instances/BurntSushi__ripgrep-2209/runs/run-1781064931580-structured-current-policy-0ac98c37`
- Protocol root:
  `/home/brasides/.ploke-eval/protocol/prototype1/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/treatments/branch-3dce62110fd6c20b`
- Baseline comparison runs (gen-0):
  - `BurntSushi__ripgrep-2295`: `run-1781062384810-structured-current-policy-119ab7d2`
  - `BurntSushi__ripgrep-2209`: `run-1781062701466-structured-current-policy-c68fbcca`

Checked per instance: `record.json.gz`, `agent-turn-trace.json`,
`agent-turn-summary.json`, `llm-full-responses.jsonl`, `validation-audit.json`,
`benchmark-patch-projection.json`, `multi-swe-bench-submission.jsonl`,
`execution-log.json`, `config/ploke/proposals.json`, protocol intent segmentation,
all tool-call reviews, all segment reviews. Ran bundled trace audit on both run
roots.

## Closure State

Treatment campaign `closure-state.json` (updated `2026-06-10T04:21:55Z`):

- registry: `complete`, 2/2 mapped
- eval: `complete`, 2/2
- protocol: `complete`, 2/2 full (all three required procedures per instance)

Parent node `runner-result.json`: `status=succeeded`, `disposition=succeeded`,
`treatment_campaign_id=...-treatment-branch-3dce62110fd6c20b-1781064596901`,
`exit_code=0`.

Branch registry (`evaluations/branch-3dce62110fd6c20b.json`):

| instance | branch disposition | primary reason |
| --- | --- | --- |
| `BurntSushi__ripgrep-2209` | keep | `tool_calls_failed` 2→1; same-file retries 2→0 |
| `BurntSushi__ripgrep-2295` | reject | `tool_calls_failed` 1→2 |
| overall | reject | 2295 regression |

Protocol counts:

| instance | total calls | reviewed | segments | usable |
| ---: | ---: | ---: | ---: | ---: |
| `BurntSushi__ripgrep-2295` | 27 | 27 | 6 | 6 |
| `BurntSushi__ripgrep-2209` | 32 | 32 | 6 | 6 |

## Execution Path

```text
broad-harness r2 applied (677d409b on ploke worktree)
  -> materialize_branch / build_child (node-03d21940fd657962)
  -> spawn_child prototype1-runner (treatment campaign)
  -> per-instance agent-single-turn / run_benchmark_turn
  -> validation-audit + benchmark patch projection
  -> protocol segmentation, tool-call review, segment review
  -> observe_child + branch_evaluation.operational_metrics.v1
```

Evidence: `node.json` maps `patch_id` to `broad-harness-request:node-e41b4e1ef747bb15:r2`;
`execution-log.json` on both runs shows `role: treatment`, `execution: agent-single-turn`;
transition journal entries label branch `broad harness edit ... r2`.

Instance checkouts live under the child node’s
`instance-targets/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-treatment-branch-3dce62110fd6c20b-1781064596901/BurntSushi/ripgrep`.

## Parent Broad-Harness Context

Slot `r2` applied eleven `suppress_emission`-related patches across nine
`ploke-tui` files at commit `677d409b`, seeded from parent gen-0 state
`f41dfe40`. The broad edit does not touch ripgrep sources; benchmark turns run
against vanilla instance base SHAs inside isolated instance-target checkouts.
Operational branch comparison therefore measures whether the ploke-side patch
correlates with eval-turn metric changes, not direct code reuse.

## Eval And Patch Output

### `BurntSushi__ripgrep-2295`

`benchmark-patch-projection.json`: passed, 2821 bytes, base
`1d35859861fa4710cee94cf0e0b2e114b152b946`.

Exported patch changes only `crates/ignore/src/dir.rs`:

- production: skip parent-loop when any match exists; rebuild relative path via
  component-overlap stripping before joining with absolute parent path
- test: adds `subdirectories_ignore_issue_1757`

Compared to gen-0 baseline (2261 bytes, `strip_prefix` strategy, different test
name): same file and bug class, lower shape overlap, treatment patch is larger.

**Verified suspicious artifact:** submission diff removes the doc comment above
`matched_ignore` (lines starting with `-    /// Performs matching only on the
ignore files`). Production logic change is present; comment loss is patch-quality
noise, not a transport failure.

`patch_artifact`: 2 applied proposals, both on `dir.rs`, `all_proposals_applied:
true`.

### `BurntSushi__ripgrep-2209`

`benchmark-patch-projection.json`: passed, 4941 bytes, base
`4dc6c73c5a9203c5a8a89ce2161feca542329812`.

Exported patch changes `crates/printer/src/util.rs` and
`crates/printer/src/standard.rs`:

- production: adds internal `replace_with_captures_in_context` helper inside
  `Replacer::replace_all`, rejecting matches at/after logical range end
- test: adds `replacement_multi_line_issue_2095`

Compared to gen-0 baseline (3141 bytes): baseline also edited `util.rs` but later
patched existing test inputs/expected outputs in `standard.rs`; treatment adds a
new regression test only. `validation-audit.json` reports
`expected_output_edit_candidates: []` (baseline had two).

**Verified suspicious artifact:** submission diff also strips the doc comment
block above `Replacer::replace_all` in `util.rs`. Minor whitespace-only hunk on
an existing `#[test]` line (`replacement_multi_line_combine_lines`).

`patch_artifact`: 2 applied proposals; `expected_file_changes` lists only
`util.rs` changed even though `standard.rs` also changed in submission — record
present, manual join needed for the second file.

## Oracle / MBE State

No MBE oracle run was joined to this treatment child at review time. Both
instances remain `oracle_eligible: true` in branch metrics. Benchmark usefulness
judgments rely on patch shape, validation-audit flags, and trace reconstruction,
not oracle pass/fail.

## LLM And Tool Behavior

Trace audit summary:

| instance | responses | provider calls | recorded | failed tools |
| --- | ---: | ---: | ---: | ---: |
| 2295 | 28 | 27 | 27 | 2 |
| 2209 | 33 | 32 | 32 | 1 |

Ledger parity is clean (zero missing/extra call ids on both runs).

### `BurntSushi__ripgrep-2295` highlights

- Early localization via `request_code_context` + truncated `read_file` on
  `dir.rs`; model had enough context to edit by call 16 (`apply_code_edit` on
  `matched_ignore`).
- Failed `code_item_lookup` on wrong module path (`crate::dir::Ignore::matched`)
  — transport failure, recovered by reads.
- Failed `insert_rust_item` for test body (`No inline module container`) —
  recovered immediately via `non_semantic_patch` adding the test hunk.
- Model-visible `cargo test -p ignore` and workspace tests passed before export.
- `validation-audit.json` flags `edit requests were mostly test-scoped (2/3)`;
  final recorded cargo resolved to focused `ignore` package, not workspace-wide.

### `BurntSushi__ripgrep-2209` highlights

- Strong localization: reads on `util.rs`, then `find_iter_at_in_context` area;
  production edit applied on call 17.
- Failed `code_item_lookup` on `Replacer` impl — internal DB error (`stored
  relation 'impl' does not have field 'name'`); model recovered via extensive
  `standard.rs` reads instead of repeating lookup.
- Added regression test via `apply_code_edit`; workspace `cargo test` on
  `grep-printer` passed (95 tests including new test).
- Final model message claims `grep-printer` validation, but last recorded cargo
  call resolved to focused `grep-searcher` manifest — weak final-validation
  scope mismatch (same class as baseline reviews).

## Positive Examples And Adjudication Candidates

1. **2295: insert failure → patch recovery**
   - `insert_rust_item` failed on module container → model issued
     `non_semantic_patch` with test hunk → subsequent `cargo test` passed.
   - Adjudication field: semantic edit tool failure recovery path.

2. **2209: lookup internal error → read thrash → targeted edit**
   - `code_item_lookup` internal DB error on impl → model read surrounding
     `standard.rs` tests → applied `replace_all` helper fix → package tests green.
   - Adjudication field: tool internal failure did not block progress when reads
     supplied sufficient context.

3. **2209: cleaner patch shape vs baseline**
   - Treatment avoided editing existing test expected outputs; baseline did not.
   - Adjudication field: patch-quality / cheating-risk signal separate from
     operational metrics.

## Trace Reconstruction

### `BurntSushi__ripgrep-2295` (turning points)

1. Calls 0–15: context search + full-file reads localize `matched_ignore` parent
   join bug.
2. Call 16: `apply_code_edit` applies production fix (overlap path logic +
   `has_match` short-circuit).
3. Calls 17–18: duplicate cargo check/test on `ignore` (audit classifies as
   duplicate_request).
4. Call 22: `insert_rust_item` fails → call 23 `non_semantic_patch` adds test →
   calls 24–26 validation → stop with patch export.

Last point model had enough information to act: call 8–10 reads of `dir.rs`
before first edit.

### `BurntSushi__ripgrep-2209` (turning points)

1. Call 0: workspace `cargo check` baseline.
2. Calls 1–15: read `util.rs`, explore matcher crate, run `grep-printer` tests.
3. Call 16: failed impl lookup (internal) — non-terminal.
4. Call 17: `apply_code_edit` on `Replacer::replace_all` with in-function helper.
5. Calls 19–28: read existing multiline replacement tests in `standard.rs`.
6. Call 29: add `replacement_multi_line_issue_2095` test.
7. Calls 30–31: workspace then focused cargo tests → final assistant summary.

Last point model had enough information: call 2–4 reads of `util.rs` before edit.

## Protocol Review And Blind Spots

All required protocol stages completed for both instances with full call review
coverage and six usable segments each.

Blind spots consistent with prior campaign reviews:

- Protocol does not flag accidental doc-comment deletion in exported patches.
- Protocol does not surface 2209 final-message vs last-cargo-manifest mismatch.
- Protocol does not distinguish baseline-style expected-output test edits from
  treatment’s new-test-only shape (2209 treatment is cleaner, but protocol counts
  alone do not encode that).
- Branch evaluation uses operational metrics only; 2209 merit improvement does
  not override 2295 regression.

## What Is Working

- Gen-1 treatment spawn/observe lifecycle completed with consistent node,
  treatment closure, and branch evaluation artifacts.
- Tool-call ledger parity on both instances.
- Non-empty patch export and projection pass on both instances.
- Failed semantic tools (`insert_rust_item`, internal `code_item_lookup`) were
  recovered without aborting turns.
- 2209 treatment reduced thrash versus baseline (32 vs 59 calls, no same-file
  edit retries).

## What Is Not Working Yet

- Branch metric sensitivity: one extra failed tool on 2295 rejects an branch that
  improved 2209 operational metrics and patch shape.
- Patch quality: both treatment patches accidentally delete doc comments during
  `apply_code_edit` / export.
- Validation scope: final cargo calls still resolve to focused manifests while
  model summaries claim package/workspace-wide proof.
- `patch_artifact.expected_file_changes` under-reports multi-file 2209 export
  (only lists `util.rs`).
- Descendant benchmark benefit from `suppress_emission` broad edit remains
  unproven; no oracle join.

## Action Items

### Non-blockers (alive bugs / follow-ups)

- File or extend alive bug for `code_item_lookup` impl lookup internal DB error
  (`stored relation 'impl' does not have field 'name'`) — observed on 2209
  treatment, same class as broad-base review.
- Add adjudication/rubric field for accidental doc-comment stripping in exported
  benchmark patches (both instances).
- Improve validation-audit or final-message playback to catch claimed
  package vs resolved manifest mismatch (2209).
- Consider branch-evaluation policy note: mixed keep/reject across instances with
  opposing merit signals (2209 cleaner patch vs 2295 tool-failure regression).

### Blockers

None. Branch reject is valid operational outcome; treatment campaign artifacts are
complete and internally consistent. No `ploke-blocker-repair-loop` handoff.
