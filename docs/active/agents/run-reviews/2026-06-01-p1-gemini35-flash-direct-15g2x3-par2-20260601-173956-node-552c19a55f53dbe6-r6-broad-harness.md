# Prototype 1 Child Run Review: 2026-06-01 Broad Harness r6

Date: 2026-06-01

Campaign: `p1-gemini35-flash-direct-15g2x3-par2-20260601-173956`

Task: `BurntSushi__ripgrep-2209`

Parent node: `node-552c19a55f53dbe6`

Attempt: `node-552c19a55f53dbe6-r6`

Model route: `google/gemini-3.5-flash` through direct Google

Review status: complete for the available broad-harness child artifacts. This is
a broad headless-TUI attempt review, not a normal eval run-root review.

## Short Verdict

`r6` mechanically produced an applied broad-harness candidate. The headless TUI
terminal record reports `terminal: applied`, proposal
`62cf73c8-6c28-52c1-8d6c-583d7e962509`, and one changed path,
`crates/ploke-protocol/src/procedure.rs`. The candidate workspace is clean at
commit `00f2e426649987a63109d817474dc79f2b27eeb6`, whose parent is the request
binding target commit `774def86e034ac0e5cb6fd15842f2bc8abf18bc6`.

The edit is plausibly related to descendant-performance cost: it changes
`FanOut::run` so default-`llm` builds run the left and right branches with
`tokio::join!`, while non-`llm` builds keep sequential execution. This is a
reasonable target because `ploke-protocol` uses `FanOut` to combine independent
LLM adjudication branches such as usefulness, redundancy, and recoverability.

The benchmark value is not proven. The model ran focused `ploke-protocol` cargo
checks/tests, not the request-required `cargo check -p ploke-eval` and
`cargo test -p ploke-eval edit_surface`. No descendant eval, timing benchmark,
or oracle/MBE record was produced for `r6`. The patch also changes failure
semantics under the `llm` feature: if the left branch fails, the right branch has
already run before `FanOutError::Left` is returned, unlike the original
short-circuiting sequential implementation. Existing tests cover successful
output/artifact preservation, not branch-failure side effects, extra LLM spend,
or measured latency.

Authority evidence is intentionally weak. The submitted result declares
`authority_boundary.admission = not_claimed`, `grant = not_claimed`, and
`child_plan = not_claimed`. I found the request, headless trace, submitted
result, and candidate commit, but no child-plan, History block, transition entry,
runner result, or child eval artifact proving admission of this candidate.

## Evidence Roots

