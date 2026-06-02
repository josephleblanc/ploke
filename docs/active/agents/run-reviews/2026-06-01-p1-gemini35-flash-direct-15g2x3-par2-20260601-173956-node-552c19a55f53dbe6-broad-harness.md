# Prototype 1 Broad-Harness Review: node-552c19a55f53dbe6

Date: 2026-06-01
Campaign: `p1-gemini35-flash-direct-15g2x3-par2-20260601-173956`
Assigned slot: `node-552c19a55f53dbe6`
Parent node: `node-552c19a55f53dbe6`
Review type: broad headless-TUI self-edit attempt

## Verdict

This broad-harness attempt is evidence-rich but mechanically incomplete. The
headless TUI applied edits to `crates/ploke-error/src/context.rs` and eventually
saw a focused `cargo test --all-features` success for the `ploke-error` crate,
but the terminal outcome was `timed_out` after 900 seconds and no submitted
broad-harness result JSON was written.

The attempt contains one useful local repair chain: the model observed a
`miette` compile failure, changed `ContextualError` from a derived transparent
diagnostic into a manual diagnostic forwarder, repaired two follow-up compile
failures, and got focused tests green. That is not enough to count as a useful
Prototype 1 descendant-performance result. The validation it ran was scoped to
`crates/ploke-error/Cargo.toml`, not the request contract commands
`cargo check -p ploke-eval` and `cargo test -p ploke-eval edit_surface`, and the
attempt timed out before returning the required evidence fields.

## Evidence Roots

- Campaign root:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-par2-20260601-173956`
- Campaign manifest:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-par2-20260601-173956/campaign.json`
- Run profile:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-par2-20260601-173956/prototype1/run-profile.toml`
- Published broad-harness request:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-par2-20260601-173956/prototype1/messages/edit-harness-request/node-552c19a55f53dbe6.json`
- Published prompt:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-par2-20260601-173956/prototype1/messages/edit-harness-request/node-552c19a55f53dbe6.md`
- Headless TUI trace:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-par2-20260601-173956/prototype1/messages/edit-harness-result/node-552c19a55f53dbe6.headless-tui.json`
- Expected submitted result path, verified absent:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-par2-20260601-173956/prototype1/messages/edit-harness-result/node-552c19a55f53dbe6.json`
- Candidate workspace:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-par2-20260601-173956/prototype1/workspaces/edit-harness/node-552c19a55f53dbe6`
- Execution-path source checked in the Ploke checkout:
  `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`

## Execution Path

The active path was not the baseline eval runner. It was the broad-harness
headless TUI path:

```text
Prototype 1 child_plan broad-harness request
-> run profile generation.source = broad-harness-request
-> run_broad_headless_tui_attempt
-> run_broad_headless_tui_attempt_with_options
-> GitWorktreeBackend::prepare_broad_harness_workspace
-> tui_adapter::run_headless_with_model
-> write_broad_headless_tui_diagnostics
-> finish_broad_headless_tui_attempt
-> TimedOut branch refuses to publish submitted broad-harness result
```

Evidence for that path:

- `run-profile.toml` has `[generation] source = "broad-harness-request"`.
- The request JSON has `request_id = "broad-harness-request:node-552c19a55f53dbe6"`,
  `parent_node_id = "node-552c19a55f53dbe6"`, and a candidate workspace under
  `prototype1/workspaces/edit-harness/node-552c19a55f53dbe6`.
- The request prompt asks the agent to modify that candidate checkout to improve
  `Prototype 1 descendant performance` and to stay outside the protected core.
- `cli_facing.rs` lines checked during review show
  `run_broad_headless_tui_attempt_with_options` preparing the broad harness
  workspace, reading the published prompt, building a TUI adapter budget, and
  calling `tui_adapter::run_headless_with_model`.
- The same file's `finish_broad_headless_tui_attempt` `TimedOut` branch reports
  that a timed-out headless TUI run, even after applying a proposal, refuses to
  publish a submitted broad-harness result.
- The headless trace records `terminal = { terminal: "timed_out", secs: 900 }`,
  and the expected `node-552c19a55f53dbe6.json` submitted-result file is absent.

