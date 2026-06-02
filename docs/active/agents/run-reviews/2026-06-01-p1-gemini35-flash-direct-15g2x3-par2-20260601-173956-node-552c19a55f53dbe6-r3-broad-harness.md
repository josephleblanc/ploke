# Prototype 1 Child Run Review: 15g2x3-par2 Broad Harness r3

Date: 2026-06-02

Campaign: `p1-gemini35-flash-direct-15g2x3-par2-20260601-173956`

Parent node: `node-552c19a55f53dbe6`

Attempt: `node-552c19a55f53dbe6-r3`

Review status: complete for the available broad-harness headless-TUI trace and
workspace evidence. This is not a normal eval run-root review, and it is not an
admissible child-result review because the attempt timed out and did not publish
a submitted broad-harness result.

## Short Verdict

`r3` produced a dirty candidate workspace with one modified file,
`crates/ploke-db/src/helpers.rs`, but the headless TUI terminal was
`timed_out` after 900 seconds. The request's `submitted_result_path` points at
`messages/edit-harness-result/node-552c19a55f53dbe6-r3.json`, but that file is
absent; only the diagnostic
`node-552c19a55f53dbe6-r3.headless-tui.json` exists. Mechanically, this should
be treated as a trace-bearing incomplete/failed broad-harness attempt, not an
admitted candidate.

The model made a plausible performance-oriented edit in `ploke-db`: it reordered
CozoScript constraints in `graph_resolve_exact`, `graph_resolve_edges`, and
`resolve_nodes_by_canon`, and inlined `node_with_context` into the edge rules.
That is outside the protected `ploke-eval` core. Benchmark usefulness is not
established: there is no descendant-performance measurement, no oracle/MBE
record, no submitted result, and no final validation after the last same-file
proposal.

The most important lifecycle finding is that validation and edit state are not
aligned. The trace records three applied proposals in the top-level `attempts`
array, but the ordered `events` stream ends immediately after staging/proposing
the third proposal. Direct workspace inspection verifies the final file contains
third-proposal changes. A reviewer therefore has to manually join top-level
attempts, event stream, and git diff to know what actually happened.

## Evidence Roots

