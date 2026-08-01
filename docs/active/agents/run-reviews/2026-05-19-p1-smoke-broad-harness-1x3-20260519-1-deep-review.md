Verified surface: read-only campaign artifact inspection, five delegated `gpt-5.5` high reviews, focused `find`/`jq`/`git` summaries, and agent-turn sidecar checks; not a live `prototype1-state` run.

# Deep Run Review: p1-smoke-broad-harness-1x3-20260519-1

Campaign: `p1-smoke-broad-harness-1x3-20260519-1`  
Worktree: `/home/brasides/.ploke-eval/worktrees/p1-smoke-broad-harness-1x3-20260519-1`  
Instance: `BurntSushi__ripgrep-2209`  
Profile: `smoke-broad-harness-1x3`  

This report supersedes the preliminary
`2026-05-19-p1-smoke-broad-harness-1x3-20260519-1.md` review for run-health
claims. The preliminary report was written while the run artifacts were still
incomplete. This pass reviewed the finished artifact surface.

Detailed sub-agent reports:

- `2026-05-19-p1-smoke-broad-harness-1x3-20260519-1/deep-review/node-a873-traces.md`
- `2026-05-19-p1-smoke-broad-harness-1x3-20260519-1/deep-review/node-81bd-applied-traces.md`
- `2026-05-19-p1-smoke-broad-harness-1x3-20260519-1/deep-review/node-81bd-timeout-traces.md`
- `2026-05-19-p1-smoke-broad-harness-1x3-20260519-1/deep-review/run-state-lineage.md`
- `2026-05-19-p1-smoke-broad-harness-1x3-20260519-1/deep-review/agent-turn-traces.md`

## Verdict

The run is finished in the policy/lineage sense, but the generated candidates
are weak. The loop reached max generation 2, sealed two History successor
decisions, evaluated six treatment branches, and advanced the active worktree to
the selected generation-2 successor branch
`prototype1-broad-broad-harness-request-node-81bd26e4b6222d08` at `663248e8`.

The main failure is not simply "high churn." The deeper pattern is that each
stage reports a different kind of success:

- headless TUI `terminal: applied` means a candidate workspace mutation was
  applied, not that the candidate validated or improved the benchmark;
- child runner `succeeded` means the branch runner completed, not that the
  Multi-SWE-Bench attempt produced a patch;
- branch `keep` can mean no regression relative to an already-empty/aborted
  baseline, not a useful fix;
- History selection records the admitted traversal policy result, not a semantic
  endorsement of patch quality.

The strongest trace-local candidate was `node-a873...-r7`, which added a
`OnceLock` history-block cache and eventually passed visible check/test
commands. Even that branch was later evaluated as `reject` because the treatment
agent-turn run aborted and produced an empty patch.

The final selected generation-2 successor, `node-8fde8d78b873fafa`, was selected
from a `keep` branch, but its agent-turn treatment also aborted and produced an
empty patch. In this run, `keep` is a relative operational outcome, not evidence
of a nonempty successful benchmark repair.

## Finished Run State

Authoritative lineage is in History, not `scheduler.json`.

| Surface | Current finding |
|---|---|
| History blocks | 2 sealed blocks, heights 0 and 1 |
| History head | block hash `ac95fcff9fcdf39831bb147fb81018b58cb13e5819a2b7249e3add04a92c0ed0` |
| Gen1 selected successor | `node-81bd26e4b6222d08`, branch `branch-1e4da15f47e52f34`, evaluation `reject` |
| Gen2 selected successor | `node-8fde8d78b873fafa`, branch `branch-b1c602731878d8ff`, evaluation `keep` |
| Active worktree branch | `prototype1-broad-broad-harness-request-node-81bd26e4b6222d08` |
| Active worktree commit | `663248e8` |
| Treatment child runner results | 6 `succeeded`, `exit_code=0` |
| `scheduler.json` | stale: still shows only root planned/frontier |
| stale node projections | root and selected gen1 node records still say `running` |

The generation-1 selection is intentionally surprising: History selected a
branch whose evaluation disposition was `reject`. That is consistent with the
admitted search policy allowing continuation from rejected branches. It should
not be reported as a benchmark win.

## Artifact Inventory

Finished-run artifact counts:

| Artifact family | Count / shape |
|---|---:|
| edit-harness requests | 18 request JSON files |
| submitted edit-harness result summaries | 7 result JSON files |
| headless TUI traces | 11 `*.headless-tui.json` files |
| timed-out headless traces | 4 |
| branch evaluations | 6 JSON files |
| child plans | 2 JSON files |
| History block records | 2 JSONL rows |
| transition journal | 102 JSONL rows |
| agent-turn run families | 1 baseline + 6 treatment branches |
| agent-turn sidecars | each family has `agent-turn-summary.json`, `agent-turn-trace.json`, and `record.json.gz` |

