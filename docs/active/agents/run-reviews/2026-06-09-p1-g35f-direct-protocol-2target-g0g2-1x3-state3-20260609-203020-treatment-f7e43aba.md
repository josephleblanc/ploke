# Prototype 1 Gen-1 Treatment Child Review: branch-f7e43aba97c12538

Date: 2026-06-09
Campaign: `p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020`
Child node: `node-0dca4fe449780e85` (`status=succeeded`, generation 1)
Branch: `branch-f7e43aba97c12538` (`candidate_id=broad-harness-g1-01`)
Parent: `node-e41b4e1ef747bb15` (broad-harness base slot)
Model route: `google/gemini-3.5-flash` through `direct_google`
Treatment artifact: `sanitize_tool_args` micro-optimization in `crates/ploke-tui/src/tools/mod.rs`
(`derived_artifact_id=artifact:git-commit:dd9d0dc143fd9450570a0321175fd6c19f4c8b07`)

## Verdict

The gen-1 treatment child runner completed successfully. Both SWE-bench instances exported
non-empty patches with passed projection checks, branch evaluation returned `keep`, and
operational metrics improved versus the gen-0 baseline on both targets.

Mechanical success does not prove the broad-harness branch improved benchmark repair quality.
The admitted change is a ploke-tui allocation tweak unrelated to ripgrep logic; treatment
eval turns re-solved the same two ripgrep issues independently and produced patches that
differ substantially from baseline (24.8% and 47.2% text similarity). Benchmark usefulness
remains mixed:

- **`BurntSushi__ripgrep-2295`**: Real localization and production edits in
  `crates/ignore/src/dir.rs`, but an alternate algorithm shape, accidental doc-comment removal
  on `matched_ignore`, and a larger patch than baseline. Treat as **useful-but-alternate-shape**
  with a patch-quality warning.

- **`BurntSushi__ripgrep-2209`**: Real production rewrite in `crates/printer/src/util.rs`, but
  the added test is a simplified duplicate-line case (`a` → `z`), not the issue's lookaround
  scenario, and a later `non_semantic_patch` changed test inputs/expected outputs after a failing
  `cargo test`. Treat as **mechanically complete, benchmark usefulness uncertain** — same
  failure mode as the gen-0 baseline on this instance.

Treatment protocol closure is **partial** (`2209` complete, `2295` protocol missing). That is
an observability gap, not evidence the eval turn failed.

**Blockers:** none for runner completion or branch `keep`. No handoff to
`ploke-blocker-repair-loop` is required before recording this branch as mechanically admitted.

## Evidence Roots

- Parent campaign:
  `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020`
- Child node:
  `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/nodes/node-0dca4fe449780e85`
- Branch evaluation:
  `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/evaluations/branch-f7e43aba97c12538.json`
- Treatment closure:
  `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-treatment-branch-f7e43aba97c12538-1781064596896/closure-state.json`
- Treatment eval run roots:
  - `BurntSushi__ripgrep-2295`:
    `/home/brasides/.ploke-eval/instances/prototype1/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/treatments/branch-f7e43aba97c12538/instances/BurntSushi__ripgrep-2295/runs/run-1781064597832-structured-current-policy-3ecd3874`
  - `BurntSushi__ripgrep-2209`:
    `/home/brasides/.ploke-eval/instances/prototype1/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/treatments/branch-f7e43aba97c12538/instances/BurntSushi__ripgrep-2209/runs/run-1781064863280-structured-current-policy-7ed69954`
- Protocol root (partial):
  `/home/brasides/.ploke-eval/protocol/prototype1/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/treatments/branch-f7e43aba97c12538`
- Broad-harness origin review:
  [`2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-broad-base.md`](2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-broad-base.md)
- Baseline eval/protocol companion:
  [`2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-baseline-eval-protocol.md`](2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-baseline-eval-protocol.md)

Checked per run: `record.json.gz`, `agent-turn-trace.json`, `agent-turn-summary.json`,
`llm-full-responses.jsonl`, `validation-audit.json`, `benchmark-patch-projection.json`,
`multi-swe-bench-submission.jsonl`, `execution-log.json`, `config/ploke/proposals.json`.
Checked node/runtime: `node.json`, `runner-request.json`, `runner-result.json`,
`results/fadb339a-7384-415d-9ccf-10701753ca18.json`, runner stdout.
Verified suspicious patch content against persisted `fix_patch` in submission JSONL because
the per-node instance checkout was no longer present on disk at review time.

## Closure State

Child node (`node.json`, `runner-result.json`):

- `status=succeeded`, `disposition=succeeded`, `exit_code=0`
- `treatment_campaign_id=p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-treatment-branch-f7e43aba97c12538-1781064596896`
- Wall clock ~17 minutes (`created_at` 04:06:12Z → `updated_at` 04:23:00Z)

Treatment closure (`closure-state.json`, updated `2026-06-10T04:23:00Z`):

