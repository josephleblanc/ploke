# Prototype 1 Child Run Review: Fixed 15g2x3 Broad Harness r12

Date: 2026-05-25

Campaign: `p1-gemini35-flash-direct-15g2x3-fixed-20260525-123824`

Parent node: `node-57e8487f70ce4abc`

Attempt: `node-57e8487f70ce4abc-r12`

Derived child node: `node-5a745aa89cb08600`

Review status: complete for the available broad-harness child artifacts. This is
a broad headless-TUI attempt review, not a normal eval run-root review.

## Short Verdict

`r12` produced an admissible broad-harness candidate edit. The edit stayed
outside protected `ploke-eval`, changed one file,
`crates/ploke-protocol/src/procedure.rs`, applied successfully through
`non_semantic_patch`, and was committed as
`7589596f5982e58bba07b6b48c639466d17c4a7c`.

The model did use cargo and tests, but only in the focused `ploke-protocol`
crate. It did not run the request-required `cargo check -p ploke-eval` or
`cargo test -p ploke-eval edit_surface`. Its strongest validation is
`cargo test --all-features` against `crates/ploke-protocol/Cargo.toml`, which
is useful for the changed crate but does not prove the protected request
contract.

The model saw tool-result feedback through the headless TUI relay. The clearest
chain is: `apply_code_edit` failed with an ambiguous `FanOut::run` target, the
model switched to `non_semantic_patch`, the patch staged and then applied, and
the model ran post-apply cargo checks/tests.

No stale same-file, `Content changed`, or `ContentMismatch` lifecycle problem
occurred in this attempt. The edit lifecycle issue was different:
`apply_code_edit` failed because the method target was ambiguous across seven
candidates, then the less structured patch tool succeeded.

Transition evidence is incomplete as authority evidence. The child-plan and
node files record `node-5a745aa89cb08600` as a planned child derived from the
`r12` commit, but the campaign tree has no `history/blocks`, no
`prototype1/evaluations`, and the transition journal contains only `r10`
materialization/build entries. Under the History/Crown model, those projections
and terminal summaries are evidence, not sealed transition authority.

## Evidence Roots

