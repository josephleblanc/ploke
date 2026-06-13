# Run review: node-e41b4e1ef747bb15 base broad harness

## Short verdict

`node-e41b4e1ef747bb15` (primary slot `base`) is mechanically complete: the headless TUI trace finished with terminal state `applied`, a submitted-result JSON exists, and the candidate workspace is clean at commit `dd9d0dc143fd9450570a0321175fd6c19f4c8b07`.

Benchmark usefulness is weak. The applied change is an 8-line micro-optimization in `crates/ploke-tui/src/tools/mod.rs` that makes `sanitize_tool_args` return `&str` instead of allocating a `String`, shifting one `.to_string()` call into `validate_and_sanitize_tool_call`. The edit sits in the seeded graph neighborhood and compiles, but the model never consulted protocol artifacts or oracle evidence, listed evaluation/history paths were absent on disk, and model-visible validation ran focused `ploke-embed` cargo commands instead of the request-declared `cargo check -p ploke-eval`. No evidence ties this allocation tweak to improved Prototype 1 descendant performance.

**Risk level:** low for harness mechanics; medium for admission merit (patch may compile and admit without helping the benchmark).

## Evidence roots

- Request JSON: `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/messages/edit-harness-request/node-e41b4e1ef747bb15.json`
- Request prompt: `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/messages/edit-harness-request/node-e41b4e1ef747bb15.md`
- Submitted result JSON: `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/messages/edit-harness-result/node-e41b4e1ef747bb15.json`
- Headless TUI record: `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/messages/edit-harness-result/node-e41b4e1ef747bb15.headless-tui.json`
- Turn trace/summary: `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/messages/edit-harness-result/node-e41b4e1ef747bb15.turn-live/agent-turn-{trace,summary}.json`
- Candidate workspace: `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/workspaces/edit-harness/node-e41b4e1ef747bb15`
- Pre-child planning: `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/messages/pre-child-planning/node-e41b4e1ef747bb15.json`
- Parent node state: `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020/prototype1/nodes/node-e41b4e1ef747bb15/node.json`

## Execution path proved

This was a published broad-harness headless-TUI attempt on the primary slot `base`, not a benchmark eval turn:

```text
published edit-harness request (base slot)
  -> headless ploke-tui adapter (google/gemini-3.5-flash, direct_google)
  -> model/tool trace (83 tool requests)
  -> apply_code_edit failures (invalid canon + staging ICE)
  -> non_semantic_patch proposal 0069bc56-e112-575e-9ca2-5eac2c3f2034
  -> approve/apply
  -> candidate commit dd9d0dc1
  -> submitted edit-harness result JSON
  -> harness-side declared validation cargo check -p ploke-eval
```

Evidence:

- Request id `broad-harness-request:node-e41b4e1ef747bb15`, model route `direct_google`, parent node `node-e41b4e1ef747bb15` (gen0, instance `BurntSushi__ripgrep-2209`).
- `node-e41b4e1ef747bb15.headless-tui.json` terminal block: `terminal: applied`, proposal id `0069bc56-e112-575e-9ca2-5eac2c3f2034`, changed path `crates/ploke-tui/src/tools/mod.rs`.
- Workspace git: commit `dd9d0dc1` with message `prototype1 broad harness result broad-harness-request:node-e41b4e1ef747bb15`, parent `f41dfe402f99e585d9eedfa36138792b84254c59`, clean working tree.

## Closure and live-run state

- Parent `node.json` reports gen0 parent for instance `BurntSushi__ripgrep-2209`; this base slot is a completed broad attempt, not the parent eval turn itself.
- No descendant eval, oracle, MBE, or branch-evaluation artifact was joined to this base slot at review time.
- Record status labels:
  - submitted result: `present`
  - headless TUI: `present`
  - turn-live trace: `record present, playback gap` (167 events with null-kind tail entries; summary `terminal_record` is null despite applied patch)
  - candidate commit hash: `record present, manual join needed` (not in submitted-result JSON)

## Eval, patch output, and submitted result