- Campaign root:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-par2-20260601-173956`
- Prototype root:
  `prototype1/`
- Request JSON:
  `prototype1/messages/edit-harness-request/node-552c19a55f53dbe6-r6.json`
- Prompt markdown:
  `prototype1/messages/edit-harness-request/node-552c19a55f53dbe6-r6.md`
- Headless TUI trace:
  `prototype1/messages/edit-harness-result/node-552c19a55f53dbe6-r6.headless-tui.json`
- Submitted result:
  `prototype1/messages/edit-harness-result/node-552c19a55f53dbe6-r6.json`
- Candidate workspace:
  `prototype1/workspaces/edit-harness/node-552c19a55f53dbe6-r6`
- Campaign state checked:
  `campaign.json`, `closure-state.json`, `prototype1/scheduler.json`,
  `prototype1/transition-journal.jsonl`
- Parent node records checked:
  `prototype1/nodes/node-552c19a55f53dbe6/node.json`,
  `prototype1/nodes/node-552c19a55f53dbe6/runner-request.json`
- Existing baseline eval/protocol review used only as surrounding context:
  `/home/brasides/code/ploke/docs/active/agents/run-reviews/2026-06-01-p1-gemini35-flash-direct-15g2x3-par2-20260601-173956-baseline-eval-protocol.md`

Expected normal eval run-root artifacts such as `record.json.gz`,
`agent-turn-trace.json`, `agent-turn-summary.json`, `llm-full-responses.jsonl`,
`validation-audit.json`, `benchmark-patch-projection.json`, and
`multi-swe-bench-submission.jsonl` exist for the baseline eval run referenced by
`closure-state.json`, but not for this `r6` broad-harness child attempt. The
child-specific trace surface is the headless TUI JSON plus the git workspace and
submitted result JSON.

## Prompt And Request

The prompt asked the model to modify the candidate checkout to improve
`Prototype 1 descendant performance`, stay outside protected core, read files in
the workspace, inspect `prototype1/nodes` if useful, and use the available edit
tools rather than creating separate bookkeeping files.

The request binding targeted:

```text
runtime_id: b65742a0-767e-4be0-83e9-f55dc5c518ac
target_artifact_id: artifact:git-commit:774def86e034ac0e5cb6fd15842f2bc8abf18bc6
policy_id: workspace except ploke-eval
```

The request validation contract listed:

```text
cargo check -p ploke-eval
cargo test -p ploke-eval edit_surface
```

The submitted result repeats the contract, records the changed file as
`crates/ploke-protocol/src/procedure.rs`, and returns the broad authority fields
as `not_claimed`. Its rationale is generic: headless `ploke-tui` produced a
bounded self-edit and expects `ploke-eval` to compile and evaluate the admitted
child artifact.

Prompt diagnostics in the headless trace prove the headless-TUI path rather than
a normal eval runner path: workspace loaded at the r6 candidate checkout,
focused root was `crates/ploke-protocol`, BM25 was ready with 6885 docs, context
mode was off, no RAG parts were pre-attached, and the user prompt was one of four
messages.

## Execution Path And Record Inventory

The active execution path for this artifact was:

```text
Prototype 1 broad-harness request
-> headless ploke-tui adapter path (`tui_adapter::run_headless_with_model` class of artifact)
-> headless TUI event stream and debug relay
-> non_semantic_patch proposal lifecycle
-> candidate workspace commit
-> submitted broad-harness result JSON
```

Evidence for that path is the artifact shape and joined git state: the trace has
`attempts`, `terminal`, `events`, `validations`, `debug_relay`, and
`prompt_diagnostics`; it has no run-root `record.json.gz` or provider response
ledger. The terminal block records proposal
`62cf73c8-6c28-52c1-8d6c-583d7e962509`; the candidate workspace branch
`prototype1-broad-broad-harness-request-node-552c19a55f53dbe6-r6` is clean at
commit `00f2e426`, and `git show HEAD` shows that same one-file patch.

Record-surface status for this review:

- campaign state: `present` for `campaign.json`, `closure-state.json`,
  `prototype1/scheduler.json`, and `prototype1/transition-journal.jsonl`.
- closure state: `present` and complete for the baseline eval/protocol instance;
  this is separate from the r6 broad-harness child artifact.
- transition journal: `present, manual join needed`; it contains only
  `parent_started` and `resource` rows for the parent start, with no r6
  materialization/admission row.
- parent node and runner request: `present`.
- parent runner result: `record absent` in the checked node directory.
- broad-harness request, prompt, headless trace, submitted result, and workspace:
  `present` for r6.
- candidate commit: `present` in the workspace git history, not a loose result
  file.
- child plan, child node, runner result, History block, and child-local eval
  artifacts for r6: `record absent` in the checked campaign tree.
- `prototype1/evaluations` and `prototype1/history/blocks`: `record absent` in
  this campaign tree, despite being named by the generic request evidence roots.
- protocol artifacts: `present` for the completed baseline eval run through the
  closure state's protocol root, but `record absent` as r6-local broad-harness
  adjudication.

## Tool Trace Reconstruction

The headless trace records one completed turn with 88 events:

```text
tool_request:   42
tool_completed: 41
tool_failed:    3
proposal:       1
turn:           1
```

High-signal sequence:

1. The model listed the workspace root, `crates`, `crates/ploke-protocol`,
   `src`, and `tool_calls`.
2. It ran a pre-edit `cargo test -p ploke-protocol`, which passed with 0 errors
   and 0 warnings in 19.818s.
3. It read `TECH_DEBT.md`, `AGENTS.md`, the parent node's `node.json` and
   `runner-request.json`, the prototype root, `run-profile.toml`, and the
   `edit-harness-result` directory.
4. It read the start of the r4 headless trace, searched for `performance`, read
   `crates/ploke-protocol/src/lib.rs`, requested context for
   `parse_protocol_json_content`, and read chunks of `llm.rs`, `procedure.rs`,
   `tool_calls/review.rs`, `step.rs`, and `tool_calls/trace.rs`.
5. It localized the target to `FanOut::run` by reading `procedure.rs` around
   lines 330-420.
6. `code_item_lookup` for `run` in `crates/ploke-protocol/src/procedure.rs`
   failed because multiple methods matched in `crate::procedure`.
7. `apply_code_edit` against `crate::procedure::FanOut::run` then failed because
   seven method candidates matched after method parsing, followed by an internal
   "failed to stage proposal" failure for the same call.
8. The model read lines 380-425 of `procedure.rs` to construct an exact diff.
9. It switched to `non_semantic_patch`; the first completion was staged-only
   (`ok: true`, `staged: 1`, `applied: 0`, `auto_confirmed: false`).
10. The proposal event recorded id
    `62cf73c8-6c28-52c1-8d6c-583d7e962509` and one path.
11. A later completion for the same patch call applied the edit
    (`ok: true`, `applied: 1`, `partial: false`).
12. The model ran post-apply `cargo test -p ploke-protocol` and a focused
    `cargo check`; both passed.
13. The turn outcome was `completed`, but the turn summary still carried
    `[success] code=TOOL_EXECUTION_FAILED` from the earlier failed semantic edit.

Concrete trace chain required by the quality gate:

```text
read procedure.rs around FanOut::run
-> model attempted semantic lookup/edit of `crate::procedure::FanOut::run`
-> lookup and apply_code_edit failed with ambiguous method target / 7 candidates
-> model interpreted this as needing an exact patch and reread lines 380-425
-> non_semantic_patch staged proposal 62cf73c8, then applied it
-> post-apply cargo test/check passed in the focused ploke-protocol crate
```

This is enough to say the model saw and used tool feedback through the headless
TUI relay. It is not enough to reconstruct raw provider messages or private
reasoning, because this broad-harness artifact has no `llm-full-responses.jsonl`.
The debug relay does include model-visible statements such as "I will attempt to
apply the semantic code edit", then "I will read lines 380 to 425 ... to make
sure we construct a perfectly matched unified diff", then "I will now apply a
non-semantic patch".

## Edit Lifecycle

The successful edit lifecycle must be read as a staged-then-applied proposal,
not as one immediate successful edit:

```text
requested non_semantic_patch
-> ToolCompleted: ok=true, staged=1, applied=0, auto_confirmed=false
-> proposal: 62cf73c8-6c28-52c1-8d6c-583d7e962509
-> ToolCompleted: ok=true, applied=1, partial=false
-> terminal: applied
```

The earlier failed semantic lifecycle was:

```text
code_item_lookup(run) -> tool_failed: multiple items matched
apply_code_edit(FanOut::run) -> tool_failed: ambiguous method target, 7 candidates
same apply_code_edit call -> tool_failed: internal failed to stage proposal
```

No stale same-file, content-mismatch, or `Content changed` lifecycle issue
appeared in r6. The relevant lifecycle pitfall is that the first successful
`non_semantic_patch` completion was only staged; an adjudicator should wait for
the later applied completion or terminal record before counting the edit as
applied.

## Applied Patch Verification

Direct git verification in the candidate workspace shows a clean branch:

```text
00f2e426 (HEAD -> prototype1-broad-broad-harness-request-node-552c19a55f53dbe6-r6)
  prototype1 broad harness result broad-harness-request:node-552c19a55f53dbe6:r6