- Campaign root:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-par2-20260601-173956`
- Request JSON:
  `prototype1/messages/edit-harness-request/node-552c19a55f53dbe6-r3.json`
- Request prompt markdown:
  `prototype1/messages/edit-harness-request/node-552c19a55f53dbe6-r3.md`
- Headless TUI diagnostic trace:
  `prototype1/messages/edit-harness-result/node-552c19a55f53dbe6-r3.headless-tui.json`
- Expected submitted result path, absent on disk:
  `prototype1/messages/edit-harness-result/node-552c19a55f53dbe6-r3.json`
- Candidate workspace:
  `prototype1/workspaces/edit-harness/node-552c19a55f53dbe6-r3`
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

Evidence tying `r3` to that path:

- The request id is `broad-harness-request:node-552c19a55f53dbe6:r3`.
- The request's candidate workspace is the inspected
  `.../workspaces/edit-harness/node-552c19a55f53dbe6-r3` directory.
- The diagnostic artifact has the exact extension produced by
  `broad_headless_tui_diagnostics_path`: `.headless-tui.json`.
- The diagnostic top-level `terminal` object is `{ "terminal": "timed_out",
  "secs": 900 }`.
- The normal submitted result JSON is absent, matching the timed-out finish path
  that refuses publication.

This is not the baseline eval path
`prototype1-step -> runner.rs::run_benchmark_turn -> record.json.gz -> protocol`.
Normal eval/protocol artifacts exist for the baseline run reviewed separately,
but not for this broad-harness child attempt.

## Prompt And Request

The prompt asked the model to modify the r3 candidate checkout to improve the
`Prototype 1 descendant performance` benchmark, stay outside protected core,
inspect repository/evidence, and stage a candidate change with the edit tools.
The prompt named the protected core via
`crates/ploke-eval/src/cli/prototype1_state/backend.rs::EVAL_CORE_SURFACE_ROOT`
and `WORKSPACE_EXCEPT_AUTHORITY_*`.

The request JSON adds details the model later read from the trace:

- admission binding target:
  `artifact:git-commit:774def86e034ac0e5cb6fd15842f2bc8abf18bc6`
- policy id: `workspace except ploke-eval`
- child budget: min 1, max 1
- evaluation scope: `prototype1_descendant_performance`
- requested validation contract:
  `cargo check -p ploke-eval` and `cargo test -p ploke-eval edit_surface`
- return-evidence authority boundary: `admission=not_claimed`,
  `grant=not_claimed`, `child_plan=not_claimed`

Prompt diagnostics in the headless trace show the workspace loaded successfully,
focused root `crates/ploke-db`, BM25 ready with 6885 docs, context mode off,
`max_leased_tokens` 2048, and four prompt messages.

## Closure State And Baseline Context

Campaign closure is mechanically complete for the baseline eval and protocol:
registry complete, eval complete, protocol complete, and all three required
procedures complete. That closure is baseline evidence, not r3 child-attempt
evidence. For r3, there is no `record.json.gz`, protocol artifact family,
Multi-SWE-bench submission, benchmark patch projection, or oracle/MBE result.

The prototype scheduler still lists `node-552c19a55f53dbe6` as the frontier node
with the parent node `status: planned`. The parent node directory contains only
`node.json` and `runner-request.json`; no `runner-result.json` exists for the
parent node directory in the checked surface.

## Eval, Patch Output, Oracle, And MBE State

Mechanical broad-harness result:

- diagnostic trace: present
- terminal: `timed_out` after 900 seconds
- submitted result JSON: absent
- child/admission result: absent
- workspace: dirty with one modified file

Patch output:

- Workspace branch:
  `prototype1-broad-broad-harness-request-node-552c19a55f53dbe6-r3`
- Workspace HEAD:
  `774def86e034ac0e5cb6fd15842f2bc8abf18bc6`
- `git status --short`: `M crates/ploke-db/src/helpers.rs`
- `git diff --stat`: 1 file changed, 26 insertions, 25 deletions

Oracle/MBE state: no oracle, MBE, or descendant-performance result is attached
to this r3 attempt. The baseline profile has MBE disabled, and this broad child
never reached published/admitted result status.

## LLM And Tool Behavior

The headless trace records 103 events:

- `tool_request`: 48
- `tool_completed`: 52
- `proposal`: 3
- no `turn` event and no final answer event in the ordered event stream

Top-level validations recorded by the diagnostic trace:

1. `cargo check`, focused `crates/ploke-db/Cargo.toml`, `ok: true`, exit 0,
   0 errors, 0 warnings. This happened before the edit phase.
2. `cargo test`, focused `crates/ploke-db/Cargo.toml`, `ok: false`, exit 101,
   `tests_failed_or_runtime`, 0 errors, 2 warnings. This also happened before
   the edit phase.
3. `cargo check`, focused `crates/ploke-db/Cargo.toml`, `ok: true`, exit 0,
   0 errors, 0 warnings. This happened after the first applied proposal but
   before the second and third proposal events.

The model did not run the request-contract commands
`cargo check -p ploke-eval` or `cargo test -p ploke-eval edit_surface`. It also
did not run a final validation after the final dirty workspace state.

## Trace Reconstruction

Concrete high-signal chain:

1. The model began with repository and campaign discovery: `list_dir` on the
   checkout, `crates`, parent node directory, and edit-harness workspace root.
2. It ran focused `cargo check` and focused `cargo test` in `ploke-db`. Check
   passed; test failed with exit 101 before any edit was made.
3. It searched for `descendant` with `request_code_context`; the tool returned
   `ok: true` but `context_len: 0` and the note "No indexed snippets matched
   `descendant`."
4. Instead of stopping on that low-information result, the model read `AGENTS.md`,
   the prototype `run-profile.toml`, the r3 prompt markdown, and the r3 request
   JSON, then listed the evidence roots named by the request. It observed missing
   `prototype1/evaluations` and missing `prototype1/history/blocks`.
5. It searched for `benchmark` and `prototype1_descendant_performance`, then read
   `crates/ploke-eval/src/cli/prototype1_state/backend.rs` for protected-surface
   context.
6. It moved into `ploke-db`: listed `crates/ploke-db`, listed
   `crates/ploke-db/benches`, read `AI_NOTES.md`, `COZO_HNSW.md`, and
   `benches/resolver_bench.rs`, then searched for `graph_resolve_exact`.
7. It read `crates/ploke-db/src/helpers.rs`, schema files, `file_mod`,
   `ANCESTOR_RULES_NOW`, `COMMON_FIELDS_EMBEDDED`, and `raw_query`, then used
   exact `code_item_lookup` for `graph_resolve_exact`, `graph_resolve_edges`,
   and `resolve_nodes_by_canon`.
8. It issued `apply_code_edit` for all three helper functions. The staged result
   was followed by proposal `01a61fd1-39c1-56c0-9cce-1680463cec1b`; a later
   completion for the same call records `applied: 1`.
9. It ran `cargo check` after that first application and saw success in the
   focused `ploke-db` crate.
10. It read `helpers.rs` again, then issued separate same-file edits for
    `graph_resolve_edges` and `resolve_nodes_by_canon`. Proposal
    `e58231d1-2b74-5eea-bc88-1792bcf5161d` records an applied completion;
    proposal `cdb61c12-8b7a-578c-a39b-d6d44572134d` appears only as a staged
    proposal in the ordered event list, while the top-level attempts array and
    workspace diff prove it affected the final file.
11. The terminal then timed out at 900 seconds. There is no final answer, no
    final validation, and no submitted result JSON.

The last point where the model had enough information to act was before event
86, after it had localized the three helper functions and supporting Cozo rules.
It did act, but the attempt did not complete the validation/admission contract.

## Edit Lifecycle

The edit lifecycle needs to be read per call id, not from the first
`ToolCompleted` alone.

First edit call:

```text
request event 86: apply_code_edit for graph_resolve_exact,
                  graph_resolve_edges, resolve_nodes_by_canon
