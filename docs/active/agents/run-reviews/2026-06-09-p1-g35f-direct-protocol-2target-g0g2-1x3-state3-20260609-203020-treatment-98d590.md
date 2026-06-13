# Prototype 1 Gen-2 Treatment Child Review: branch-98d590831a932f41

Date: 2026-06-09
Campaign: `p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020`
Child node: `node-f4a0067a0d3d0234` (`status=succeeded`, generation 2)
Branch: `branch-98d590831a932f41` (`candidate_id=broad-harness-g2-02`)
Parent: `node-0dca4fe449780e85` (gen-1 selected successor, broad-harness r3 slot)
Model route: `google/gemini-3.5-flash` through `direct_google`
Treatment artifact: early `NodeKind::Impl` rejection guard in
`crates/ploke-tui/src/tools/code_item_lookup.rs`
(`derived_artifact_id=artifact:git-commit:a6cccca417f2edec44e49364526eb47457076dc4`)

## Verdict

The gen-2 treatment child runner completed successfully. Both SWE-bench instances
exported non-empty patches with passed projection checks, branch evaluation returned
`keep` versus the gen-1 parent treatment baseline (`branch-f7e43aba97c12538`), and
treatment eval+protocol closure is **full** (2/2).

Mechanical success does not prove the broad-harness branch improved benchmark repair
quality. The admitted change is a ploke-tui tool guard unrelated to ripgrep logic;
treatment eval turns re-solved the same two ripgrep issues independently. Benchmark
usefulness remains mixed, similar to gen-1:

- **`BurntSushi__ripgrep-2295`**: Real production edits in `crates/ignore/src/dir.rs`,
  but doc-comment removal on `matched_ignore`, mis-indented function header, and
  alternate parent-skip logic versus gen-1. Treat as **useful-but-alternate-shape**
  with patch-quality warnings.

- **`BurntSushi__ripgrep-2209`**: Real production rewrite in `crates/printer/src/util.rs`
  plus test in `standard.rs`, but the test is a simplified duplicate-line case (`a` →
  `z`), not the issue's lookaround scenario, with test-input rewriting after failing
  `cargo test`. Treat as **mechanically complete, benchmark usefulness uncertain**.

Operational metrics improved on 2209 (`same_file_patch_retry_count` 1→0,
`same_file_patch_max_streak` 2→0). That is a legitimate mechanized `keep` reason but
not oracle proof.

**Blockers:** none for runner completion or branch `keep`. No handoff to
`ploke-blocker-repair-loop` is required before recording this branch as mechanically
admitted.

## Evidence Roots

- Parent campaign:
  `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020`
- Child node:
  `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/nodes/node-f4a0067a0d3d0234`
- Branch evaluation:
  `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/evaluations/branch-98d590831a932f41.json`
- Treatment closure:
  `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-treatment-branch-98d590831a932f41-1781067063806/closure-state.json`
- Treatment eval run roots:
  - `BurntSushi__ripgrep-2295`:
    `/home/brasides/.ploke-eval/instances/prototype1/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/treatments/branch-98d590831a932f41/instances/BurntSushi__ripgrep-2295/runs/run-1781067064898-structured-current-policy-5b934052`
  - `BurntSushi__ripgrep-2209`:
    `/home/brasides/.ploke-eval/instances/prototype1/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/treatments/branch-98d590831a932f41/instances/BurntSushi__ripgrep-2209/runs/run-1781067382340-structured-current-policy-34f93564`
- Protocol root:
  `/home/brasides/.ploke-eval/protocol/prototype1/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/treatments/branch-98d590831a932f41`
- Broad-harness origin review:
  [`2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-gen1-broad-r3.md`](2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-gen1-broad-r3.md)
- Gen-1 treatment baseline companion:
  [`2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-treatment-f7e43aba.md`](2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-treatment-f7e43aba.md)
- Terminal campaign synthesis:
  [`2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-terminal-outcome.md`](2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-terminal-outcome.md)

Checked per run: `record.json.gz`, `agent-turn-trace.json`, `agent-turn-summary.json`,
`llm-full-responses.jsonl`, `validation-audit.json`, `benchmark-patch-projection.json`,
`multi-swe-bench-submission.jsonl`, `execution-log.json`, `config/ploke/proposals.json`.
Checked node/runtime: `node.json`, `runner-request.json`, `runner-result.json`,
`results/cd31c644-851f-44c5-8e29-77501656a048.json`.
Verified suspicious patch content against persisted `fix_patch` in submission JSONL
because per-node instance checkouts were not guaranteed on disk at review time.

## Closure State

Child node (`node.json`, `runner-result.json`):

- `status=succeeded`, `disposition=succeeded`, `exit_code=0`
- `treatment_campaign_id=p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-treatment-branch-98d590831a932f41-1781067063806`
- Wall clock ~20 minutes (`created_at` 04:47:41Z → `updated_at` 05:07:29Z)

