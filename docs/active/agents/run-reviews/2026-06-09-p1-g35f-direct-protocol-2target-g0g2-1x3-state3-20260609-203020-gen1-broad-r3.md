# Run review: node-0dca4fe449780e85-r3 gen-1 broad harness

Date: 2026-06-09
Campaign: `p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020`
Parent node: `node-0dca4fe449780e85` (generation 1, branch `branch-f7e43aba97c12538`, `overall_disposition=keep`)
Retry slot: `r3`
Inherited artifact: `artifact:git-commit:b5d2c10a873c7fbe272ed44031d19426bcfb3001` (treatment binary built from kept gen-1 child)

## Short verdict

`node-0dca4fe449780e85-r3` is mechanically complete: the headless TUI trace finished with terminal state `applied`, a submitted-result JSON exists, and the candidate workspace is clean at commit `a6cccca417f2edec44e49364526eb47457076dc4`.

Benchmark usefulness is **plausible but unproven**. The applied change adds an early guard in `crates/ploke-tui/src/tools/code_item_lookup.rs` that rejects `node_kind: impl` with a model-actionable error instead of reaching the Cozo schema failure documented in `docs/active/bugs/2026-06-06-code-item-lookup-impl-relation-missing-name-field.md`. The model read that bug doc, branch-evaluation JSON for sibling gen-1 treatments, and parent runner evidence before editing. That is stronger evidence use than typical gen-0 broad slots, but no descendant eval, oracle, or MBE artifact joins this r3 candidate yet, and the fix is a guardrail rather than the full impl-relation repair the bug doc describes.

**Risk level:** low for harness mechanics; medium-low for admission merit (targeted tool-failure mitigation with compile proof, descendant benefit still hypothetical).

## Evidence roots

- Request JSON: `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/messages/edit-harness-request/node-0dca4fe449780e85-r3.json`
- Request prompt: `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/messages/edit-harness-request/node-0dca4fe449780e85-r3.md`
- Submitted result JSON: `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/messages/edit-harness-result/node-0dca4fe449780e85-r3.json`
- Headless TUI record: `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/messages/edit-harness-result/node-0dca4fe449780e85-r3.headless-tui.json`
- Turn trace/summary: `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/messages/edit-harness-result/node-0dca4fe449780e85-r3.turn-live/agent-turn-{trace,summary}.json`
- Candidate workspace: `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/workspaces/edit-harness/node-0dca4fe449780e85-r3`
- Pre-child planning: `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/messages/pre-child-planning/node-0dca4fe449780e85.json`
- Parent node state: `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/nodes/node-0dca4fe449780e85/node.json`
- Parent base-slot result (sibling context): `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/messages/edit-harness-result/node-0dca4fe449780e85.json`
- Parent treatment review: [`2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-treatment-f7e43aba.md`](2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-treatment-f7e43aba.md)

Checked per attempt: headless TUI terminal record, turn trace/summary, submitted result, workspace git diff, harness validations array.
Verified suspicious patch content against checkout (`git diff b5d2c10a..a6cccca4`) and against staged `non_semantic_patch` diff in the turn trace (byte-identical guard block).

## Execution path proved

This was a published broad-harness headless-TUI attempt on retry slot `r3` under a gen-1 treatment parent, not a benchmark eval turn:

```text
child-plan publication (broad-harness-request:node-0dca4fe449780e85:r3)
  -> headless ploke-tui adapter (google/gemini-3.5-flash, direct_google)
  -> model/tool trace (71 tool requests on winning attempt)
  -> apply_code_edit failure (canon mismatch + staging ICE)
  -> non_semantic_patch proposal 23b16db0-8464-5b6d-b5f2-69172255e151
  -> approve/apply (attempt 4 of 4)
  -> candidate commit a6cccca4
  -> submitted edit-harness result JSON
  -> harness-side declared validation cargo check -p ploke-eval
```

Evidence:

- Request id `broad-harness-request:node-0dca4fe449780e85:r3`, parent `node-0dca4fe449780e85`, workspace seeded from treatment commit `b5d2c10a`.
- `node-0dca4fe449780e85-r3.headless-tui.json` terminal block: `terminal: applied`, proposal id `23b16db0-8464-5b6d-b5f2-69172255e151`, changed path `crates/ploke-tui/src/tools/code_item_lookup.rs`.
- Workspace git: branch `prototype1-broad-broad-harness-request-node-0dca4fe449780e85-r3`, parent `b5d2c10a873c7fbe272ed44031d19426bcfb3001`, clean working tree.

## Closure and live-run state