- Prototype root:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-fixed-20260525-123824/prototype1`
- Request:
  `messages/edit-harness-request/node-57e8487f70ce4abc-r12.json`
- Prompt markdown:
  `messages/edit-harness-request/node-57e8487f70ce4abc-r12.md`
- Submitted result:
  `messages/edit-harness-result/node-57e8487f70ce4abc-r12.json`
- Headless TUI trace:
  `messages/edit-harness-result/node-57e8487f70ce4abc-r12.headless-tui.json`
- Workspace:
  `workspaces/edit-harness/node-57e8487f70ce4abc-r12`
- Child node:
  `nodes/node-5a745aa89cb08600/node.json`
- Child runner request:
  `nodes/node-5a745aa89cb08600/runner-request.json`
- Parent child plan:
  `messages/child-plan/node-57e8487f70ce4abc.json`
- Campaign journal:
  `transition-journal.jsonl`

Expected normal eval run-root artifacts such as `record.json.gz`,
`agent-turn-trace.json`, `agent-turn-summary.json`,
`llm-full-responses.jsonl`, `validation-audit.json`,
`benchmark-patch-projection.json`, and `multi-swe-bench-submission.jsonl` were
not found for this child attempt under the campaign tree or r12 workspace. The
available child-specific record is the broad-harness headless JSON plus the git
workspace/commit.

## Prompt And Request

The prompt asked the model to modify the r12 candidate checkout to improve the
`Prototype 1 descendant performance` benchmark, stay outside protected core,
inspect `prototype1/nodes` if prior history was useful, and use the available
edit tools rather than creating separate result/bookkeeping files.

The request binding targeted
`artifact:git-commit:c374d2970947ca8068e16a55ab09025c3629a8df` with policy
`workspace except ploke-eval`. The request validation contract listed:

```text
cargo check -p ploke-eval
cargo test -p ploke-eval edit_surface
```

Prompt diagnostics in the headless trace show the workspace loaded, the focused
root was `crates/ploke-protocol`, BM25 was ready with 6514 docs, context mode
was off, no RAG parts were pre-attached, and the user prompt was one of four
messages.

## Tool Trace Reconstruction

The headless trace records 86 events in one completed turn. The high-signal
sequence is:

1. The model first ran `cargo check` in the focused `ploke-protocol` crate.
2. It listed and read the parent node files:
   `nodes/node-57e8487f70ce4abc/node.json` and `runner-request.json`.
3. It inspected workspace structure and `Cargo.toml`, then searched for
   `descendant`, `benchmark`, and `EVAL_CORE_SURFACE_ROOT`.
4. It read the protected-surface reference in
   `crates/ploke-eval/src/cli/prototype1_state/backend.rs`.
5. It ran `cargo test` in the focused `ploke-protocol` crate.
6. It read `crates/ploke-protocol/src/lib.rs`, `llm.rs`, `procedure.rs`,
   `tool_calls/trace.rs`, `tool_calls/review.rs`, and
   `tool_calls/segment.rs`.
7. `code_item_lookup` failed internally:
   `stored relation 'impl' does not have field 'name'`.
8. `apply_code_edit` against `crate::procedure::FanOut::run` failed because
   seven method candidates matched after method parsing.
9. The model switched to `non_semantic_patch`.
10. `non_semantic_patch` first returned `ok: true`, `staged: 1`,
    `applied: 0`, `auto_confirmed: false`.
11. The same proposal later returned `ok: true`, `applied: 1`,
    `partial: false`.
12. The model ran post-apply `cargo check`, `cargo test`, read
    `tool_calls/segment.rs`, then ran `cargo test --all-features`.
13. The terminal record reported `terminal: applied` and
    `Request summary: [success]`.

This is enough to say the model saw tool outputs at the TUI relay level: later
tool choices respond to earlier failures and applied-state changes. It is not
enough to reconstruct the provider's private reasoning or exact model message
text because no `llm-full-responses.jsonl` or full `agent-turn-trace.json` was
present for this broad-harness child.

## Edit Lifecycle

The semantic edit did not stage:

```text
apply_code_edit: Ambiguous method target for canon=crate::procedure::FanOut::run
... 7 candidates matched after method parsing.
```

The tool also emitted a second failure for the same call:

```text
apply_code_edit: Internal compiler error: apply_code_edit failed to stage proposal
```

The successful edit was the later `non_semantic_patch` call
`function-call-f2de4bf9-842c-4994-902c-9c6ce767b371`. Its lifecycle must be
read in two phases:

```text
requested -> staged only -> applied
```

The first successful completion was only staged:

```text
ok: true
staged: 1
applied: 0
files: ["crates/ploke-protocol/src/procedure.rs"]
auto_confirmed: false
```

The later completion and terminal record prove application:

```text
proposal_id: 67eb891e-8bff-57b1-ab95-a942a2665629
applied_proposal_ids: [67eb891e-8bff-57b1-ab95-a942a2665629]
changed_paths: [.../crates/ploke-protocol/src/procedure.rs]
```

There were no stale same-file or content-mismatch failures in this lifecycle.
The negative lifecycle example is instead a successful staged result that would
be misleading if counted as an applied edit before the later apply event.

## Applied Patch

The r12 workspace branch is
`prototype1-broad-broad-harness-request-node-57e8487f70ce4abc-r12`.
`git status --short --branch` showed it clean at the child commit. `git log`
shows:

```text
7589596f prototype1 broad harness result broad-harness-request:node-57e8487f70ce4abc:r12
c374d297 prototype1: initializing gen 0 parent node-57e8487f70ce4abc
```

The patch changes `FanOut::run` in `crates/ploke-protocol/src/procedure.rs`.
With the default `llm` feature enabled, it runs left and right branches
concurrently via `tokio::join!`; without the `llm` feature, it preserves the
existing sequential branch execution:

```text
#[cfg(feature = "llm")]
let (left_res, right_res) = tokio::join!(
    self.left.run(subject.clone()),
    self.right.run(subject.clone())
);

