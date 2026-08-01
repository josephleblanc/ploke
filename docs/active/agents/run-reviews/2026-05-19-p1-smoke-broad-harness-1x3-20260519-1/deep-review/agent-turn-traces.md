Verified surface: agent-turn sidecar JSON plus compact `record.json.gz` packaging selectors; not headless TUI trace review, not History authority, and not a live `prototype1-state` run.

# Deep Review: Agent-Turn Trace Artifacts

Campaign: `p1-smoke-broad-harness-1x3-20260519-1`  
Instance: `BurntSushi__ripgrep-2209`  
Trace root: `/home/brasides/.ploke-eval/instances/prototype1/p1-smoke-broad-harness-1x3-20260519-1`

Important boundary: `agent-turn-summary.json` and `agent-turn-trace.json` are sidecar turn artifacts. In this run they have the same top-level shape, and sampled runs had identical byte sizes for summary and trace. The branch evaluation record is the compressed `record.json.gz`; History admission/selection authority is outside this review.

## Exact Runs Found

All expected baseline and treatment trace families were present with `agent-turn-summary.json`, `agent-turn-trace.json`, and `record.json.gz`.

| Arm | Branch | Run directory |
|---|---|---|
| baseline | baseline | `BurntSushi__ripgrep-2209/runs/run-1779181312436-structured-current-policy-a752fb66` |
| treatment | `branch-1e4da15f47e52f34` | `treatments/branch-1e4da15f47e52f34/instances/BurntSushi__ripgrep-2209/runs/run-1779186538438-structured-current-policy-d5325c36` |
| treatment | `branch-b1c602731878d8ff` | `treatments/branch-b1c602731878d8ff/instances/BurntSushi__ripgrep-2209/runs/run-1779193199476-structured-current-policy-6f0d6e1c` |
| treatment | `branch-d43eb69306af2401` | `treatments/branch-d43eb69306af2401/instances/BurntSushi__ripgrep-2209/runs/run-1779186539452-structured-current-policy-3a01415f` |
| treatment | `branch-d68716f09d5982ae` | `treatments/branch-d68716f09d5982ae/instances/BurntSushi__ripgrep-2209/runs/run-1779193194784-structured-current-policy-966f1ec8` |
| treatment | `branch-dafe57f7e75214ad` | `treatments/branch-dafe57f7e75214ad/instances/BurntSushi__ripgrep-2209/runs/run-1779193201150-structured-current-policy-4fa00dcd` |
| treatment | `branch-e707a012bdd47f71` | `treatments/branch-e707a012bdd47f71/instances/BurntSushi__ripgrep-2209/runs/run-1779186539316-structured-current-policy-1e772150` |

## Sidecar Summary

| Arm / branch | Terminal outcome | Attempts | Events | Messages | Tool requested / completed / failed | TurnFinished | Patch sidecar state |
|---|---:|---:|---:|---:|---:|---:|---|
| baseline | completed | 21 | 164 | 64 | 17 / 16 / 4 | 1 | 1 edit proposal, `Applied`; expected `crates/printer/src/util.rs` changed; `applied=true` |
| `branch-1e4da15f47e52f34` | aborted | 31 | 242 | 91 | 28 / 28 / 0 | 1 | no proposals; expected file unchanged; `applied=false` |
| `branch-b1c602731878d8ff` | aborted | 23 | 190 | 71 | 22 / 22 / 0 | 1 | no proposals; expected file unchanged; `applied=false` |
| `branch-d43eb69306af2401` | aborted | 21 | 174 | 65 | 20 / 20 / 0 | 1 | no proposals; expected file unchanged; `applied=false` |
| `branch-d68716f09d5982ae` | aborted | 29 | 238 | 89 | 28 / 28 / 0 | 1 | no proposals; expected file unchanged; `applied=false` |
| `branch-dafe57f7e75214ad` | aborted | 31 | 251 | 94 | 29 / 25 / 5 | 1 | no proposals; expected file unchanged; `applied=false` |
| `branch-e707a012bdd47f71` | aborted | 31 | 242 | 91 | 28 / 28 / 0 | 1 | no proposals; expected file unchanged; `applied=false` |

Tool mix by run:

- baseline: `apply_code_edit` 2, `cargo` 6, `list_dir` 1, `read_file` 3, `request_code_context` 5.
- `branch-1e4da15f47e52f34`: `list_dir` 11, `read_file` 6, `request_code_context` 11.
- `branch-b1c602731878d8ff`: `list_dir` 7, `read_file` 4, `request_code_context` 11.
- `branch-d43eb69306af2401`: `list_dir` 5, `read_file` 3, `request_code_context` 12.
- `branch-d68716f09d5982ae`: `list_dir` 10, `read_file` 5, `request_code_context` 13.
- `branch-dafe57f7e75214ad`: `apply_code_edit` 1, `code_item_lookup` 3, `list_dir` 8, `read_file` 9, `request_code_context` 8.
- `branch-e707a012bdd47f71`: `list_dir` 11, `read_file` 6, `request_code_context` 11.

## Record Alignment

`record.json.gz` aligns with the sidecar terminal status for every run. Each record has exactly one `phases.agent_turns` entry.

