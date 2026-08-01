# Deep Review: node-81bd26e4b6222d08 Applied Headless TUI Traces

Scope: four applied headless TUI traces for campaign `p1-smoke-broad-harness-1x3-20260519-1`.

Important boundary: `terminal: applied` proves the headless TUI accepted and applied a proposal in the harness workspace. It does not by itself prove validation success, History admission, successor selection, or benchmark improvement. I verified run-state separately where noted.

Useful path shorthand used below:

```bash
BASE=/home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1
RESULT=$BASE/messages/edit-harness-result
REQUEST=$BASE/messages/edit-harness-request
```

## Trace: node-81bd26e4b6222d08.headless-tui.json

Terminal state: `applied`.

Counts:

- Attempts/events: 2 attempt records, 114 events.
- Event counts: 55 `tool_request`, 56 `tool_completed`, 1 `tool_failed`, 1 `proposal`, 1 `turn`.
- Tool requests: `request_code_context` 17, `read_file` 16, `list_dir` 15, `cargo` 5, `apply_code_edit` 1, `code_item_lookup` 1.
- Final proposal: `282747e1-ba83-5eb7-a0f3-b6d1b4c72a6d`, 1 edit.

Changed path:

- `crates/ingest/syn_parser/src/parser/visitor/mod.rs`
- Durable git evidence exists as commit `663248e8`: 11-line diff, replacing `par_bridge()` over `crate_contexts.values()` with a collected `Vec` plus `par_iter()`.

Validation visible in trace:

- `cargo check -p syn_parser`: success, exit 0, 6 warnings.
- `cargo test -p syn_parser`: failed, exit 101, 23 warnings.
- `cargo test -p syn_parser parse_workspace_defaults_to_all_members`: success.
- `cargo test -p syn_parser parse_workspace`: failed.
- `cargo test -p syn_parser phase2`: failed.
- The requested contract validation was `cargo check -p ploke-eval` plus `cargo test -p ploke-eval edit_surface`; those exact checks were not the visible validation surface in this trace.

Model behavior:

- The model found a plausible parser hot path and made a small source edit aimed at Rayon overhead.
- It repeatedly inspected context and ran several `syn_parser` validation probes.
- It did not repair the failing broad package/test-filter validation before accepting the proposal.

Framework/tool-contract failures:

- One `read_file` failed against a missing prior node artifact: `nodes/node-a87394840086768d/runner-result.json`.
- This is mostly a missing-artifact/outside-assumption issue, not a model patch-quality issue.

Credible candidate?

- Weak. The edit is small and durable, but the trace contains failing validation tests. Later run-state confirms `node-81bd26e4b6222d08` was selected in generation 1 and later had 3 scored descendants with best descendant score `-500` and improvement `0`; that is History evidence for the base node, not proof that this trace improved the benchmark.

Smallest verifying commands:

```bash
jq '{terminal, attempts_len:(.attempts|length), events_len:(.events|length)}' "$RESULT/node-81bd26e4b6222d08.headless-tui.json"
jq '[.events[].kind] | group_by(.) | map({key:.[0], count:length})' "$RESULT/node-81bd26e4b6222d08.headless-tui.json"
jq -r '.events[] | select(.kind=="tool_request") | .tool' "$RESULT/node-81bd26e4b6222d08.headless-tui.json" | sort | uniq -c
git show --stat --oneline --no-renames 663248e8 -- crates/ingest/syn_parser/src/parser/visitor/mod.rs
git show --unified=3 --no-renames 663248e8 -- crates/ingest/syn_parser/src/parser/visitor/mod.rs | sed -n '1,80p'
jq '.entries[] | select(.core.payload.kind=="selection_decision") | .core.payload.metrics.candidates[]? | select(.candidate|contains("node-81bd26e4b6222d08")) | {candidate, descendant_count:.imp_at_k.descendant_count, scored_descendant_count:.imp_at_k.scored_descendant_count, best_descendant_score:.imp_at_k.best_descendant_score, improvement:.imp_at_k.improvement}' "$BASE/history/blocks/segment-000000.jsonl"
```

