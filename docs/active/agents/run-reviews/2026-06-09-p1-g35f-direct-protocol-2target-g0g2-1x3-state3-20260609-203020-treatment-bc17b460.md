# Prototype 1 Gen-1 Treatment Eval Review: branch-bc17b460acc98b0d

Date: 2026-06-09
Campaign: `p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020`
Child node: `node-f1bb9e07069d5678` (generation 1, status `succeeded`)
Branch: `branch-bc17b460acc98b0d` / candidate `broad-harness-g1-03`
Broad slot: `broad-harness-request:node-e41b4e1ef747bb15:r3` (see
[`2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-broad-r3.md`](2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-broad-r3.md))
Model route: `google/gemini-3.5-flash` through `direct_google`

## Verdict

The gen-1 treatment child completed mechanically: node runtime `succeeded`, treatment
closure reports eval 2/2 and protocol 2/2 complete, both instances exported
non-empty Multi-SWE-bench submissions with passed patch projection, and
provider/recorded tool-call ledgers match on both runs (30/30 and 52/52).

Branch evaluation correctly **rejected** the branch overall because
`BurntSushi__ripgrep-2295` regressed `tool_calls_failed` (1 → 2) even though
`BurntSushi__ripgrep-2209` **kept** with improved operational metrics (fewer
calls, fewer failures, fewer same-file edit retries).

Benchmark usefulness is mixed and only partially correlated with branch metrics:

- **`BurntSushi__ripgrep-2209`**: Real production rewrite in `util.rs` with
  compile/test-driven repair, but the exported patch has **no regression test**
  (baseline adds `regression_replacement_multi_line_lookaround` in `standard.rs`).
  Operational metrics improved, yet benchmark proof is **weaker than baseline**.
  Treat as mechanically complete alternate-shape production fix with missing test
  coverage.

- **`BurntSushi__ripgrep-2295`**: Same bug class as baseline (`dir.rs` parent-path
  dedup), but the exported `has_match` guard uses **AND** across match categories
  where baseline uses **OR** — verified against persisted submission diffs. That
  is a likely correctness regression independent of the extra failed
  `code_item_lookup` calls. Doc-comment stripping on `matched_ignore` adds patch
  noise. Treat as **useful localization with suspect logic shape**.

The broad-harness edit (write-path preflight refactor in `tools/mod.rs` at commit
`39c6bd67`) is not directly exercised in ripgrep benchmark checkouts; this review
cannot prove descendant benchmark benefit from the parent patch alone.

**No campaign blockers.** Branch reject is expected operational disposition, not a
lifecycle or evidence defect.

## Evidence Roots

- Parent campaign node:
  `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/nodes/node-f1bb9e07069d5678`
- Broad-harness workspace (derived artifact `39c6bd67`):
  `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/workspaces/edit-harness/node-e41b4e1ef747bb15-r3`
- Treatment campaign closure:
  `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-treatment-branch-bc17b460acc98b0d-1781064596901/closure-state.json`
- Branch evaluation:
  `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/evaluations/branch-bc17b460acc98b0d.json`
- Eval run roots:
  - `BurntSushi__ripgrep-2295`:
    `/home/brasides/.ploke-eval/instances/prototype1/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/treatments/branch-bc17b460acc98b0d/instances/BurntSushi__ripgrep-2295/runs/run-1781064597955-structured-current-policy-d97fa27d`
  - `BurntSushi__ripgrep-2209`:
    `/home/brasides/.ploke-eval/instances/prototype1/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/treatments/branch-bc17b460acc98b0d/instances/BurntSushi__ripgrep-2209/runs/run-1781064909365-structured-current-policy-df67ba52`
- Protocol root:
  `/home/brasides/.ploke-eval/protocol/prototype1/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/treatments/branch-bc17b460acc98b0d`
- Baseline comparison runs (gen-0):
  - `BurntSushi__ripgrep-2295`: `run-1781062384810-structured-current-policy-119ab7d2`
  - `BurntSushi__ripgrep-2209`: `run-1781062701466-structured-current-policy-c68fbcca`

