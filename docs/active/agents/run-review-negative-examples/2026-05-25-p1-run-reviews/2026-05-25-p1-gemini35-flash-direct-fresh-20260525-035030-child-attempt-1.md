# Prototype 1 Child Attempt Review: `p1-gemini35-flash-direct-fresh-20260525-035030`

## Verdict

The first child broad-harness attempt is admissible as a committed child
artifact and useful trace evidence, but it is not clean improvement evidence.
It produced branch
`prototype1-broad-broad-harness-request-node-dfbca03c896b03ae` at commit
`357aeb147c0508a42870df6102dfc2a48c51798c`, and the headless-TUI terminal
state is `applied`.

The attempt is useful because cargo failures were recorded as model-visible
tool payloads, later edits responded to failing validation, and the final cargo
check/test calls succeeded. It is suspicious because all cargo calls resolved
to the focused `xtask/Cargo.toml`, not the request's required
`cargo check -p ploke-eval` and `cargo test -p ploke-eval edit_surface`
commands. It also made broad side-effect changes in `ploke-core`, `ploke-io`,
fixture generation, and `xtask` test setup without performance measurement.

The most important follow-up is an artifact-accounting bug: the committed
patch changes seven files, but the submitted result and terminal projection
only report the last four changed paths. That can hide earlier-turn edits from
admission, review, and adjudication.

## Evidence Roots

- Campaign:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-fresh-20260525-035030`
- Parent node: `node-dfbca03c896b03ae`
- Request:
  `prototype1/messages/edit-harness-request/node-dfbca03c896b03ae.{json,md}`
- Result:
  `prototype1/messages/edit-harness-result/node-dfbca03c896b03ae{,.headless-tui}.json`
- Candidate workspace:
  `prototype1/workspaces/edit-harness/node-dfbca03c896b03ae`
- Commit reviewed:
  `357aeb147c0508a42870df6102dfc2a48c51798c`

I did not advance the loop or rerun cargo in the candidate workspace. This
review uses persisted artifacts and read-only git inspection so the run
artifacts are not mutated.

## Execution Path

This is the Prototype 1 child-plan broad-harness path:

```text
prototype1-step -> child_plan broad-harness fanout
  -> tui_adapter::run_headless_with_model
  -> headless-TUI result artifacts
  -> committed candidate workspace