There are two distinct trace families:

1. Headless TUI traces under
   `prototype1/messages/edit-harness-result/*.headless-tui.json` describe the
   model's broad-harness candidate-generation session.
2. Agent-turn traces under
   `.ploke-eval/instances/prototype1/<campaign>/.../runs/...` describe the
   Multi-SWE-Bench baseline/treatment attempt for the selected branch.

Those surfaces must not be collapsed. A headless TUI edit can compile locally
while the later benchmark treatment aborts with an empty patch.

## Headless TUI Review

Across 11 headless TUI traces:

- `7` traces ended `applied`;
- `4` traces timed out at 900 seconds;
- `807` tool requests were recorded;
- `826` tool completions were recorded;
- `41` `tool_failed` events were recorded;
- `60` proposal events were recorded.

The headless trace groups:

| Group | Traces | Main result |
|---|---|---|
| root parent `node-a873...` | base, r2, r7 | all applied; r7 was strongest trace-local candidate; all later evaluated `reject` |
| selected gen1 parent `node-81bd...` applied | base, r2, r3, r9 | applied, but weak: failing tests, markdown-only change, unverified committed r3, or timeout/backoff increase |
| selected gen1 parent `node-81bd...` timed out | r5, r6, r7, r8 | all timed out; r5/r6 had local edits but no submitted result summaries; r7/r8 were no-edit timeouts |

### Strongest And Weakest Headless Candidates

`node-a873...-r7` is the strongest trace-local candidate. It added a
`OnceLock` cache for `FsRunStore::load_history_blocks`, repaired initial compile
errors, then passed visible `cargo check` / `cargo test` surfaces. Its weakness
is that the cache still clones the stored block vector and no measurement proved
the improvement. Downstream evaluation rejected the branch with an aborted,
empty treatment run.

`node-a873...-r2` is the clearest false-positive terminal. It ended
`terminal: applied`, but visible validation failed, the committed `fs.rs` diff
was malformed, and the run still materialized it as a child that reached
`built` / `spawned` / `acknowledged`. This exposes a serious status-boundary
problem: trace-local validation failure did not prevent later child lifecycle
progression.

`node-81bd...-r9` was durable and compile-clean, but it increased BM25 timeout
and retry backoff constants. That is reliability tuning at best; it is
directionally suspect as a performance candidate without measurement.

The timed-out traces are not submitted candidates. `node-81bd...-r5` doubled
`LLM_TIMEOUT_SECS` without visible validation. `node-81bd...-r6` eventually got
`ploke-tui` checking after scaffold churn, but timed out and had no submitted
result summary. `r7` and `r8` had no proposal events and no workspace changes.

## Agent-Turn Review

The baseline and all six treatment branch runs had agent-turn sidecars and
`record.json.gz`.

| Arm | Outcome | Patch state |
|---|---|---|
| baseline | completed | one applied proposal, nonempty `fix_patch` length 1767 |
| all six treatments | aborted | no applied proposals, expected file unchanged, empty `fix_patch` |

The baseline repaired `crates/printer/src/util.rs`. Every treatment run aborted
with an empty Multi-SWE-Bench submission. Every treatment trace showed
provider-side `429` / rate-limit evidence; the baseline did not.

Only baseline and `branch-dafe57f7e75214ad` had explicit `ToolFailed` events.
The other treatment failures were provider-abort failures after search/read
churn, not local tool failures. `branch-dafe57f7e75214ad` had the clearest
malformed-output evidence: invalid `non_semantic_patch` / malformed diff, but
no proposal was recorded.

`patch_projection_check_state=passed` appeared in records, but that is only a
packaging/projection fact. It did not imply a nonempty treatment patch.

## Evaluation Outcomes

Six treatment branches were evaluated:

| Branch | Node | Disposition | Important interpretation |
|---|---|---|---|
| `branch-d43eb69306af2401` | `node-49f627d37985c53e` | reject | treatment aborted, empty patch, regressed from nonempty baseline |
| `branch-e707a012bdd47f71` | `node-26e5f6b29d99238e` | reject | malformed/failed headless candidate; treatment aborted, empty patch |
| `branch-1e4da15f47e52f34` | `node-81bd26e4b6222d08` | reject | selected for continuation despite reject; policy allowed exploration from rejected branch |
| `branch-b1c602731878d8ff` | `node-8fde8d78b873fafa` | keep | selected gen2 successor; keep is relative to a no/empty baseline |
| `branch-dafe57f7e75214ad` | `node-db394066dad6eaf9` | reject | treatment had tool-call failures |
| `branch-d68716f09d5982ae` | `node-21eaf04f2c01c6f2` | keep | keep, but treatment still had empty patch |