The campaign manifest and closure state identify the overall campaign as
`google/gemini-3.5-flash` via direct Google. I did not find a raw provider/model
ledger inside this individual child headless trace, so I am not treating provider
identity as independently proved for the broad-harness attempt itself.

## Closure State

The baseline campaign closure is complete, as covered in the separate baseline
review. That closure does not mean this child broad-harness attempt completed.
For this slot, the relevant terminal state is the headless trace terminal:

```text
terminal: timed_out
secs: 900
```

The attempt therefore did not mechanically complete as an admitted child edit.
The absence of the submitted result JSON is expected for this terminal outcome,
because the implementation refuses to publish broad-harness results when the
headless TUI times out.

## Eval And Patch Output

This slot is a Ploke self-edit candidate, not a Multi-SWE-bench instance patch.
There is no child submitted-result JSON, no executor id, and no candidate result
payload with `change_summary`, `guiding_evidence`, `improvement_rationale`, or
`suggested_checks`.

The candidate workspace does contain one uncommitted source diff:

```text
M crates/ploke-error/src/context.rs
1 file changed, 59 insertions(+), 2 deletions(-)
```

Direct git diff verification in the candidate workspace shows that the final
change removed `#[cfg_attr(feature = "diagnostic", derive(miette::Diagnostic))]`
and `#[cfg_attr(feature = "diagnostic", diagnostic(transparent))]` from
`ContextualError`, then added a manual `#[cfg(feature = "diagnostic")] impl
miette::Diagnostic for ContextualError` that forwards `code`, `severity`,
`help`, `url`, `source_code`, `labels`, and `related` to the boxed source error.
Direct file inspection of the final workspace confirms the same manual impl at
`crates/ploke-error/src/context.rs` lines 76-143.

The trace records three proposal ids touching this same file:

- `b4350088-53de-5a23-8fd9-e4c1590a51eb`
- `a368c171-61e8-58d3-bf7c-e8343d66e03a`
- `ff966462-3f47-581b-86f6-52d35f131d18`

The event stream and debug relay need to be joined carefully. The first two edit
calls appear in the compact event stream as staged proposal completions, while
the trace-level `attempts` array and retained debug relay record them as applied.
The third `non_semantic_patch` call has an explicit applied completion in the
compact event stream. The final checkout diff is the authority for the resulting
workspace state.

## Oracle And MBE State

No oracle or MBE artifact exists for this broad-harness slot, and that is not the
right success surface for this attempt. The run profile has `[execution.mbe]
enabled = false`. The relevant outcome would have been a submitted broad-harness
result and later child admission/evaluation, but the timeout prevented that
submission.

## LLM And Tool Behavior

The headless trace has 139 compact events:

- 66 `tool_request`
- 70 `tool_completed`
- 3 `proposal`
- 0 `tool_failed`

It also has five validation records, all focused on the `ploke-error` crate:

1. `cargo test`, focused manifest `crates/ploke-error/Cargo.toml`, success.
2. `cargo test --all-features`, same focused manifest, compile failed with 2
   errors.
3. `cargo test --all-features`, same focused manifest, compile failed with 2
   errors.
4. `cargo test --all-features`, same focused manifest, compile failed with 4
   errors.
5. `cargo test --all-features`, same focused manifest, success.

The request contract asked for different checks:

- `cargo check -p ploke-eval`
- `cargo test -p ploke-eval edit_surface`

Those contract checks are present in the request JSON but absent from the trace
validation ledger. This is the main distinction between local compile progress
and benchmark-useful completion.

The model spent many early calls on broad or low-yield discovery: searching for
`benchmark`, `descendant`, `descendants`, `performance`, and `test`; listing
Prototype 1 state directories; reading Ploke error modules; and later reading
old commit-review notes under `crates/active/agents/2026-05-18_x-commit-review`.
Some of this discovered real context, but much of it did not connect directly to
the descendant-performance objective.

## Positive Examples And Adjudication Candidates

The strongest positive trace chain is a validation-repair loop:

```text
event 108: cargo test --all-features requested on focused ploke-error manifest
-> event 109: compile_failed; miette reports diagnostic(transparent) requires exactly one field
-> retained model message: it reads ContextualError lines 70-95 and looks up the enum
-> retained model message: it interprets the failure as caused by WithContext having two fields
-> event 114 / proposal b435...: apply manual miette::Diagnostic implementation
-> event 119-120: validation now fails with E0308 mismatched return types
-> retained model message: it decides to call miette::Diagnostic trait methods explicitly
-> event 121 / proposal a368...: update the manual impl forwarding calls
-> event 126-127: validation now fails with E0119 duplicate Diagnostic impl
-> retained model message: it reads lines 140-205 and removes the duplicate impl
-> event 132 / proposal ff966...: non-semantic patch removes the duplicate impl
-> event 136-137: cargo test --all-features succeeds on the focused ploke-error manifest
-> event 138: model starts another request_code_context search for Error::Fatal
-> terminal: timed_out after 900s; no submitted result is published
```

This is a good candidate adjudication signal for "validation failure led to a
targeted repair instead of blind repetition." It is also a negative signal for
"final successful validation did not lead to a timely final answer or submitted
result." The model had enough information to stop and submit after event 137,
but it continued exploring and hit the timeout.

A second useful signal is the focused-validation mismatch. The model-visible
cargo output was real and materially used, but it was the wrong validation scope
for the published broad-harness contract. Future protocol or run-review tooling
should distinguish "some cargo passed" from "the requested contract checks ran."

## Trace Reconstruction

1. The prompt loaded with context mode off and no automatic RAG parts. Prompt
   diagnostics show the workspace root as the candidate edit-harness directory
   and the focused root as `crates/ploke-error`.
2. The model first ran `cargo test` through the TUI cargo tool. That succeeded
   on the focused `ploke-error` manifest, not the whole workspace.
3. It did broad repository discovery: root listing, reading `Cargo.toml`, listing
   Prototype 1 node directories, searching `benchmark`, `descendant`,
   `descendants`, `performance`, `test`, and reading several
   `prototype1_state` and `ploke-error` files.
4. It narrowed to `crates/ploke-error/src/context.rs` and related error modules,
   especially `with_backtrace`, `DiagnosticInfo`, `result_ext`, severity, and
   pretty-printing helpers.
5. It detoured into stale or indirect documentation under
   `crates/active/agents/2026-05-18_x-commit-review`, then ran
   `cargo test --all-features`. This revealed the `miette` transparent derive
   failure in `ContextualError`.
6. It applied the first manual-diagnostic edit, then saw a type mismatch because
   forwarding calls like `source.severity()` did not resolve to the desired
   `miette::Diagnostic` trait method return type.
7. It applied the second edit to call `miette::Diagnostic::severity(source.as_ref())`
   and analogous trait methods explicitly, then saw a duplicate-impl conflict.
8. It inspected the file around the duplicate impl and removed the stale second
   implementation with a non-semantic patch.
9. The final focused `cargo test --all-features` succeeded.
10. Instead of ending, the model issued one more `request_code_context` call for
    `Error::Fatal`; the run then timed out at 900 seconds.

The last point where the model had enough information to act was immediately
after the final focused validation success. The useful local repair had been
made, and continuing discovery after that success converted an applied diff into
an unsubmitted timed-out attempt.

## Protocol Review And Blind Spots

There are no protocol adjudication artifacts for this broad-harness child slot
comparable to the baseline eval/protocol review. The review had to manually join
request JSON, prompt text, the headless trace, implementation source, the
candidate workspace diff, and file-existence checks.

Observed blind spots:

1. The compact event stream alone is not enough to classify proposal lifecycle.
   For the first two edits it shows staged completions, while the attempts array
   and debug relay record applied proposals. Reviewers must join these surfaces
   and verify the final checkout diff.
2. The validation ledger records cargo success, but the scope is focused on
   `crates/ploke-error/Cargo.toml`. Without checking the request contract, that
   could be over-credited as successful broad-harness validation.