## Trace: node-81bd26e4b6222d08-r2.headless-tui.json

Terminal state: `applied`.

Counts:

- Attempts/events: 5 attempt records, 177 events.
- Event counts: 85 `tool_request`, 86 `tool_completed`, 2 `tool_failed`, 3 `proposal`, 1 `turn`.
- Tool requests: `request_code_context` 42, `read_file` 22, `list_dir` 14, `cargo` 3, `non_semantic_patch` 3, `code_item_lookup` 1.
- Applied proposals: 3 single-file proposals.

Changed paths:

- `crates/ploke-core/tool_text/apply_code_edit.md`
- `crates/ploke-core/tool_text/insert_rust_item.md`
- `crates/ploke-core/tool_text/create_file.md`
- Durable git evidence exists as commit `df57aee1`: 3 markdown tool-text files changed, 15 insertions, 3 deletions.

Validation visible in trace:

- `cargo check --lib -p ploke-eval`: success twice, exit 0, 316 warnings.
- `cargo check --lib -p ploke-core`: success, exit 0.
- No visible `cargo test -p ploke-eval edit_surface` run.

Model behavior:

- The model did not make a runtime performance change. It changed tool descriptions to steer future model tool choice toward semantic edits and new-file insertion.
- It spent very heavily on `request_code_context` relative to the size of the final change: 42 requests for 3 short documentation edits.
- The patch is coherent as tool-contract prompt tuning, but it is indirect for a descendant performance benchmark.

Framework/tool-contract failures:

- One `code_item_lookup` failed looking for `TurnOutcome` in `crates/ploke-eval/src/record.rs`.
- One `read_file` failed because the model tried to read `$BASE/AGENTS.md`, outside the configured workspace roots.
- These are tool-use/navigation failures, not evidence that the final markdown edits are syntactically invalid.

Credible candidate?

- Not credible as a direct performance candidate. It compiles because it changes markdown tool text, but no runtime path, benchmark path, or edit-surface test outcome proves a performance improvement. At most, it is a harness/tool-instruction experiment.

Smallest verifying commands:

```bash
jq '{terminal, attempts_len:(.attempts|length), events_len:(.events|length)}' "$RESULT/node-81bd26e4b6222d08-r2.headless-tui.json"
jq '[.events[].kind] | group_by(.) | map({key:.[0], count:length})' "$RESULT/node-81bd26e4b6222d08-r2.headless-tui.json"
jq -r '.events[] | select(.kind=="tool_request") | .tool' "$RESULT/node-81bd26e4b6222d08-r2.headless-tui.json" | sort | uniq -c
git show --stat --oneline --no-renames df57aee1 -- crates/ploke-core/tool_text/apply_code_edit.md crates/ploke-core/tool_text/insert_rust_item.md crates/ploke-core/tool_text/create_file.md
git show --unified=2 --no-renames df57aee1 -- crates/ploke-core/tool_text/apply_code_edit.md crates/ploke-core/tool_text/insert_rust_item.md crates/ploke-core/tool_text/create_file.md | sed -n '1,140p'
```

## Trace: node-81bd26e4b6222d08-r3.headless-tui.json

Terminal state: `applied`.

Counts:

- Attempts/events: 15 attempt records, 182 events.
- Event counts: 75 `tool_request`, 85 `tool_completed`, 5 `tool_failed`, 16 `proposal`, 1 `turn`.
- Proposal churn: 16 proposal events but only 10 unique proposal IDs; terminal applied proposal `aa697cea-c920-54a9-8147-5363c5859acc`.
- Tool requests: `read_file` 25, `request_code_context` 18, `list_dir` 10, `cargo` 9, `non_semantic_patch` 7, `apply_code_edit` 5, `code_item_lookup` 1.

Changed path:

- Terminal/result summary reports `crates/ploke-rag/src/core/mod.rs`.
- Current durable git evidence is weak: no `node-81bd26e4b6222d08-r3` commit was found by `git log --all --grep='broad-harness-request:node-81bd26e4b6222d08:r3'`, the r3 workspace is clean, and `git diff 2b4d3c62..HEAD -- crates/ploke-rag/src/core/mod.rs` is empty. The trace says a workspace mutation happened, but I did not find a committed artifact for it.

