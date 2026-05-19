# Deep Review Worker A: node-a87394840086768d Headless TUI Traces

Scope: reviewed the three persisted headless TUI traces for parent/root node
`node-a87394840086768d` in campaign
`p1-smoke-broad-harness-1x3-20260519-1`.

Important boundary: `terminal: applied` is only trace-local workspace mutation.
The associated submitted result summaries explicitly keep
`return_evidence.authority_boundary.{admission,grant,child_plan}` at
`not_claimed`; History admission and successor selection are not inferred from
the trace itself. Where run-state artifacts are mentioned below, they were
checked separately from `transition-journal.jsonl`, node records, and evaluation
reports.

## Common Verification Commands

Small selectors used across all traces:

```bash
jq '{terminal, attempts_len:(.attempts|length), events_len:(.events|length),
     event_types:([.events[].kind?] | group_by(.) | map({kind:.[0], count:length}))}' \
  <trace>.headless-tui.json

jq '[.events[] | select(.kind=="tool_request") | .tool] |
    group_by(.) | map({tool:.[0], count:length})' \
  <trace>.headless-tui.json

jq '{proposal_events:([.events[]|select(.kind=="proposal")]|length),
     unique_proposals:([.events[]|select(.kind=="proposal")|.id] | unique | length),
     applied:(.terminal.applied_proposal_ids|length)}' \
  <trace>.headless-tui.json

jq '{authority_boundary:.return_evidence.authority_boundary,
     changed_files:.return_evidence.change_summary.changed_files,
     checks:.return_evidence.checks}' \
  <result>.json
```

Run-state and evaluation checks used only to avoid overclaiming trace-local
status:

```bash
jq -s '[.[] | select((.refs.branch_label? // "") == "broad harness edit broad-harness-request:node-a87394840086768d") |
        {kind, phase, result, node_id:.refs.node_id, candidate_id:.refs.candidate_id,
         lifecycle:.world.child_lifecycle}]' \
  ~/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/transition-journal.jsonl

jq '{branch_id, overall_disposition, reasons,
     metrics:.compared_instances[0].treatment_metrics}' \
  ~/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/evaluations/<branch>.json
```

## node-a87394840086768d.headless-tui.json

Trace file:
`/home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/messages/edit-harness-result/node-a87394840086768d.headless-tui.json`

Terminal and counts:

- Terminal: `applied`.
- Applied proposal ids: 2.
- Attempts: 6.
- Events: 139.
- Event counts: `tool_request=66`, `tool_completed=66`, `tool_failed=4`,
  `proposal=2`, `turn=1`.
- Tool requests: `request_code_context=28`, `read_file=17`, `list_dir=15`,
  `apply_code_edit=2`, `cargo=2`, `code_item_lookup=1`,
  `non_semantic_patch=1`.
- Proposal counts: 2 proposal events, 2 unique proposals, 2 applied.

Changed paths:

- `crates/ingest/syn_parser/src/parser/visitor/code_visitor.rs`
- `crates/ingest/syn_parser/src/parser/visitor/code_visitor_syn1.rs`

Committed diff:

```bash
git -C ~/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/workspaces/edit-harness/node-a87394840086768d \
  show --stat --oneline --no-renames HEAD
git -C ~/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/workspaces/edit-harness/node-a87394840086768d \
  show --no-ext-diff --unified=3 -- \
  crates/ingest/syn_parser/src/parser/visitor/code_visitor.rs \
  crates/ingest/syn_parser/src/parser/visitor/code_visitor_syn1.rs
```

Observed diff: commit `270d80f8` changes only
`s.split_whitespace().collect::<Vec<&str>>().join(" ")` to
`s.split_whitespace().join(" ")` in both parser visitor files.

Validation visible in trace:

- `cargo check` succeeded with `exit_code=0`, `errors=0`, `warnings=6`,
  duration about 8.9s.
- `cargo test` succeeded with `exit_code=0`, `errors=0`, `warnings=23`,
  duration about 17.2s.