3. The debug relay retained 128 records, reports 84 dropped and 42 truncated,
   and mainly preserves late messages. This is a record-present playback gap for
   full natural-language reasoning, not a total record absence.
4. A timed-out run can still leave a useful-looking workspace diff. The
   implementation correctly refuses to publish a submitted result for this
   terminal outcome, but downstream dashboards should make the applied-diff vs
   admitted-result split obvious.

## Mechanical Completion Vs Benchmark Usefulness

Mechanical completion:

- Headless trace: present.
- Terminal state: `timed_out` after 900 seconds.
- Proposal attempts: present, three proposal ids touching one file.
- Candidate workspace diff: present.
- Submitted result JSON: absent.
- Executor/admission evidence: absent.

Benchmark usefulness:

- The local patch plausibly fixes a `ploke-error` all-features compile issue.
- It does not directly target the published contract checks for `ploke-eval` or
  `edit_surface`.
- There is no evidence that it improves the Prototype 1 descendant-performance
  benchmark or future child selection.
- Because no submitted result was published, the attempt has no authority as an
  admitted child artifact.

Authority/admission usefulness:

- The request's return-evidence authority boundary was `not_claimed` for
  admission, grant, and child plan.
- Since the submitted result file is absent, even that non-claiming return
  evidence was never bound into an artifact.
- The right classification is a timed-out partial with a verified workspace diff,
  not a successful broad-harness child result.

## Record Inventory

- `campaign.json`: present.
- `closure-state.json`: present for the baseline campaign state.
- `prototype1/run-profile.toml`: present; generation source is
  `broad-harness-request`.
- Request JSON and prompt markdown for this slot: present.
- Headless TUI diagnostics/trace JSON: present.
- Candidate workspace: present.
- Final checkout diff: present, manual join needed via git in the workspace.
- Submitted broad-harness result JSON: record absent for this slot.
- Proposal lifecycle: record present, manual join needed across compact events,
  trace-level attempts, debug relay, and final git diff.
- Full assistant/provider transcript: record present only partially for review;
  debug relay reports dropped/truncated messages, so this is a playback gap.
- Baseline eval/protocol artifacts: present but separate from this child attempt.
- Oracle/MBE: not applicable for this profile and slot.

## What Is Working

- The broad-harness path records a useful headless trace even for timed-out
  attempts.
- The implementation refuses to publish a submitted result for a timed-out
  headless TUI attempt, which protects admission from partial artifacts.
- The model used compile diagnostics to make targeted repairs rather than
  ignoring validation output.
- The final candidate workspace diff is directly verifiable and localized to a
  single non-protected file.

## What Is Not Working Yet

- The model did not complete the broad-harness contract. It timed out without a
  submitted result even after achieving a local validation success.
- The validation tool scope was misleading for this request: all recorded cargo
  checks targeted `ploke-error`, while the request contract targeted
  `ploke-eval` and `edit_surface`.
- The run spent many tool calls on broad, low-signal discovery and stale review
  notes before and after the useful repair.
- The trace makes proposal lifecycle reconstructable, but not from one surface.
  A reviewer has to join compact events, attempts, debug relay, and git diff.
- No artifact connects the local `ploke-error` change to descendant-performance
  improvement or child admission.

## Action Items

1. Non-blocker: add a broad-harness review/protocol signal for validation-scope
   mismatch: recorded cargo success should identify whether it ran the request
   contract commands or only a focused crate command.
2. Non-blocker: improve headless TUI terminalization after a successful
   post-edit validation. If an applied edit has just passed validation near the
   timeout budget, the agent should be nudged or forced to return rather than
   continuing discovery.
3. Non-blocker: expose proposal lifecycle as a single joined playback table:
   call id, staged result, proposal id, applied result, final file hash, and
   final checkout diff status.
4. Non-blocker: surface timed-out-applied attempts explicitly in fan-in views as
   `partial/applied-diff/no-submission`, so downstream reviewers do not confuse
   workspace diffs with admitted child artifacts.
5. Follow-up adjudication candidate: preserve this trace as a positive example
   of validation-driven local repair, but label it negative for benchmark
   usefulness because it missed the requested validation scope and timed out.