complete event 87: ok=true, staged=3, applied=0
proposal event 88: 01a61fd1-39c1-56c0-9cce-1680463cec1b, edit_count=3
complete event 89: ok=true, staged=3, applied=0
complete event 90: ok=true, applied=1, new_file_hash=65b8bdcf-...
validation events 91-92: focused cargo check succeeds
```

Second edit call:

```text
request event 95: apply_code_edit for graph_resolve_edges
complete event 96: ok=true, staged=1, applied=0
proposal event 97: e58231d1-2b74-5eea-bc88-1792bcf5161d
complete event 98: ok=true, staged=1, applied=0
complete event 99: ok=true, applied=1, new_file_hash=9854ab96-...
```

Third edit call:

```text
request event 100: apply_code_edit for resolve_nodes_by_canon
complete event 101: ok=true, staged=1, applied=0
proposal event 102: cdb61c12-8b7a-578c-a39b-d6d44572134d
ordered events end here
```

The top-level `attempts` array nevertheless lists all three proposal ids as
`result: applied`. Direct checkout verification agrees that the final workspace
includes changes in `resolve_nodes_by_canon`, so the third proposal did affect
the file even though the ordered event stream does not include its applied
completion. This is a `record present, manual join needed` / playback gap, not a
reason to assume the third proposal failed.

## Applied Patch Verification

I verified the important tool/result claim against the workspace instead of
trusting the trace summary alone:

- `git status --short` in the r3 workspace reports exactly one modified file:
  `crates/ploke-db/src/helpers.rs`.
- `git diff --stat` reports 26 insertions and 25 deletions.
- `git diff` shows the candidate changed the CozoScript in
  `graph_resolve_exact`, `graph_resolve_edges`, and `resolve_nodes_by_canon`.
- Direct file reads of the r3 workspace confirm final changed lines:
  - `graph_resolve_exact` now binds `file_path`, `mod_path`, and `name` before
    the `file_mod`, `module`, `file_owner_for_module`, relation, and `ancestor`
    clauses.
  - `graph_resolve_edges` now uses the same early bindings and inlines the
    former `node_with_context` clauses into `edges_from_focus` and
    `edges_to_focus`.
  - `resolve_nodes_by_canon` now binds `mod_path` and `name` before the module
    and relation clauses.
- Reading the base checkout at `/home/brasides/code/ploke/crates/ploke-db/src/helpers.rs`
  confirmed the original code used trailing comparisons such as
  `name == {item_name_lit}`, `file_path == {file_path_lit}`, and
  `mod_path == {mod_path_lit}` and used a separate `node_with_context` rule.

The edit is plausibly aimed at descendant performance because it tries to
constrain Cozo queries earlier and remove an intermediate rule. It is not proven
correct or faster by this attempt. The only post-edit validation is a focused
`cargo check`, and that check occurred before the later same-file proposals.

## Positive Examples And Adjudication Candidates

Positive examples:

- The model did not over-credit the empty `descendant` retrieval result. It used
  that failure to inspect request/evidence files and then searched more specific
  terms.
- The model localized the actual edited functions with exact lookup after broad
  search and file reads.
- It ran a post-apply focused `cargo check` after the first proposal and saw
  compiler success.

Candidate adjudication signals:

- Successful retrieval with `context_len: 0` should be scored as low-information,
  even with `ok: true`.
- Validation should be tied to the final applied file hash or last applied
  proposal id. A validation that precedes later same-file proposals should not
  certify the final candidate.
- Broad-harness child reviews need a first-class distinction between
  `diagnostic trace exists`, `submitted result published`, and `child admitted`.
- Request-contract validation commands should be surfaced separately from the
  TUI's focused cargo convenience commands.

## Protocol Review And Blind Spots

There is no r3 protocol pass to review. The protocol artifacts cited by closure
belong to the baseline eval run, not this child broad-harness attempt.

The headless-TUI diagnostic itself has concrete blind spots:

1. The ordered event stream does not include the applied completion for the
   third proposal, while the top-level `attempts` array and workspace prove the
   final file changed.
2. Validation records are not associated with the final applied proposal/file
   hash. The trace makes it easy to overstate validation coverage unless a
   reviewer manually orders validation events against edit events.
3. The terminal summary says only `timed_out`; the actionable reason is stronger
   when joined with source code: timed-out runs are refused publication, so no
   submitted broad-harness result should exist.
4. There is no normal provider-response ledger such as `llm-full-responses.jsonl`
   for this broad child. The trace is enough for tool lifecycle review, but not
   for full provider-message replay.

## Record Inventory

- campaign closure state: `present`; complete for baseline eval/protocol only
- scheduler state: `present`; parent node remains the frontier/planned node
- parent node record and runner request: `present`
- parent runner result: `record absent` in the checked parent node directory
- broad-harness request JSON and prompt markdown for r3: `present`
- broad-harness headless-TUI diagnostic for r3: `present`
- submitted broad-harness result JSON for r3: `record absent`, expected because
  timed-out runs are refused publication
- r3 workspace: `present`, dirty, one modified file
- r3 candidate commit/admitted child node: `record absent`
- `prototype1/evaluations`: `record absent` for this child surface
- `prototype1/history/blocks`: `record absent` in the checked campaign surface
- normal eval run-root artifacts for r3 (`record.json.gz`,
  `agent-turn-trace.json`, `llm-full-responses.jsonl`, validation audit,
  benchmark projection, Multi-SWE-bench submission): `not applicable` / `record
  absent` for this broad-harness child attempt
- baseline eval/protocol artifacts: `present`, reviewed in the separate baseline
  report

## What Is Working

- The broad-harness diagnostic preserved enough tool lifecycle to reconstruct the
  search, edit, validation, and timeout sequence.
- The workspace survived with a concrete git diff, allowing independent
  verification of proposal effects.
- The timeout path correctly did not publish a submitted result JSON.
- The edit policy boundary was respected at the file level; the candidate touched
  `ploke-db`, not protected `ploke-eval` core.

## What Is Not Working Yet

- Timed-out attempts can leave applied dirty workspaces and top-level applied
  attempts without a submitted/admitted result. Reviewers must not confuse that
  with candidate success.
- The event stream is incomplete for the final proposal's apply lifecycle.
- The only successful post-edit validation was not final-state validation.
- The model ignored or failed to satisfy the request's validation contract for
  `ploke-eval` commands.
- The attempt offers no benchmark-useful evidence beyond a plausible edit idea;
  there is no performance delta, oracle, MBE, or descendant run.

## Action Items

1. Non-blocker / trace quality: include applied-completion events for every
   proposal listed as applied in the top-level `attempts` array, or explicitly
   mark when the ordered event stream was truncated by timeout.
2. Non-blocker / validation accounting: tie each validation record to the latest
   applied proposal id and file hash at the time it ran. Flag validation as stale
   when later proposals modify the same file.
3. Non-blocker / prompt-contract audit: distinguish focused cargo convenience
   checks from the request's declared validation commands. For this request, the
   required `ploke-eval` commands were not run.
4. Fan-in guidance: treat r3 as a trace-bearing timed-out broad-harness attempt,
   not a durable successful child. Do not promote its patch to child/admission
   evidence without a new run that publishes a submitted result and validates the
   final state.
5. Candidate idea, if reused manually: benchmark or unit-test the Cozo query
   reordering in `helpers.rs` before considering it useful. This attempt only
   shows that one intermediate state compiled under focused `ploke-db` check.
