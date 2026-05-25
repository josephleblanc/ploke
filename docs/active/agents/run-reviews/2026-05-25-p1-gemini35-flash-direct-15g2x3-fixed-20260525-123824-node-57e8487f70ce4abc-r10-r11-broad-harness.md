# Prototype 1 Child Run Review: Fixed 15g2x3 Broad Harness r10/r11

Date: 2026-05-25

Campaign: `p1-gemini35-flash-direct-15g2x3-fixed-20260525-123824`

Parent node: `node-57e8487f70ce4abc`

Attempts: `node-57e8487f70ce4abc-r10` and `node-57e8487f70ce4abc-r11`

Review status: complete for the available broad-harness child artifacts. This
review intentionally keeps the two child attempts separate and does not treat
baseline closure or protocol artifacts as authority for either child attempt.

## Short Verdict

Both attempts produced admissible broad-harness candidate edits in clean child
workspaces. Both edits were applied and committed on branches whose parent is
the request target commit `c374d2970947ca8068e16a55ab09025c3629a8df`. Neither
attempt has authority-bearing History admission evidence under the Prototype 1
campaign tree.

`r10` is the stronger validation run. It changed one non-protected file,
`crates/ingest/syn_parser/src/parser/graph/parsed_graph.rs`, and committed
`7f044a6556df167e93498e7fd5443527ebfe9a61`. The model ran cargo, including
`cargo check -p ploke-eval`, but did not run the request-required
`cargo test -p ploke-eval edit_surface`. Its `cargo test -p syn_parser` calls
failed. The final applied edit is still an admissible broad-surface edit.

`r11` changed three non-protected files,
`crates/ingest/syn_parser/src/parser/graph/parsed_graph.rs`,
`crates/ingest/syn_parser/src/parser/graph/mod.rs`, and
`crates/ingest/syn_parser/src/resolve/module_tree.rs`, and committed
`97d5d7decde4e8093401a657c6a28ab45540164f`. It ran several focused cargo
checks/tests, but the focused manifest was often
`proc_macros/syn_parser/ploke-test-macros/Cargo.toml`. It did not run
`cargo check -p ploke-eval` or `cargo test -p ploke-eval edit_surface`, and
`cargo test -p syn_parser` failed. Its final summary overstates validation.

Neither attempt showed stale same-file hash/content-mismatch failures. Both
showed the same lifecycle pattern: semantic `apply_code_edit` failed because
the graph node lookup found no matching node for the supplied canonical path,
then `non_semantic_patch` staged and later applied proposals. The first
successful `non_semantic_patch` completion reported `staged: 1, applied: 0`;
the later applied completion and terminal record are the relevant edit-lifecycle
evidence.

## Evidence Roots

