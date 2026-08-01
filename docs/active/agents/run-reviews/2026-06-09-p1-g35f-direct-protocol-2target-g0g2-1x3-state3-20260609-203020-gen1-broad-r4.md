# Run review: node-0dca4fe449780e85-r4 gen-1 broad harness (with r2 failure contrast)

Date: 2026-06-09
Campaign: `p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020`
Parent node: `node-0dca4fe449780e85` (generation 1, branch `branch-f7e43aba97c12538`, `overall_disposition=keep`)
Retry slot: `r4` (separate published request; started after sibling `r2` infrastructure failure)
Inherited artifact: `artifact:git-commit:b5d2c10a873c7fbe272ed44031d19426bcfb3001`

## Short verdict

`node-0dca4fe449780e85-r4` is **mechanically complete**: headless TUI terminal state `applied`, submitted-result JSON present, and candidate workspace clean at commit `f1059e0b1707a8b206a91587638ce48ceca74aa9`.

Benchmark usefulness is **weak**. The applied change adds a 16-line `adapt_error` override on `RequestCodeContextGat` in `crates/ploke-tui/src/tools/request_code_context.rs`, attaching retry hints for missing `search_term` / “No query available” failures. Verified against checkout: the helper did not exist at `b5d2c10a`; the patch is syntactically correct and matches the staged `non_semantic_patch` diff byte-for-byte. It improves tool-error ergonomics, not descendant-eval throughput or ripgrep treatment metrics.

Contrast with sibling **`r2`**, which is **mechanically failed** despite three successfully applied edits: headless terminal `tool_failed` (`timed out waiting for BM25 readiness after applying proposal batch after 180s`), **no submitted-result JSON**, and a **dirty uncommitted workspace** (`b5d2c10a` HEAD with three modified `ploke-transform` files). r4 is not the harness-internal retry of the r2 request (different `request_hash`); it is a **later fanout slot** that started ~2 minutes after r2’s BM25 timeout and completed the full submit/commit path r2 could not.

**Risk level:** low for r4 harness mechanics; medium-low for admission merit (evidence-driven tool-error patch, unproven descendant benefit); **medium for r2 lifecycle accounting** (applied edits + diagnostics without submitted result or commit).

## Evidence roots

### r4 (primary review target)

- Request JSON: `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/messages/edit-harness-request/node-0dca4fe449780e85-r4.json`
- Submitted result JSON: `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/messages/edit-harness-result/node-0dca4fe449780e85-r4.json`
- Headless TUI record: `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/messages/edit-harness-result/node-0dca4fe449780e85-r4.headless-tui.json`
- Turn trace/summary: `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/messages/edit-harness-result/node-0dca4fe449780e85-r4.turn-live/agent-turn-{trace,summary}.json`
- Provider sidecar: `.../node-0dca4fe449780e85-r4.turn-live/llm-full-responses.jsonl` (`record absent`, 0 bytes)
- Candidate workspace: `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/workspaces/edit-harness/node-0dca4fe449780e85-r4`

### r2 (failure contrast)

- Request JSON: `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/messages/edit-harness-request/node-0dca4fe449780e85-r2.json`
- Submitted result JSON: **absent** (expected path `.../node-0dca4fe449780e85-r2.json` not written)
- Headless TUI record: `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/messages/edit-harness-result/node-0dca4fe449780e85-r2.headless-tui.json`
- Turn trace/summary: `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/messages/edit-harness-result/node-0dca4fe449780e85-r2.turn-live/agent-turn-{trace,summary}.json`
- Candidate workspace: `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/workspaces/edit-harness/node-0dca4fe449780e85-r2` (dirty, uncommitted)

### Shared parent context

- Parent runtime stream: `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/nodes/node-0dca4fe449780e85/streams/377cc8c8-8f30-4f79-8ec7-5c41393e1389/stdout.log`
- Parent base-slot review: [`2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-gen1-broad-base.md`](2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-gen1-broad-base.md)
- Parent r3 review: [`2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-gen1-broad-r3.md`](2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-gen1-broad-r3.md)

Checked per attempt: headless TUI terminal record, turn trace, submitted result presence/absence, workspace git state, harness validations array.
Verified suspicious r4 patch content against checkout (`git diff b5d2c10a..f1059e0b`) and against staged `non_semantic_patch` diff in turn trace.

## Execution path proved

Both slots used the published broad-harness headless-TUI path (`tui_adapter::run_headless_with_model`), not a benchmark eval turn:

```text
child-plan publication (broad-harness-request:node-0dca4fe449780e85:r4)
  -> headless ploke-tui adapter (google/gemini-3.5-flash, direct_google)
  -> model/tool trace (60 tool requests, 121 headless events)
  -> non_semantic_patch proposal 5526796e-fb89-58ba-b881-7d3efe968c80
  -> approve/apply
  -> candidate commit f1059e0b
  -> INVALID_MODEL_RESPONSE warning during finalize (non-terminal)
  -> submitted edit-harness result JSON
  -> harness-side declared validation cargo check -p ploke-eval
```

r2 followed the same entrypoint but terminated differently:

```text
child-plan publication (broad-harness-request:node-0dca4fe449780e85:r2)
  -> parallel fanout with base/r3 (~21:28:40 local)
  -> insert_rust_item + apply_code_edit batch (3 applied proposals on ploke-transform)
  -> post-apply BM25 reindex + ContentMismatch on edited functions.rs
  -> headless terminal tool_failed (180s BM25 readiness timeout)
  -> no candidate commit, no submitted result JSON
```

Timing from parent stream `377cc8c8.../stdout.log`:

| Slot | Prompt constructed (local) | Terminal outcome |
|------|---------------------------|------------------|
| r2 | 2026-06-09 21:28:40 | BM25 timeout ~21:40; `INVALID_MODEL_RESPONSE` ~21:40:08 |
| r4 | 2026-06-09 21:42:08 | commit + `INVALID_MODEL_RESPONSE` ~21:47:37 |

## Closure and live-run state

- Parent `node.json` reports `status: running` at review time; r4 is a completed broad attempt on the gen-1 parent.
- No descendant eval, oracle, MBE, or branch-evaluation artifact was joined to this r4 candidate at review time.
- Record status labels (r4):

| Surface | Status |
|---------|--------|
| submitted result | `present` |
| headless TUI | `present` |
| turn-live trace | `record present, playback gap` (`terminal_record` null in summary despite applied patch) |
| provider `llm-full-responses.jsonl` | `record absent` (0-byte file) |
| candidate commit hash | `record present, manual join needed` (not in submitted-result JSON) |

Record status labels (r2):

| Surface | Status |
|---------|--------|
| submitted result | **`record absent`** |
| headless TUI | `present` (terminal `tool_failed`) |
| turn-live trace | `record present, playback gap` |
| candidate commit | **`record absent`** (dirty workspace only) |
| applied proposals in headless | `present` (3 proposals marked applied) |

## r2 versus r4 comparison

| Dimension | r2 | r4 |
|-----------|----|----|
| Request hash | `04da8894…` | `fe2eb33b…` (distinct slot, not same-request retry) |
| Terminal state | `tool_failed` (BM25 180s) | `applied` |
| Submitted result | absent | present |
| Workspace | dirty @ `b5d2c10a`, 3 modified files | clean commit `f1059e0b` |
| Changed files | `ploke-transform` ×3 (`mod.rs`, `type_node.rs`, `functions.rs`) | `request_code_context.rs` |
| Edit tools | `insert_rust_item`, `apply_code_edit` | `non_semantic_patch` |
| Graph restriction | **violated** — ingest crate outside `tools/mod.rs` seed neighborhood | **compliant** — edit in seeded tools neighborhood |
| Headless tool requests | 67 (`read_file` 49, `request_code_context` 8) | 60 (`read_file` 46) |
| Harness `cargo check -p ploke-eval` | not reached (no submit) | exit 0, 73 warnings |
| Root failure mode | infrastructure: BM25 readiness after batch apply | none at harness level; finalize `INVALID_MODEL_RESPONSE` only |

## Eval, patch output, and submitted result (r4)

- Submitted-result JSON records changed file `crates/ploke-tui/src/tools/request_code_context.rs`.
- Verified patch (`git diff b5d2c10a..f1059e0b --stat`): one file, `16 insertions(+)`.
- Patch summary: add `RequestCodeContextGat::adapt_error` mapping domain UI errors and “No query available” internal errors to `ToolError` with explicit `search_term` retry hints.
- Prior state at `b5d2c10a`: no `adapt_error` on this tool (confirmed via checkout grep); `tool_call_error_matrix.md` lists the tool as missing per-tool error adaptation — the model read that matrix before editing.
- Harness validations in headless TUI:
  - Model-visible: focused `cargo check` / `cargo test` on `ploke-transform` (exit 0); `cargo test -p ploke-tui` failed (exit 101).
  - Harness declared: `cargo check -p ploke-eval` (`declared_validation_1_0`, exit 0). Not in model-visible turn trace.