- Submitted-result JSON is present and records changed file `crates/ploke-tui/src/tools/mod.rs`.
- Verified patch against checkout (`git diff f41dfe40..dd9d0dc1 --stat`): one file, `4 insertions(+), 4 deletions(-)`.
- Patch summary:
  - Change `sanitize_tool_args` return type from `String` to `&str`, returning trimmed subslices instead of allocating.
  - Add `.to_string()` at the single call site in `validate_and_sanitize_tool_call` that stores into `sanitized.function.arguments`.
  - Leave `process_tool` call site using the `&str` return directly (line 245), which is the intended hot-path win.
- Stated model reasoning: eliminate redundant `String` allocation during tool sanitization in `process_tool` and `validate_and_sanitize_tool_call`.
- Harness validations array in headless TUI includes post-attempt `cargo check -p ploke-eval` (`declared_validation_1_0`, exit 0, 73 warnings). That validation is not in the model-visible turn trace.

## LLM and tool behavior

Trace inventory from headless TUI events (83 tool requests, 80 completions, 4 failures):

| Tool | Requested | Completed | Failed |
|------|-----------|-----------|--------|
| `read_file` | 44 | 43 | 1 |
| `list_dir` | 18 | 18 | 0 |
| `request_code_context` | 16 | 16 | 0 |
| `cargo` | 2 | 2 | 0 |
| `code_item_edges` | 1 | 0 | 1 |
| `apply_code_edit` | 1 | 0 | 2 |
| `non_semantic_patch` | 1 | 1 | 0 |

Model route: `google/gemini-3.5-flash` via `direct_google` / `aiplatform.googleapis.com`.

### Concrete trace chain

1. **Early validation off-contract:** `cargo check` and `cargo test` both succeeded against focused manifest `crates/ingest/ploke-embed/Cargo.toml`, not `-p ploke-eval`.
2. **Planning intake:** `read_file` on pre-child-planning JSON. Planner cited `tools/mod.rs` and scoped the edit neighborhood correctly, but also cited the not-yet-written `edit-harness-result/node-e41b4e1ef747bb15.json`.
3. **Missing self-result read (expected):** `read_file` on `edit-harness-result/node-e41b4e1ef747bb15.json` failed (`No such file or directory`). The model recovered by listing `messages/`, reading edit-harness-request md/json, and reading parent node artifacts (`node.json`, `runner-request.json`).
4. **Absent evidence paths:** `list_dir` on `prototype1/evaluations` and `prototype1/history/blocks` returned `exists: false` with empty entries. No protocol-artifact reads were observed in the headless trace.
5. **Localization thrash:** sixteen `request_code_context` calls with terms like `validate_and_sanitize_tool_calls`, `process_tool`, `RagService`, `eval_core_surface_root`. BM25 hits were mostly low-signal (for example `validate_and_reorder` in `ploke-embed`, `BroadTui` in `ploke-records`, `process_file` in `ploke-io`).
6. **Verified suspicious lookup failure:** `code_item_edges` for `process_tool` in `crates/ploke-tui/src/tools/mod.rs` with `node_kind=function` failed (`No code item named process_tool found`). Checkout verification shows the symbol exists as `pub(crate) async fn process_tool` at line 235 of `mod.rs`. This is a graph-index or lookup-tool gap, not absence of the function.
7. **Semantic edit failure → patch fallback:** `apply_code_edit` targeting `crate::tools::sanitize_tool_args` and `crate::tools::validate_and_sanitize_tool_call` failed twice (`Invalid 'canon': method targets must look like crate::module::Type::method`, then internal staging ICE). The model switched to `non_semantic_patch` with a unified diff matching the verified git patch.
8. **Edit lifecycle:** `non_semantic_patch` first completed as staged only (`staged:1, applied:0`). Headless `attempts[]` and terminal block record final `applied` proposal `0069bc56`. Turn-live summary reports `patch_artifact.applied: true` but `terminal_record` fields are null — reviewers must join headless events or git state for the apply boundary.
9. **No model-visible ploke-eval check:** the turn trace contains no `cargo check -p ploke-eval`. Harness-side declared validation passed afterward.