| Arm / branch | Record outcome | Record tool calls | Packaging submission | Patch projection check | MSB `fix_patch` length | Alignment |
|---|---:|---:|---:|---:|---:|---|
| baseline | `ToolCalls`, count 17 | 17 | `nonempty` | `passed` | 1767 | matches sidecar completed/applied patch |
| `branch-1e4da15f47e52f34` | `Error`, aborted | 28 | `empty` | `passed` | 0 | matches sidecar aborted/no patch |
| `branch-b1c602731878d8ff` | `Error`, aborted | 22 | `empty` | `passed` | 0 | matches sidecar aborted/no patch |
| `branch-d43eb69306af2401` | `Error`, aborted | 20 | `empty` | `passed` | 0 | matches sidecar aborted/no patch |
| `branch-d68716f09d5982ae` | `Error`, aborted | 28 | `empty` | `passed` | 0 | matches sidecar aborted/no patch |
| `branch-dafe57f7e75214ad` | `Error`, aborted | 29 | `empty` | `passed` | 0 | matches sidecar aborted/no applied proposal |
| `branch-e707a012bdd47f71` | `Error`, aborted | 28 | `empty` | `passed` | 0 | matches sidecar aborted/no patch |

`patch_projection_check_state=passed` is a packaging/projection fact here. It did not mean a treatment produced a nonempty patch; all six treatment MSB submissions had `fix_patch` length 0.

## Baseline vs Treatments

Baseline is the only successful sidecar run. It completed after 21 attempts, staged/applied one edit proposal against `crates/printer/src/util.rs`, changed the expected file hash, and produced a nonempty MSB submission.

All six treatment branches aborted. Their sidecar terminal records all carried an `error_id`, their patch artifacts had no edit/create proposals, their expected file hash stayed unchanged, and their MSB `fix_patch` was empty.

The treatment aborts share a provider-side 429 signature: each treatment trace had two matches for `status 429` / `Rate limit exceeded`. The baseline trace had no 429 match.

Only two runs had explicit `ToolFailed` events:

- baseline: two `cargo` invalid package-format failures for package `printer`, then `apply_code_edit` target typing/staging failures before the later applied proposal.
- `branch-dafe57f7e75214ad`: `code_item_lookup` misses, `apply_code_edit` target/staging failures, and a malformed `non_semantic_patch` attempt visible as provider invalid tool arguments. It still aborted with empty patch state.

The other five aborted treatment sidecars had no `ToolFailed` entries. Their visible failure pattern is provider abort after tool/search/read churn, not a local tool failure or timeout.

## Recurring Patterns

- Aborted runs: 6/7 agent-turn runs, all treatments.
- Patch attempted/applied: baseline only. `branch-dafe57f7e75214ad` reached an invalid patch attempt, but the sidecar patch artifact recorded no proposal and no applied patch.
- Empty submissions: all six treatments. Baseline had a nonempty `fix_patch`.
- Provider failures: all six treatments show 429/rate-limit text in sidecar messages.
- Tool failures: baseline and `branch-dafe57f7e75214ad` only.
- Malformed output: visible in `branch-dafe57f7e75214ad` as invalid `non_semantic_patch` arguments / malformed diff.
- Timeout: no timeout/timed-out pattern was found in the bounded search.
- Missing turn evidence: none at the sidecar level; every run had one `TurnFinished`, and every record had one `phases.agent_turns` entry.
- Final assistant message: null in all seven sidecars; terminal state and patch artifacts carry the usable sidecar conclusion.

## Verification Commands

Smallest commands used or sufficient to recheck the claims without dumping raw traces:

```bash
ROOT=/home/brasides/.ploke-eval/instances/prototype1/p1-smoke-broad-harness-1x3-20260519-1

find "$ROOT" -path '*/BurntSushi__ripgrep-2209/runs/*/agent-turn-summary.json' -o -path '*/BurntSushi__ripgrep-2209/runs/*/agent-turn-trace.json' -o -path '*/BurntSushi__ripgrep-2209/runs/*/record.json.gz'

jq -c '{file: input_filename, event_count: (.events|length), terminal: {outcome: .terminal_record.outcome, attempts: .terminal_record.attempts, error_id: .terminal_record.error_id}, patch: .patch_artifact, counts: {messages: ([.events[]?|select(has("MessageUpdated"))]|length), tool_requested: ([.events[]?|select(has("ToolRequested"))]|length), tool_completed: ([.events[]?|select(has("ToolCompleted"))]|length), tool_failed: ([.events[]?|select(has("ToolFailed"))]|length), turn_finished: ([.events[]?|select(has("TurnFinished"))]|length)}}' "$ROOT"/**/agent-turn-trace.json

gzip -cd "$ROOT/BurntSushi__ripgrep-2209/runs/run-1779181312436-structured-current-policy-a752fb66/record.json.gz" | jq -c '{agent_turns:(.phases.agent_turns|length), outcome:.phases.agent_turns[0].outcome, tool_calls:(.phases.agent_turns[0].tool_calls|length), submission:.phases.packaging.submission_artifact_state, projection:.phases.packaging.patch_projection_check_state, total_secs:.timing.total_wall_clock_secs}'

jq -c '{run:(input_filename|split("/")[-2]), fix_patch_len:(.fix_patch|length), nonempty:(.fix_patch|length > 0)}' "$ROOT"/**/multi-swe-bench-submission.jsonl

rg -c 'status 429|Rate limit exceeded' "$ROOT"/**/agent-turn-trace.json
rg -c 'MalformedDiff|invalid arguments for tool|Provider emitted invalid arguments' "$ROOT"/**/agent-turn-trace.json
```

The `**` globs above require a shell with globstar enabled. Without globstar, use the exact run paths listed in the inventory table.
