Verified surface: read-only campaign artifact inspection plus worktree identity check; not a live `prototype1-state` run.

**Run Report**
Campaign: `p1-smoke-broad-harness-1x3-20260519-1`  
Primary worktree: `/home/brasides/.ploke-eval/worktrees/p1-smoke-broad-harness-1x3-20260519-1`  
Parent branch: `prototype1-parent-p1-smoke-broad-harness-1x3-20260519-1-gen0`  
Benchmark: `multi_swe_bench_rust`, dataset `prototype1/ripgrep-burntsushi-ripgrep-2209`  
Model/provider: `inception/mercury-2` / `inception`

The trace evidence is incomplete as a live Prototype 1 run record. The campaign has real broad-harness request/result artifacts and two real headless TUI trace files, but I did not find a live `prototype1-state` trace, a History directory, or node completion records. `prototype1/nodes/node-a87394840086768d/node.json` still says `status: "running"`, `prototype1/scheduler.json` still has the gen0 node in `frontier_node_ids` with scheduler status `planned`, and `prototype1/transition-journal.jsonl` has only two rows: `parent_started` and a `cargo_target` resource measurement. I treat `closure-state.json` and `slice.jsonl` as run/protocol metadata only, not History authority.

There is also an artifact-shape mismatch: `closure-state.json` reports eval/protocol `complete`, while the Prototype 1 state artifacts do not show a completed parent/child transition. No `prototype1/evaluations` directory exists, even though the broad-harness result contract advertises it as available request evidence.

**Trace Inventory**
Real run artifacts:

| Artifact Class | Evidence |
|---|---|
| Campaign metadata | `campaign.json`, `closure-state.json`, `slice.jsonl` |
| Prototype run metadata | `prototype1/run-profile.toml`, `prototype1/run-profile.commitment.json`, `prototype1/scheduler.json`, `prototype1/transition-journal.jsonl` |
| Parent node metadata | `prototype1/nodes/node-a87394840086768d/node.json`, `runner-request.json` |
| Broad-harness requests | 9 JSON requests in `prototype1/messages/edit-harness-request/`, plus matching prompt `.md` files |
| Broad-harness results | 2 submitted result JSON files and 2 `.headless-tui.json` traces in `prototype1/messages/edit-harness-result/` |
| Candidate workspaces | 6 workspace directories under `prototype1/workspaces/edit-harness/` |

Archive/copy/source docs, not live attempt traces:

- Worktree `.orchestrator.archive.2026-05-12-egui-task-readability/.../*.report.md`
- Worktree `docs/active/agents/*.md` and `docs/active/bugs/*.md`
- Worktree source files and `.ploke/prototype1/parent_identity.json`

**Headless TUI Summary**
Across the two real headless traces: 153 unique tool requests, 10 `tool_failed` events, 9 proposal events, and both terminal states report `applied`.

| Trace | Terminal | Event Counts | Tool Requests | Main Pattern |
|---|---:|---:|---:|---|
| `node-a87394840086768d.headless-tui.json` | applied | 139 events, 66 tool requests, 66 tool completions, 4 failures, 2 proposal events | 28 `request_code_context`, 17 `read_file`, 15 `list_dir`, 2 `cargo`, 2 `apply_code_edit`, 1 `non_semantic_patch`, 1 `code_item_lookup` | Context/search churn, one outside-root artifact read, invalid read range, code-item miss, partial patch failure, then two `syn_parser` edits. Both cargo checks succeeded. |
| `node-a87394840086768d-r2.headless-tui.json` | applied | 189 events, 87 tool requests, 88 tool completions, 6 failures, 7 proposal events | 31 `read_file`, 23 `list_dir`, 20 `request_code_context`, 5 `non_semantic_patch`, 4 `cargo`, 3 `apply_code_edit`, 1 `code_item_lookup` | Heavy read/search churn, protected-path attempts, dependency edit attempt, repeated same-file repair attempts, and four failed `cargo check -p ploke-tree` runs. Terminal still reports applied. |

The r2 terminal state is the clearest failure signal. It changed only `crates/ploke-tree/src/store/fs.rs`, but every visible cargo completion in the trace is `ok:false` / `compile_failed`: first unresolved `rayon` after a protected `Cargo.toml` edit was denied, then repeated unclosed-delimiter errors after patch repair attempts. The trace still ended as `terminal: "applied"` because the adapter accepted an applied proposal, not because validation succeeded.