**Last point with enough information to act:** after reading `mod.rs` around `sanitize_tool_args` and `validate_and_sanitize_tool_call` (via repeated line-range `read_file` calls following failed retrieval). The model had sufficient local context to propose the `&str` return refactor even though semantic lookup and `apply_code_edit` both failed.

## Mechanical completion versus benchmark usefulness

| Layer | base outcome |
|-------|--------------|
| Mechanical completion | Yes — applied proposal, submitted result, clean commit |
| Request validation (model-visible) | No — only focused `ploke-embed` cargo |
| Request validation (harness) | Yes — post-apply `cargo check -p ploke-eval` exit 0 |
| Evidence-driven edit intent | Weak — no protocol/oracle reads; listed eval/history dirs absent |
| Benchmark-path proof | None — micro-allocation tweak in TUI tool dispatch, not tied to descendant eval metrics |
| Graph restriction compliance | Yes — edit stayed in seeded `tools/mod.rs` neighborhood, outside protected core |

## Positive examples and adjudication candidates

1. **Planner → executor alignment:** pre-child planning named `tools/mod.rs`; base edited that file. Good citation chain for rubric field `evidence_citations_used`.
2. **apply_code_edit failure → non_semantic_patch recovery:** semantic edit canon rejection and staging ICE were followed by a patch that matched the verified checkout diff. Candidate field: `recovered_from_tool_failure`.
3. **Missing-result read recovery:** failed read of the self-result path was followed by listing messages and reading the edit-harness request instead of aborting.
4. **Low-information retrieval tolerated:** repeated BM25 searches returned unrelated symbols while still marking `ok:true`. Candidate negative signal: `semantic_search_credit_without_localization`.

## Protocol/read-side blind spots

- `agent-turn-summary.json` reports `patch_artifact.applied: true` but `edit_proposals: []`, `expected_file_changes: []`, `any_expected_file_changed: false`, and `terminal_record` is null.
- Turn-live trace has 167 events but tail entries lack `kind`/`tool` fields; applied completion must be joined from headless TUI or git.
- Submitted-result JSON omits candidate commit hash and harness validation outcomes (those live in headless TUI `validations[]`).
- Pre-child planning cites `edit-harness-result/...json` as evidence even though that file is written only after the attempt completes.

## What is working

- Broad-harness path produced a clean committed candidate from a staged proposal after semantic edit failures.
- Edit stayed inside the graph-restricted seed neighborhood and outside the protected eval core.
- Headless terminal record, git checkout, and submitted result agree on the changed file.
- Harness declared validation `cargo check -p ploke-eval` passed after apply.

## What is not working yet

- Model-visible validation did not exercise the request contract; focused cargo on `ploke-embed` passed instead.
- Evidence listed in the request (evaluations, history blocks, protocol artifacts, oracle) was not materially consulted; two listed directories did not exist.
- Semantic retrieval and `code_item_edges` failed to locate `process_tool` despite it being present in the target file.
- `apply_code_edit` rejected free-function canon targets that `non_semantic_patch` could apply cleanly.
- Turn-live playback and submitted-result projection require manual joins for apply boundary and validation proof.

## Action items

### Non-blockers

1. **Validation-contract visibility:** Record whether model-visible cargo matched request-declared commands. Here the model ran focused `ploke-embed`; harness ran `ploke-eval` afterward.
2. **Graph lookup gap:** Investigate why `code_item_edges` cannot resolve `process_tool` at `crates/ploke-tui/src/tools/mod.rs:235` while `read_file` succeeds.
3. **apply_code_edit canon gap:** Free-function targets like `crate::tools::sanitize_tool_args` fail canon validation but succeed via `non_semantic_patch`; consider clearer tool guidance or canon support.
4. **Projection gap:** Include candidate commit hash, proposal id, and harness validation results in submitted-result JSON.
5. **Pre-child planning citation:** Do not cite the self-result path as evidence before the attempt writes it.

### Blockers

None for this base attempt review. Admission merit remains unproven; downstream selection/eval should treat this as a compile-clean but benchmark-unverified candidate unless descendant evidence says otherwise.