The two `keep` outcomes should be read carefully. In generation 2, baseline and
treatment patch states were both `no` / empty, so a `keep` did not mean the
treatment produced a useful patch. It means the operational comparator did not
record a regression under the available metrics.

## Failure Analysis

1. Terminal-state names are too coarse. `applied`, `succeeded`, and `keep` each
   describe different layers. None alone means "valid benchmark improvement."

2. Trace-local validation is not gating child materialization strictly enough.
   The `node-a873...-r2` candidate visibly failed validation and had malformed
   committed Rust, yet later run-state lifecycle records reached built/spawned.

3. The broad-harness request surface still causes read/search churn. Across the
   11 headless traces there were 807 tool requests; repeated
   `request_code_context`, `read_file`, and `list_dir` usage was the normal path
   rather than the exception.

4. Protected-path policy works, but too late in the model's planning loop. The
   model repeatedly designed fixes around protected `ploke-eval` or manifest
   surfaces, then learned the boundary through denied edits.

5. Semantic edit tools struggle with crate/module-root targets such as
   `crate::core` and `crate::lib`, pushing models into noisy non-semantic patch
   loops.

6. Same-file repair loops are still dangerous. The bad `fs.rs` trace produced
   malformed code; the better `fs.rs` trace needed many proposal events before
   compiling.

7. Evaluation failures were dominated by provider aborts and empty submissions,
   not by the headless trace's local compile/test outcome. All six treatment
   agent-turn runs aborted and produced empty patches.

8. Projection drift remains. `scheduler.json` and some `node.json` statuses are
   stale relative to History, transition journal, runner results, and active
   worktree state.

## Recommendations

1. Rename or split terminal states by layer: `workspace_applied`,
   `validated`, `submitted_result`, `runner_succeeded`, `evaluated_keep`, and
   `history_selected` should be distinct review facts.

2. Block child materialization when visible validation failed, or record the
   failure as a first-class child state that cannot be mistaken for built/spawned
   success.

3. Add a compact joined run summary that projects, per candidate: headless
   terminal, changed files, validation outcomes, submitted-result presence,
   child runner result, evaluation disposition, agent-turn terminal, patch
   nonemptiness, and History selection role.

4. Put protected-path and readable-root constraints into the prompt/tool
   metadata before the model plans a fix. Repeated denials should become a
   candidate-level warning.

5. Improve semantic-edit support for crate/module-root targets or explicitly
   steer those edits to a safer patch path with tighter validation.

6. Treat provider 429/rate-limit aborts as a run-quality surface. This run's
   treatment evaluations mostly measured provider abort/empty submission
   behavior, not code-change quality.

7. Stop using `scheduler.json` as an operator health surface unless it is
   updated with the same authority as History/transition-journal state, or label
   it clearly as stale/legacy when newer artifacts exist.

## Small Verification Commands

```bash
CAM=/home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1
WT=/home/brasides/.ploke-eval/worktrees/p1-smoke-broad-harness-1x3-20260519-1
ROOT=/home/brasides/.ploke-eval/instances/prototype1/p1-smoke-broad-harness-1x3-20260519-1
```

```bash
for f in "$CAM"/prototype1/messages/edit-harness-result/*.headless-tui.json; do
  jq -r --arg f "$f" '[($f|split("/")|last), (.terminal.terminal // ""), (.attempts|length), (.events|length), ([.events[]|select(.kind=="tool_request")]|length), ([.events[]|select(.kind=="tool_completed")]|length), ([.events[]|select(.kind=="tool_failed")]|length), ([.events[]|select(.kind=="proposal")]|length)] | @tsv' "$f"
done | sort
```

```bash
sed -n '1,2p' "$CAM"/prototype1/history/blocks/segment-000000.jsonl |
  jq -r '[.state.header.common.block_height, .state.header.selected_parent_identity.node_id, .state.header.selected_parent_identity.generation, .state.header.selected_parent_identity.branch_id, .state.header.block_hash] | @tsv'
```

```bash
for f in "$CAM"/prototype1/evaluations/*.json; do
  jq -r '[.branch_id, .overall_disposition, (.compared_instances|length), (.compared_instances[0].baseline_metrics.patch_apply_state // ""), (.compared_instances[0].treatment_metrics.patch_apply_state // ""), (.compared_instances[0].treatment_metrics.aborted // "")] | @tsv' "$f"
done | sort
```

```bash
find "$ROOT" -path '*/BurntSushi__ripgrep-2209/runs/*/agent-turn-summary.json' \
  -o -path '*/BurntSushi__ripgrep-2209/runs/*/agent-turn-trace.json' \
  -o -path '*/BurntSushi__ripgrep-2209/runs/*/record.json.gz' | sort
```

```bash
git -C "$WT" branch --show-current
git -C "$WT" rev-parse --short HEAD
```

