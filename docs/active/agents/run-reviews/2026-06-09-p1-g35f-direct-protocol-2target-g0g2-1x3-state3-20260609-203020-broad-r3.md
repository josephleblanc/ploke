# Run review: node-e41b4e1ef747bb15-r3 broad harness

## Short verdict

`node-e41b4e1ef747bb15-r3` is mechanically complete: the headless TUI trace finished with terminal state `applied`, a submitted-result JSON exists, and the candidate workspace is clean at commit `39c6bd67a469129830b6f964e2f665d86d8f5845`.

Benchmark usefulness is weak. The applied change refactors write-path preflight in `crates/ploke-tui/src/tools/mod.rs` to avoid double JSON deserialization on edit tools. That sits in the seeded graph neighborhood and compiles, but the model never inspected protocol artifacts, evaluation reports, or oracle evidence, and its model-visible validation ran focused `ploke-transform` cargo commands instead of the request-declared `cargo check -p ploke-eval`. No evidence ties this micro-optimization to improved Prototype 1 descendant performance.

**Risk level:** low for harness mechanics; medium for admission merit (patch may compile and admit without helping the benchmark).

## Evidence roots

- Request JSON: `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/messages/edit-harness-request/node-e41b4e1ef747bb15-r3.json`
- Request prompt: `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/messages/edit-harness-request/node-e41b4e1ef747bb15-r3.md`
- Submitted result JSON: `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/messages/edit-harness-result/node-e41b4e1ef747bb15-r3.json`
- Headless TUI record: `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/messages/edit-harness-result/node-e41b4e1ef747bb15-r3.headless-tui.json`
- Turn trace/summary: `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/messages/edit-harness-result/node-e41b4e1ef747bb15-r3.turn-live/agent-turn-{trace,summary}.json`
- Candidate workspace: `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/workspaces/edit-harness/node-e41b4e1ef747bb15-r3`
- Pre-child planning: `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/messages/pre-child-planning/node-e41b4e1ef747bb15.json`
- Parent node state: `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/nodes/node-e41b4e1ef747bb15/node.json`

## Execution path proved

This was a published broad-harness headless-TUI attempt on retry slot `r3`, not a benchmark eval turn:

```text
published edit-harness request r3
  -> headless ploke-tui adapter (google/gemini-3.5-flash, direct_google)
  -> model/tool trace
  -> non_semantic_patch proposal 0fdec5d1-d5b2-53ef-a00c-5ed0e3a38a3d
  -> approve/apply
  -> candidate commit 39c6bd67
  -> submitted edit-harness result JSON
  -> harness-side declared validation cargo check -p ploke-eval
```

Evidence:

- Request id `broad-harness-request:node-e41b4e1ef747bb15:r3`, model route `direct_google`, parent node `node-e41b4e1ef747bb15` (gen0, instance `BurntSushi__ripgrep-2209`).
- `node-e41b4e1ef747bb15-r3.headless-tui.json` terminal block: `terminal: applied`, proposal id `0fdec5d1-d5b2-53ef-a00c-5ed0e3a38a3d`, changed path `crates/ploke-tui/src/tools/mod.rs`.
- Workspace git: branch `prototype1-broad-broad-harness-request-node-e41b4e1ef747bb15-r3`, parent `f41dfe402f99e585d9eedfa36138792b84254c59`, clean working tree.

## Closure and live-run state

- Parent `node.json` reports `status: running` at review time; this r3 slot is a completed broad attempt, not the parent eval turn itself.
- No descendant eval, oracle, MBE, or branch-evaluation artifact was joined to this r3 slot at review time.
- Record status labels:
  - submitted result: `present`
  - headless TUI: `present`
  - turn-live trace: `record present, playback gap` (ends at staged patch completion; no `TurnFinished`, no applied follow-up event)
  - candidate commit hash: `record present, manual join needed` (not in submitted-result JSON)

## Eval, patch output, and submitted result

- Submitted-result JSON is present and records changed file `crates/ploke-tui/src/tools/mod.rs`.
- Verified patch against checkout (`git diff f41dfe40..39c6bd67 --stat`): one file, `38 insertions(+), 43 deletions(-)`.
- Patch summary: remove global `preflight_write_paths(name, &args, &ctx)` before the tool `match`; add per-arm `preflight_write_paths_direct(...)` after each write-tool deserialize (`ApplyCodeEdit`, `InsertRustItem`, `CreateFile`, `NsPatch`); rename helper to `preflight_write_paths_direct(tool_name, paths, ctx)` and delete `write_paths_from_args`.
- Stated reasoning in the patch request: eliminate double deserialization of tool arguments during write-path preflight.
- Harness validations array includes a post-attempt `cargo check -p ploke-eval` (`declared_validation_1_0`, exit 0, 73 warnings). That validation is not in the model-visible turn trace.

## LLM and tool behavior

Trace inventory from `agent-turn-trace.json` (50 tool requests, 49 completions, 2 failures, 100 total events):

| Tool | Requested | Completed | Failed |
|------|-----------|-----------|--------|
| `read_file` | 25 | 24 | 1 |
| `request_code_context` | 14 | 14 | 0 |
| `list_dir` | 7 | 7 | 0 |
| `cargo` | 2 | 2 | 0 |
| `code_item_lookup` | 1 | 0 | 1 |
| `non_semantic_patch` | 1 | 1 | 0 |

Model route: `google/gemini-3.5-flash` via `direct_google` / `aiplatform.googleapis.com`.

### Concrete trace chain