Validation visible in trace:

- `cargo check -p ploke-rag`: failed four times with 33, 28, 27, and 27 errors before passing.
- Later checks passed: `cargo check -p ploke-rag`, workspace `cargo check`, `cargo check --all-features`, `cargo check -p ploke-mbe --tests`, and final `cargo check -p ploke-rag`.
- The first compiler failure included duplicate import `Arc` (`E0252`) in `crates/ploke-rag/src/core/mod.rs`.
- No visible `cargo test -p ploke-eval edit_surface` run.

Model behavior:

- The model entered a same-file repair loop around `ploke-rag/src/core/mod.rs`, repeatedly staging one-file patches and recovering from compile failures.
- It eventually produced a compile-clean state in the trace, but only after substantial churn.
- It appears to have tried to change manifest/dependency shape during repair, then backed away after a protected-path denial.

Framework/tool-contract failures:

- `apply_code_edit` failed with no matching semantic node for `canon=crate::core`.
- `apply_code_edit` also reported an internal staging failure.
- `non_semantic_patch` was denied on protected `crates/ploke-rag/Cargo.toml`.
- `code_item_lookup` was called on a non-`.rs` path.
- `list_dir` was called on a file path.
- This trace mixes model misuse with real framework/tool-contract rough edges: the semantic edit target for module-level `core/mod.rs` failed, and recovery fell back to repeated non-semantic patching.

Credible candidate?

- Not credible from durable artifacts. The trace has a successful final cargo surface, but I could not verify a committed r3 artifact or current workspace diff corresponding to the reported change. Treat it as applied-in-trace only unless another run-state artifact identifies the r3 patch.

Smallest verifying commands:

```bash
jq '{terminal, attempts_len:(.attempts|length), events_len:(.events|length)}' "$RESULT/node-81bd26e4b6222d08-r3.headless-tui.json"
jq '[.events[].kind] | group_by(.) | map({key:.[0], count:length})' "$RESULT/node-81bd26e4b6222d08-r3.headless-tui.json"
jq '[.events[] | select(.kind=="proposal") | .id] | {total:length, unique:(unique|length), ids:unique}' "$RESULT/node-81bd26e4b6222d08-r3.headless-tui.json"
jq -r '.events[] | select(.kind=="tool_request") | .tool' "$RESULT/node-81bd26e4b6222d08-r3.headless-tui.json" | sort | uniq -c
git log --all --grep='broad-harness-request:node-81bd26e4b6222d08:r3' --oneline --max-count=20
git -C "$BASE/workspaces/edit-harness/node-81bd26e4b6222d08-r3" diff --stat 2b4d3c62..HEAD -- crates/ploke-rag/src/core/mod.rs
```

## Trace: node-81bd26e4b6222d08-r9.headless-tui.json

Terminal state: `applied`.

Counts:

- Attempts/events: 12 attempt records, 152 events.
- Event counts: 66 `tool_request`, 71 `tool_completed`, 4 `tool_failed`, 10 `proposal`, 1 `turn`.
- Proposal churn: 10 proposal events, 8 unique proposal IDs; terminal applied proposal `1db18575-b25a-51e1-bf27-6b89cf0a7e7d`.
- Tool requests: `read_file` 25, `request_code_context` 15, `list_dir` 15, `non_semantic_patch` 9, `cargo` 1, `apply_code_edit` 1.

Changed path:

- `crates/ploke-rag/src/lib.rs`
- Durable git evidence exists as commit `99645b2f`: `BM25_TIMEOUT_MS` increased from `250` to `1000`; `BM25_RETRY_BACKOFF_MS` increased from `[50, 100]` to `[200, 400]`.

Validation visible in trace:

- `cargo check -p ploke-rag`: success, exit 0, 6 warnings.
- No visible workspace check, all-features check, or `cargo test -p ploke-eval edit_surface` run in this trace.