Checked per instance: `record.json.gz`, `agent-turn-trace.json`,
`agent-turn-summary.json`, `llm-full-responses.jsonl`, `validation-audit.json`,
`benchmark-patch-projection.json`, `multi-swe-bench-submission.jsonl`,
`execution-log.json`, `config/ploke/proposals.json`, protocol intent segmentation,
all tool-call reviews, all segment reviews. Ran bundled trace audit on both run
roots. Verified suspicious patch content against persisted `fix_patch` in
submission JSONL because the per-node instance checkout was no longer present on
disk at review time.

## Closure State

Treatment campaign `closure-state.json` (updated `2026-06-10T04:26:04Z`):

- registry: `complete`, 2/2 mapped
- eval: `complete`, 2/2
- protocol: `complete`, 2/2 full (all three required procedures per instance)

Parent node `runner-result.json`: `status=succeeded`, `disposition=succeeded`,
`treatment_campaign_id=...-treatment-branch-bc17b460acc98b0d-1781064596901`,
`exit_code=0`.

Branch registry (`evaluations/branch-bc17b460acc98b0d.json`):

| instance | branch disposition | primary reason |
| --- | --- | --- |
| `BurntSushi__ripgrep-2209` | keep | `tool_calls_failed` 2→1; same-file retries 2→1; max streak 3→2 |
| `BurntSushi__ripgrep-2295` | reject | `tool_calls_failed` 1→2 |
| overall | reject | 2295 regression |

Protocol counts:

| instance | total calls | reviewed | segments | usable |
| ---: | ---: | ---: | ---: | ---: |
| `BurntSushi__ripgrep-2295` | 30 | 30 | 12 | 12 |
| `BurntSushi__ripgrep-2209` | 52 | 52 | 4 | 4 |

Wall clock (from `record.json.gz` timing): 2295 ~308s agent+setup; 2209 ~446s.

## Execution Path

```text
broad-harness r3 applied (39c6bd67 on ploke worktree)
  -> materialize_branch / build_child (node-f1bb9e07069d5678)
  -> spawn_child prototype1-runner (treatment campaign)
  -> per-instance agent-single-turn / run_benchmark_turn
  -> validation-audit + benchmark patch projection
  -> protocol segmentation, tool-call review, segment review
  -> observe_child + branch_evaluation.operational_metrics.v1
```

Evidence: `node.json` maps `patch_id` to `broad-harness-request:node-e41b4e1ef747bb15:r3`;
`execution-log.json` on both runs shows `role: treatment`, `execution: agent-single-turn`;
`target_relpath=crates/ploke-tui/src/tools/mod.rs`.

Instance checkouts were under the child node's
`instance-targets/...-treatment-branch-bc17b460acc98b0d-1781064596901/BurntSushi/ripgrep`
at run time (`benchmark-patch-projection.json`); checkout absent at review time.

## Parent Broad-Harness Context

Slot `r3` applied a write-path preflight refactor in `crates/ploke-tui/src/tools/mod.rs`
at commit `39c6bd67`, seeded from parent gen-0 state `f41dfe40`. The broad edit
eliminates double JSON deserialization during write-tool preflight by calling
`preflight_write_paths_direct` per tool arm after deserialize. It does not touch
ripgrep sources; benchmark turns run against vanilla instance base SHAs inside
isolated instance-target checkouts.

## Eval And Patch Output

### `BurntSushi__ripgrep-2295`

`benchmark-patch-projection.json`: passed, 2077 bytes, base
`1d35859861fa4710cee94cf0e0b2e114b152b946`.

Exported patch changes only `crates/ignore/src/dir.rs`:

- production: `has_match` short-circuit before parent loop; simpler
  `file_name`/`strip_prefix` path dedup before joining with `absolute_base()`
- test: adds `absolute_parent_anchored_duplicate`

Compared to gen-0 baseline (2261 bytes, component-overlap stripping, test
`test_subdirectory_duplicate_path`):

- **Verified suspicious artifact:** treatment `has_match` joins match categories
  with `&&` where baseline uses `||`. Example from submission diff:

  ```text
  +        let has_match = !m_custom_ignore.is_none()
  +            && !m_ignore.is_none()
  +            && (!m_gi.is_none() || !any_git || saw_git)
  +            && (!m_gi_exclude.is_none() || !any_git || saw_git);
  ```

  Baseline equivalent uses `||` across the four `!m_* .is_none()` checks. AND
  requires all categories to match simultaneously; that is a different (likely
  broken) predicate than "any category matched."

