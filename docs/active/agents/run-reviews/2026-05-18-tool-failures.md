Verified surface: read-only campaign artifact inspection plus source inspection; not tested.

**Run Report**
Campaign: `p1-broad-batch-admission-20260518-2`  
Benchmark: `multi_swe_bench_rust`, dataset `prototype1/ripgrep-burntsushi-ripgrep-2209`  
Model/provider: `inception/mercury-2` / `inception`  
Closure status: `eval=complete`, `protocol=complete`, `20/20` calls reviewed, `8` usable segments, `0` mismatched or missing segments.

There are no literal `agent-turn-*.json` files in this campaign. The trace material is embedded in `prototype1/messages/edit-harness-result/*.headless-tui.json`.

**Trace Summary**
Across the five headless TUI traces: `351` tool requests, `334` completions, `45` failures.

| Trace | Terminal | Tool Counts | Main Failure Pattern |
|---|---:|---:|---|
| `node-01c9e8fdc70e3ee8.headless-tui.json` | applied | `79/71/14` | Repeated protected-path edits: `Cargo.toml`, `.cargo/config.toml`, `xtask/Cargo.toml`, `crates/common/Cargo.toml`; then `apply_code_edit` staging failures. |
| `node-01c9e8fdc70e3ee8-r2.headless-tui.json` | timed out, 900s | `75/73/8` | Invalid `read_file` ranges, protected `operational_metrics.rs`, failed `insert_rust_item`; lots of search/read churn without useful convergence. |
| `node-01c9e8fdc70e3ee8-r5.headless-tui.json` | applied | `65/64/6` | Outside-root path reads, `code_item_lookup` misses, protected `crates/ploke-eval/Cargo.toml`, failed canon lookup in `file_hash.rs`. |
| `node-01c9e8fdc70e3ee8-r7.headless-tui.json` | applied | `77/74/12` | Repeated protected manifest edits: `xtask/Cargo.toml`, root `Cargo.toml`, `crates/ploke-core/Cargo.toml`; applied only after substantial churn. |
| `node-25c81ff9f15162e0.headless-tui.json` | timed out, 900s | `55/52/5` | 29 `request_code_context` calls, then `code_item_lookup` missing `module_path`, then `apply_code_edit` canon/static-item failures. No valid patch reached. |

**Likely Framework-Induced Failures**
The top issue is success ambiguity. In [tui_adapter.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:471), `ToolCallCompleted` is recorded before the adapter settles and applies staged edits. The final applied state is checked later around [tui_adapter.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:665). That means the model can see a completed tool call while the actual mutation is still `staged`, not `applied`.

Second, protected-path policy is reactive. The model repeatedly spends tool turns trying denied manifest/config paths. These should be preflighted and fed back once as a hard boundary, not rediscovered through many failed edit calls.

Third, tool contracts are inconsistent. `code_item_lookup` rejected missing `module_path`, while its hint points toward `show_module_tree`; the traces then fall into `request_code_context` loops. `apply_code_edit` also reports method-shaped canon requirements and then emits generic staging/internal failures when the target is a static/module item.

Fourth, context tools dominate. The timed-out `node-25...` trace made 29 `request_code_context` calls. `request_code_context` can infer a missing search term from the last user message in [request_code_context.rs](/home/brasides/code/ploke/crates/ploke-tui/src/tools/request_code_context.rs:149), which is useful interactively but risky for trace attribution and loop discipline.

**Improvements**
1. Split edit states into explicit `ToolStaged`, `ToolApplyStarted`, `ToolApplied`, and `ToolApplyFailed` records. Keep `ToolCallCompleted` for tool execution, but do not let it imply mutation success.

2. Add path-policy preflight before tool execution. For protected manifests/configs, return one structured denial with allowed alternatives, then suppress repeated identical attempts in the same trace.

3. Fix `code_item_lookup` schema/help consistency: either make `module_path` optional in the real validator, or require it in the schema and expose a real module-tree discovery path.

4. Make `apply_code_edit` failures more actionable: distinguish invalid target shape, static item unsupported, no node found, and staging failure. Avoid the duplicate “invalid_format” plus “internal compiler error” pattern.

5. Persist a compact typed trace summary per attempt: terminal state, counts, first failure, repeated failure clusters, staged/applied counts, changed paths, and final patch target. That is the shape ploke-egui should display before raw trace drilldown.

6. Add default caps and visible truncation markers for `list_dir`, `request_code_context`, and trace-facing payloads. The current failures are more tool-choice and staging churn than compiler/test-output problems.

I did not edit files. The worktree was already dirty, so this stayed read-only.

