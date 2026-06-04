# Prototype 1 Child Run Review: 15g2x3-par2 Broad Harness r4

Date: 2026-06-02

Campaign: `p1-gemini35-flash-direct-15g2x3-par2-20260601-173956`

Parent node: `node-552c19a55f53dbe6`

Attempt: `node-552c19a55f53dbe6-r4`

Review status: complete for the available broad-harness request, headless-TUI
trace, and candidate workspace evidence. This is not a normal eval run-root
review and not an admissible child-result review because the attempt timed out
and did not publish a submitted broad-harness result.

## Short Verdict

`r4` produced a dirty candidate workspace with two modified files,
`crates/ploke-db/src/type_graph/fixed_rules.rs` and
`crates/ploke-db/src/type_graph.rs`, but the headless TUI terminal was
`timed_out` after 900 seconds. The request's `submitted_result_path` points at
`messages/edit-harness-result/node-552c19a55f53dbe6-r4.json`, but that file is
absent; only the diagnostic
`node-552c19a55f53dbe6-r4.headless-tui.json` exists.

The edits are plausible `ploke-db` performance work, not a completed or admitted
Prototype 1 descendant-performance result. The first edit rewrites the
`TypeTargetPaths` fixed rule to use hash maps/sets and cache per-root traversal
paths. The second edit hoists `is_type_alias(owner_id)` and
`is_const_generic_alias(owner_id)` out of the loop in
`expand_owner_type_context`. Focused cargo checks passed, and a focused
`reachability` test passed, but broader tests were already failing before edits
and still failed later. There is no final validation after the second edit, no
request-contract `ploke-eval` validation, no oracle/MBE evidence, and no
submitted result JSON.

The highest-value lifecycle finding is that the ordered event stream alone is
insufficient for the second edit. Top-level `events` end after staging/proposing
proposal `38c75715-0424-5705-8555-64dad69fd5e4`; the top-level `attempts` array,
`debug_relay`, and direct workspace diff are needed to prove it was actually
applied. Reviewers should not infer final edit state from the first
`ToolCompleted` or even from the ordered `events` stream alone.

## Evidence Roots