**Failure Analysis**
1. The largest framework/tool-contract failure is state ambiguity around edit success. A single edit call can emit multiple `tool_completed` events: staged completions such as `{"ok":true,"staged":1,"applied":0}` and later applied completions such as `{"applied":1,"ok":true}` share the same call id. In r2, the run also reaches terminal `applied` after compile failures. For trace review, `applied` means mutation reached the candidate workspace; it does not mean the candidate compiled, passed the contract checks, or was a valid successor.

2. Protected path policy is reactive and costly. The r2 trace attempted `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs` and `crates/ploke-tree/Cargo.toml`, then received protected-path denials. The model had already spent substantial context on a dependency-based Rayon idea, so the denial arrived after the plan was committed. This is a framework feedback problem as much as a model problem: protected surfaces should be predeclared in the prompt/tool schema or denied before the model designs around them.

3. The headless prompt advertises evidence paths outside the configured tool root. The first trace immediately tried `list_dir` on the campaign `prototype1` directory and got `path outside configured roots`, even though the request text told it that prior attempts, protocol artifacts, History blocks, and evaluations lived there. Later r2 made many absolute reads under the candidate workspace, some accepted and some failed depending on configured roots. This contract mismatch drives avoidable read/search churn.

4. `request_code_context` dominated both traces: 28 calls in the first trace and 20 in r2. Context mode was `Off`, `included_rag_parts` was 0, and the prompt estimated only about 400 input tokens. The traces therefore show the model trying to reconstruct the workspace through repeated tool calls rather than starting from useful benchmark or run context.

5. `code_item_lookup` failures were ordinary tool-target misses, but the resulting recovery path was noisy. The first trace missed `name_impl` as a function and was told to retry as a method; r2 missed `LoopCommand` in `crates/ploke-eval/src/cli.rs`. These are model/tool targeting failures, not evidence that the underlying source was unavailable.

6. Model behavior is also poor in r2. It pursued a Rayon parallelization idea, tried to edit a protected manifest, then damaged `FsRunStore::load` with repeated non-semantic patches containing restore markers such as `START RESTORE`. That is not just a tool-contract issue. The tool framework made the bad path easier to continue, but the candidate edits themselves were low-quality.

7. The first trace is healthier mechanically but still weak semantically. It changed `type_to_string` in two `syn_parser` visitor files from `split_whitespace().collect::<Vec<&str>>().join(" ")` to `split_whitespace().join(" ")`, and cargo check/test for `syn_parser` succeeded. That is a plausible micro-optimization, but the trace does not show live descendant performance evidence, History admission, or selection evidence tying it to the campaign goal.

**Smallest Verification Commands**
These commands support the claims without dumping whole trace files:

```bash
find /home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1 -maxdepth 4 -type f | sort | sed -n '1,220p'
find /home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/messages/edit-harness-result -maxdepth 1 -type f -printf '%f\n' | sort
jq '{terminal, attempt_count:(.attempts|length), event_count:(.events|length)}' /home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/messages/edit-harness-result/*.headless-tui.json
jq -r '.events[] | .kind' /home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/messages/edit-harness-result/node-a87394840086768d-r2.headless-tui.json | sort | uniq -c | sort -nr
jq -r '.events[] | select(.kind=="tool_request") | .tool' /home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/messages/edit-harness-result/node-a87394840086768d-r2.headless-tui.json | sort | uniq -c | sort -nr
jq -r '([.events[] | select(.kind=="tool_request") | {key:.call_id, value:.tool}] | from_entries) as $m | .events[] | select(.kind=="tool_failed") | [$m[.call_id], (.error.preview // "")] | @tsv' /home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/messages/edit-harness-result/node-a87394840086768d-r2.headless-tui.json
jq '{node_id, generation, status, instance_id}' /home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/nodes/node-a87394840086768d/node.json
jq '{frontier_node_ids, completed_node_ids, failed_node_ids, nodes}' /home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/scheduler.json
wc -l /home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/transition-journal.jsonl /home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/slice.jsonl
find /home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1 -maxdepth 5 -type f \( -name '*prototype1-state*' -o -name 'agent-turn-*.json' -o -name 'agent-turn-*.jsonl' \) | sort
```

**Open Uncertainty**
I did not find a live `prototype1-state` trace, History blocks, or completed node records, so this note cannot determine whether a parent process later admitted, evaluated, or selected a successor through the real History path. Based on the artifacts present, the safest conclusion is: the campaign contains two real headless TUI attempts and useful failure evidence, but not enough live Prototype 1 state evidence to call the run healthy.