Main model behavior:

- The model explored the codebase and settled on a narrow allocation reduction
  in `type_to_string`.
- It connected the change to impl parsing and claimed a performance benefit from
  removing an intermediate `Vec`.
- The code change is plausible but extremely small relative to the descendant
  performance benchmark. It is a legitimate cleanup, not strong benchmark
  evidence by itself.

Framework/tool-contract failure patterns:

- The first tool call tried to list the campaign prototype root and failed as an
  outside-configured-root read. The model recovered by listing the workspace
  root instead.
- One `read_file` call had an invalid range.
- One `code_item_lookup` call used `node_kind=function` for `name_impl` and was
  told to retry as a method.
- One `non_semantic_patch` partially applied; the model recovered with
  `apply_code_edit`.
- No protected-path write denial occurred in this trace.

Run-state/evaluation check:

- `transition-journal.jsonl` maps this request to child
  `node-49f627d37985c53e`, candidate `broad-harness-g1-01`, branch
  `branch-d43eb69306af2401`; lifecycle reached `built`, `spawned`,
  `acknowledged`, and treatment `terminated`.
- Evaluation report `branch-d43eb69306af2401.json` has
  `overall_disposition="reject"`. Treatment metrics regressed from baseline:
  `aborted=true`, `patch_attempted=false`, `submission_artifact_state="empty"`,
  `nonempty_valid_patch=false`, `convergence=false`, `oracle_eligible=false`.

Credibility:

- Trace-local candidate: credible as a compiling, tested, bounded source edit.
- Performance candidate: weak; the change is plausible but tiny.
- Run-evaluated descendant: not credible as an improvement; the evaluation
  rejected it because the treatment failed to produce a valid nonempty patch.

## node-a87394840086768d-r2.headless-tui.json

Trace file:
`/home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/messages/edit-harness-result/node-a87394840086768d-r2.headless-tui.json`

Terminal and counts:

- Terminal: `applied`.
- Applied proposal ids: 4.
- Attempts: 11.
- Events: 189.
- Event counts: `tool_request=87`, `tool_completed=88`, `tool_failed=6`,
  `proposal=7`, `turn=1`.
- Tool requests: `read_file=31`, `list_dir=23`, `request_code_context=20`,
  `non_semantic_patch=5`, `cargo=4`, `apply_code_edit=3`,
  `code_item_lookup=1`.
- Proposal counts: 7 proposal events, 5 unique proposals, 4 applied.

Changed path:

- `crates/ploke-tree/src/store/fs.rs`

Committed diff:

```bash
git -C ~/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/workspaces/edit-harness/node-a87394840086768d-r2 \
  show --stat --oneline --no-renames HEAD
git -C ~/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/workspaces/edit-harness/node-a87394840086768d-r2 \
  show --no-ext-diff --unified=3 -- crates/ploke-tree/src/store/fs.rs
```

Observed diff: commit `d7e06f68` changes 44 lines in `fs.rs`, inserts a
`/* START RESTORE */` marker, splits an `impl FsRunStore` block, removes large
parts of `load`, references `nodes_dir` before it is declared, and interleaves
`load_run_root_summary` into the `load` body. The committed result is visibly
structurally broken.

Validation visible in trace:

- First `cargo check` failed with unresolved `rayon` (`E0433`) and missing
  `par_iter` (`E0599`) after the model tried to parallelize loading without an
  allowed dependency change.
- Later `cargo check` attempts failed with unclosed delimiter errors at the end
  of `fs.rs`.
- No successful `cargo check` or `cargo test` is visible in this trace.

Main model behavior:

- The model first tried to speed up `FsRunStore::load` with Rayon-based parallel
  loading.
- It then attempted to add `rayon` to `crates/ploke-tree/Cargo.toml`, which the
  edit policy rejected.
- After the dependency path was denied, it pivoted to preallocation and repeated
  repair attempts on the same file.