- Campaign root:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-par2-20260601-173956`
- Request JSON:
  `prototype1/messages/edit-harness-request/node-552c19a55f53dbe6-r4.json`
- Request prompt markdown:
  `prototype1/messages/edit-harness-request/node-552c19a55f53dbe6-r4.md`
- Headless TUI diagnostic trace:
  `prototype1/messages/edit-harness-result/node-552c19a55f53dbe6-r4.headless-tui.json`
- Expected submitted result path, absent on disk:
  `prototype1/messages/edit-harness-result/node-552c19a55f53dbe6-r4.json`
- Candidate workspace:
  `prototype1/workspaces/edit-harness/node-552c19a55f53dbe6-r4`
- Baseline eval/protocol review for campaign context:
  `/home/brasides/code/ploke/docs/active/agents/run-reviews/2026-06-01-p1-gemini35-flash-direct-15g2x3-par2-20260601-173956-baseline-eval-protocol.md`

Source-path evidence for the execution path was checked in
`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`: lines 1585-1604 call
`tui_adapter::run_headless_with_model`, obtain the terminal, write headless TUI
diagnostics, and then finish the attempt; lines 1731-1739 reject timed-out runs
and refuse to publish a submitted broad-harness result; lines 2029-2045 write
`run.evidence()` to `submitted_result_path.with_extension("headless-tui.json")`.

## Execution Path

The active path for this artifact is:

```text
published Prototype 1 broad-harness request
-> cli_facing broad headless-TUI attempt
-> tui_adapter::run_headless_with_model
-> write_broad_headless_tui_diagnostics(.headless-tui.json)
-> finish_broad_headless_tui_attempt
-> TimedOut branch refuses submitted broad-harness result
```

Evidence tying `r4` to that path:

- Request id:
  `broad-harness-request:node-552c19a55f53dbe6:r4`.
- The request's candidate workspace is the inspected
  `.../workspaces/edit-harness/node-552c19a55f53dbe6-r4` directory.
- The diagnostic artifact has the exact extension produced by
  `broad_headless_tui_diagnostics_path`: `.headless-tui.json`.
- The diagnostic top-level `terminal` object is
  `{ "terminal": "timed_out", "secs": 900 }`.
- The normal submitted result JSON is absent, matching the timed-out finish path
  that refuses publication.

This is not the baseline eval path
`prototype1-step -> runner.rs::run_benchmark_turn -> record.json.gz -> protocol`.
Normal eval/protocol artifacts exist for the baseline run reviewed separately,
but not for this broad-harness child attempt.

## Prompt And Request

The request asked the model to work in the `node-552c19a55f53dbe6-r4` candidate
checkout, inspect repository/evidence, treat protocol diagnostics as guidance,
edit broad surface outside protected core, choose a likely descendant-performance
improvement, and stage the candidate change with edit tools.

Request JSON facts checked for this attempt:

- edit policy: `workspace_except_ploke_eval`
- child budget: min 1, max 1
- protected core anchor:
  `crates/ploke-eval/src/cli/prototype1_state/backend.rs` symbol
  `eval_core_surface_root`
- evaluation scope: `prototype1_descendant_performance`
- selection: `history_backed_successor_selection`
- requested validation contract:
  `cargo check -p ploke-eval` and `cargo test -p ploke-eval edit_surface`
- return-evidence authority boundary:
  `admission=not_claimed`, `grant=not_claimed`, `child_plan=not_claimed`

The model did not run the request-contract validation commands. All persisted
cargo validations in the r4 trace are focused on `crates/ploke-db/Cargo.toml`.

## Closure State And Baseline Context

Campaign closure is mechanically complete for the baseline eval and protocol:
registry complete, eval complete, protocol complete, and all three required
procedures complete. That closure is baseline evidence, not r4 child-attempt
evidence.

For `r4`, there is no `record.json.gz`, protocol artifact family,
Multi-SWE-bench submission, benchmark patch projection, oracle/MBE result, or
submitted broad-harness result. The r4-specific evidence is the request JSON,
headless-TUI diagnostic trace, and dirty candidate workspace.

## Eval, Patch Output, Oracle, And MBE State

Mechanical broad-harness result:

- diagnostic trace: present
- terminal: `timed_out` after 900 seconds
- submitted result JSON: absent
- child/admission result: absent
- workspace: dirty with two modified files

Patch output verified in the candidate workspace:

- workspace branch:
  `prototype1-broad-broad-harness-request-node-552c19a55f53dbe6-r4`
- workspace HEAD:
  `774def86e034ac0e5cb6fd15842f2bc8abf18bc6`
- `git status --short`:
  - `M crates/ploke-db/src/type_graph.rs`
  - `M crates/ploke-db/src/type_graph/fixed_rules.rs`
- `git diff --stat`:
  - `crates/ploke-db/src/type_graph.rs`: 14 changed lines
  - `crates/ploke-db/src/type_graph/fixed_rules.rs`: 79 changed lines
  - total: 2 files changed, 59 insertions, 34 deletions

The fixed-rules edit replaces per-root `BTreeMap`/`BTreeSet` traversal storage
with `std::collections::HashMap`/`HashSet`, caches each `root_type_id` traversal
as `Vec<(terminal_type_id, target_id, relation_kind, depth)>`, and emits cached
paths for every root row. The `type_graph.rs` edit precomputes `is_alias` and
`is_const_generic` once before looping over `type_targets_reachable_from_owner`.

Oracle/MBE state: no oracle, MBE, descendant-performance measurement, or
admission evidence is attached to this r4 attempt. The patch may be a useful
performance hypothesis, but the benchmark usefulness is unproven.

## LLM And Tool Behavior

The headless trace records 133 ordered events:

- `tool_request`: 64
- `tool_completed`: 64
- `proposal`: 2
- `tool_failed`: 3

Top-level validations recorded by the diagnostic trace:

1. `cargo check`, focused `crates/ploke-db/Cargo.toml`, `ok: true`, exit 0,
   0 warnings.
2. `cargo test`, focused `crates/ploke-db/Cargo.toml`, `ok: false`, exit 101,
   `tests_failed_or_runtime`, 2 warnings. This happened before the r4 edits.
3. `cargo check --benches`, focused `crates/ploke-db/Cargo.toml`, `ok: true`,
   exit 0, 0 warnings.
4. After proposal `5630df4f...` applied, `cargo check` passed again in 432 ms.
5. After proposal `5630df4f...`, `cargo check --benches` passed again in 946 ms.
6. After proposal `5630df4f...`, `cargo test -- reachability` passed in 2514 ms,
   with 2 warnings.
7. `cargo test --features typed_type_graph -- type_graph` failed with exit 101,
   `tests_failed_or_runtime`, 2 warnings.

The cargo validation records are useful but not complete enough to prove final
correctness. The failed test records preserve exit status and summary, but the
persisted trace preview does not preserve enough stdout/failure details to name
the failing test cases. More importantly, there is no validation after proposal
`38c75715...` is applied.

## Positive Examples And Adjudication Candidates

Positive trace signals:

- The model found the right neighborhood for its chosen performance hypothesis:
  `TypeTargetPaths` in `crates/ploke-db/src/type_graph/fixed_rules.rs` and
  `expand_owner_type_context` in `crates/ploke-db/src/type_graph.rs`.
- It did not repeat the protected-manifest patch after the harness denied the
  attempted `Cargo.toml` edit. The error explicitly said to avoid protected
  manifests/configs, and the model moved back to source-code edits.
- It recovered from an invalid semantic edit target:
  `crate::type_graph::expand_owner_type_context` failed because method targets
  must look like `crate::module::Type::method`; the model retried with
  `crate::type_graph::Database::expand_owner_type_context` and staged proposal
  `38c75715...`.
- It ran some focused validation after the first edit: `cargo check`,
  `cargo check --benches`, and `cargo test -- reachability`.

Candidate adjudication signals:

- Distinguish `staged` edit results from applied edit results per call id.
- Award partial credit for tool-failure recovery only when the next action
  materially changes strategy, as in the invalid-canon recovery here.
- Penalize validation mismatch: the model ran `ploke-db` checks instead of the
  request-contract `ploke-eval` commands.
- Penalize missing final validation after the last applied proposal.
- Treat cargo failures with missing failing-test stdout as weak validation
  evidence, not as a complete semantic diagnosis.

## Trace Reconstruction

Concrete high-signal chain:

1. The model inspected repository/test structure and localized type-graph query
   tests under `crates/ploke-db/tests/unit/type_graph_queries`.
2. It read large sections of `crates/ploke-db/src/type_graph.rs`, searched for
   `TypeTargetPaths`, and read `crates/ploke-db/src/type_graph/fixed_rules.rs`.
3. It ran the existing focused `cargo test` in `ploke-db`; the run failed with
   exit 101 before any edit. It then ran `cargo check --benches`, which passed.
4. It read performance/type-resolution docs and decided to optimize the
   `TypeTargetPaths` fixed rule.
5. Event 100 requested `apply_code_edit` for
   `crate::type_graph::fixed_rules::type_target_paths_rule`. Events 101 and 103
   reported only `staged=1, applied=0`; proposal event 102 recorded proposal
   `5630df4f-a619-5b07-be4b-0c02a520750a`; event 104 then recorded
   `applied=1` with a new file hash for `fixed_rules.rs`.
6. The model validated that first edit with `cargo check`,
   `cargo check --benches`, and `cargo test -- reachability`; all three passed.
7. It then ran `cargo test --features typed_type_graph -- type_graph`; this
   failed with exit 101.
8. The model attempted to add `fxhash = { workspace = true }` to
   `crates/ploke-db/Cargo.toml` via `non_semantic_patch`. Event 120 denied the
   write because `Cargo.toml` is protected and told the model to edit source
   files or state dependency changes in prose.
9. The model read `expand_owner_type_context`, recognized repeated
   `is_type_alias`/`is_const_generic_alias` queries inside the target loop, and
   attempted a semantic edit with canon
   `crate::type_graph::expand_owner_type_context`. Events 128 and 129 failed
   that call because method targets require the type path.
10. The model retried with canon
    `crate::type_graph::Database::expand_owner_type_context`. Event 131 records
    `staged=1, applied=0`; proposal event 132 records proposal
    `38c75715-0424-5705-8555-64dad69fd5e4` for `type_graph.rs`.
11. The ordered events stream ends there, but the top-level `attempts` array says
    proposal `38c75715...` was `applied`, `debug_relay` retains
    `ApproveEdits { proposal_id: 38c75715-... }`, and direct workspace `git diff`
    verifies the hoisted alias checks are present in `type_graph.rs`.
12. The terminal timed out at 900 seconds. No final answer, request-contract
    validation, admission evidence, or submitted result JSON was produced.

The last point where the model had enough information to act was after it
recovered to the correct `Database::expand_owner_type_context` canon and staged
proposal `38c75715...`. It needed to run final validation and publish/return
candidate evidence, but the run timed out instead.

## Edit Lifecycle

Read edit lifecycles per `call_id`:

```text
function-call-db8765e5-2859-4053-9d20-9690fe6abc61
  event 100: apply_code_edit request for type_target_paths_rule
  event 101: ok=true, staged=1, applied=0
  event 102: proposal 5630df4f-a619-5b07-be4b-0c02a520750a
  event 103: ok=true, staged=1, applied=0
  event 104: ok=true, applied=1, file=fixed_rules.rs