- Parent `node.json` reports `status: running` at review time; this r3 slot is a completed broad attempt on the gen-1 parent, not the parent treatment runner itself.
- No descendant eval, oracle, MBE, or branch-evaluation artifact was joined to this r3 candidate at review time.
- Record status labels:
  - submitted result: `present`
  - headless TUI: `present`
  - turn-live trace: `record present, playback gap` (`terminal_record` null in summary despite applied patch; trace ends at staged `non_semantic_patch` completion)
  - candidate commit hash: `record present, manual join needed` (not in submitted-result JSON)
  - protocol artifacts / history blocks: `record absent` in trace reads (evaluations directory and branch JSON were read; no joined protocol-artifact or sealed-history reads observed)
  - oracle / final_report: `not applicable` at review time (no attached oracle read in trace)

## Eval, patch output, and submitted result

- Submitted-result JSON is present and records changed file `crates/ploke-tui/src/tools/code_item_lookup.rs`.
- Verified patch against checkout (`git diff b5d2c10a..a6cccca4 --stat`): one file, `6 insertions(+)`.
- Patch summary: after `node_kind` parsing, reject `NodeKind::Impl` early with a domain UI error explaining that impl blocks lack a unique name and directing the model to use `node_kind: method`, `struct`, or `trait` instead. This prevents the downstream Cozo query that references non-existent `impl.name` (the failure mode in the open bug doc).
- Parent base slot (`node-0dca4fe449780e85.json`) changed `crates/ploke-tui/src/tools/error.rs`; r3 is a distinct retry candidate on the same inherited treatment workspace.
- Harness validations array includes post-attempt `cargo check -p ploke-eval` (`declared_validation_1_0`, exit 0, 73 warnings). That validation is not in the model-visible turn trace.

## Oracle / MBE state

No oracle or MBE evidence was joined to this r3 slot. The request cited oracle guidance, but the trace shows no `final_report.json` read.

## LLM and tool behavior

Trace inventory from `agent-turn-trace.json` on the winning attempt (71 tool requests, 69 completions, 3 failures, 143 total events):

| Tool | Requested | Completed | Failed |
|------|-----------|-----------|--------|
| `read_file` | 38 | 37 | 1 |
| `list_dir` | 19 | 19 | 0 |
| `request_code_context` | 9 | 9 | 0 |
| `cargo` | 3 | 3 | 0 |
| `apply_code_edit` | 1 | 0 | 2 |
| `non_semantic_patch` | 1 | 1 | 0 |

Model route: `google/gemini-3.5-flash` via `direct_google` / `aiplatform.googleapis.com`.

Headless harness recorded four attempts: attempts 0–2 ended `tool_failed`; attempt 3 ended `applied`.

Model-visible cargo (from headless `validations`):

| Command | OK | Manifest |
|---------|----|----------|
| `cargo check` | yes | `crates/ploke-protocol/Cargo.toml` |
| `cargo test` | yes | `crates/ploke-protocol/Cargo.toml` |
| `cargo test -p ploke-tui` | no (exit 101) | workspace root |
| `cargo check -p ploke-eval` | yes | workspace root (harness-only, not model-visible) |

## Trace reconstruction

### Concrete trace chain

1. **Evidence intake:** `list_dir` on `prototype1/evaluations`, then `read_file` on `branch-bc17b460acc98b0d.json`, `branch-f7e43aba97c12538.json`, and later `branch-3dce62110fd6c20b.json`. The model inspected sibling gen-1 branch metrics before choosing an edit target.
2. **Parent context:** reads of parent `node.json`, `runner-result.json`, treatment result JSON `fadb339a-7384-415d-9ccf-10701753ca18.json`, and `PROTOTYPE1_ADMISSION_FIX_CHANGELOG.md`.
3. **Missing self-result read:** `read_file` on `edit-harness-result/node-0dca4fe449780e85-r3.json` failed (`No such file or directory`) because the result file is the attempt output, not pre-existing evidence. Expected circular-reference behavior; the model continued via evaluations and bug docs.
4. **Bug localization:** `read_file` on `docs/active/bugs/2026-06-06-code-item-lookup-impl-relation-missing-name-field.md`, then targeted reads of `code_item_lookup.rs` and related tool modules. Verified: the bug doc's broken contract matches the pre-patch failure mode (Cozo `impl.name` schema error on `node_kind: impl`).
5. **Off-contract validation:** model-visible `cargo check` / `cargo test` succeeded on `ploke-protocol`, not `-p ploke-eval`. Mid-attempt `cargo test -p ploke-tui` failed (exit 101) but did not block apply.
6. **Edit lifecycle:** `apply_code_edit` targeting `crate::tools::code_item_lookup::CodeItemLookup::execute` failed twice (`No matching node found` for canon, then staging ICE). `non_semantic_patch` staged the guard block (`staged:1, applied:0` in turn trace). Headless attempt 3 records terminal `applied` with the same proposal id; git checkout confirms the guard landed at lines 169–173.
7. **No model-visible ploke-eval check:** turn trace contains no `cargo check -p ploke-eval`; harness declared validation passed afterward.