## r2 failure reconstruction

**Concrete trace chain (r2):**

1. **Parallel evidence race:** first-turn `read_file` on `node-0dca4fe449780e85.json` (base slot result) failed with `No such file or directory` because base/r2/r3 fanout had not finished base submission yet.
2. **Recovery reads:** model read pre-child planning JSON, listed repo root, ran focused `ploke-transform` cargo check/test (both exit 0).
3. **Broad retrieval thrash:** eight `request_code_context` calls (`descendant`, `performance`, router URL symbols, etc.) with BM25-only index notes.
4. **Pivot outside neighborhood:** model listed `crates/ingest/ploke-transform`, read transform sources/benches, then staged batch `db_put_batch` optimization across three transform modules via `insert_rust_item` + `apply_code_edit`.
5. **Apply succeeded:** headless `attempts` array records three `applied` proposals (`e7e538dd…`, `98fb6353…`, `84a4b71f…`).
6. **Post-apply infrastructure failure:** parent stream logs `ContentMismatch` on `functions.rs` during embed reindex, then terminal error `timed out waiting for BM25 readiness after applying proposal batch after 180s`.
7. **Lifecycle gap:** no git commit, no submitted-result JSON, workspace left dirty — applied edits exist only as working-tree modifications.

**Last point r2 had enough information to act:** after reading `ploke-transform` transform pipeline code and passing focused transform tests (~line 687+ in r2 trace when benches directory was listed). The model chose a plausible performance hypothesis (batch Cozo puts) but outside the graph-restricted edit surface and without descendant-eval linkage.

## r4 trace reconstruction

**Concrete trace chain (r4):**

1. **Evidence available (post-base):** `read_file` on base slot submitted result succeeded (unlike r2’s early failure); model listed evaluations directory and read all three branch-evaluation JSON files (`branch-f7e43aba`, `branch-bc17b460`, `branch-3dce62110`).
2. **Tool-error matrix intake:** read `tool_call_error_matrix.md` showing `request_code_context` lacks `adapt_error`.
3. **Target localization:** read `request_code_context.rs` including the `No query available (search_term missing)` internal error path (~lines 230–330 in file reads).
4. **Single bounded edit:** `non_semantic_patch` staged `adapt_error` block; headless records `staged:1, applied:0` then later `applied:1` with proposal `5526796e-fb89-58ba-b881-7d3efe968c80`.
5. **Commit + submit:** git commit `f1059e0b` on branch `prototype1-broad-broad-harness-request-node-0dca4fe449780e85-r4`; submitted-result JSON written.
6. **Non-terminal finalize warning:** parent stream `INVALID_MODEL_RESPONSE` at ~21:47:37 after commit message logged — same pattern as gen-1 base/r3 finalize warnings, not a turn-aborting failure on this slot.

**Last point r4 had enough information to act:** after reading `tool_call_error_matrix.md` and the `No query available` branch in `request_code_context.rs`. The model had the exact missing `adapt_error` gap and a copyable pattern from sibling tools (`code_item_lookup`, `list_dir`, etc.).

## Oracle / MBE state

No oracle or MBE evidence was joined to r4 or r2. Request cited oracle guidance; neither trace shows `final_report.json` read.

## LLM and tool behavior

### r4

- 60 headless tool requests: 46 `read_file`, 8 `list_dir`, 3 `cargo`, 1 `non_semantic_patch`, 1 `code_item_lookup` (failed wrong struct name), 1 `request_code_context`.
- One early `read_file` failure on a mistyped evaluation path (`node-0dca4fe449780e85.json` under evaluations/) recovered via directory listing.
- Model did not reattempt the r2-style `ploke-transform` batch optimization; instead stayed in tools neighborhood and addressed a documented error-coverage gap.
- `cargo test -p ploke-tui` failed in model-visible trace; model still proceeded to patch and submit after focused transform checks passed.

### r2

- Higher exploration cost: 49 `read_file`, 12 `list_dir`, 8 `request_code_context`, 2 `apply_code_edit`, 1 `insert_rust_item`.
- Applied edits were real (verified dirty diff against `b5d2c10a`) but harness classified attempt as failure due to BM25 timeout, not edit rejection.
- Early missing-file reads were parallel-fanout timing artifacts, not model hallucination.

## Positive examples and adjudication candidates

