# Run review: node-e41b4e1ef747bb15-r2 broad harness

## Short verdict

`node-e41b4e1ef747bb15-r2` is mechanically complete: the headless TUI trace finished with terminal state `applied`, a submitted-result JSON exists, eleven proposals were applied across nine files, and the candidate workspace is clean at commit `677d409b281ddf27d2ab43eae51bfdc18e82d97a`.

Benchmark usefulness remains weak and the stated fix is likely incomplete. The model spread a `suppress_emission: bool` flag through `ToolCallParams` and several write-tool execution paths to gate duplicate `ToolCallCompleted` / failure event-bus emissions, then repaired compile failures driven by model-visible `cargo test -p ploke-tui` output. It never edited the seeded anchor `crates/ploke-tui/src/tools/mod.rs` (where `preflight_write_paths` still runs unchanged), did not consult protocol/oracle/evaluation evidence, and ran early model-visible cargo against focused `ploke-embed` rather than the request-declared `cargo check -p ploke-eval`. Harness declared validation failed ten times mid-attempt before passing on the eleventh check; reviewers must not treat `terminal: applied` alone as compile proof.

**Risk level:** low for harness mechanics; medium-high for admission merit (multi-file behavioral change with unproven benchmark benefit and incomplete coverage of the stated preflight-duplicate-emission hypothesis).

## Evidence roots

- Request JSON: `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/messages/edit-harness-request/node-e41b4e1ef747bb15-r2.json`
- Request prompt: `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/messages/edit-harness-request/node-e41b4e1ef747bb15-r2.md`
- Submitted result JSON: `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/messages/edit-harness-result/node-e41b4e1ef747bb15-r2.json`
- Headless TUI record: `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/messages/edit-harness-result/node-e41b4e1ef747bb15-r2.headless-tui.json`
- Turn trace/summary: `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/messages/edit-harness-result/node-e41b4e1ef747bb15-r2.turn-live/agent-turn-{trace,summary}.json`
- Candidate workspace: `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/workspaces/edit-harness/node-e41b4e1ef747bb15-r2`
- Pre-child planning: `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/messages/pre-child-planning/node-e41b4e1ef747bb15.json`
- Parent node state: `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/nodes/node-e41b4e1ef747bb15/node.json`

## Execution path proved

This was a published broad-harness headless-TUI attempt on retry slot `r2`, not a benchmark eval turn:

```text
published edit-harness request r2
  -> headless ploke-tui adapter (google/gemini-3.5-flash, direct_google)
  -> model/tool trace (98 tool requests)
  -> apply_code_edit failures on ToolCallParams canon
  -> eleven non_semantic_patch proposals (staged then applied)
  -> incremental declared_validation cargo check -p ploke-eval (10 fail, 1 pass)
  -> candidate commit 677d409b
  -> submitted edit-harness result JSON
```

Evidence:

- Request id `broad-harness-request:node-e41b4e1ef747bb15:r2`, model route `direct_google`, parent node `node-e41b4e1ef747bb15` (gen0, instance `BurntSushi__ripgrep-2209`).
- `node-e41b4e1ef747bb15-r2.headless-tui.json` terminal block: `terminal: applied`, final proposal id `c8f61618-6dc9-5660-acee-7b72ee428953`, eleven `applied_proposal_ids`, nine unique changed paths.
- Workspace git: branch `prototype1-broad-broad-harness-request-node-e41b4e1ef747bb15-r2`, parent `f41dfe402f99e585d9eedfa36138792b84254c59`, clean working tree at `677d409b`.

## Closure and live-run state

- Parent `node.json` reports gen0 parent for instance `BurntSushi__ripgrep-2209`; this r2 slot is a completed broad attempt, not the parent eval turn itself.
- No descendant eval, oracle, MBE, or branch-evaluation artifact was joined to this r2 slot at review time.
- Record status labels:
  - submitted result: `present`
  - headless TUI: `present`
  - turn-live trace: `record present, playback gap` (`terminal_record` null; summary `edit_proposals: []` despite applied multi-file patch)
  - candidate commit hash: `record present, manual join needed` (not in submitted-result JSON)
  - harness validation history: `record present, manual join needed` (ten intermediate `compile_failed` rows before final success)

## Eval, patch output, and submitted result