- **Verified suspicious artifact:** submission diff removes the doc comment block
  above `matched_ignore` (`-    /// Performs matching only on the ignore files`).

`validation-audit.json`: production-only changed paths; final cargo resolved to
focused `crates/regex/Cargo.toml` (does not cover `crates/ignore/src/dir.rs`);
workspace `cargo test` passed mid-turn. `fmt_check_observed: false`.

`config/ploke/proposals.json`: 3 applied proposals (1 `apply_code_edit`, 2
`non_semantic_patch`).

### `BurntSushi__ripgrep-2209`

`benchmark-patch-projection.json`: passed, 3354 bytes, base
`4dc6c73c5a9203c5a8a89ce2161feca542329812`.

Exported patch changes only `crates/printer/src/util.rs`:

- production: extracts `replace_with_captures_at_in_context` helper; clamps end to
  `min(range.end, subject.len())`; rejects matches at/after range end inside
  `captures_iter_at` loop
- **no test file changes** (`validation-audit.json`: `test_changed_path_count: 0`)

Compared to gen-0 baseline (adds inline fix in `util.rs` **and**
`regression_replacement_multi_line_lookaround` test in `standard.rs`): treatment
has production-only export with weaker benchmark proof despite passing workspace
tests during the turn.

- **Verified suspicious artifact:** submission diff strips doc comments above
  `Replacer::replace_all` and leaves bad indentation on the function signature
  (`+                pub fn replace_all`).

`validation-audit.json`: final cargo is workspace `cargo test` covering changed
files (`final_cargo_covers_changed_files: true`); no fmt evidence.

`config/ploke/proposals.json`: 5 applied proposals (multiple `apply_code_edit` and
`non_semantic_patch` on same file — same-file retry streak 2).

## Oracle / MBE State

No MBE oracle run was joined to this treatment child at review time. Both
instances remain `oracle_eligible: true` in branch metrics. Benchmark usefulness
judgments rely on patch shape, validation-audit flags, and trace reconstruction,
not oracle pass/fail.

## LLM And Tool Behavior

Trace audit summary:

| instance | responses | provider calls | recorded | failed tools |
| --- | ---: | ---: | ---: | ---: |
| 2295 | 31 | 30 | 30 | 2 |
| 2209 | 53 | 52 | 52 | 1 |

Ledger parity is clean (zero missing/extra call ids on both runs).

### `BurntSushi__ripgrep-2295` highlights

- Early localization via `request_code_context` + truncated `read_file` on
  `gitignore.rs` and `dir.rs`; model had enough context to edit by call 20
  (`apply_code_edit` on `matched_ignore`).
- Failed `code_item_lookup` on `gitignore.rs` (`matched`, wrong node kind) and on
  `dir.rs` (`matched_ignore`) — transport failures, recovered by reads and
  context search without retrying lookup with corrected args.
- `apply_code_edit` staged production fix; mid-turn workspace `cargo test` failed
  once (call 223) then passed after `non_semantic_patch` repair and test hunk add.
- Final model message describes `strip_prefix` dedup and loop optimization; message
  is coherent with patch shape but does not acknowledge the AND/OR `has_match`
  divergence from typical "any match" semantics.

### `BurntSushi__ripgrep-2209` highlights

- Strong localization: reads on `util.rs`, `matcher` crate, and extensive
  `standard.rs` exploration for replacement behavior.
- Failed `apply_code_edit` on `crate::util::replace_all` (call 34,
  `transport_failure`) — recovered via trait-path `Replacer::replace_all` edit
  and subsequent `non_semantic_patch` hunks after compile failures.
- Workspace `cargo check` failed twice mid-repair (calls 315, 347) before passing;
  final workspace `cargo test` passed (call 484).
- Model never added a regression test despite reading existing multiline
  replacement tests in `standard.rs`; final message claims robust fix but export
  is production-only.

## Positive Examples And Adjudication Candidates

1. **2209: apply failure → alternate canon path → compile repair**
   - `apply_code_edit` on free-function canon failed → model retried on
     `Replacer::replace_all` impl path → compile errors visible →
     `non_semantic_patch` cleanup → workspace tests green.
   - Adjudication field: semantic edit transport failure recovery without abort.