- Prototype root:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-fixed-20260525-123824/prototype1`
- `r10` request:
  `messages/edit-harness-request/node-57e8487f70ce4abc-r10.json`
- `r10` result:
  `messages/edit-harness-result/node-57e8487f70ce4abc-r10.json`
- `r10` headless record:
  `messages/edit-harness-result/node-57e8487f70ce4abc-r10.headless-tui.json`
- `r10` workspace:
  `workspaces/edit-harness/node-57e8487f70ce4abc-r10`
- `r11` request:
  `messages/edit-harness-request/node-57e8487f70ce4abc-r11.json`
- `r11` result:
  `messages/edit-harness-result/node-57e8487f70ce4abc-r11.json`
- `r11` headless record:
  `messages/edit-harness-result/node-57e8487f70ce4abc-r11.headless-tui.json`
- `r11` workspace:
  `workspaces/edit-harness/node-57e8487f70ce4abc-r11`

Expected per-turn artifacts such as `agent-turn-trace.json`,
`llm-full-responses.jsonl`, `validation-audit.json`,
`benchmark-patch-projection.json`, `multi-swe-bench-submission.jsonl`, and
`record.json.gz` were not present inside the child workspaces or the
prototype child message directories. For these attempts, the durable
child-specific trace is the broad-harness headless JSON plus the child git
workspaces.

## Authority Boundaries

The broad-harness request target for both attempts is
`artifact:git-commit:c374d2970947ca8068e16a55ab09025c3629a8df`, with policy
`workspace except ploke-eval`. The request contract asked the child model to
run:

```text
cargo check -p ploke-eval
cargo test -p ploke-eval edit_surface
```

The campaign also has baseline closure/protocol artifacts for
`run-1779712736739-structured-current-policy-a34790bb`. Those are not child
attempt artifacts. A trace audit of that baseline run reports 72 provider
responses and 71 recorded tool calls, but that audit belongs to the baseline
eval run, not to `r10` or `r11`.

The Prototype 1 scheduler still points at the parent node as planned/frontier
state, and `prototype1/nodes/node-57e8487f70ce4abc/runner-result.json` was
absent. `prototype1/evaluations` and `prototype1/history/blocks` were absent
when inspected. Under the history-crown rule, the broad-harness terminal
records are evidence and projections, not sealed History authority. The child
commits prove applied candidate edits; they do not prove admitted state
transitions.

## r10 Reconstruction

### Prompt And Discovery

The `r10` request id is
`broad-harness-request:node-57e8487f70ce4abc:r10` with request hash
`095c34dae94fe03ced16199cba070fa19509aa7c20af60a5cc4deaec6ff2ef49`.
Prompt diagnostics report `context_mode: Off`, no included RAG parts, four
message prompts, and model `google/gemini-3.5-flash`.

The model listed the workspace and `crates/ingest/syn_parser`, inspected
`prototype1/nodes/node-57e8487f70ce4abc/node.json` and `runner-request.json`,
and attempted to read the missing
`prototype1/nodes/node-57e8487f70ce4abc/runner-result.json`. That failure is
missing transition evidence, not an edit failure.

### Tool Outputs And Model Visibility

The model saw tool outputs through the headless TUI debug relay. The relay has
tool-output messages followed by immediate model actions, including the visible
`apply_code_edit` failure, subsequent file reads, the switch to
`non_semantic_patch`, and later cargo calls. The final model message describes
optimizing `ParsedCodeGraph::prune` and mentions the cargo checks it ran.

### Edit Lifecycle

The model first tried `apply_code_edit` against
`crate::parser::graph::parsed_graph::prune`. It failed because strict and
fallback lookup found no matching node:

```text
No matching node found (strict+fallback) for canon
crate::parser::graph::parsed_graph::prune
```

The harness then reported an internal staging failure for that semantic edit.
The model recovered by using `non_semantic_patch` on
`crates/ingest/syn_parser/src/parser/graph/parsed_graph.rs`.

The important lifecycle split is visible in the `non_semantic_patch` result:
the first completion reported `ok: true`, `staged: 1`, `applied: 0`,
`auto_confirmed: false`; the later proposal/application event reported an
applied proposal. The terminal record says:

```text
terminal: applied
proposal_id: be6b87d8-e647-53a5-9855-e9e333e1f9c7
applied_proposal_ids: [be6b87d8-e647-53a5-9855-e9e333e1f9c7]
```

There was no `Content changed`, `ContentMismatch`, stale hash, or same-file
apply lifecycle failure in the `r10` headless record.

### Applied Patch

The `r10` workspace was clean after the attempt. Its branch head is
`7f044a6556df167e93498e7fd5443527ebfe9a61`, with the broad-harness result
commit message for `r10`, parented by the request target commit. The diff from
the target changes one file:

```text
crates/ingest/syn_parser/src/parser/graph/parsed_graph.rs | 32 +++++++++++++---------
```

The patch reduces repeated linear membership checks in
`ParsedCodeGraph::prune`. It creates a `HashSet` for pruned item ids, a
`HashSet` for removed items, and a `HashSet<TreeRelation>` for pruned
relations, then uses those sets in retain filters over functions, types,
constants, statics, macros, use statements, impls, traits, non-file modules,
module-overlap filtering, and relations.

This is admissible at the broad-harness policy level: it is outside
`ploke-eval`, applied in the child workspace, and committed. It is not an
authority-bearing History admission.

### Cargo And Tests

`r10` used cargo, but did not meet the full request contract.

Observed cargo evidence:

- `cargo check -p syn_parser`: passed.
- `cargo test -p syn_parser`: failed twice with exit 101.
- `cargo check -p syn_parser --benches`: passed twice.
- post-patch `cargo check` in the focused `syn_parser` crate: passed.
- `cargo test -p ploke-transform`: passed.
- `cargo check -p ploke-eval`: passed with warnings.

Missing required evidence:

- `cargo test -p ploke-eval edit_surface` was not run.

## r11 Reconstruction

### Prompt And Discovery

The `r11` request id is
`broad-harness-request:node-57e8487f70ce4abc:r11` with request hash
`37a10a76d8cd680839be067b7957d785af13c1d550918f0ad88a7d0625db563a`.
Prompt diagnostics again report `context_mode: Off`, no included RAG parts,
four message prompts, and model `google/gemini-3.5-flash`.

The model initially focused on
`proc_macros/syn_parser/ploke-test-macros`, then inspected the Prototype 1
node, request, run profile, scheduler, message directories, the `r11` prompt
and request, and the prior `r10` result and headless JSON. It also listed
`prototype1/evaluations` and `prototype1/history/blocks`; both were absent.

That inspection is useful evidence. It shows the model had visible prior-child
context and visible absence of History/evaluation records. It also means that
embedded `r10` failures inside the `r11` headless file should not be counted as
new `r11` failures.

### Tool Outputs And Model Visibility

The model saw tool outputs through the headless TUI relay. The `r11` event log
has 210 events and five attempt records. The model-visible sequence includes
the failed semantic edit, three staged-then-applied non-semantic patches, and
cargo results after edits.

### Edit Lifecycle

The first `r11` semantic `apply_code_edit` failed the same way as `r10`:
no matching graph node was found for the canonical
`crate::parser::graph::parsed_graph::prune`, followed by an internal staging
failure. The model then used three `non_semantic_patch` calls:

```text
3a4858f5-339c-590c-8242-6ae0fa51b8d3 -> parsed_graph.rs
eaa9c1da-9dcf-58f3-be5a-85c65c3c7395 -> graph/mod.rs
94e881e0-2067-5473-b135-0ab7b2d53869 -> module_tree.rs
```

The terminal record says:

```text
terminal: applied
proposal_id: 94e881e0-2067-5473-b135-0ab7b2d53869
applied_proposal_ids:
  3a4858f5-339c-590c-8242-6ae0fa51b8d3
  eaa9c1da-9dcf-58f3-be5a-85c65c3c7395
  94e881e0-2067-5473-b135-0ab7b2d53869