Treatment closure (`closure-state.json`, updated `2026-06-10T05:07:28Z`):

| layer | status | detail |
| --- | --- | --- |
| registry | complete | 2/2 mapped |
| eval | complete | 2/2 complete |
| protocol | complete | 2/2 full |

Branch evaluation (`branch-98d590831a932f41.json`):

- `overall_disposition=keep`
- Compared instances: 2/2 versus **gen-1 parent treatment** baseline
  (`branch-f7e43aba97c12538` runs), not gen-0 campaign baseline
- 2209 keep reasons: `same_file_patch_retry_count improved: 1 -> 0`,
  `same_file_patch_max_streak improved: 2 -> 0`
- Mechanized reasons only; no oracle/MBE proof

## Execution Path

```text
child plan admission (broad-harness-g2-02, gen-1 r3 slot)
  -> prototype1-runner on node-f4a0067a0d3d0234
  -> treatment campaign spawn (ploke-eval built from a6cccca4)
  -> per-instance agent-single-turn / run_benchmark_turn
  -> record.json.gz + validation-audit + MSB submission
  -> branch evaluation (operational metrics vs f7e43aba baseline)
  -> protocol on both instances
  -> successor handoff attempt -> max_generations hard stop (expected)
```

Evidence: both `execution-log.json` files show `role=treatment`,
`execution=agent-single-turn`, model `google/gemini-3.5-flash`. The treatment
binary was built from the gen-1 broad-harness r3 commit carrying the
`code_item_lookup` impl guard; ripgrep checkouts were separate instance targets
under the child node.

## Eval And Patch Output

### `BurntSushi__ripgrep-2295`

| metric | gen-1 baseline (f7e43aba) | gen-2 treatment |
| --- | ---: | ---: |
| tool calls | 22 | 31 |
| tool failures | 1 | 1 |
| patch bytes | 5091 | 2563 |
| same_file retry / streak | 0 / 1 | 0 / 1 |

`benchmark-patch-projection.json`: passed, base `1d35859861fa4710cee94cf0e0b2e114b152b946`.

Persisted `fix_patch` (verified from submission JSONL):

- Production: rewrites `matched_ignore` parent-loop guard with `skip_parents` logic
  and relative-path stripping before joining with `absolute_base()`.
- Quality defects (verified in submission diff): removes the preceding doc comment
  (`-    /// Performs matching only on the ignore files...`) and leaves mis-indented
  function header (`+            fn matched_ignore`).
- Test path: `insert_rust_item` failed (`transport_failure` in trace audit) → recovery
  via subsequent edits/`non_semantic_patch` chain (same pattern as gen-1).

`validation-audit.json`: final workspace `cargo test` covers changed files after one
intermediate failure; early cargo calls resolved to focused `grep-printer` manifest
(**not** covering `ignore` edits).

### `BurntSushi__ripgrep-2209`

| metric | gen-1 baseline (f7e43aba) | gen-2 treatment |
| --- | ---: | ---: |
| tool calls | 43 | 44 |
| tool failures | 1 | 1 |
| patch bytes | 3264 | 4096 |
| same_file retry / streak | 1 / 2 | **0 / 0** |

`benchmark-patch-projection.json`: passed, base `4dc6c73c5a9203c5a8a89ce2161feca542329812`.

Persisted `fix_patch` (verified from submission JSONL):

- Production (`util.rs`): alternate `replace_with_captures_at` rewrite with manual
  capture iteration and multi-line-aware tail extension — substantive but not baseline
  shape.
- Test (`standard.rs`): adds `replacement_multi_line_lookahead_kludge` using matcher
  `r"a"` and replacement `x`, expecting simplified duplicate-line output — **not** the
  issue's lookaround scenario (#2209 / #2095).
- Minor formatting noise: existing test attribute re-indented (`-    #[test]` →
  `+        #[test]`).
- Repair chain (`validation-audit.json`): workspace `cargo test` failed once
  (event 298, `ok=false`) before later passes; consistent with test-input rewriting
  rather than production repair.

`validation-audit.json`: changed paths `util.rs`, `standard.rs`; final cargo covers
changed files on workspace scope after intermediate failure.

## Oracle / MBE State

No oracle or MBE adjudication artifacts were joined to this treatment branch.
Branch `keep` is operational-metric only. Do not treat `oracle_eligible=true` in
branch metrics as benchmark proof.

## LLM And Tool Behavior

Trace audit summary (provider vs recorded parity perfect on both runs):

| instance | responses | provider calls | recorded calls | failed tools |
| --- | ---: | ---: | ---: | ---: |
| `2295` | 32 | 31 | 31 | 1 (`insert_rust_item`) |
| `2209` | 45 | 44 | 44 | 1 (`transport_failure` class) |

Shared patterns:

- Early `request_code_context` + truncated `read_file` chains localized the right
  modules before first production edit — real information success.
- Both runs recovered structured-insert failures with alternate edit paths
  (`non_semantic_patch` / follow-up edits).
- Duplicate cargo requests deduplicated in audit while leaving model-visible validation.
- All `read_file` completions on 2209 carried `truncated=True`; model compensated with
  sequential chunk reads of `util.rs` and `matcher/src/lib.rs`.

Ledger parity: **0 missing provider call IDs** on both runs.

## Positive Examples And Adjudication Candidates

1. **2209 same-file retry improvement**
   - Gen-1 baseline had `same_file_patch_max_streak=2`; gen-2 treatment reached 0/0
     while still exporting a non-empty patch.
   - Candidate field: *operational metric improvement without oracle confirmation*.

2. **2295 localization chain**
   - `list_dir` + repeated `request_code_context("matched*")` → `dir.rs` reads →
     `apply_code_edit` on `matched_ignore`.
   - Candidate field: *context retrieval found correct module before first edit*.

3. **2209 validation-driven test rewrite (negative signal)**
   - Production edit → failing workspace test → simplified matcher/expected-output
     rewrite until green.
   - Candidate field: *protocol/adjudication should downgrade test-expectation edits
     as hypothesis validation* (same blind spot as gen-1).

## Trace Reconstruction

### Concrete trace chain — `BurntSushi__ripgrep-2295`

```text
cargo test (focused grep-printer manifest, ok but wrong scope)
  -> context searches for "matched" in ignore/
  -> dir.rs + gitignore.rs reads
  -> apply_code_edit matched_ignore (doc comment stripped in export)
  -> cargo check/test workspace (one failure, then pass)
  -> insert_rust_item test (FAILED transport_failure)
  -> follow-up edits / patch recovery
  -> final workspace cargo test (ok)
```

Last point with enough information to act: after initial `dir.rs` reads (~call 8–10),
before production edit. The model had the parent-loop location; later reads were
confirmatory.

### Concrete trace chain — `BurntSushi__ripgrep-2209`

```text
cargo check -p grep-printer (ok)
  -> request_code_context find_iter_at_in_context / replace_with_captures_at
  -> sequential truncated read_file chunks of util.rs + matcher/src/lib.rs
  -> production apply_code_edit on util.rs
  -> workspace cargo test (FAILED)
  -> test addition in standard.rs with simplified a/x scenario
  -> workspace cargo test (ok, final)
```

Last point with enough information to act: after reading `util.rs` replacement loop
(~calls 2–5). The model understood duplicative replacement in multiline mode before
editing; subsequent matcher reads supported but did not change the target file choice.

Turning point: failed structured insert on 2295 did not block export; 2209 test
rewrite masked weak issue coverage.

## Protocol Review And Blind Spots

Treatment protocol completed on both instances (intent segmentation, call review,
segment review). Protocol counts: 2209 — 44 calls / 4 segments; 2295 — 31 calls /
15 segments; all usable.

Blind spots (inherited from gen-1 pattern):

- Protocol can mark progress on workspace-green cargo after test-input simplification
  without checking issue-shaped test coverage.
- Operational `keep` on 2209 does not require patch similarity to gold or baseline
  fix shape.

## Mechanical Completion Versus Benchmark Usefulness

| claim | supported? |
| --- | --- |
| Treatment runner succeeded | yes |
| Non-empty exported patches | yes |
| Protocol complete | yes |
| Broad-harness artifact plausibly helps tool failures | plausible guard only |
| Ripgrep patches match issue intent | **uncertain** (2209 test shape; 2295 doc/indent defects) |
| Descendant benefit proven | no (campaign stopped at max_generations) |

## What Is Working

- Full treatment closure on both instances with ledger parity.
- Branch evaluation correctly compared against gen-1 parent treatment baseline.
- Broad-harness r3 evidence intake (bug doc + branch-eval reads) preceded a targeted
  guard edit; see gen-1 broad-r3 review.
- Same-file patch retry metrics improved on 2209 without increasing tool failures.

## What Is Not Working Yet

- Benchmark patches still use alternate algorithms and simplified tests versus issue
  descriptions (2209 lookaround; 2295 absolute-parent scenario).
- Doc-comment stripping and indentation damage on 2295 export persist across
  generations.
- Early cargo validation often hits focused manifests that do not cover edited crates.
- No oracle/MBE join to validate whether `code_item_lookup` guard materially helps
  descendant eval turns.

## Action Items

### Non-blockers

- Add adjudication field distinguishing test-expectation rewrites from production
  repair on 2209-style instances (observed in validation-audit +
  `expected_output_edit_candidates`).
- Track doc-comment / formatting regressions in patch export as patch-quality signals
  separate from operational metrics.

### Blockers

- None for mechanical admission of this branch.
