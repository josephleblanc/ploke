# Prototype 1 Child Attempt Review: `p1-gemini35-flash-direct-fresh-20260525-035030` r3

## Verdict

The r3 broad-harness attempt is mechanically admissible as a committed child
artifact, but it is not clean improvement evidence. It produced branch
`prototype1-broad-broad-harness-request-node-dfbca03c896b03ae-r3` at commit
`303eb599a5549b11f9048c3b22afe9201a28cf5b`, changing
`crates/ploke-db/src/get_by_id/mod.rs` and `crates/ploke-db/src/helpers.rs`
with 16 insertions and 17 deletions.

The attempt did real work: it found the relevant `ploke-db` resolver code,
applied a small Cozo query-ordering patch, saw a compile failure caused by its
own edit, and then attempted a syntax repair. The problem is the terminal and
validation state. The headless-TUI terminal is `timed_out` at 900 seconds, and
the last recorded cargo validation is a failing focused `cargo check` against
`crates/ploke-db/Cargo.toml`. The final syntax-fix proposal is present in the
attempt ledger as applied, but there is no recorded post-fix cargo rerun.

Treat this as useful trace evidence and a candidate to penalize during
selection/adjudication. Do not treat it as benchmark-clean or validation-clean
evidence.

## Evidence Roots

- Campaign:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-fresh-20260525-035030`
- Parent node: `node-dfbca03c896b03ae`
- Reviewed request slot:
  `prototype1/messages/edit-harness-request/node-dfbca03c896b03ae-r3.{json,md}`
- Reviewed result artifacts:
  `prototype1/messages/edit-harness-result/node-dfbca03c896b03ae-r3{,.headless-tui}.json`
- Reviewed workspace:
  `prototype1/workspaces/edit-harness/node-dfbca03c896b03ae-r3`
- Reviewed commit:
  `303eb599a5549b11f9048c3b22afe9201a28cf5b`
- Materialized child node:
  `prototype1/nodes/node-3ec666c7491f8619/node.json`

I did not advance the loop, rerun cargo in the candidate workspace, or mutate
run artifacts. This review uses persisted artifacts plus read-only git
inspection.

## Execution Path

This is the Prototype 1 child-plan broad-harness path:

```text
prototype1-step -> child_plan broad-harness fanout
  -> tui_adapter::run_headless_with_model
  -> headless-TUI result artifacts
  -> committed candidate workspace
  -> materialized child node