- The repair loop made the file worse: stale/partial patching left restore
  markers and malformed Rust.

Framework/tool-contract failure patterns:

- Protected path denial: `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs`
  was rejected under the `workspace_except_ploke_eval` policy.
- Protected path denial: `crates/ploke-tree/Cargo.toml` was rejected, preventing
  the model's attempted Rayon dependency addition.
- One `code_item_lookup` for `LoopCommand` failed.
- One `read_file` call had an invalid range.
- One `list_dir` call targeted a file rather than a directory.
- A `non_semantic_patch` partially applied to `fs.rs`.
- Same-file repair loop: five `non_semantic_patch` requests plus multiple
  proposals all targeting `fs.rs`.

Run-state/evaluation check:

- `transition-journal.jsonl` maps this request to child
  `node-26e5f6b29d99238e`, candidate `broad-harness-g1-02`, branch
  `branch-e707a012bdd47f71`; lifecycle reached `built`, `spawned`,
  `acknowledged`, and treatment `terminated`.
- This is a framework inconsistency worth calling out: the trace-local
  validation failed and the committed diff is malformed, yet the run-state
  lifecycle still records the child as built/spawned.
- Evaluation report `branch-e707a012bdd47f71.json` has
  `overall_disposition="reject"`. Treatment metrics: `aborted=true`,
  `patch_attempted=false`, `submission_artifact_state="empty"`,
  `nonempty_valid_patch=false`, `convergence=false`, `oracle_eligible=false`.

Credibility:

- Trace-local candidate: not credible. It did not compile in the visible
  validation surface.
- Run-evaluated descendant: not credible. The downstream evaluation rejected it
  and reported no valid nonempty patch.

## node-a87394840086768d-r7.headless-tui.json

Trace file:
`/home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/messages/edit-harness-result/node-a87394840086768d-r7.headless-tui.json`

Terminal and counts:

- Terminal: `applied`.
- Applied proposal ids: 6.
- Attempts: 9.
- Events: 143.
- Event counts: `tool_request=57`, `tool_completed=70`, `tool_failed=0`,
  `proposal=15`, `turn=1`.
- Tool requests: `read_file=17`, `request_code_context=14`, `list_dir=12`,
  `cargo=5`, `non_semantic_patch=5`, `apply_code_edit=4`.
- Proposal counts: 15 proposal events, 9 unique proposals, 6 applied.

Changed path:

- `crates/ploke-tree/src/store/fs.rs`

Workspace availability:

- The submitted result and trace point at
  `.../workspaces/edit-harness/node-a87394840086768d-r7`, but that workspace is
  no longer present under `workspaces/edit-harness`.
- The corresponding branch still exists:
  `prototype1-broad-broad-harness-request-node-a87394840086768d-r7`.

Committed branch diff:

```bash
git -C ~/.ploke-eval/worktrees/p1-smoke-broad-harness-1x3-20260519-1 \
  show --stat --oneline --no-renames \
  prototype1-broad-broad-harness-request-node-a87394840086768d-r7
git -C ~/.ploke-eval/worktrees/p1-smoke-broad-harness-1x3-20260519-1 \
  show --no-ext-diff --unified=3 \
  prototype1-broad-broad-harness-request-node-a87394840086768d-r7 -- \
  crates/ploke-tree/src/store/fs.rs
```

Observed diff: commit `2b4d3c62` adds `std::sync::OnceLock`, adds
`history_blocks_cache: OnceLock<Vec<SealedBlockRecord>>` to `FsRunStore`, removes
`Eq` from the derive, initializes the cache in `new`, and caches
`load_history_blocks` results. The method returns `cached.clone()` on hits and
sets the cache after loading.

Validation visible in trace:

- Earlier `cargo check` failed with duplicate `history_blocks_cache`, duplicate
  field initialization, and a duplicated `Ok(blocks)` syntax error.