| layer | status | detail |
| --- | --- | --- |
| registry | complete | 2/2 mapped |
| eval | complete | 2/2 complete |
| protocol | partial | 1/2 full; `BurntSushi__ripgrep-2295` protocol missing |

Branch evaluation (`branch-f7e43aba97c12538.json`):

- `overall_disposition=keep`
- Compared instances: 2/2
- Mechanized reasons only (tool failure counts, same-file retry streaks); no oracle/MBE proof

## Execution Path

```text
child plan admission (broad-harness-g1-01)
  -> prototype1-runner on node-0dca4fe449780e85
  -> treatment campaign spawn (ploke-eval built from dd9d0dc1)
  -> per-instance agent-single-turn / run_benchmark_turn
  -> record.json.gz + validation-audit + MSB submission
  -> branch evaluation (operational metrics)
  -> protocol on 2209 only (2295 missing at closure time)
```

Evidence: both `execution-log.json` files show `role=treatment`,
`execution=agent-single-turn`, `command=run single agent`, model `google/gemini-3.5-flash`.
The treatment binary was built from the broad-harness derived commit carrying the
`sanitize_tool_args` `&str` change; ripgrep checkouts were separate instance targets under
the child node.

## Eval And Patch Output

### `BurntSushi__ripgrep-2295`

| metric | baseline | treatment |
| --- | ---: | ---: |
| tool calls | 35 | 22 |
| tool failures | 1 | 1 |
| patch bytes | 2261 | 5091 |
| agent wall time | — | 195s |

`benchmark-patch-projection.json`: passed, base `1d35859861fa4710cee94cf0e0b2e114b152b946`.

Persisted `fix_patch` (verified from submission JSONL):

- Production: rewrites parent-loop guard to run only while any of the four match slots is
  still unset; rebuilds relative path via ancestor `ends_with` / `strip_prefix` overlap before
  joining with `absolute_base()`.
- Quality defect: the `apply_code_edit` on `matched_ignore` removed the preceding doc comment
  and left mis-indented function header (`-    /// ...` removed, `+        fn matched_ignore`).
- Test: adds `absolute_parent_subdirectory_duplicate_components` via `non_semantic_patch` after
  failed `insert_rust_item`.

`proposals.json`: 2 `Applied` proposals (production edit + test patch).
`validation-audit.json`: final cargo is workspace `test` covering changed files; no fmt evidence;
flags mostly test-scoped edit requests (2/3).

Patch similarity to baseline fix: **0.248** (different shape from gen-0 baseline on the same issue).

### `BurntSushi__ripgrep-2209`

| metric | baseline | treatment |
| --- | ---: | ---: |
| tool calls | 59 | 43 |
| tool failures | 2 | 1 |
| patch bytes | 3141 | 3264 |
| agent wall time | — | 261s |

`benchmark-patch-projection.json`: passed, base `4dc6c73c5a9203c5a8a89ce2161feca542329812`.

Persisted `fix_patch` (verified from submission JSONL):

- Production (`util.rs`): replaces `replace_with_captures_at` loop with manual
  `captures_iter_at`, tracking `last_match` and extending tail through a multi-line-aware
  limit — a substantive alternate implementation, not the baseline shape.
- Test (`standard.rs`): adds `replacement_multi_line_duplicate` using matcher `r"a"` and
  replacement `b"z"`, expecting `1:z\n2:z\n` — a different scenario from issue #2209's
  lookaround replacement bug.
- Repair chain: `insert_rust_item` failed → `non_semantic_patch` added test → failing workspace
  `cargo test` (event 345, `ok=false`) → second `non_semantic_patch` simplified matcher inputs
  from `hello(?=\nworld)` / `WORLD` to `a` / `z` (`validation-audit.json`
  `expected_output_edit_candidates`).

`proposals.json`: 3 `Applied` proposals.
Final recorded cargo resolves to focused `grep` manifest, **not** covering changed printer paths
(`final_cargo_covers_changed_files=false`), despite earlier successful workspace tests.

Patch similarity to baseline fix: **0.472**.

## Oracle / MBE State

No oracle or MBE adjudication artifacts were joined to this treatment branch at review time.
Branch `keep` is operational-metric only (`tool_calls_failed`, `same_file_patch_retry_count`,
`same_file_patch_max_streak`, total tool-call count). Do not treat `oracle_eligible=true` in
branch metrics as benchmark proof.

## LLM And Tool Behavior

Trace audit summary (provider vs recorded parity perfect on both runs):

| instance | responses | provider calls | recorded calls | failed tools |
| --- | ---: | ---: | ---: | ---: |
| `2295` | 23 | 22 | 22 | 1 (`insert_rust_item`) |
| `2209` | 44 | 43 | 43 | 1 (`insert_rust_item`) |

Shared patterns:

- Early `request_code_context` + truncated `read_file` chains localized the right files before
  first production edit — real information success.
- Both runs recovered `insert_rust_item` transport failures with `non_semantic_patch`.
- Duplicate cargo requests were deduplicated in the audit (`duplicate_request` class) while
  still leaving model-visible successful validation on workspace or package scopes.