#[cfg(not(feature = "llm"))]
let left = match self.left.run(subject.clone()).await { ... };
#[cfg(not(feature = "llm"))]
let right = match self.right.run(subject.clone()).await { ... };
```

`crates/ploke-protocol/Cargo.toml` has `default = ["llm"]`, `llm` includes
`dep:tokio`, and `tokio` is optional in that crate. That makes the patched
`tokio::join!` path compile in the default/all-features validation modes that
the model ran.

The edit is plausibly benchmark-relevant because broad protocol fan-out can
execute independent branches concurrently. It is not proven benchmark-positive
by this attempt because no descendant performance benchmark, eval run, or
oracle record was produced for the r12 child.

## Cargo And Test Evidence

The model used cargo five times, all against
`crates/ploke-protocol/Cargo.toml`:

- pre-edit `cargo check`: pass, 0 errors, 0 warnings.
- pre-edit `cargo test`: pass, 0 errors, 0 warnings.
- post-edit `cargo check`: pass, 0 errors, 0 warnings.
- post-edit `cargo test`: pass, 0 errors, 0 warnings.
- post-edit `cargo test --all-features`: pass, 0 errors, 0 warnings.

The request-required commands were not run:

- Missing: `cargo check -p ploke-eval`.
- Missing: `cargo test -p ploke-eval edit_surface`.

So the validation signal is real but narrower than the request contract.

## Child Plan And Transition Evidence

The parent `child-plan` contains three children for the parent node. The r12
entry maps to:

```text
node_id: node-5a745aa89cb08600
patch_id: broad-harness:broad-harness-request:node-57e8487f70ce4abc:r12
derived_artifact_id: artifact:git-commit:7589596f5982e58bba07b6b48c639466d17c4a7c
candidate_id: broad-harness-g1-03
harness run_id: 7c5f6bea-fd61-4e34-a84d-91e032ec4a25
harness attempt_id: 67eb891e-8bff-57b1-ab95-a942a2665629
```

`nodes/node-5a745aa89cb08600/node.json` and `runner-request.json` exist and
record `status: planned`, `generation: 1`, the same derived artifact commit,
and runner args for `prototype1-runner`.

What is missing:

- no `nodes/node-5a745aa89cb08600/runner-result.json`;
- no child-local `record.json.gz`;
- no child-local `agent-turn-trace.json` or `llm-full-responses.jsonl`;
- no `prototype1/evaluations` directory;
- no `prototype1/history/blocks` directory;
- no r12 entries in `transition-journal.jsonl`.

The transition journal has entries for `r10` materialization and build only.
Therefore, for r12, the committed child workspace and child-plan node are
evidence of a candidate artifact, not proof of sealed History admission or
successor handoff. Treating the terminal `applied` summary or child-plan entry
as authority would violate the History/Crown distinction: projections are not
the sealed authority object.

## Positive Examples For Adjudication

- `tool_failure_recovery`: after `apply_code_edit` failed with an ambiguous
  canonical method target, the model switched tools and produced an applied
  patch instead of stopping.
- `post_apply_validation`: after the patch was applied, the model ran
  `cargo check`, `cargo test`, and `cargo test --all-features` in the changed
  crate.
- `protected_surface_awareness`: the model searched for and read
  `EVAL_CORE_SURFACE_ROOT`/backend protected-surface context before editing,
  and the final changed file was outside `crates/ploke-eval`.
- `benchmark_plausibility`: the patch targets concurrency in `FanOut::run`,
  a plausible descendant-performance surface, rather than making an unrelated
  docs or bookkeeping edit.

## Negative Examples For Adjudication

- `validation_scope_mismatch`: cargo passed, but the model did not run either
  request-specified `ploke-eval` command.
- `staged_vs_applied_ambiguity`: the first successful `non_semantic_patch`
  result was staged-only. An adjudicator should not count that as applied until
  the later applied completion or terminal record.
- `semantic_edit_tool_blocker`: `code_item_lookup` failed internally and
  `apply_code_edit` could not disambiguate a method target, so useful semantic
  editing still depended on a non-semantic patch.
- `authority_projection_gap`: child-plan and terminal summaries record a
  planned candidate artifact, but no History block, evaluation, runner result,
  or transition journal entry proves authority admission.
- `benchmark_unproven`: the edit is plausible and compiles, but no benchmark
  or oracle evidence proves it improves the descendant performance target.

## Answer Checklist

- Did the model produce an admissible edit? Yes, at the broad-harness edit
  policy level: one non-protected file changed, applied, committed, and
  represented as child `node-5a745aa89cb08600`.
- Did it use cargo/tests? Yes, five focused `ploke-protocol` cargo runs, all
  green. No, it did not run the request-required `ploke-eval` commands.
- Did the model see tool outputs? Yes, the headless TUI relay records
  tool-result events followed by responsive later tool choices. Full provider
  response records are absent, so exact raw model-visible message text cannot
  be reconstructed.
- Did stale same-file/content-mismatch/apply lifecycle problems occur? No
  stale same-file or content-mismatch problem occurred. The apply lifecycle did
  include a failed semantic edit and a staged-only patch result before the
  later applied result.
- Is transition evidence missing or misleading? Yes. The terminal and
  child-plan records are useful projections, but r12 lacks runner result,
  evaluation, History block, and transition-journal authority evidence.