Model behavior:

- The model targeted BM25 timeout/backoff constants, likely interpreting prior failures as search/index readiness or latency problems.
- The final patch is tiny and compile-clean, but it increases wait/backoff values. That may improve reliability under slow BM25 conditions, but it is not inherently a performance improvement and could increase worst-case latency.
- Tool churn is still high for a 4-line constant edit.

Framework/tool-contract failures:

- One `read_file` failed on missing `nodes/node-81bd26e4b6222d08-r9/runner-result.json`.
- `non_semantic_patch` was denied on protected `crates/ploke-eval/src/runner.rs`.
- `apply_code_edit` failed with no matching semantic node for `canon=crate::lib` in `crates/ploke-rag/src/lib.rs`, followed by an internal staging failure.
- The semantic-edit failure on a crate root/module file again pushed the model toward non-semantic patching.

Credible candidate?

- Weak but durable. It has a committed artifact and `ploke-rag` compile success. The performance hypothesis is unproven and directionally suspicious because it raises timeout/backoff values. It should require benchmark evidence before being treated as beneficial.

Smallest verifying commands:

```bash
jq '{terminal, attempts_len:(.attempts|length), events_len:(.events|length)}' "$RESULT/node-81bd26e4b6222d08-r9.headless-tui.json"
jq '[.events[].kind] | group_by(.) | map({key:.[0], count:length})' "$RESULT/node-81bd26e4b6222d08-r9.headless-tui.json"
jq '[.events[] | select(.kind=="proposal") | .id] | {total:length, unique:(unique|length), ids:unique}' "$RESULT/node-81bd26e4b6222d08-r9.headless-tui.json"
jq -r '.events[] | select(.kind=="tool_request") | .tool' "$RESULT/node-81bd26e4b6222d08-r9.headless-tui.json" | sort | uniq -c
git show --stat --oneline --no-renames 99645b2f -- crates/ploke-rag/src/lib.rs
git show --unified=3 --no-renames 99645b2f -- crates/ploke-rag/src/lib.rs | sed -n '1,80p'
```

## Cross-Trace Findings

- All four traces end with `terminal: applied`, but only base, r2, and r9 have durable commits I found by grep/log. r3 is applied in the trace but not verified as a committed artifact.
- The visible validation surfaces do not consistently match the request contract. The contract asks for `cargo check -p ploke-eval` and `cargo test -p ploke-eval edit_surface`; only r2 ran `ploke-eval` check, and none visibly ran the edit-surface tests.
- `request_code_context` churn is high, especially r2 with 42 calls for markdown-only edits.
- The semantic edit tools struggle with crate/module-root targets (`crate::core`, `crate::lib`), causing fallback to non-semantic patch loops.
- Protected-path behavior worked: attempted edits to `Cargo.toml` and `crates/ploke-eval/src/runner.rs` were denied before mutation.
- Outside-root/missing-artifact reads appear in multiple traces and should be treated separately from patch quality.
- Only the base node has run-state evidence under `nodes/` and `history/blocks` in this pass. I did not find node/evaluation artifacts for r2, r3, or r9 under the bounded file searches.

Smallest run-state checks:

```bash
find "$BASE/nodes" -maxdepth 2 -type f -path '*node-81bd26e4b6222d08*' -printf '%P\n' | sort | head -n 80
find "$BASE/evaluations" -maxdepth 3 -type f -path '*node-81bd26e4b6222d08*' -printf '%P\n' | sort | head -n 80
jq '{node_id, branch_id, generation, status, disposition, exit_code, recorded_at}' "$BASE/nodes/node-81bd26e4b6222d08/runner-result.json"
jq '.entries[] | select(.core.payload.kind=="selection_decision") | {subject:.core.subject.value, selected_candidate:.core.payload.selected_candidate.value, output_refs:.core.output_refs, decision_candidate:(.core.payload.decision.candidate_node_id // null), selected_branch:(.core.payload.decision.selected_branch_id // null)}' "$BASE/history/blocks/segment-000000.jsonl"
```