- A second `cargo check` failed on the remaining duplicated `Ok(blocks)`.
- Later `cargo check` succeeded with `exit_code=0`, `errors=0`, `warnings=6`,
  duration about 1.1s.
- `cargo test` succeeded with `exit_code=0`, `errors=0`, `warnings=6`, duration
  about 40.7s.
- A final `cargo check` succeeded with `exit_code=0`, `errors=0`, but
  `warnings=316`, duration about 24.3s.

Main model behavior:

- The model identified repeated history block loading in `FsRunStore` as a
  performance target.
- It added an instance-local `OnceLock` cache for loaded history blocks.
- This is a more benchmark-relevant hypothesis than the first trace because it
  targets run-record loading and History projection, but the implementation
  still clones the cached `Vec` on every hit and changes the public equality
  derive surface by dropping `Eq`.

Framework/tool-contract failure patterns:

- No `tool_failed` events were recorded, but semantic rejection still happened:
  three proposal attempts were rejected as "No semantic edits were applied."
- Same-file repair churn was high: 15 proposal events, 9 unique proposals,
  6 applied proposals, and 5 `non_semantic_patch` requests all targeting
  `fs.rs`.
- The trace demonstrates a framework distinction: compile failures surfaced as
  completed cargo tool events with `ok=false`, not as `tool_failed`.

Run-state/evaluation check:

- `transition-journal.jsonl` maps this request to child
  `node-81bd26e4b6222d08`, candidate `broad-harness-g1-03`, branch
  `branch-1e4da15f47e52f34`; lifecycle reached `built`, `spawned`,
  `acknowledged`, and treatment `terminated`.
- `node-81bd26e4b6222d08/node.json` later shows `status="running"` and
  `source_state_id="branch-1e4da15f47e52f34"`, so this candidate became an
  active parent/runtime surface later in the run. That is a run-state fact, not
  a trace-local conclusion.
- Evaluation report `branch-1e4da15f47e52f34.json` has
  `overall_disposition="reject"`. Treatment metrics: `aborted=true`,
  `patch_attempted=false`, `submission_artifact_state="empty"`,
  `nonempty_valid_patch=false`, `convergence=false`, `oracle_eligible=false`.

Credibility:

- Trace-local candidate: credible enough to compile and pass the visible test
  command after repairs.
- Performance candidate: plausible but unproven; the cache may help repeated
  history-block reads, but it returns cloned block vectors and needs measurement.
- Run-evaluated descendant: not credible as an improvement; the evaluation
  rejected it with the same treatment failure pattern as the other branches.

## Cross-Trace Findings

- All three traces end at `terminal: applied`, but only the first and r7 have
  visible successful validation inside the trace. r2 is the clear terminal-state
  false positive.
- The associated result summaries are submission records, not admission records;
  all three say `admission`, `grant`, and `child_plan` are `not_claimed`.
- The run-state artifacts materialized all three as generation-1 children:
  original -> `node-49f627d37985c53e`, r2 -> `node-26e5f6b29d99238e`,
  r7 -> `node-81bd26e4b6222d08`.
- All three branch evaluations were later rejected with the same treatment
  metrics pattern: treatment aborted, no patch attempted, empty submission
  artifact, no nonempty valid patch, no convergence, not oracle eligible.
- The model repeatedly chased `crates/ploke-tree/src/store/fs.rs` in r2 and r7.
  r2 shows the bad version of this loop: protected dependency edit denied,
  stale/partial repairs, malformed committed Rust. r7 shows a repaired version:
  initial compile failures, then successful check/test.
- Tool-contract failures are mixed with model behavior. Outside-root reads,
  invalid read ranges, protected-path denials, and cargo `ok=false` completions
  are framework/tool-contract signals; bad target choice, dependency attempts
  under a protected policy, and malformed repair patches are model behavior.
- The strongest trace-local candidate is r7. The strongest overall run-review
  conclusion is still negative: none of the three produced a downstream
  evaluation-approved improvement.