```

There was no `Content changed`, `ContentMismatch`, stale hash, or same-file
apply lifecycle failure in the `r11` attempt itself.

### Applied Patch

The `r11` workspace was clean after the attempt. Its branch head is
`97d5d7decde4e8093401a657c6a28ab45540164f`, with the broad-harness result
commit message for `r11`, parented by the request target commit. The diff from
the target changes three files:

```text
crates/ingest/syn_parser/src/parser/graph/mod.rs             | 19 ++++++++------
crates/ingest/syn_parser/src/parser/graph/parsed_graph.rs    | 29 +++++++++++++---------
crates/ingest/syn_parser/src/resolve/module_tree.rs          | 12 ++++++---
```

The patch again optimizes `ParsedCodeGraph::prune` with set-backed membership
checks. It also changes graph query helpers:

- `find_methods_in_module` collects impl ids into a `HashSet` and then scans
  impl-associated-item relations once.
- feature-gated `get_child_modules` collects child ids into a `HashSet` before
  filtering module records.
- `find_definition_for_declaration` uses `relations_by_source` to jump to
  candidate relation indices instead of scanning all `tree_relations`.

This is admissible at the broad-harness policy level: it is outside
`ploke-eval`, applied in the child workspace, and committed. It is not an
authority-bearing History admission. It also carries more semantic risk than
`r10` because it touches indexed relation lookup behavior without targeted
tests.

### Cargo And Tests

`r11` used cargo, but did not meet the request contract.

Observed cargo evidence:

- Initial `cargo check`, `cargo test`, and `cargo test --tests` passed in the
  focused `proc_macros/syn_parser/ploke-test-macros` manifest.
- `cargo test -p ploke-core` passed.
- After patches, repeated `cargo check` calls passed, but they were still
  focused on the proc-macro manifest while compiling `syn_parser` as a
  dependency.
- `cargo test -p syn_parser` failed with exit 101.
- The model then listed `tests`, noted missing fixture material, and ran
  `cargo check --tests`, which passed in the focused proc-macro manifest.

Missing required evidence:

- `cargo check -p ploke-eval` was not run.
- `cargo test -p ploke-eval edit_surface` was not run.

The final model response should therefore not be read as proof that the changed
workspace passed its requested validation. It conflates focused manifest checks
and a failed package test with broad success.

## Transition Evidence

The transition evidence is missing for both child attempts. The child
broad-harness result boxes and terminal records prove that a headless TUI
attempt applied a proposal and wrote a candidate commit. They do not prove that
Prototype 1 admitted either child into History.

Concrete gaps:

- `prototype1/nodes/node-57e8487f70ce4abc/runner-result.json` was missing.
- `prototype1/evaluations` was absent.
- `prototype1/history/blocks` was absent.
- The scheduler still projected the parent as planned/frontier state.
- Baseline closure/protocol artifacts point to
  `run-1779712736739-structured-current-policy-a34790bb`, not to `r10` or
  `r11`.

Under the history-crown rule, terminal summaries such as `terminal: applied`
and `applied_proposal_ids` are useful evidence about the edit executor. They
are not sufficient authority for a state transition.

## Adjudication Examples

Positive examples for later LLM adjudication fields:

- `r10`: The model received a failed `apply_code_edit` result, refreshed local
  file context, switched to `non_semantic_patch`, and produced one applied
  candidate commit. This is a recoverable edit-tool detour.
- `r10`: The model ran `cargo check -p ploke-eval`, which is directly one of
  the request-required validation commands.
- `r11`: The model inspected the prior `r10` result and the missing
  `evaluations` and `history/blocks` directories. That is useful transition
  evidence gathering, even though it did not create authority.
- `r11`: The model decomposed the optimization into three independent applied
  proposals and left the workspace clean.

Negative examples for later LLM adjudication fields:

- `r10`: `cargo test -p syn_parser` failed twice, and
  `cargo test -p ploke-eval edit_surface` was never run. A final validation
  field should not mark the attempt fully validated.
- `r10`: The first successful `non_semantic_patch` completion was only staged,
  not applied. Treating that event alone as an applied edit would be a lifecycle
  error.
- `r11`: The final checks were mostly focused on
  `proc_macros/syn_parser/ploke-test-macros`; they are weaker than workspace or
  request-contract validation.
- `r11`: `cargo test -p syn_parser` failed, yet the final response presents the
  attempt as broadly compiled and tested.
- Both: The missing `runner-result.json`, absent History blocks, and absent
  evaluations mean terminal summaries must not be promoted into transition
  authority.
- Both: The semantic edit failures were graph lookup/staging failures, not
  stale same-file content mismatch. Labeling them as stale hash problems would
  be misleading.