- Submitted-result JSON is present and lists nine changed files under `return_evidence.change_summary.changed_files`.
- Verified patch against checkout (`git diff f41dfe40..677d409b --stat`): nine files, `70 insertions(+), 18 deletions(-)`.
- Patch theme:
  - Add `suppress_emission: bool` to `ToolCallParams` in `rag/utils.rs` and gate `tool_call_failed*` plus completion sends when true.
  - Mirror the flag on `CreateFileCtx`, set `suppress_emission: true` in several tool `execute` paths (`create_file`, `code_edit`, `ns_patch`, `insert_rust_item`), and add `adapt_error` retry hints on `CreateFile` / semantic edit tools.
  - Guard `ToolCallCompleted` sends in `rag/tools.rs` and `create_file.rs` with `if !tool_call_params.suppress_emission`.
  - Update test helpers in `rag/tests/apply_code_edit_tests.rs`; add `..` destructuring fixes in `rag/tools.rs` and `create_file.rs` after compile failures.
  - Minor analogous guards in `cargo.rs` and `request_code_context.rs`.
- **Verified suspicious gap:** checkout grep shows `crates/ploke-tui/src/tools/mod.rs` still calls `preflight_write_paths(name, &args, &ctx)` at line 261 with no `suppress_emission` integration. The multi-file patch therefore does not cover the preflight dispatch path the model cited in its planning message, despite planner scope naming `mod.rs`.
- Harness declared validation `cargo check -p ploke-eval` (`declared_validation_1_0`) failed with `compile_failed` / exit 101 on ten intermediate checks (error counts 4→2→1), then succeeded on the eleventh (exit 0, 73 warnings). Independent re-check at review time also exits 0.
- Model-visible `cargo test -p ploke-tui` initially failed with `E0027` pattern-missing-field errors on `suppress_emission`; a focused rerun of `validate_and_sanitize_tool_call_strips_suffix_tokens` later passed.

## LLM and tool behavior

Trace inventory from headless TUI events (98 tool requests, 105 completions, 4 failures):

| Tool | Requested | Completed | Failed |
|------|-----------|-----------|--------|
| `read_file` | 53 | 51 | 2 |
| `request_code_context` | 21 | 21 | 0 |
| `non_semantic_patch` | 11 | 21 | 0 |
| `list_dir` | 7 | 7 | 0 |
| `cargo` | 5 | 5 | 0 |
| `apply_code_edit` | 1 | 0 | 2 |

Model route: `google/gemini-3.5-flash` via `direct_google` / `aiplatform.googleapis.com`.

Note: `non_semantic_patch` completions exceed requests because each patch emits staged then applied completion events.

### Concrete trace chain

1. **Early validation off-contract:** `cargo check` succeeded against focused manifest `crates/ingest/ploke-embed/Cargo.toml`, not `-p ploke-eval`.
2. **Planning intake:** `read_file` on pre-child-planning JSON. Planner cited `tools/mod.rs` and the tool-neighborhood scope.
3. **Missing prior-result reads (expected):** `read_file` on `edit-harness-result/node-e41b4e1ef747bb15.json` and `runner-result.json` failed (`No such file or directory`). Model recovered via `list_dir` / `read_file` on `node.json` and `runner-request.json`.
4. **No protocol/oracle/evidence reads:** no reads of evaluation reports, protocol artifacts, history blocks, or oracle JSON were observed in the headless trace.
5. **Semantic edit failure → patch cascade:** `apply_code_edit` targeting `crate::rag::utils::ToolCallParams` failed twice (canon not found, then staging ICE). Model switched to eleven sequential `non_semantic_patch` proposals across the tool/RAG neighborhood.
6. **Verified compile-driven repair:** after mid-attempt patches, model-visible `cargo test -p ploke-tui` returned `E0027: pattern does not mention field suppress_emission` in `rag/tools.rs:960`. Model read the failing region, then patched destructuring to use `..` in `rag/tools.rs` and `create_file.rs`. Follow-up declared validations stepped error counts down until pass — a real tool-output → repair chain.
7. **Edit lifecycle:** each `non_semantic_patch` first completed as staged only (`staged:1, applied:0`); headless `attempts[]` records matching `applied` entries. Turn-live summary reports `patch_artifact.applied: true` but empty `edit_proposals` and null `terminal_record` — reviewers must join headless events or git.
8. **No model-visible final ploke-eval check:** turn trace cargo calls used focused embed scope or `ploke-tui` tests; harness ran `-p ploke-eval` afterward.