**Last point with enough information to act:** after reading the impl-lookup bug doc and `code_item_lookup.rs` around the `node_kind` parse block (~line 159). The model had the exact failure mode, the intended user-facing error shape from the bug write-up, and the insertion point before the Cozo lookup path.

## Mechanical completion versus benchmark usefulness

| Layer | r3 outcome |
|-------|------------|
| Mechanical completion | Yes — applied proposal, submitted result, clean commit |
| Request validation (model-visible) | No — focused `ploke-protocol` cargo; `ploke-tui` tests failed |
| Request validation (harness) | Yes — post-apply `cargo check -p ploke-eval` exit 0 |
| Evidence-driven edit intent | Moderate — evaluations, bug doc, and parent runner evidence consulted; protocol artifacts / history / oracle not consulted |
| Benchmark-path proof | None yet — guard may reduce `code_item_lookup` impl ICE recoverability failures, but no descendant eval measures the effect |
| Graph restriction compliance | Yes — edit in seeded `tools/mod.rs` neighborhood (`code_item_lookup.rs`), outside protected core |

## Positive examples and adjudication candidates

1. **Bug doc → targeted guard:** open bug doc describing `node_kind: impl` Cozo failure → read `code_item_lookup.rs` → `non_semantic_patch` inserting an early actionable error. Strong candidate field: `evidence_citations_used`.
2. **Branch evaluation cross-read:** model read `branch-f7e43aba`, `branch-bc17b460`, and `branch-3dce62110` JSON before editing under the same parent campaign. Candidate field: `consulted_eval_guidance`.
3. **`apply_code_edit` failure → patch recovery:** canon mismatch on `CodeItemLookup::execute` recovered via `non_semantic_patch` with identical diff to final git state. Candidate field: `recovered_from_tool_failure`.
4. **Headless multi-attempt accounting:** three recovered `tool_failed` attempts before final `applied`. Useful for edit-lifecycle adjudication even though turn-live trace truncates at staging.
5. **Negative signal — validation scope drift:** model passed focused `ploke-protocol` checks while `cargo test -p ploke-tui` failed and request-declared `ploke-eval` check ran only harness-side.

## Protocol / read-side blind spots

- `agent-turn-summary.json` reports `terminal_record: null` despite headless terminal `applied`.
- Turn-live trace ends at staged `non_semantic_patch` completion; applied boundary must be joined from headless events or git.
- Submitted-result JSON omits candidate commit hash, proposal id, intermediate `ploke-tui` test failure, and harness validation outcomes.
- Bug-doc fix is a guardrail; protocol adjudication on prior campaigns marked impl lookup recoverability as blocked — this patch may change recoverability classification but no replay proof was run during the broad attempt.

## What is working

- Broad-harness path produced a clean committed candidate from a recovered patch after `apply_code_edit` failure.
- Edit stayed inside the graph-restricted seed neighborhood and outside the protected eval core.
- Headless terminal record, git checkout, and submitted result agree on the changed file and patch bytes.
- Harness declared validation `cargo check -p ploke-eval` passed after apply.
- Model used durable campaign evidence (evaluations + bug doc) rather than only BM25 thrash.

## What is not working yet

- Model-visible validation did not exercise the request contract; focused `ploke-protocol` cargo passed while `ploke-tui` tests failed.
- Fix mitigates symptoms; it does not implement the bug doc's preferred impl-relation resolution path.
- No descendant eval/oracle join proves the guard improves ripgrep treatment outcomes.
- Turn-live playback truncates before apply completion, forcing manual joins for lifecycle review.
- Protocol artifacts, sealed history, and oracle paths listed in the request were not consulted.

## Action items

### Non-blockers

1. **Validation-contract visibility:** Record model-visible cargo scope versus request-declared commands. Here the model ran `ploke-protocol` check/test and a failing `ploke-tui` test; harness ran `-p ploke-eval` afterward.
2. **Bug-doc alignment signal:** When a broad edit directly cites an open bug doc, record whether the patch is a guardrail workaround or the documented structural fix — adjudication should not over-credit guardrails as full contract repair.
3. **Projection gap:** Include candidate commit hash, proposal id, per-attempt outcomes, and harness validation results in submitted-result JSON.
4. **Playback gap:** Extend turn-live export to include post-staging apply events and terminal finish for broad-harness attempts.
5. **Descendant proof gap:** Run treatment eval on candidate `a6cccca4` (or admit and measure) before treating impl-lookup guardrails as benchmark-useful.

### Blockers

None for this r3 attempt review. No handoff to `ploke-blocker-repair-loop` is required for mechanical admission recording. Downstream selection should treat this as compile-clean with plausible tool-failure mitigation but unproven descendant benefit until eval/oracle evidence exists.