774def86 prototype1: initializing gen 0 parent node-552c19a55f53dbe6
```

`git show HEAD --stat` reports:

```text
crates/ploke-protocol/src/procedure.rs | 17 +++++++++++++++--
1 file changed, 15 insertions(+), 2 deletions(-)
```

The actual patch in `procedure.rs` changes `FanOut::run` from sequential left
then right awaits into cfg-gated result collection:

```text
#[cfg(feature = "llm")]
let (left_res, right_res) = tokio::join!(
    self.left.run(subject.clone()),
    self.right.run(subject.clone())
);

#[cfg(not(feature = "llm"))]
let (left_res, right_res) = (
    self.left.run(subject.clone()).await,
    self.right.run(subject.clone()).await,
);

let left = match left_res { ... };
let right = match right_res { ... };
```

I verified the `tokio::join!` dependency claim against the checkout:
`crates/ploke-protocol/Cargo.toml` has `tokio = { workspace = true, optional =
true }`, `default = ["llm"]`, and `llm = ["dep:ploke-llm", "dep:reqwest",
"dep:tokio"]`. I also verified `tool_calls/review.rs` uses `FanOut` for the
`BranchPair` and `BranchSet` types that combine local usefulness, redundancy,
and recoverability adjudication branches.

Important caveat verified against the patch: default-`llm` behavior no longer
short-circuits the right branch when the left branch fails. Both futures are
polled to completion by `tokio::join!`; only after that does the code return
`FanOutError::Left` or `FanOutError::Right`. That may be acceptable for
independent adjudication branches, but it is a semantic change that the r6
validation did not test.

## Cargo And Test Evidence

The headless trace's validation list contains three cargo records:

- pre-edit `cargo test -p ploke-protocol`: pass, 0 errors, 0 warnings,
  19.818s, manifest path at the workspace root.
- post-edit `cargo test -p ploke-protocol`: pass, 0 errors, 0 warnings,
  1.520s, manifest path at the workspace root.
- post-edit `cargo check`: pass, 0 errors, 0 warnings, 13.876s, `scope:
  focused`, manifest path `crates/ploke-protocol/Cargo.toml`.

The request-required commands were not run:

- Missing: `cargo check -p ploke-eval`.
- Missing: `cargo test -p ploke-eval edit_surface`.

The final debug-relay assistant message overstates the validation by saying the
"full test suite passed successfully in 1.36 seconds". The persisted validation
record shows the 1.36/1.52s result was `cargo test -p ploke-protocol`, not a full
workspace or request-contract test. The later `cargo check` was also focused on
`ploke-protocol`, despite the display command appearing as `cargo check`.

So the validation signal is real for the changed crate and default feature set,
but narrower than the request contract and not a benchmark/performance proof.
There is no recorded `cargo fmt`, `clippy`, no-feature compile, branch-failure
regression test, or measured latency comparison.

## Mechanical Completion Vs Benchmark Usefulness

Mechanical completion:

- request file present;
- headless trace present;
- submitted result present;
- terminal state `applied`;
- proposal id applied;
- candidate workspace committed and clean;
- submitted result lists the changed file.

Benchmark usefulness:

- positive: the patch targets a plausible bottleneck in protocol fan-out for LLM
  adjudication branches;
- positive: focused compile/test evidence passed after the edit;
- negative: no requested `ploke-eval` validation ran;
- negative: no descendant performance benchmark, eval run, oracle, or MBE record
  measured the effect;
- negative: the changed failure semantics could increase work/cost on failing
  branches and was not tested.

Treat r6 as a mechanically applied, plausibly useful candidate, not as a proven
benchmark improvement.

## Authority And Admission Usefulness

The submitted result explicitly declines authority claims:

```text
authority_boundary.admission = not_claimed
authority_boundary.grant = not_claimed
authority_boundary.child_plan = not_claimed
```

That matches the campaign evidence I found. There is no r6 child-plan file, no
child node, no runner result, no r6 transition-journal row, no History block, and
no child-local eval/protocol artifact. The terminal `applied` result and git
commit are useful candidate-artifact evidence, but they are not sealed successor
admission evidence.

The campaign closure state does show the baseline eval and protocol completed:
1 expected/complete eval instance, 1 expected/full protocol instance, and all
three required procedures complete. That closure state belongs to the baseline
run rooted at `run-1780361227892-structured-current-policy-2a93bb07`; it should
not be reinterpreted as r6 child success.

## Protocol Review And Blind Spots

There is no r6-local protocol adjudication artifact. For r6, the only protocol
relevant signal is indirect: the broad-harness request used protocol diagnostics
as guidance and the edited file is inside `ploke-protocol`.

Blind spots visible from the r6 artifacts:

1. The terminal block correctly reports `applied`, but the turn summary still
   contains `[success] code=TOOL_EXECUTION_FAILED` from an earlier failed
   semantic edit. Consumers need proposal/terminal lifecycle joins, not a single
   summary string.
2. The first successful patch completion was staged-only. A protocol or replay
   view that collapses `ok:true` into applied would over-credit the edit.
3. The validation ledger does not compare actual commands to the request
   contract, so it can over-credit focused crate checks as if the required
   `ploke-eval` checks ran.
4. No branch-failure or performance measurement was produced, so the semantic
   risk of `tokio::join!` on fallible branches is invisible to the submitted
   result summary.

## Positive Examples And Adjudication Candidates

- `tool_failure_recovery`: after `code_item_lookup` and `apply_code_edit` failed
  on ambiguous `FanOut::run` targets, the model switched to an exact
  `non_semantic_patch` and produced an applied edit.
- `staged_vs_applied_lifecycle`: the trace has a clean staged-only completion
  followed by a later applied completion for the same call. This is a good
  adjudication fixture for not confusing `ok:true` with applied.
- `post_apply_validation`: the model ran cargo after the applied edit, and the
  changed crate passed with 0 errors and 0 warnings.
- `benchmark_plausibility`: the edit targets protocol fan-out concurrency rather
  than unrelated formatting/docs or bookkeeping.
- `validation_scope_mismatch`: the model's final answer overstates the cargo
  evidence. Actual persisted commands should be compared against request-listed
  validation commands.
- `authority_honesty`: the submitted result correctly marks admission/grant/child
  plan as `not_claimed`; reviewers should preserve that distinction.

## What Is Working

- Broad-harness request/result files and the headless trace are persisted and
  joinable by slot id.
- The headless trace exposes enough tool lifecycle to reconstruct recovery from
  semantic edit failure to non-semantic patch success.
- The terminal record, proposal id, submitted result, and git commit agree on the
  changed path.
- The model selected a plausible high-leverage performance surface and did not
  edit protected `ploke-eval` code.
- Focused crate validation passed after the edit.

## What Is Not Working Yet

- There is no r6-local raw provider ledger, normal eval record, benchmark patch
  projection, or protocol adjudication output.
- The broad-harness validation surface did not enforce or even flag missing
  request-contract checks.
- The final model summary overclaims "full test suite" relative to the persisted
  cargo records.
- Semantic edit tooling could not uniquely target `FanOut::run` and fell back to
  non-semantic patching.
- The candidate's central benchmark claim remains unmeasured.
- Authority/admission artifacts for r6 are absent, consistent with the
  submitted result's `not_claimed` fields.

## Action Items

1. Non-blocker: add validation-contract comparison for broad-harness attempts so
   reports distinguish focused crate checks from required request commands.
2. Non-blocker: keep staged and applied patch lifecycle states separate in any
   replay/protocol projection; the r6 `non_semantic_patch` call is a compact
   regression fixture.
3. Non-blocker: improve semantic edit disambiguation for methods in files with
   multiple `run` methods, or expose a retry shape that can identify the impl
   owner more precisely.
4. Candidate follow-up: if this patch is admitted for further evaluation, add or
   run tests/benchmarks that cover `FanOut` branch failure semantics and actual
   wall-clock improvement for independent LLM adjudication branches.
5. Authority follow-up: do not treat r6 as admitted until a child plan, History
   block, transition row, runner result, or equivalent authority artifact exists.

## Answer Checklist

- Did the model produce an admissible edit? Mechanically yes at the broad-harness
  edit level: one non-protected file changed, proposal applied, workspace
  committed cleanly.
- Did it use cargo/tests? Yes, focused `ploke-protocol` cargo checks/tests passed.
  No, it did not run the request-required `ploke-eval` commands.
- Did the model see and use tool outputs? Yes. The trace chain shows failure
  feedback from semantic tools, a reread for exact context, then a successful
  non-semantic patch and validation.
- Did stale same-file/content-mismatch problems occur? No. The lifecycle issue
  was staged-only versus applied, plus semantic target ambiguity.
- Is it benchmark-success evidence? No. It is plausible candidate evidence, not
  measured descendant-performance success.
- Is it authority/admission evidence? No. The submitted result says
  `not_claimed`, and the campaign lacks r6 child/admission artifacts.