**Last point with enough information to act:** after reading `rag/tools.rs` around the unconditional `ToolCallCompleted` send (~line 355) and seeing parallel patterns in `create_file.rs`. The model had enough local context to propose `suppress_emission`, even though it never validated duplicate-emission behavior in `mod.rs` preflight or protocol diagnostics.

## Mechanical completion versus benchmark usefulness

| Layer | r2 outcome |
|-------|------------|
| Mechanical completion | Yes — applied proposals, submitted result, clean commit |
| Request validation (model-visible) | Partial — `ploke-tui` tests used; no model-visible `-p ploke-eval` |
| Request validation (harness) | Yes — eventual `cargo check -p ploke-eval` exit 0 after ten failures |
| Evidence-driven edit intent | Weak — no protocol/oracle/eval reads; hypothesis not verified against preflight path |
| Benchmark-path proof | None — event-suppression refactor, not tied to descendant eval metrics |
| Graph restriction compliance | Mostly — edits stay in tool/RAG neighborhood seeded from `tools/mod.rs`, outside protected core; anchor `mod.rs` itself untouched |
| Stated fix completeness | Questionable — `mod.rs` preflight unchanged despite duplicate-emission rationale |

## Positive examples and adjudication candidates

1. **Compile failure → targeted repair:** `cargo test -p ploke-tui` `E0027` output led directly to `..` destructuring patches. Strong candidate field: `validation_output_drove_repair`.
2. **apply_code_edit failure → non_semantic_patch recovery:** semantic edit and staging ICE were followed by a patch series that eventually compiled. Candidate field: `recovered_from_tool_failure`.
3. **Incremental apply with harness revalidation:** ten failed declared checks preserved in `validations[]` show the harness did not silently accept a broken intermediate state. Useful for admission adjudication even though terminal state alone hides the failure streak.
4. **Incomplete hypothesis execution:** model articulated preflight duplicate-emission fix but left `mod.rs` untouched — candidate negative signal: `edit_intent_not_fully_applied`.

## Protocol/read-side blind spots

- `agent-turn-summary.json` reports `patch_artifact.applied: true` but `edit_proposals: []`, `expected_file_changes: []`, `any_expected_file_changed: false`, and `terminal_record: null`.
- Submitted-result JSON omits candidate commit hash, proposal ids, and harness validation failure/success history.
- Headless terminal `applied` does not surface the ten intermediate `compile_failed` declared validations; downstream readers can over-credit compile safety.
- `non_semantic_patch` staged/applied pairs inflate completion counts relative to provider request counts.

## What is working

- Broad-harness path produced a clean committed candidate from eleven staged/applied proposals.
- Edits stayed outside the protected eval core and within the broader tool/RAG neighborhood.
- Headless terminal record, git checkout, and submitted result agree on nine changed files.
- Model used real compile/test output to finish the patch series; final harness and review-time `cargo check -p ploke-eval` both pass.

## What is not working yet

- Stated duplicate-emission fix does not modify `tools/mod.rs` preflight dispatch.
- Model-visible validation did not exercise the request contract early; focused `ploke-embed` cargo passed instead.
- Evidence listed in the request (evaluations, protocol artifacts, oracle, history blocks) was not consulted.
- Semantic retrieval (`request_code_context`) returned BM25-only hits with typed-graph unavailable notes; localization relied on brute-force reads.
- Turn-live playback truncates lifecycle detail, forcing manual joins for apply boundaries and validation history.

## Action items

### Non-blockers

1. **Validation-contract visibility:** Record whether model-visible cargo matched request-declared commands and whether harness validation failed before final success. Here the model ran focused `ploke-embed` early and `ploke-tui` tests mid-attempt; harness ran `-p ploke-eval` ten failing times then once successfully.
2. **Edit-intent completeness signal:** Flag attempts whose assistant rationale names a file/symbol path that never appears in the verified git diff (here `mod.rs` / `preflight_write_paths`).
3. **Projection gap:** Include candidate commit hash, proposal ids, per-attempt validation outcomes, and intermediate compile failures in submitted-result JSON.
4. **Playback gap:** Extend turn-live export to include multi-proposal apply sequences, declared-validation rows, and terminal finish for broad-harness attempts.

### Blockers

None for this r2 attempt review. Admission merit remains unproven; downstream selection/eval should treat this as compile-clean but benchmark-unverified, and should weigh the incomplete preflight coverage before preferring r2 over narrower sibling attempts.
