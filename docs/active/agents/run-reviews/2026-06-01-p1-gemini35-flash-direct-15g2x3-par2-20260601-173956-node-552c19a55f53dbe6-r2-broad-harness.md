# Prototype 1 Broad-Harness Review: node-552c19a55f53dbe6-r2

Date: 2026-06-01
Campaign: `p1-gemini35-flash-direct-15g2x3-par2-20260601-173956`
Parent node: `node-552c19a55f53dbe6`
Assigned slot: `node-552c19a55f53dbe6-r2`
Model route: `google/gemini-3.5-flash` through direct Google

## Verdict

The r2 broad-harness attempt did not mechanically complete as a candidate edit.
It produced a headless-TUI diagnostic trace, but the terminal state is
`timed_out` after 900 seconds, no submitted result file exists, the trace records
no proposal events, and the candidate workspace is clean at the admitted base
commit `774def86e034ac0e5cb6fd15842f2bc8abf18bc6`.

The attempt is therefore benchmark-useless for descendant performance: it ran
some cargo commands and inspected relevant Prototype 1 edit-surface files, but
it never staged, applied, submitted, or admitted a patch. The most useful review
signal is not a candidate improvement; it is an observability/tooling failure
chain where successful `read_file` calls returned empty content for valid line
ranges, the model chased truncated or low-information validation evidence, and
time expired without an edit.

This is a complete review of a mechanically incomplete child attempt. Evidence
is sufficient to classify the attempt as timed-out/no-edit; this is not an
incomplete-review placeholder.

## Evidence Roots

