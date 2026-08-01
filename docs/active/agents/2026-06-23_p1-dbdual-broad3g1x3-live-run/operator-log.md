# Operator Log

## 2026-06-23 Setup

- Created run packet directory.
- Cloned settings from `/home/brasides/.ploke-eval/profiles/prototype1/p1-walk5g3x5-pplxembed-g25p-p25f-evidencefix-20260621-170937.toml`.
- Changed requested run bounds to `max_generations = 3`, child `min = 1`, `max = 3`, `parallel_targets = 3`, and `control.parallel_cap = 3`.
- Set `[storage.eval] backend = "dual-strict"` for the DB-dual canary.
- Kept broad harness generation, direct-Google parent/protocol routes, Perplexity embedding setup flags, MBE disabled, and broad TUI retry/timeout settings from the source profile.
- Set `max_total_nodes = 24` as a bounded ceiling with headroom for broad-harness retries/projections under the requested 3-generation / 1x3 child policy.
- Provider environment presence in the setup shell: `GOOGLE_API_KEY` present, `OPENROUTER_API_KEY` present; live-test opt-in vars absent. No provider call was made for this check.
- Created seed worktree `/home/brasides/.ploke-eval/worktrees/p1-dbdual-broad3g1x3-pplxembed-g25p-p25f-20260623-174553` from `feature/ploke-loop` at `70a72815` on branch `seed-p1-dbdual-broad3g1x3-pplxembed-g25p-p25f-20260623-174553`.
- Ran setup:
  ```bash
  cd /home/brasides/.ploke-eval/worktrees/p1-dbdual-broad3g1x3-pplxembed-g25p-p25f-20260623-174553
  /home/brasides/code/ploke/target/debug/ploke-eval loop prototype1-setup \
    --campaign p1-dbdual-broad3g1x3-pplxembed-g25p-p25f-20260623-174553 \
    --profile /home/brasides/.ploke-eval/profiles/prototype1/p1-dbdual-broad3g1x3-pplxembed-g25p-p25f-20260623-174553.toml \
    --embedding-model-id perplexity/pplx-embed-v1-4b \
    --embedding-provider perplexity \
    --format json
  ```
- Setup succeeded. Parent/node id: `node-7704d8d7c2249df9`. Parent artifact branch: `prototype1-parent-p1-dbdual-broad3g1x3-pplxembed-g25p-p25f-20260623-174553-gen0`. Admitted profile SHA-256: `66f40b81d24872ed58821400b4094ef0f5e126956f606ba97e4cc85619d5dd59`.
- Built the worktree binary with `cargo build -p ploke-eval`; build succeeded with existing warnings.
- Ran `./target/debug/ploke-eval loop prototype1-doctor --headless-tui-setup-preflight --format json`; doctor passed with no blockers, effective `parallel_cap = 3`, and phase `baseline_eval`.

## Start commands

Not run during setup. To start the live walk from the prepared worktree, use the worktree-local binary and explicit live gates:

```bash
cd /home/brasides/.ploke-eval/worktrees/p1-dbdual-broad3g1x3-pplxembed-g25p-p25f-20260623-174553
./target/debug/ploke-eval loop walk use .
./target/debug/ploke-eval loop walk step --until r7 --watch --format json
./target/debug/ploke-eval loop walk step --until r13b --watch --allow git-changes --format json
./target/debug/ploke-eval loop walk step --until r14b --format json
./target/debug/ploke-eval loop walk summary -v
```

For later generations, stop/reset and restart from the successor checkout identity if the server remains at the prior terminal state. The admitted profile has `max_generations = 3`; do not bypass that stop.

## 2026-06-24 Live continuation and final audit

Command logs for this continuation are under `command-logs/019-post-complete-summary.log` through `command-logs/038-final-status-after-stop.log`.

### Progress summary

- Initial `loop walk` advanced through R7 but concurrent fanout exposed Cozo SQLite lock failures. Recovery switched to sequential `loop prototype1-step`.
- Gen0 parent `node-7704d8d7c2249df9` produced three gen1 children:
  - `node-86e8348c40b668ae` / `branch-f28a0ffb72238508`
  - `node-d0d08e9f12bb9727` / `branch-0a15a41c28f68761`
  - `node-2ee45b1f1ce50161` / `branch-3a70660cf7002577`
- `node-d0d08e9f12bb9727` failed with unsupported broad target relpath `crates/ingest/ploke-embed/src/indexer/mod.rs`; `branch-3a70660cf7002577` evaluated `keep`.
- Handoff proceeded to `node-2ee45b1f1ce50161`, then history traversal later selected `node-86e8348c40b668ae` as an active historical successor. This crossed the `previous_parent_id`/`parent_node_id` boundary intentionally enough to audit.
- `node-86e8348c40b668ae` produced three gen2 candidates. `node-c16b828fb6c007bc` failed with unsupported broad target relpath `crates/ingest/ploke-transform/src/transform/mod.rs`.
- Handoff then selected `node-dba39707d18e9fba`, which produced three gen3 candidates:
  - `node-142ac0367c924f36` / `branch-1d8ea1f7505c5921`
  - `node-73301a9c2e10a629` / `branch-17e8c4a2a391d99c`
  - `node-990612bcbfc25bab` / `branch-8318d3b382767ef5`
