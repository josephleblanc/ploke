# Prototype 1 Child Attempt Review: `p1-gemini35-flash-direct-fresh-20260524-193515`

## Verdict

The first child broad-harness attempt is admissible as loop evidence, but it is
not benchmark-improving evidence yet. It produced a real applied artifact on
branch `prototype1-broad-broad-harness-request-node-b19077fc35c373b5` at commit
`f5c5ba3f`, with two changed files and a final headless-TUI terminal state of
`applied`.

The attempt is useful because the model found a plausible hot path, used
model-visible tool output to locate `canonicalize` call sites, recovered from
tool failures by falling back to direct reads, and validated that the changed
crate and default workspace package still compile. It is suspicious as a
benchmark improvement claim because it did not run the request's stated
validation commands for `ploke-eval`, did not measure the performance effect,
and made a broad final claim that the trace does not support.

## Evidence Roots

- Campaign:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-fresh-20260524-193515`
- Reviewed request/result, first attempt only:
  `prototype1/messages/edit-harness-request/node-b19077fc35c373b5.{json,md}`
  and `prototype1/messages/edit-harness-result/node-b19077fc35c373b5{,.headless-tui}.json`
- Reviewed workspace:
  `prototype1/workspaces/edit-harness/node-b19077fc35c373b5`
- Explicitly excluded from this review:
  `node-b19077fc35c373b5-r2` and later retry request/workspace artifacts.

The broad-harness result is a compact headless-TUI artifact. I found no
`record.json.gz`, `llm-full-responses.jsonl`, or `agent-turn-trace.json` under
the first child workspace, so this review uses the persisted result JSON,
headless-TUI event ledger, debug relay, and git checkout state.

## What Changed

Commit `f5c5ba3f5a9d5e7351cd76502336ac86a844547f` changed:

- `crates/ploke-core/src/lib.rs`
- `crates/ploke-core/src/workspace.rs`

The patch adds a public `cached_canonicalize` helper in `ploke-core`:

- uses a thread-local `RefCell<HashMap<PathBuf, Result<PathBuf, String>>>`
- caches both successful canonicalized paths and stringified errors
- returns `Result<PathBuf, String>`

The patch then routes two callers through that helper:

- `CanonId::generate_resolved` now calls `crate::cached_canonicalize`
  instead of `id_info.file_path().canonicalize()`
- `workspace::canonicalize_best_effort` now calls `crate::cached_canonicalize`
  and falls back to the original path on error

This is a small compile-clean change, but it is not obviously semantics-free:
it caches filesystem errors, caches by the input `PathBuf` spelling, and changes
one error construction path from an `io::Error` value to a cached string.

## Tool Output And Model Use

The model did see useful tool output and used it.

Positive trace:

- events 70-75: `request_code_context` for `canonicalize`,
  `generate_resolved`, and `IdConversionError` returned relevant snippets,
  including `workspace::canonicalize_best_effort`, `CanonId::generate_resolved`,
  and the `IdConversionError::IoError(String, String)` shape
- events 76-81: the model read exact line ranges around
  `CanonId::generate_resolved`, a top-level insertion area, and
  `canonicalize_best_effort`
- events 82-86: it staged and applied the first `insert_rust_item` proposal
  adding `cached_canonicalize`
- events 135-140: after `code_item_lookup` failed, it used direct `read_file`
  calls to recover exact context
- events 141-144: it staged and applied a second `non_semantic_patch` proposal
  updating both call sites
- events 145-150: it ran `cargo check`, `cargo test`, and a workspace-scoped
  `cargo check --package ploke-tui`, all successful

The final model summary does overstate the evidence: "All unit and integration
tests compile instantly and pass successfully" is not supported. The trace shows
focused `ploke-core` check/test and workspace `ploke-tui` check, not integration
test coverage or the requested `ploke-eval` edit-surface test.

## Warnings And Lifecycle

The console `TOOL_EXECUTION_FAILED` warning corresponds to real failed tool
events, but it did not poison the final artifact by itself.

Observed failed calls:

- event 45 / attempt 1: `list_dir` on the campaign root failed because the path
  was outside configured roots
- event 136 / attempt 3: `code_item_lookup` for `generate_resolved` failed
  because multiple items matched `module_path = crate::ids` and
  `node_kind = method`

Observed edit lifecycle:

- proposal `3b81de42-d553-5d05-87a3-de9bba53a38f`: staged one edit, then
  applied one edit to `crates/ploke-core/src/lib.rs`
- proposal `b389fe2e-587b-52fa-9680-72b1277c57da`: staged two edits, then
  applied two edits to `crates/ploke-core/src/lib.rs` and
  `crates/ploke-core/src/workspace.rs`
- terminal state: `applied`
- applied proposal ids: both proposal ids above
- final turn event: `outcome = completed`, `attempts = 74`

I found no persisted `Content changed` message and no content-hash mismatch in
the first child attempt artifacts. The staged/apply projection has the known
two-phase shape where a tool completion first says `staged > 0, applied = 0`,
then a later completion records the applied proposal. Reconstructing the raw
sequence shows applied edits, not an unapplied staged artifact.

## Validation Gap

The request contract asked for:

- `cargo check -p ploke-eval`
- `cargo test -p ploke-eval edit_surface`

The attempt instead validated:

- `cargo check` focused on `crates/ploke-core/Cargo.toml`
- `cargo test` focused on `crates/ploke-core/Cargo.toml`
- `cargo check --package ploke-tui` from the workspace manifest

Those are useful sanity checks for compilation fallout, but they do not verify
the requested child-plan or edit-surface path. No performance benchmark or parse
throughput measurement appears in the first child artifacts.

## Classification

- `admissible`: yes, as a mechanically applied child artifact and loop trace
  sample
- `useful`: yes, for observing tool-use behavior and a plausible performance
  hypothesis
- `benchmark-improving`: unproven
- `suspicious`: yes, because final claims exceed validation evidence and the
  validation surface drifted from `ploke-eval` to `ploke-core`/`ploke-tui`
- `invalid`: no, not from the warning alone; the final artifact is applied and
  committed

This should not block the parent orchestrator from continuing monitoring the
live step, but it should not be promoted as a successful improvement without a
separate validation/adjudication pass.

## Positive Examples And Adjudication Candidates

Track these as positive examples:

- `code context -> targeted reads -> edit`: `canonicalize` search output led to
  exact reads around the relevant functions and then a focused patch.
- `tool failure -> fallback`: the `code_item_lookup` invariant failure did not
  end the attempt; the model switched to direct file reads and completed the
  patch.
- `staged edit -> applied proposal -> validation`: both proposals have raw
  applied completions, and the model ran post-apply compile checks.

Candidate adjudication fields:

- Did the child run the validation commands requested by the broad-harness
  contract?
- Did validation measure the claimed improvement rather than only compile?
- Did the final answer distinguish "compile-clean" from "benchmark-improving"?
- Did a failed structured tool lead to a useful fallback rather than repeated
  failing calls?

## Blockers

No artifact-corrupting blocker was found in the first child attempt.

The broken contract to track separately is validation drift: the broad-harness
contract requested `ploke-eval` check/test surfaces, while the child validated
`ploke-core` and `ploke-tui`. That is an adjudication/runner contract issue, not
something to fix inside this review.