- All `read_file` completions carried truncated content (`truncated=True` in audit); the model
  still progressed, but post-truncation re-reads were frequent on `2209`.

Ledger parity: **0 missing provider call IDs** on both runs.

## Positive Examples And Adjudication Candidates

1. **2295 localization chain**
   - `request_code_context("ignore path subdirectories")` → reads of `dir.rs` and
     `pathutil.rs` → `apply_code_edit` on `matched_ignore` → package `ignore` tests.
   - Candidate field: *context retrieval found correct module before first edit*.

2. **2209 validation-driven repair**
   - Production edit on `Replacer::replace_all` → workspace `cargo test` failure visible →
     test patch → second test-input patch until green.
   - Candidate field: *failed validation led to follow-up edit*, but downgrade when the
     follow-up edits test expectations rather than production behavior.

3. **2209 protocol blind spot**
   - Final segment review marked `key_progress` on successful cargo after test-input rewrite.
   - Candidate field: *protocol over-credits test-expectation edits as hypothesis validation*.

## Trace Reconstruction

### `BurntSushi__ripgrep-2295`

```text
cargo check (focused grep manifest, ok)
  -> context search + dir.rs reads
  -> apply_code_edit matched_ignore parent-loop rewrite
  -> cargo test ignore (ok)
  -> insert_rust_item test (FAILED transport_failure)
  -> non_semantic_patch adds absolute_parent_subdirectory_duplicate_components
  -> workspace cargo test (ok, final)
```

Last point with enough information to act: after the first `dir.rs` reads (~call 8), before
production edit. The model had the parent-loop bug location; later reads were confirmatory.

Turning point: failed structured insert did not block export because `non_semantic_patch`
appended the test successfully.

### `BurntSushi__ripgrep-2209`

```text
cargo check (focused, ok)
  -> util.rs + matcher reads
  -> apply_code_edit Replacer::replace_all rewrite
  -> grep-printer tests (ok)
  -> standard.rs reads
  -> insert_rust_item test (FAILED)
  -> non_semantic_patch adds replacement_multi_line_duplicate (lookaround case)
  -> workspace cargo test (FAILED)
  -> non_semantic_patch rewrites test to a/z duplicate-line case
  -> workspace cargo test (ok)
  -> final focused grep cargo test (ok, weak scope)
```

Last point with enough information to act: after reading `util.rs` replacement loop (~call 6-7).
The model understood multi-line replacement semantics before editing.

Turning point: failing workspace test after first test patch triggered test-input tampering
rather than production repair — the same semantic risk flagged on the gen-0 baseline.

## Protocol Review And Blind Spots

`BurntSushi__ripgrep-2209` protocol completed (43/43 reviewed, 10/10 usable segments).
Sample segment reviews credit `non_semantic_patch` on `standard.rs` and final cargo success as
`key_progress` without flagging the expected-output rewrite (`hello/WORLD` → `a/z`).

`BurntSushi__ripgrep-2295` protocol artifacts are **absent** (`protocol_status=missing` in
treatment closure). Eval and patch export still succeeded; this is a post-eval adjudication gap,
not a turn failure.

## What Is Working

- Treatment runner lifecycle: node admission, dual-instance eval, branch evaluation, and
  `keep` disposition all completed with consistent authority records.
- Tool-call ledger parity on both instances (no missing provider IDs).
- Patch export and projection checks passed for both instances.
- Operational metric improvements are real (fewer total calls, fewer same-file retries on both
  instances; one fewer failed tool on `2209`).
- `insert_rust_item` failure recovery via `non_semantic_patch` worked on both runs.

## What Is Not Working Yet

- Branch merit signal measures transport/convergence metrics, not SWE correctness or relation
  to the admitted ploke-tui micro-optimization.
- `2209` repeats baseline-style test-expectation editing; validation audit flags it but branch
  evaluation ignores it.
- `2295` treatment protocol never ran to completion in closure accounting.
- Final cargo scope on `2209` remains weak (`grep` manifest) despite printer-file edits.
- No fmt/oracle evidence on either instance.
- Instance checkout directories were not retained for live re-verification post-run.

## Action Items

### Non-blockers

- Run or backfill protocol for treatment `BurntSushi__ripgrep-2295`
  (`run-1781064597832-structured-current-policy-3ecd3874`) so treatment closure protocol is not
  stuck at partial.
- Add branch-evaluation/adjudication field for `expected_output_edit_candidates` so `keep`
  cannot ignore test tampering on `2209`-class instances.
- Teach protocol segment review to downgrade `key_progress` when
  `validation-audit.patch_quality.expected_output_edit_candidates` is non-empty.
- Preserve or snapshot treatment instance checkouts until run review completes (currently
  `record present, manual join needed` via submission JSONL only).

### Blockers

- None identified for this child node. Runner succeeded, patches exported, branch evaluation
  `keep` is internally consistent with its operational metric contract.