```

The request asked for an edit outside the protected `ploke-eval` authority
surface to improve the `Prototype 1 descendant performance` benchmark. The
request contract asked for:

```text
cargo check -p ploke-eval
cargo test -p ploke-eval edit_surface
```

The model did not run either command.

## What Changed

Commit `303eb599` changed two files:

- `crates/ploke-db/src/get_by_id/mod.rs`
- `crates/ploke-db/src/helpers.rs`

The patch reorders Cozo query clauses so more selective bindings appear earlier:

- `paths_from_id` now binds `node_id = to_uuid(...)` before joining through
  `has_embedding`, `ancestor`, and `module`.
- `graph_resolve_exact`, `graph_resolve_edges`, and
  `resolve_nodes_by_canon` now place `mod_path == ...` and file/name filters
  earlier around the module/file joins.

This is plausibly performance-motivated but unmeasured. It is also a subtle
query-planner/order patch, so it needs either a benchmark or a focused
behavioral test before being considered an improvement.

## Trace Reconstruction

The headless artifact contains 135 compact events:

- 61 `tool_request` events
- 67 `tool_completed` events
- 2 `tool_failed` events
- 5 proposal events
- 7 attempt ledger rows
- 3 cargo validation rows

The compact event stream is truncated by the debug relay: 128 retained relay
entries, 72 dropped, and 45 marked truncated.

Early exploration was broad but eventually useful:

- The model listed the workspace and `crates/ploke-db`.
- It read `ploke-db` notes and `COZO_HNSW.md`.
- It ran an initial focused `cargo check`, which passed against
  `crates/ploke-db/Cargo.toml`.
- It ran `cargo test -p ploke-db`, which failed with exit code 101.
- It looked at protocol/node evidence, `backend.rs`, `resolver_bench.rs`,
  `graph_resolve_exact`, schema files, `file_mod`, `raw_query`, and
  `get_unembed_rel` context.

Tool failures:

- `function-call-76ba88d4-d913-4e94-b9ab-ff072cd97919` tried to list the
  campaign `prototype1` directory and failed because the path was outside the
  configured roots.
- `function-call-59a98cf3-8e10-480f-8dd3-4faae9c17517` tried to look up
  `paths_from_id` as a method and failed. The model recovered by looking up the
  `GetNodeInfo` trait instead.

Edit lifecycle:

- `function-call-73f1a413-7f17-44cf-9435-41063cfe9786` staged four edits
  across `get_by_id/mod.rs` and `helpers.rs`. The apply result reports
  `applied: 2`, but one result for `helpers.rs` says `Content changed`.
- The model then split the `helpers.rs` edits into separate calls:
  `a4a7587c-86de-459b-a1af-ffd411a7323c`,
  `5726d832-94c9-40a5-98ce-96ad508b50fe`, and
  `25ab4d55-b57d-4967-b88c-169f2aa1b911`; all three report applied.
- The model ran focused `cargo check`, saw a compile failure in
  `helpers.rs`, and said it would fix `format` to `format!`.
- `function-call-ae3b4de8-6326-4beb-8a50-a759c4568fcf` staged that syntax
  repair in the compact event stream.
- The attempt ledger records the final proposal
  `d71d5452-ffc0-5d03-9679-213838eacc98` as applied to `helpers.rs`.

The terminal state remains `timed_out`. There is no compact event showing a
post-fix cargo rerun after `d71d5452`, and the final validation list still ends
with the failing focused `cargo check`.

## Cargo Visibility And Validation

Cargo output was model-visible. The cargo completions are persisted as
`tool_completed` events with JSON payloads including `ok`, `status_reason`,
`manifest_path`, exit code, diagnostics, and stderr tails.

Recorded validations:

- `function-call-4d27ec33-7005-464a-a259-128ade15e2c1`: `cargo check`,
  focused manifest `crates/ploke-db/Cargo.toml`, success.
- `function-call-4227d15c-378e-4dec-bdfa-602147a431ef`: `cargo test -p
  ploke-db`, workspace manifest, failed with exit code 101.
- `function-call-93b64acd-1a61-4697-9cc6-539f2de4d41f`: `cargo check`,
  focused manifest `crates/ploke-db/Cargo.toml`, failed with two compile
  errors in `helpers.rs`.

The failed cargo payload was materially used: after seeing the `format` /
`format!` diagnostic, the model explicitly attempted to fix that typo. That is
a good adjudication example. The missing part is verification after the repair.
The request-contract validations were never run, and there is no fmt check or
benchmark measurement.

## Stale Context And Console Warnings

The persisted headless artifact contains an apply-time stale-content symptom:
the first multi-file apply reports `Content changed` for `helpers.rs`. That
matches the live orchestration observation that r3 spent time in stale
file-hash or `ContentMismatch` behavior around `helpers.rs`.

I did not find literal `ContentMismatch`, `INVALID_MODEL_RESPONSE`, `channel
closed`, or non-unique import-id warning text in the persisted messages/nodes
artifacts for this slot. Those appear to have been console-visible or
debug-channel-visible, not preserved in the durable r3 message artifacts I
reviewed.

Classification:

- The stale same-file edit behavior is review evidence and a non-blocking
  tooling issue for this attempt. The model recovered enough to apply later
  edits.
- The timeout after a red validation is more serious. It is not proof the
  campaign state is corrupt, because the child node was materialized and the
  branch contains a real commit. It is a blocker for treating this child as
  validation-clean or promoting it without separate verification.

## Materialization State

The child-plan artifact includes r3 as child node `node-3ec666c7491f8619` with:

- patch id:
  `broad-harness:broad-harness-request:node-dfbca03c896b03ae:r3`
- derived artifact:
  `artifact:git-commit:303eb599a5549b11f9048c3b22afe9201a28cf5b`
- target relpath:
  `crates/ploke-db/src/get_by_id/mod.rs`
- changed paths:
  `crates/ploke-db/src/get_by_id/mod.rs`,
  `crates/ploke-db/src/helpers.rs`

The changed-path accounting matches the commit stat for this attempt.

## Positive Examples And Adjudication Candidates

Positive examples:

- `failed validation -> targeted repair`: the focused cargo failure identified
  `format` versus `format!`; the model stated the right repair and staged a
  follow-up edit.
- `tool failure -> recovery`: after `code_item_lookup` failed for
  `paths_from_id`, the model retrieved the enclosing trait and continued.
- `context -> targeted patch`: after reading resolver and schema code, the
  model produced a small two-file query-ordering patch rather than unrelated
  churn.

Candidate adjudication fields:

- Did the model rerun validation after repairing a compiler error?
- Was the terminal state `applied`, `timed_out`, or otherwise ambiguous after
  the final edit?
- Did final validation satisfy the request contract or only a focused changed
  crate?
- Did the patch include benchmark evidence for a performance claim?
- Did same-file stale hash/content-change failures occur after an earlier
  apply, and did the model recover?
- Were console-visible model/provider/channel errors persisted in reviewable
  artifacts?

## Classification

- `admissible`: yes, as a real committed child artifact and materialized child
  node.
- `useful`: yes, as trace evidence for cargo-feedback use, edit recovery, and
  stale same-file lifecycle behavior.
- `benchmark-improving`: unproven. No benchmark was run.
- `validation-clean`: no. The last recorded validation failed, and the
  request-contract validations were not run.
- `blocker`: not a blocker to preserving the run as evidence. It should block
  promotion of this child as a successful improvement unless a later runner or
  admission phase independently validates it.

## Action Items

1. Add an adjudication signal for "repair after failed cargo was followed by
   validation rerun" versus "repair staged but timeout/no rerun."
2. Treat `timed_out` terminal state with a committed patch as a distinct child
   quality flag, especially when the last recorded validation is red.
3. Preserve console/debug-channel errors such as `INVALID_MODEL_RESPONSE`,
   channel-closed events, non-unique import-id warnings, and literal
   `ContentMismatch` events in durable attempt artifacts if they affect review.
4. Add or promote a stale same-file edit lifecycle check: after an apply to a
   file, later same-file semantic edits should refresh or clearly report stale
   file-hash state.
5. Require or separately run request-contract validation before treating a
   child as validation-clean; focused changed-crate checks are useful but not a
   substitute for the declared `ploke-eval` contract.