1. **Matrix doc → targeted `adapt_error` (r4):** `tool_call_error_matrix.md` gap → read error path → `non_semantic_patch` with retry hints. Candidate field: `evidence_citations_used`.
2. **Branch evaluation cross-read before edit (r4):** all three gen-1 treatment branch JSONs read under same parent campaign. Candidate field: `consulted_eval_guidance`.
3. **r2 apply success hidden by infrastructure timeout:** three proposals applied but terminal `tool_failed` and no submit — strong negative lifecycle signal for admission accounting. Candidate field: `infrastructure_failure_after_apply`.
4. **Graph restriction bypass (r2):** batch transform rewrite outside seeded neighborhood without surface rejection. Candidate field: `graph_restriction_enforcement`.
5. **Parallel fanout evidence race (r2):** base submitted-result path missing at r2 start; r4 succeeded reading it later. Candidate field: `fanout_evidence_availability`.

## Mechanical completion versus benchmark usefulness

| Layer | r4 | r2 |
|-------|----|----|
| Mechanical completion | Yes — applied, submitted, committed | No — applied edits but failed terminal, no submit |
| Request validation (model-visible) | Partial — transform cargo ok; `ploke-tui` tests failed | Partial — transform cargo ok pre-edit; workspace tests failed |
| Request validation (harness) | Yes — post-apply `cargo check -p ploke-eval` | Not reached |
| Evidence-driven edit intent | Moderate — matrix + branch evals + base result; no protocol/history/oracle reads | Low — BM25 thrash then off-neighborhood transform batching |
| Benchmark-path proof | None — retry hints ≠ descendant performance | None — transform batch uncommitted and unadmitted |
| Graph restriction compliance | Yes | No |

## Protocol / read-side blind spots

- `agent-turn-summary.json` reports `terminal_record: null` for r4 despite headless terminal `applied` (same playback gap as base/r3).
- r2 headless records three `applied` attempts but negative terminal — playback must join attempts vs terminal manually.
- Submitted-result JSON omits candidate commit hash, proposal id, intermediate `ploke-tui` test failure, and harness validation outcomes.
- r2 dirty workspace is invisible to child-admission readers that only check submitted results.
- Empty `llm-full-responses.jsonl` on both slots prevents provider/recorded-tool parity audit.

## What is working

- r4 completed the full broad-harness contract path r2 could not: apply → commit → submit → declared validation.
- r4 edit stayed inside graph-restricted tools neighborhood and outside protected eval core.
- r4 model used durable campaign evidence (base result + evaluations + error matrix) rather than only BM25 thrash.
- r2 diagnostics preserved enough trace to distinguish **model apply success** from **harness infrastructure failure** (BM25 timeout).
- Later r4 slot could read base submitted result, showing fanout ordering matters for evidence paths.

## What is not working yet

- r2: BM25 readiness timeout after multi-file apply leaves dirty workspace and no submitted result despite applied proposals.
- r2: graph restriction not enforced when model edits `ploke-transform` far from `tools/mod.rs` seed.
- r2: parallel fanout exposes evidence paths before sibling slots finish submission.
- r4: patch improves tool-error messages, not descendant eval performance; weak admission merit.
- r4: model-visible validation did not exercise request-declared `ploke-eval` check; `ploke-tui` tests failed without blocking submit.
- Both slots: turn-live playback truncates finalize boundary; provider sidecar absent.

## Action items

### Non-blockers

1. **Alive bug — post-apply BM25 timeout without submit (r2):** headless terminal `tool_failed` after three applied proposals; workspace dirty @ `b5d2c10a`; no `node-0dca4fe449780e85-r2.json`. Tie to parent stream ContentMismatch + 180s BM25 wait. Preserve dirty workspace as repro artifact.
2. **Alive bug — applied-but-unsubmitted candidate accounting:** distinguish `applied` headless attempts from negative terminal for child-plan negative rows; r2 should not be classified as “no edit”.
3. **Alive bug — parallel fanout evidence race:** r2 first-turn read of base submitted-result path fails when base slot still running; consider deferring path exposure or stub availability in request digest.
4. **Adjudication signal — graph restriction bypass (r2):** batch `ploke-transform` edits applied without surface rejection despite `tool_neighborhood` policy seeded from `tools/mod.rs`.
5. **Playback gap — turn-live `terminal_record` null on applied r4:** same class as gen-1 base/r3; join headless terminal + git for lifecycle reviews.

### Blockers

None for campaign continuation on r4 mechanical grounds. r2 failure is an infrastructure/lifecycle blocker for **using r2 as an admitted child**, not for the parent loop advancing on r4.