```

The broad-harness request used `edit_policy = workspace_except_ploke_eval` and
asked the child to improve the `Prototype 1 descendant performance` benchmark
outside the protected `ploke-eval` authority surface.

## What Changed

Commit `357aeb14` changed seven files:

- `crates/ingest/syn_parser/src/parser/graph/mod.rs`
- `crates/ingest/syn_parser/src/resolve/module_tree.rs`
- `crates/ploke-core/src/lib.rs`
- `crates/ploke-core/src/workspace.rs`
- `crates/ploke-io/src/path_policy.rs`
- `crates/test-utils/src/fixture_dbs.rs`
- `xtask/tests/parse_debug_commands.rs`

The changes fall into three groups:

- `syn_parser`: replaces quadratic duplicate-relation checks using
  `Vec::contains` with `HashSet`-based uniqueness checks.
- `ploke-core` / `ploke-io`: adds thread-local canonicalization caches for
  ID/path policy helpers, including a strict path cache that stores only
  `std::io::ErrorKind` on failure.
- fixtures / `xtask`: adds on-demand backup fixture DB generation behind the
  `test_setup` feature and makes one `xtask` test invoke
  `cargo run --package xtask -- setup-github-fixtures` if the serde fixture is
  missing.

The `syn_parser` change is plausibly performance-oriented. The path-cache and
fixture setup changes are broad and side-effectful enough that they should not
be treated as proven descendant-performance improvements without a separate
review.

## Trace Reconstruction

The headless result contains 395 compact events:

- two completed model turns
- 13 cargo validations
- 10 failed tool events
- 8 proposal events
- final terminal state `applied`

Turn 1 produced three applied proposals:

- `10c47027-5846-5f50-8123-cd1d4ba26d29`:
  `crates/ploke-core/src/workspace.rs`
- `3d189c4d-460b-5ba2-991b-c21889ca23be`:
  `crates/ploke-core/src/lib.rs`
- `594f17cd-089d-534e-b5d4-8c0bc7d6948c`:
  `crates/ploke-io/src/path_policy.rs`

Turn 1 also showed recoverable tool failures:

- `list_dir` on the campaign `prototype1` directory failed as outside the
  configured roots.
- `code_item_lookup` for `generate_resolved` failed once with an invalid item
  shape.
- A later `code_item_lookup` on `ploke-core/src/lib.rs` failed with
  `Content changed`, indicating stale snippet/hash state after the first edit.

Turn 2 produced five more applied proposals:

- `28dcddd2-5fce-5e5a-b2f1-b5851629beb9`:
  `crates/test-utils/src/fixture_dbs.rs`
- `ab79427c-3e72-506c-a95e-09048a554374`:
  `crates/test-utils/src/fixture_dbs.rs`
- `04b1c356-2da4-5bce-be53-444ff9d70331`:
  `xtask/tests/parse_debug_commands.rs`
- `a5524c9e-0f7a-5cdc-bd53-c87ef20b5234`:
  `crates/ingest/syn_parser/src/resolve/module_tree.rs`
- `8852bcd2-4992-5a42-8bc2-0fdedc2c5534`:
  `crates/ingest/syn_parser/src/parser/graph/mod.rs`

Turn 2 also had stale/edit-tool failures that the model worked around:

- `apply_code_edit` on `fixture_dbs.rs` failed because the file version could
  not be verified after earlier edits.
- `code_item_lookup` on `fixture_dbs.rs` then failed with `Content changed`.
- `code_item_lookup` misses for `merge_new` and `validate_unique_rels` led to
  broader direct reads and `non_semantic_patch` fallback.
- `apply_code_edit` for
  `crate::resolve::module_tree::validate_unique_rels` failed with no matching
  node, and the later non-semantic patch applied instead.

The final applied state is real, but the summary projection is incomplete. The
headless terminal `changed_paths` and the submitted result's `change_summary`
list only four files:

- `crates/test-utils/src/fixture_dbs.rs`
- `xtask/tests/parse_debug_commands.rs`
- `crates/ingest/syn_parser/src/resolve/module_tree.rs`
- `crates/ingest/syn_parser/src/parser/graph/mod.rs`

The actual commit also contains first-turn edits to:

- `crates/ploke-core/src/lib.rs`
- `crates/ploke-core/src/workspace.rs`
- `crates/ploke-io/src/path_policy.rs`

That mismatch should be treated as a runner/admission accounting bug.

## Cargo Visibility And Validation

The cargo output was visible to the model as tool payloads. The headless event
ledger records each cargo call as a `tool_completed` event with a JSON payload;
for example the first failed `cargo test` completion has `content.chars = 19218`
and includes `ok:false`, `status_reason:"tests_failed_or_runtime"`,
`exit_code:101`, and a stderr tail.

The validation chain is a useful positive example:

- the model hit failed `cargo test` results five times:
  `function-call-3c383a19-8b57-44f5-a837-e206f79e8cbc`,
  `function-call-01b1c370-9145-4a8c-b077-75e8c2c82e39`,
  `function-call-e258e1f0-99a5-454f-af4f-a11ab6b66987`,
  `function-call-fb5408da-3c96-43da-b9ca-b6d774eb86a4`, and
  `function-call-6cb4d139-805a-4064-8341-9d01cf692c18`
- after fixture/test edits, `cargo test` succeeded at
  `function-call-55679a14-a6e9-4b81-93bc-ddbacce43641`
- after the final `syn_parser` edits, `cargo check` and `cargo test` succeeded
  at `function-call-829ff166-a014-4c50-81e2-b83b44d4ac76` and
  `function-call-261b8672-be70-45f1-b99a-65f21058a662`

However, every recorded cargo validation resolved to:

```text
.../prototype1/workspaces/edit-harness/node-dfbca03c896b03ae/xtask/Cargo.toml
```

So the final check/test are meaningful as focused `xtask`-manifest validation
that compiled several changed dependencies, but they do not satisfy the
broad-harness contract:

- required: `cargo check -p ploke-eval`
- required: `cargo test -p ploke-eval edit_surface`
- observed final: bare `cargo check` and bare `cargo test` under focused
  `xtask/Cargo.toml`

No formatting check or performance measurement appears in the artifact.

## Classification

- `admissible`: yes, as a committed broad-harness child attempt
- `useful`: yes, for cargo-feedback/retry evidence and for identifying
  validation/accounting gaps
- `benchmark-improving`: unproven
- `suspicious`: yes, because broad side-effect changes are under-validated and
  the persisted summary omits three committed files
- `blocker`: no immediate artifact-corrupting blocker for continuing the loop,
  but the changed-path accounting bug should become a high-priority follow-up

## Positive Examples And Adjudication Candidates

Positive examples:

- `failed cargo -> further edits -> passing cargo`: the run includes multiple
  failed cargo tests followed by fixture/test and parser edits, then final
  passing cargo test.
- `stale semantic tool -> fallback`: the model recovered from `Content changed`
  and missing-node semantic edit failures by using direct reads and
  `non_semantic_patch`.
- `search -> targeted performance edit`: the later `collect` /
  `validate_unique_rels` search sequence led to a plausible `HashSet`
  optimization in `syn_parser`.

Candidate adjudication fields:

- Did the child run the validation commands requested by the broad-harness
  contract?
- Did final validation cover every committed changed path, not just the focused
  manifest?
- Did cargo failure cause a production performance fix, or mostly test/setup
  accommodation?
- Did the persisted child summary account for all committed files and all
  applied proposals across model turns?
- Did stale semantic tools recover through useful fallback, or did they hide
  missing post-edit reindexing?

## Action Items

1. File an alive bug for child-attempt result accounting: terminal
   `changed_paths`, terminal `applied_proposal_ids`, and submitted
   `change_summary` should cover all committed changes across all turns, not
   only the final turn.
2. Fix broad-harness cargo validation so requested validation commands are
   either executed exactly or the child is marked as weakly validated when it
   only runs bare cargo under a focused manifest.
3. Add adjudication fields that distinguish validation repair by production
   change from validation repair by fixture/test setup changes.
4. Continue tracking post-edit `Content changed` failures as non-blocking
   evidence of stale semantic lookup state until the reindex/refresh boundary
   is verified.
5. Review the `ploke-core` / `ploke-io` path-cache changes before promotion;
   they cache filesystem state and error kinds and may change behavior across
   symlink, deletion, or fixture-generation boundaries.