2. **2295: failed test → non_semantic_patch test hunk**
   - Mid-turn workspace test failure → `non_semantic_patch` adds
     `absolute_parent_anchored_duplicate` → subsequent tests pass.
   - Adjudication field: validation-driven test addition after production edit.

3. **2209: operational metric improvement without benchmark-shape improvement**
   - Branch `keep` on 2209 (52 calls, 1 failure) vs baseline (59 calls, 2 failures),
     but treatment patch lacks baseline's regression test.
   - Adjudication field: separate operational metrics from patch-quality /
     oracle-eligibility signals.

## Trace Reconstruction

### `BurntSushi__ripgrep-2295`

1. Calls 0–15: `cargo check`, context search, reads localize parent-path join in
   `dir.rs`.
2. Calls 16–19: continued reads; two failed `code_item_lookup` attempts
   (non-terminal).
3. Call 20: `apply_code_edit` on `matched_ignore` with `has_match` guard and path
   dedup logic.
4. Calls 21–23: duplicate cargo check/test; workspace test fails once.
5. Call 24: `non_semantic_patch` repairs production hunk.
6. Calls 25–27: tests pass; call 27 adds test via `non_semantic_patch`; final
   validation and stop.

Last point model had enough information to act: calls 10–12 reads of `dir.rs`
before first edit.

### `BurntSushi__ripgrep-2209`

1. Call 0: focused `cargo check`.
2. Calls 1–33: read `util.rs`, explore matcher/standard printer code, successful
   `code_item_lookup` on `StandardSink`.
3. Call 34: failed `apply_code_edit` on `replace_all` (transport) — non-terminal.
4. Call 35: successful `apply_code_edit` on `Replacer::replace_all`; package
   check passes.
5. Calls 37–46: further edits and `non_semantic_patch` after compile failures;
   duplicate reads of `util.rs`.
6. Calls 47–51: workspace check/test cycles; call 484 final workspace test pass.

Last point model had enough information: calls 2–4 reads of `util.rs` before
first successful edit. Model had test examples in `standard.rs` by call 28+ but
did not add an exported regression test.

## Protocol Review And Blind Spots

All required protocol stages completed for both instances with full call review
coverage.

Blind spots:

- Protocol does not flag 2295 `has_match` AND-vs-OR logic divergence from baseline
  shape or likely correctness risk.
- Protocol does not flag 2209 missing regression test despite model reading test
  neighborhood in `standard.rs`.
- Protocol does not flag accidental doc-comment deletion in exported patches (both
  instances).
- Protocol does not surface 2295 final-cargo focused-manifest mismatch
  (`crates/regex` vs changed `crates/ignore`).
- Branch evaluation uses operational metrics only; 2209 metric `keep` does not
  encode missing test coverage.

## What Is Working

- Gen-1 treatment spawn/observe lifecycle completed with consistent node,
  treatment closure, and branch evaluation artifacts.
- Tool-call ledger parity on both instances.
- Non-empty patch export and projection pass on both instances.
- 2209 compile/test-driven repair converged without abort; fewer total calls and
  failures than baseline.
- Full protocol closure on both instances (contrast with partial protocol on
  sibling branch `branch-f7e43aba97c12538`).

## What Is Not Working Yet

- Branch metric sensitivity: one extra failed lookup on 2295 rejects a branch that
  improved 2209 operational metrics.
- 2295 patch likely correctness risk from AND-combined `has_match` predicate.
- 2209 export lacks regression test present in baseline — weak benchmark proof
  despite branch `keep`.
- Patch quality: doc-comment stripping and indentation damage on both exports.
- 2295 final validation scope weak (focused regex manifest).
- Descendant benchmark benefit from r3 preflight refactor remains unproven.

## Action Items

### Non-blockers (alive bugs / follow-ups)

- Add adjudication/rubric field for `has_match`-style logic-shape regression vs
  baseline (2295 AND/OR).
- Add adjudication field for production-only export when model read test
  neighborhood but omitted regression test (2209).
- Extend alive bug for accidental doc-comment stripping in benchmark patch export
  (both instances; same class as sibling treatment reviews).
- Consider branch-evaluation policy note: 2209 operational `keep` vs missing test
  coverage — metrics alone over-credit mechanical improvement.

### Blockers

None. Branch reject is valid operational outcome; treatment campaign artifacts are
internally consistent. No handoff to `ploke-blocker-repair-loop` before campaign
continues.