```

```text
function-call-d8c70d33-cc45-474a-b674-a49c230327a1
  event 119: non_semantic_patch request for crates/ploke-db/Cargo.toml
  event 120: denied before execution; protected manifest/config path
```

```text
function-call-0c94cf3d-268b-4d73-b085-9de5fd7fd02c
  event 127: apply_code_edit request with canon crate::type_graph::expand_owner_type_context
  event 128: invalid_format; method targets must look like crate::module::Type::method
  event 129: internal staging failure derived from the invalid target
```

```text
function-call-bb76c790-40c7-415e-b7c8-7ea148e48ba7
  event 130: apply_code_edit request with canon crate::type_graph::Database::expand_owner_type_context
  event 131: ok=true, staged=1, applied=0
  event 132: proposal 38c75715-0424-5705-8555-64dad69fd5e4
  top-level attempts/debug_relay/workspace diff: proposal applied
```

This lifecycle is a record/playback gap, not merely a model issue. For the
second proposal, the ordered events stream records staging and proposal creation
but not the final applied completion. Manual joins are required.

## Verification Against Checkout And Persisted Artifacts

Verified result/admission claim:

- The request JSON says the submitted result path is
  `prototype1/messages/edit-harness-result/node-552c19a55f53dbe6-r4.json`.
- Filesystem inventory found only
  `node-552c19a55f53dbe6-r4.headless-tui.json` in the r4 result family.
- Therefore the timeout branch did not publish a normal submitted result.

Verified edit-state claim:

- The top-level `attempts` array says proposals `5630df4f...` and `38c75715...`
  were applied.
- Direct workspace `git status --short` reports exactly two modified files:
  `type_graph.rs` and `type_graph/fixed_rules.rs`.
- Direct workspace `git diff` shows the fixed-rule cache/hash-map edit and the
  alias-query hoist in `expand_owner_type_context`.

Verified suspicious tool-result claim:

- The trace `debug_relay` retained a successful read of
  `crates/ploke-db/src/type_graph.rs` lines 1000-1200 with `content:""` and
  `exists:true`.
- Direct checkout inspection of the same workspace shows `type_graph.rs` has
  1342 lines, and lines 1000-1060 contain real code in `expand_target_fields` and
  nearby methods.
- That completed read should not be counted as information success. The model
  later recovered with overlapping reads, but the tool payload itself was
  misleading.

## Protocol Review And Blind Spots

There is no r4 protocol artifact family to adjudicate because this broad-harness
attempt did not publish a submitted result or become a normal eval run. The
baseline protocol review is separate campaign context.

Broad-harness diagnostic blind spots found in r4:

1. The ordered `events` stream does not contain the final applied completion for
   proposal `38c75715...`; the reviewer must join `attempts`, `debug_relay`, and
   workspace diff.
2. Cargo failure records preserve exit status but not enough failing-test stdout
   to identify what failed without rerunning or finding another artifact.
3. The trace does not enforce or highlight the mismatch between the request's
   `ploke-eval` validation contract and the model's focused `ploke-db` commands.
4. Successful read calls can carry empty content for existing line ranges, so
   `ok:true`/`exists:true` is not a sufficient information-success signal.

## What Is Working

- The broad headless-TUI path leaves a useful diagnostic trace even when timeout
  prevents a submitted result.
- Protected-surface enforcement worked for `Cargo.toml`; the attempted manifest
  edit was denied before execution.
- Semantic edit tooling gave a useful error for the invalid method canon, and the
  model used that feedback to retry with the `Database::method` path.
- Workspace inspection can recover the actual patch state when trace projections
  are incomplete.

## What Is Not Working Yet

- Timeout after applied edits creates a dirty workspace without an admitted
  result. That may be useful review evidence, but it is benchmark-useless as a
  candidate patch until admitted or rerun to completion.
- Final validation is weak: no validation after the second applied edit and no
  request-contract validation at all.
- The model's chosen work area (`ploke-db` type graph performance) may be a
  reasonable descendant-performance hypothesis, but it is not tied to a measured
  descendant outcome or oracle result.
- Tool result summaries hide important lifecycle and information-quality issues:
  staged vs applied, missing applied event, empty reads, and insufficient cargo
  failure detail.

## Action Items

Non-blocking observability/tooling items:

1. Persist the final applied/failed/stale state for every staged proposal inside
   the ordered `events` stream, not only in `attempts`/`debug_relay`.
2. Preserve enough cargo stdout/failure detail in headless diagnostics to name
   failing test cases, or explicitly mark the validation as summary-only.
3. Add a diagnostic flag when model-run validations do not match the request
   contract commands.
4. Treat successful reads with empty content for existing line ranges as a
   negative information-success signal in future tool-call review/adjudication.

Candidate/model-facing items:

1. Do not admit or index r4 as a completed child candidate; it has no submitted
   result JSON and no final validation.
2. If this performance hypothesis is worth pursuing, rerun or hand off as a new
   bounded task with explicit validation: request-contract `ploke-eval` checks,
   targeted type-graph tests, and a descendant-performance measurement.
3. Require final validation after the last applied proposal before counting the
   run as mechanically useful.
