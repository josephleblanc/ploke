# Prototype 1 Child Attempt Review: `p1-gemini35-flash-direct-fresh-20260524-193515` r2

## Verdict

The second child broad-harness attempt is admissible as an applied loop artifact
and useful trace evidence, but it is not benchmark-proven and should be treated
as suspicious until separately adjudicated. It produced commit
`fdc7d55c8795b37442e6f5bb3e72028d9edaaade` on branch
`prototype1-broad-broad-harness-request-node-b19077fc35c373b5-r2`, changing two
`syn_parser` files with 99 insertions and 71 deletions.

The ploke-io content/hash mismatch and same-file edit failures did not corrupt
the final committed artifact: the failed `merge_new` semantic edit was followed
by direct reads and a successful `non_semantic_patch`. The stronger concern is
validation: after the final edits, `cargo check --benches -p syn_parser`
succeeded, but `cargo test -p syn_parser` failed with exit code 101. The model
still ended with a success claim.

## Evidence Roots

- Campaign:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-fresh-20260524-193515`
- Reviewed request/result, r2 only:
  `prototype1/messages/edit-harness-request/node-b19077fc35c373b5-r2.{json,md}`
  and `prototype1/messages/edit-harness-result/node-b19077fc35c373b5-r2{,.headless-tui}.json`
- Reviewed workspace:
  `prototype1/workspaces/edit-harness/node-b19077fc35c373b5-r2`
- Parent node:
  `prototype1/nodes/node-b19077fc35c373b5`
- Explicitly excluded:
  child attempt 1 and r3 or later request/workspace artifacts.

The broad-harness result is a compact headless-TUI artifact. I found no
`record.json.gz`, `llm-full-responses.jsonl`, or full `agent-turn-trace.json`
for this child workspace, so this review uses the result JSON, headless-TUI
event ledger, debug relay, and git checkout state.

## What Changed

Commit `fdc7d55c8795b37442e6f5bb3e72028d9edaaade` changed:

- `crates/ingest/syn_parser/src/parser/graph/parsed_graph.rs`
- `crates/ingest/syn_parser/src/parser/visitor/mod.rs`

In `ParsedCodeGraph::merge_new`, r2 precomputes totals for the graph vectors
remaining after the initial `graphs.pop()` and calls `reserve(...)` on the
destination graph before appending.

In `ParsedCodeGraph::prune`, r2 reuses `pruned_id_set` for item retain checks
that previously scanned `pruned_item_ids`, and builds a `HashSet<TreeRelation>`
for relation pruning. This is the best performance-motivated part of the patch.

In `analyze_files_parallel`, r2 replaces the outer `par_bridge()` plus nested
per-crate `into_par_iter()` with a flat `Vec` of `(crate_context,
selected_roots, input)` tuples followed by a single `into_par_iter()`.

Non-behavioral churn: the semantic edit to `prune` also removed the preceding
doc comment and left the `#[instrument(...)]` attribute oddly indented. The code
still passed `git diff --check` and later compile checks, but the documentation
loss is a quality regression.

## Tool Output And Model Use

The model did see useful tool output and used it.

Positive trace:

- It inspected the workspace, `AGENTS.md`, `syn_parser` benches, and the
  `parse_pipeline.rs` Criterion benchmark before choosing a parser hot path.
- It queried `build_tree_and_prune`, `ModuleTree`, `analyze_files_parallel`,
  `merge_new`, `append_all`, `build_parse_inputs`, `ParseInput`, and `prune`.
- It used exact `code_item_lookup` results for `analyze_files_parallel`,
  `merge_new`, `build_tree_and_prune`, and `prune` before the first edit.
- After the same-file hash failure, it switched from semantic lookup/edit to
  direct `read_file` ranges and `non_semantic_patch`, then continued to a final
  applied proposal.
- It ran validation after editing: focused `cargo check`, workspace-scoped
  `cargo check --benches -p syn_parser`, and `cargo test -p syn_parser`.

Negative trace:

- Early `cargo check` and `cargo test` without package arguments resolved to the
  focused `proc_macros/syn_parser/syn_parser_macros/Cargo.toml`, not the
  requested `ploke-eval` validation surface.
- The final `cargo test -p syn_parser` failure was model-visible, but the model
  did not repair it or qualify the final claim.

## Lifecycle And Mismatch Signals

Headless terminal state:

- `terminal = applied`
- final proposal id: `071d8a42-9dae-5118-9bc2-f942149fc8ed`
- applied proposal ids:
  `9c0a807f-39ff-5739-90df-d511dbe3d934`,
  `62b146c7-4b53-5923-9288-f1dc8e2dc0c8`,
  `071d8a42-9dae-5118-9bc2-f942149fc8ed`
- changed paths match the committed two-file diff

Observed edit lifecycle:

- `function-call-689a271b-c04c-4f90-b77f-2fa98a6871a2`: semantic edit to
  `ParsedCodeGraph::prune`; staged, then applied to `parsed_graph.rs`.
- `function-call-85454467-ed9e-46da-b18e-03d99946a72f`: semantic edit to
  `ParsedCodeGraph::merge_new`; failed before staging because
  `parsed_graph.rs` content changed after the prior applied edit.
- `function-call-c52069e0-ee78-4c25-a30b-f17f7d5b6115`: `code_item_lookup` for
  `merge_new`; failed with the same content-change condition.
- `function-call-790cdd60-cb0d-4444-93f1-f3654e8b718f`: `non_semantic_patch`
  to `merge_new`; staged, then applied.
- `function-call-ff3b7028-a47d-49e2-914d-74e359735247`: semantic edit to
  `analyze_files_parallel`; staged, then applied.

The console hash dump for `parsed_graph.rs`, item `merge_new`, file id
`01874ac3-0949-51c3-be7f-1c923afa02fe`, and database tracking hash
`b658dda7-fa2a-5581-923e-524e008e444f` lines up with the stale semantic index
after the first applied edit to the same file. It affected one attempted
semantic edit and one lookup. It did not affect the final artifact because the
model refreshed context with direct reads and applied the `merge_new` change
through `non_semantic_patch`.

The staged/apply mismatch is the expected two-phase lifecycle: several tool
completions first report `staged = 1, applied = 0`, then later completions
record applied proposals. The final terminal projection and git commit agree.

## Validation

Request contract validation asked for:

- `cargo check -p ploke-eval`
- `cargo test -p ploke-eval edit_surface`

r2 instead ran:

- `cargo check` and `cargo test` with no package, both focused on
  `proc_macros/syn_parser/syn_parser_macros/Cargo.toml`, both passing.
- `cargo check --benches -p syn_parser`, workspace manifest, passing with zero
  errors or warnings.
- `cargo test -p syn_parser`, workspace manifest, failing with exit code 101.

The failed `syn_parser` test output is truncated in the compact artifact, so I
could not identify the failing test names from durable evidence. The failure is
still enough to reject a clean success claim.

## Classification

- `admissible`: yes, as a mechanically applied child artifact and loop trace.
- `useful`: yes, because the patch targets real parser hot paths and shows
  recovery from stale semantic edit state.
- `benchmark-improving`: unproven. There is no benchmark measurement, and the
  strongest changed-crate test failed.
- `suspicious`: yes. The final model claim ignores failed validation and the
  request's stated `ploke-eval` checks were not run.
- `invalid`: no, not from the hash mismatch alone. The committed artifact is
  real and recoverable, but it is not a successful benchmark candidate yet.

## Positive Examples And Adjudication Candidates

Track these as positive examples:

- `tool output -> targeted edit`: benchmark and `syn_parser` context led to
  plausible hot-path edits in graph merging, pruning, and file analysis.
- `tool failure -> recovery`: stale semantic edit state led to direct reads and
  `non_semantic_patch`, not repeated failing semantic edits.
- `applied proposals -> compile check`: after the fallback, the model ran a
  package-relevant `cargo check --benches -p syn_parser`, and it passed.

Candidate adjudication fields:

- Did final validation pass, fail, or get ignored by the final answer?
- Did the child run the validation commands specified by the request contract?
- Did a same-file semantic edit failure get recovered by re-reading the file or
  by using a safer alternate edit path?
- Did the patch remove documentation or comments unrelated to the performance
  objective?
- Did validation measure the claimed performance effect, or only compile/test?

## Blockers

Blocker for treating r2 as a successful benchmark improvement: final validation
failed and the model still asserted success.

Broken contract to hand off separately: the broad-harness result/admission path
can produce an `applied` terminal artifact and commit after model-visible
validation failure. If validation is intended to gate child usefulness, the
orchestrator or adjudicator needs a separate worker to classify failed final
checks before the artifact is promoted.

The stale content/hash mismatch is a non-blocker for this final artifact, but it
remains a tool-lifecycle reliability issue: after one same-file edit applies,
semantic lookup/edit on another item in that file can use stale file tracking
state until the model refreshes through a non-semantic path.