- Automatic continuation hit `Database error: disk I/O error (code 10)` from `owner_eval_db.backup`; sequential recovery completed gen3 child observation.

### Final status

`command-logs/038-final-status-after-stop.log` shows:

- Active parent identity: `node-dba39707d18e9fba`, generation `2`, branch `branch-a807a6ed0229e41e`, `previous_parent_id = node-86e8348c40b668ae`, lineage `parent_node_id = node-2ee45b1f1ce50161`.
- Walk summary: `terminal_condition = "not terminal"`, journal entries `171`, latest cursor `r13b successor.selected` for `node-2ee45b1f1ce50161`.
- The final transition-journal tail repeats successor-selection records with `disposition = stop_historical_traversal_budget` while also carrying selected-successor evidence for `node-2ee45b1f1ce50161`.
- Reconstructing to R12 reports: `blocked edge r12 -> r13a: selected successor 'node-2ee45b1f1ce50161' stopped by policy but no durable stopped successor record exists`.
- `walk step --until r13a` refuses because selected-successor evidence exists; `walk step --until r14a` also refuses and suggests `r13b/r14b --watch --allow git-changes`. I did not force R13b because that would continue through a stop-budget disposition and risk turning a policy stop into a handoff.

### Final DB/file parity notes

Owner eval DB audited: `/home/brasides/.ploke-eval/campaigns/p1-dbdual-broad3g1x3-pplxembed-g25p-p25f-20260623-174553/prototype1/eval-store.cozo.sqlite`.

Key DB relation counts from `command-logs/036-final-eval-db-audit.log`:

| Relation | Count |
| --- | ---: |
| `eval_transition_event` | 3 |
| `eval_record_ref` | 62 |
| `eval_attempt` | 13 |
| `eval_invocation` | 13 |
| `eval_channel_message` | 9 |
| `eval_channel_receipt` | 33 |
| `eval_import_event` | 31 |
| `eval_trace_event` | 3 |
| `eval_evaluation` | 4 |
| `eval_continuation_decision` | 4 |
| `eval_selection_decision` | 2 |
| `eval_selection_candidate` | 3 |
| `eval_artifact` | 4 |
| `eval_binary_ref` | 17 |
| `eval_build_event` | 17 |
| `eval_operation` | 3 |
| `eval_apply_event` | 3 |

Filesystem snapshot from `command-logs/037-final-file-parity-snapshot.log`:

| File evidence | Count |
| --- | ---: |
| transition journal entries | 171 |
| child-plan messages | 4 |
| node files | 11 |
| runner requests | 11 |
| canonical runner results | 10 |
| UUID runner results | 10 |
| invocations | 13 |
| child-to-parent channel logs | 13 files / 36 lines |
| evaluations | 4 |

Audit conclusions:

- Files remain the authority for this run. DB rows are useful evidence/projection mirrors but are not complete enough for DB-only runtime reconstruction.
- Invocation, attempt, evaluation, build, binary, operation, and apply mirroring had reasonable coverage for the records emitted by implemented sinks.
- Transition coverage is sparse: only 3 `eval_transition_event` rows versus 171 transition-journal entries.
- Trace coverage is sparse: only 3 `eval_trace_event` rows.
- Channel-message coverage is partial: 36 child-to-parent file lines, 33 DB receipts, but only 9 `eval_channel_message` rows and 31 import events. This is enough to see some terminal/ready/evaluating evidence but not full channel parity.
- Selection/continuation DB rows captured the final policy contradiction: `eval_continuation_decision` has `stop_historical_traversal_budget`, while `eval_selection_decision` records selected `node-2ee45b1f1ce50161` with outcome `accepted`/disposition `keep`.
- Final history projection summary (`command-logs/039-final-history-projection-summaries.log`) found 11 child-evidence rows across generations 0..3, but score projection had 0 scored rows (`7` incomplete, `4` invalid) and score/selection review had only 1 projected row out of 21.

### Issues recorded

1. Cozo SQLite `database is locked` during concurrent fanout under `dual-strict`.
2. Cozo SQLite `disk I/O error (code 10)` during `owner_eval_db.backup` in automatic continuation, despite sufficient free disk space.
3. R12/R13 stopped-successor semantics are inconsistent: a stop-budget continuation can still be durable-selected, causing R13a/R14a refusal and R13b suggestion.
4. DB mirror/projection coverage remains incomplete for transition journal, traces, channel messages, history score rows, and score-selection review rows.
5. Broad harness can select target relpaths that are not accepted known tool-description relpaths, causing candidate failures.
6. Google live provider returned `429 RESOURCE_EXHAUSTED` during the run.

### Recommended follow-ups

- Add a durable stopped-successor record/path, or prevent `successor.selected` emission when continuation disposition is `stop_historical_traversal_budget`.
- Serialize or retry owner eval-store writes/backups for Cozo SQLite, especially around fanout and `backup`.
- Complete trace and transition sinks for `walk/controller.rs`/`run/core.rs` paths.
- Mirror terminal `ToParent::Result`/successor channel messages consistently.
- Decide whether broad-harness target selection should be constrained earlier or whether known tool-description relpaths should include the failed target classes.