- Request JSON:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-par2-20260601-173956/prototype1/messages/edit-harness-request/node-552c19a55f53dbe6-r2.json`
- Request prompt:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-par2-20260601-173956/prototype1/messages/edit-harness-request/node-552c19a55f53dbe6-r2.md`
- Headless-TUI trace:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-par2-20260601-173956/prototype1/messages/edit-harness-result/node-552c19a55f53dbe6-r2.headless-tui.json`
- Expected submitted result path, absent in this review:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-par2-20260601-173956/prototype1/messages/edit-harness-result/node-552c19a55f53dbe6-r2.json`
- Candidate workspace:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-par2-20260601-173956/prototype1/workspaces/edit-harness/node-552c19a55f53dbe6-r2`
- Campaign closure state:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-par2-20260601-173956/closure-state.json`
- Run profile:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-par2-20260601-173956/prototype1/run-profile.toml`
- Baseline eval/protocol review for context:
  `/home/brasides/code/ploke/docs/active/agents/run-reviews/2026-06-01-p1-gemini35-flash-direct-15g2x3-par2-20260601-173956-baseline-eval-protocol.md`

Checked source/path evidence included:

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`, lines 1585-1603,
  where broad headless-TUI execution calls `tui_adapter::run_headless_with_model`
  and writes diagnostics.
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs`,
  lines 122-248, where the headless run starts, wraps the attempt in the budget
  timeout, and records `HeadlessTerminal::TimedOut` when the timeout fires.
- Candidate checkout `git status`, `git rev-parse HEAD`, and `git diff` from the
  r2 workspace.

## Closure State

Campaign-level closure is complete for the baseline eval/protocol run, not for
this r2 child attempt. `closure-state.json` reports one expected eval and one
expected protocol instance, both complete, with all required procedures complete.
That matches the existing baseline eval/protocol review.

The r2 artifact is a later broad-harness child slot. Its own headless trace
reports:

- `terminal.terminal`: `timed_out`
- `terminal.secs`: `900`
- `events`: 166 total, consisting of 83 `tool_request`, 81 `tool_completed`, and
  2 `tool_failed`
- no `proposal` events
- `debug_relay`: 128 retained entries, 127 dropped, and 52 truncated

## Eval And Patch Output

There is no r2 patch output.

The request declared the submitted result output box as:

```text
.../prototype1/messages/edit-harness-result/node-552c19a55f53dbe6-r2.json
```

Direct file verification found that path absent. Direct workspace verification
showed:

- branch: `prototype1-broad-broad-harness-request-node-552c19a55f53dbe6-r2`
- `HEAD`: `774def86e034ac0e5cb6fd15842f2bc8abf18bc6`
- `git status --short`: empty
- `git diff --stat HEAD`: empty
- `git diff --name-only HEAD`: empty

That `HEAD` matches the request admission target artifact
`artifact:git-commit:774def86e034ac0e5cb6fd15842f2bc8abf18bc6`. The absence of a
submitted result, proposal events, and checkout diff agree: the attempt did not
produce a candidate edit.

## Oracle And MBE State

The run profile has `[execution.mbe] enabled = false`. No r2 oracle, MBE, or
child-protocol artifact was found or expected for this timed-out broad-harness
attempt. Benchmark-facing evidence for r2 is only the attempted headless edit
trace and candidate workspace state, not an oracle verdict.

## LLM And Tool Behavior

The trace tool mix was:

- `read_file`: 44 requests
- `list_dir`: 23 requests
- `request_code_context`: 10 requests
- `cargo`: 5 requests
- `code_item_lookup`: 1 request

The two recorded tool failures were recoverable tool-surface failures, not patch
failures:

1. `function-call-197e5e57-a209-48fb-97cc-04d0a6b86147`: `list_dir` on
   `/home/brasides/.ploke-eval/campaigns` failed because that path was outside
   configured read roots.
2. `function-call-1bb4fd11-cce1-4ada-addd-3fd0f865fff9`:
   `code_item_lookup` failed to find
   `real_tui_resolver_touch_is_checked_before_adapter_apply` in
   `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs`.

Cargo behavior was mixed and not edit-producing:

- `function-call-a18bf271-647c-4988-9999-ae537665a539`: focused `cargo test`
  in `tests/fixture_crates/fixture_impls` passed.
- `function-call-7ba16a06-8b8a-4f1c-b6f1-dca07f683941`: focused `cargo check`
  in `tests/fixture_crates/fixture_impls` passed.
- `function-call-2f032cca-7dec-4b46-93e4-59ba0015bc14`:
  `cargo check -p ploke-eval` passed with 148 warnings.
- `function-call-a5cea7aa-f99f-4ebe-b3ef-8546ca52f127`:
  `cargo test -p ploke-eval -- edit_surface` failed with exit 101 and 230
  warnings.
- `function-call-13446bc3-85fd-4ba5-ba5e-08dcd4c214cc`:
  `cargo test -p ploke-eval --all-features -- edit_surface` also failed with
  exit 101 and 220 warnings.

The failed cargo tool previews do not retain the failing test names or relevant
stdout/stderr tail. They preserve exit status and summary counts, but not enough
semantic detail to reconstruct the actual validation failure from the trace
alone.

## Positive Examples And Adjudication Candidates

There is no positive patch chain for r2 because no edit was attempted. The
limited positive operational behavior is:

- The request/prompt was correctly rooted in the candidate workspace.
- The headless-TUI adapter captured a durable timed-out diagnostic trace.
- The model did run the requested broad validation targets
  (`cargo check -p ploke-eval` and `cargo test -p ploke-eval -- edit_surface`),
  and it retried the failing test with `--all-features` before timing out.
- The trace preserved enough tool lifecycle data to classify the attempt as
  no-edit/no-proposal.

Candidate adjudication signals this attempt should teach:

- A headless broad-harness attempt with `timed_out`, no proposal events, no
  submitted result, and a clean candidate workspace should score as
  benchmark-useless regardless of earlier successful cargo calls.
- Successful read calls with `ok:true`, `exists:true`, nonzero `byte_len`, but
  empty `content` for valid line ranges should be a negative information-success
  signal.
- Cargo validation summaries should not count as model-visible repair evidence
  unless the failing test name or actionable failure text is retained.
- Debug relay truncation/drop counts should be surfaced beside any claim about
  model interpretation, because here half the retained chain is already a
  reduced late-window view.

## Trace Reconstruction

The active execution path was:

```text
Prototype 1 child-plan broad-harness slot
-> published edit-harness request for node-552c19a55f53dbe6-r2
-> cli_facing.rs broad headless-TUI path
-> tui_adapter::run_headless_with_model
-> ploke-tui headless tools in the candidate workspace
-> edit-harness-result/node-552c19a55f53dbe6-r2.headless-tui.json
-> terminal timed_out after 900s, no submitted result
```

Evidence for that path:

- The request JSON names `request_id`
  `broad-harness-request:node-552c19a55f53dbe6:r2`, the r2 candidate workspace,
  and the r2 submitted result output box.
- The request prompt is the model-facing broad-harness instruction to modify the
  candidate checkout for `Prototype 1 descendant performance` while staying
  outside protected core.
- `cli_facing.rs` calls `tui_adapter::run_headless_with_model` for the published
  slot and then writes broad headless-TUI diagnostics.
- `tui_adapter.rs` wraps the run in the 900-second budget timeout and converts
  timeout to `HeadlessTerminal::TimedOut`.
- The trace `prompt_diagnostics.workspace.root` is the r2 candidate workspace,
  and the trace terminal is `timed_out` after 900 seconds.

Concrete trace chain:

```text
cargo test -p ploke-eval -- edit_surface fails
-> model says it will read around line 1772 to see the database relation
-> read_file function-call-cd7bb6c0... returns ok:true/exists:true but content:""
-> model searches/looks up the exact test name instead
-> request_code_context returns irrelevant/broad matches and code_item_lookup fails
-> model performs many overlapping reads, several also empty
-> retry with --all-features still fails
-> model continues browsing harness_request/docs/tools
-> timeout fires with no proposal, no submitted result, and no workspace diff
```

The suspicious read claim was verified directly against the candidate checkout.
Trace call `function-call-cd7bb6c0-97b8-4928-9324-57aa0c08b4e5` requested
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs` lines
1750-1800 and recorded `ok:true`, `exists:true`, `byte_len:80742`,
`truncated:true`, and `content:""`. Direct file inspection of the same checkout
shows those lines exist and include the target async test:

```text
1769 #[tokio::test(flavor = "multi_thread")]
1770 async fn real_tui_resolver_touch_is_checked_before_adapter_apply() {
1771     let fixture_db =
1772         Arc::new(fresh_backup_fixture_db(&FIXTURE_NODES_CANONICAL).expect("load fixture db"));
...
1799         confidence: Some(0.95),
1800     };
```

The failed `code_item_lookup` was also suspicious: it said no code item named
`real_tui_resolver_touch_is_checked_before_adapter_apply` existed in that file,
but the direct checkout read shows the function at line 1770. This may be an
indexing/module-path limitation rather than a file absence, but it compounded
the empty-read failure and sent the model into broad search.

The empty-read pattern was not isolated. Among parseable `read_file` completions,
I found 11 successful empty reads with valid-looking file paths and nonzero byte
lengths, including:

- `tests.rs` lines 1750-1800
- `tests.rs` lines 1600-1750
- `tests.rs` lines 1100-1250
- `tests.rs` lines 1500-2000
- `harness_request.rs` lines 1490-1530
- `harness_request.rs` lines 1201-1400
- `harness_request.rs` lines 1045-1060

Direct file inspection verified that `harness_request.rs` lines 1490-1530 and
1040-1079 also contain real content. For example, lines 1496-1530 contain the
`prompt_gives_minimal_edit_request_with_paths_and_metrics` test assertions, and
lines 1042-1063 define `SubmissionAuthorityBoundary` and
`SubmissionAuthorityClaim`.