1. **Planning intake:** `list_dir .` then `read_file` on pre-child-planning JSON. Planner cited `tools/mod.rs` and scoped the edit neighborhood correctly.
2. **Missing prior-result read:** `read_file` on `edit-harness-result/node-e41b4e1ef747bb15.json` failed (`No such file or directory`). The model recovered by listing/reading parent node artifacts instead. This is expected for an r3-only parent slot with no base submitted result.
3. **Early validation off-contract:** `cargo check` and `cargo test` both succeeded against focused manifest `crates/ingest/ploke-transform/Cargo.toml`, not `-p ploke-eval`.
4. **Localization thrash:** fourteen `request_code_context` calls with terms like `descendant performance`, `benchmark`, `validate_tool_args`, `ToolDefinition`, `allowed_tool_names`, `enum ToolName`. Most hits were low-signal (for example `PerformanceMetrics` in `ploke-llm`, `BenchmarkFamily` in `ploke-records`, `allowed()` in `xtask/src/commands/check.rs`).
5. **Verified suspicious retrieval:** `request_code_context` for `validate_tool_args` returned `crates/test-utils/src/nodes.rs::args`, but checkout verification shows the real function at `crates/ploke-tui/src/tools/mod.rs:209`. The model eventually read `mod.rs` directly in line-range chunks and found `preflight_write_paths`.
6. **Lookup failure and recovery:** `code_item_lookup` for `process_tool` with `node_kind=function` failed (`Hint: retry with node_kind=method`). The model did not retry lookup; it continued with `read_file` on `mod.rs` lines 220–520, which exposed the pre-dispatch preflight block.
7. **Edit lifecycle:** `non_semantic_patch` first completed as staged only (`staged:1, applied:0`). Headless TUI events then record applied completion (`applied:1, partial:false`) and terminal `applied`. Turn-live trace stops at the staged completion — reviewers must join headless events or git state for the apply boundary.
8. **No model-visible ploke-eval check:** the turn trace contains no `cargo check -p ploke-eval`. Harness-side declared validation passed afterward.

**Last point with enough information to act:** after reading `mod.rs` around `process_tool` and seeing `preflight_write_paths(name, &args, &ctx)` before the write-tool arms, plus the per-tool deserialize blocks below. The model had sufficient local context to propose the refactor even though retrieval quality was poor.

## Mechanical completion versus benchmark usefulness

| Layer | r3 outcome |
|-------|------------|
| Mechanical completion | Yes — applied proposal, submitted result, clean commit |
| Request validation (model-visible) | No — only focused `ploke-transform` cargo |
| Request validation (harness) | Yes — post-apply `cargo check -p ploke-eval` exit 0 |
| Evidence-driven edit intent | Weak — no reads of evaluations, protocol artifacts, oracle, or history blocks |
| Benchmark-path proof | None — micro-optimization in TUI tool dispatch, not tied to descendant eval metrics |
| Graph restriction compliance | Yes — edit stayed in seeded `tools/mod.rs` neighborhood, outside protected core |

## Positive examples and adjudication candidates

1. **Planner → executor alignment:** pre-child planning named `tools/mod.rs`; r3 edited that file. Good citation chain for rubric field `evidence_citations_used`.
2. **Lookup failure → direct read recovery:** `code_item_lookup` failure on `process_tool` was followed by targeted `read_file` line ranges that surfaced the preflight pattern. Candidate field: `recovered_from_tool_failure`.
3. **Staged → applied lifecycle preserved in headless record:** headless `attempts[]` shows two recovered `tool_failed` entries plus final `applied` proposal. Useful for edit-lifecycle adjudication even though turn-live trace truncates early.
4. **Low-information retrieval tolerated:** repeated BM25 searches returned unrelated symbols while still marking `ok:true`. Candidate negative signal: `semantic_search_credit_without_localization`.

## Protocol/read-side blind spots

- `agent-turn-summary.json` reports `patch_artifact.applied: true` but `edit_proposals: []`, `expected_file_changes: []`, `any_expected_file_changed: false`. Real changed file must be joined from headless terminal or git.
- Turn-live trace lacks the applied `non_semantic_patch` completion and any `TurnFinished` event despite a completed attempt (`record present, playback gap`).
- Submitted-result JSON omits candidate commit hash and harness validation outcomes.
- Headless `attempts[]` records recovered tool failures, but summary surfaces do not expose that distinction cleanly to downstream protocol.

## What is working

- Broad-harness path produced a clean committed candidate from a staged proposal.
- Edit stayed inside the graph-restricted seed neighborhood and outside the protected eval core.
- Headless terminal record, git checkout, and submitted result agree on the changed file.
- Harness declared validation `cargo check -p ploke-eval` passed after apply.

## What is not working yet

- Model-visible validation did not exercise the request contract; focused cargo on an unrelated crate passed instead.
- Evidence listed in the request (evaluations, protocol artifacts, oracle, history blocks) was not consulted before editing.
- Semantic retrieval returned misleading matches; the model relied on brute-force reads.
- Turn-live playback truncates before apply completion, forcing manual joins for lifecycle review.

## Action items

### Non-blockers

1. **Validation-contract visibility:** Record whether model-visible cargo matched request-declared commands. Here the model ran focused `ploke-transform`; harness ran `ploke-eval` afterward.
2. **Retrieval quality signal:** Flag `request_code_context` hits whose top snippets are not in the target file/module when the model later localizes via `read_file`.
3. **Projection gap:** Include candidate commit hash, proposal id, and harness validation results in submitted-result JSON.
4. **Playback gap:** Extend turn-live export to include post-staging apply events and terminal finish for broad-harness attempts.

### Blockers

None for this r3 attempt review. Admission merit remains unproven; downstream selection/eval should treat this as a compile-clean but benchmark-unverified candidate unless descendant evidence says otherwise.