The last point where the model had enough information to choose a broad action
was after the request had been read, `cargo check -p ploke-eval` had passed, and
`cargo test -p ploke-eval -- edit_surface` had failed. Instead of selecting a
small improvement or explicitly reporting blocked validation evidence, it tried
to debug the failure through low-information reads and broad searches. The trace
never reaches an edit/proposal point.

## Protocol Review And Blind Spots

No child-specific protocol review exists for r2. The campaign baseline protocol
completed mechanically for the original eval run, but that protocol is not a
semantic auditor for this broad-harness child attempt.

The r2 headless trace has useful lifecycle evidence, but its read-side surfaces
have blind spots:

1. Cargo failure detail is too compressed. The trace keeps exit code, warnings,
   and high-level status, but not enough model-visible failure text to know which
   assertion or test drove the later investigation.
2. `read_file` can return a successful empty payload for valid line ranges in
   real files. If protocol or selection later counts these as completed reads,
   it will over-credit information success.
3. `debug_relay` is explicitly lossy here: 127 entries were dropped and 52 were
   truncated. Any claim about model interpretation before the retained late
   window must be treated as unavailable.
4. The trace has no proposal/apply lifecycle to audit. No proposal event is
   missing from an otherwise successful edit; the edit lifecycle simply never
   started.

## Record Inventory

- request JSON and prompt: `present`
- request admission binding and target commit: `present`
- headless-TUI diagnostics: `present`
- submitted result JSON: `record absent`
- candidate workspace: `present, manual join needed`
- workspace git state: `present via manual checkout verification`, clean at the
  request target commit
- proposal lifecycle: `record absent`; trace contains no proposal events
- validation summaries: `present, playback gap` because failure details are
  truncated to summaries/previews
- model interpretation/debug relay: `present, playback gap` because retained
  window dropped/truncated many entries
- campaign baseline eval/protocol closure: `present`
- child r2 protocol/oracle/MBE: `not applicable` for this timed-out no-result
  child attempt

## What Is Working

- Broad-harness request publication preserved the workspace path, request hash,
  admission binding, expected output box, validation commands, and read roots.
- The headless adapter persisted a diagnostic JSON even though the attempt timed
  out.
- The tool lifecycle is sufficient to prove no edit was staged: there are no
  edit/proposal tools, no proposal events, no submitted result, and no git diff.
- The workspace write policy appears to have prevented accidental authority
  mutation; the attempted parent `/home/brasides/.ploke-eval/campaigns` listing
  was rejected as outside configured roots.

## What Is Not Working Yet

- The model spent the attempt on discovery/debugging and never made a candidate
  change.
- Successful empty `read_file` responses for valid ranges likely caused or
  amplified the repeated-read/search spiral.
- Exact symbol search/lookup did not recover from the empty read even though the
  function exists in the checkout.
- Cargo failure evidence was not retained with enough semantic detail for a
  reviewer or a later model to know what actually failed.
- Timeout/no-edit attempts currently require manual joins across request JSON,
  trace JSON, submitted-result absence, and git state to prove they are
  benchmark-useless.

## Action Items

1. Non-blocker / alive bug: fix or guard the headless `read_file` path so
   `ok:true`, `exists:true`, and a valid line range cannot produce empty content
   unless the file range is actually empty. Add an audit signal for this case.
2. Non-blocker / playback: persist actionable cargo failure detail for headless
   attempts, especially failing test names and stdout/stderr tail around the
   failure, not just exit code and warning counts.
3. Non-blocker / adjudication: classify timed-out broad-harness attempts with no
   proposal, no submitted result, and clean workspace as no-edit/no-benchmark
   signal automatically.
4. Non-blocker / observability: expose debug-relay dropped/truncated counts in
   any summary UI that invites reviewers to rely on model interpretation.
5. Follow-up review signal: if later attempts touch `edit_surface` or
   `harness_request` based on this r2 trace, require them to cite a reproduced
   validation failure or full persisted cargo output; this r2 artifact alone does
   not contain enough failure detail to justify a targeted code change.
