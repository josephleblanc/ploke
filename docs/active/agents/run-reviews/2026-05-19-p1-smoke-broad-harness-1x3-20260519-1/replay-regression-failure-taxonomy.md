Verified surface: read-only synthesis of the finished deep run review and five per-lane review reports; no live `prototype1-state` run, no code-owner mapping, and no concrete regression tests selected yet.

# Replay Regression Failure Taxonomy

Campaign: `p1-smoke-broad-harness-1x3-20260519-1`

Source reports:

- `../2026-05-19-p1-smoke-broad-harness-1x3-20260519-1-deep-review.md`
- `deep-review/node-a873-traces.md`
- `deep-review/node-81bd-applied-traces.md`
- `deep-review/node-81bd-timeout-traces.md`
- `deep-review/run-state-lineage.md`
- `deep-review/agent-turn-traces.md`

This document only sorts the observed failures into replay-oriented families.
It deliberately does not assign responsible code areas or concrete test names.
Those mappings should happen in the next pass.

## Taxonomy Shape

Each family below has:

- **Bad behavior:** what the system allowed or overclaimed.
- **Observed examples:** trace or artifact examples from the review.
- **Replay evidence surface:** the persisted record family that can later drive
  deterministic replay or typed reconstruction.
- **Future fixed contract:** the behavior a later regression test should be able
  to demonstrate after the system is improved.

Replay evidence surfaces are named at the artifact level, not the code-owner
level:

- **Headless TUI replay:** persisted model/provider output and headless TUI
  trace/result records, replayed through the real session/tool loop where the
  test needs tool behavior.
- **Agent-turn record replay:** `agent-turn-*` sidecars and `record.json.gz`
  facts, especially provider errors, tool-call outcomes, patch artifacts, and
  Multi-SWE-Bench submission shape.
- **Lifecycle reconstruction:** typed reconstruction from child plans, runner
  results, evaluations, transition journal, History blocks, and active checkout
  state.
- **Projection reconstruction:** typed reconstruction of operator-facing
  summaries from authoritative records, not raw log text or stale projection
  files.

## RF-01: Outcome-Layer Conflation

**Bad behavior:** A status word from one layer is easy to read as success at a
stronger layer.

Subcases:

- `headless terminal: applied` can mean only that a workspace proposal was
  applied, not that validation passed, a child was validly admitted, or a
  benchmark improved.
- `runner-result: succeeded` can mean the branch runner completed, not that the
  treatment produced a useful patch.
- `evaluation: keep` can mean no regression relative to an already-empty or
  aborted baseline, not a useful repair.
- `patch_projection_check_state=passed` can mean packaging/projection succeeded,
  not that the submitted patch was nonempty.

**Observed examples:**

- `node-a87394840086768d-r2` ended `terminal: applied` despite malformed Rust and
  failed visible validation.
- All six treatment runner results were `succeeded`, but all six treatment
  agent-turn runs aborted with empty `fix_patch`.
- Generation 2 selected `keep` branches even though baseline and treatment patch
  states were both empty.
- Treatment records had `patch_projection_check_state=passed` with
  `fix_patch` length 0.

**Replay evidence surface:** Headless TUI trace/result records, branch
evaluation records, runner results, agent-turn records, and packaging records.

**Future fixed contract:** A replay or reconstruction test should require the
summary layer to preserve outcome scope explicitly. No single status should
answer a stronger question than its evidence supports.

## RF-02: Validation-to-Materialization Gate Leakage

**Bad behavior:** A candidate with failed validation, malformed code, or missing
submitted evidence can still appear later as a materialized child or successful
runner artifact.

Subcases:

- Visible compile/test failure did not block later child lifecycle progression.
- Malformed committed workspace state did not become a first-class blocked or
  invalid candidate state.
- Timed-out local edits had no submitted result summary, but adjacent node-level
  surfaces were still easy to read as if all request variants completed.

**Observed examples:**

- `node-a87394840086768d-r2` had failed `cargo check`, malformed `fs.rs`, and
  later `built` / `spawned` / `acknowledged` lifecycle records.
- `node-81bd26e4b6222d08-r5` and `-r6` applied local edits but timed out and had
  no adjacent submitted result summary.
- `node-81bd26e4b6222d08-r7` and `-r8` had no proposals and no submitted result
  summary.

**Replay evidence surface:** Headless TUI trace/result records plus lifecycle
reconstruction from child plans, transition journal, runner results, and branch
evaluations.

**Future fixed contract:** A replayed candidate with failed validation or no
submitted result should reconstruct as blocked, invalid, or incomplete. It
should not be indistinguishable from a validated materialized child.

## RF-03: Trace and Artifact Completeness Ambiguity

**Bad behavior:** Missing result artifacts, timed-out traces, and partial trace
families are not separated cleanly from completed candidate submissions.

Subcases:

- Request basenames can exist without matching result basenames.
- Timed-out trace files can contain local edits but no submitted result summary.
- A trace can say `applied` while durable committed artifact evidence is missing
  or weak.

**Observed examples:**

- The campaign had 18 edit-harness requests, 11 headless TUI traces, and 7
  request basenames without matching result artifacts.
- `node-81bd26e4b6222d08-r3` was applied in the trace, but no verified committed
  artifact was found in the review.
- `node-81bd26e4b6222d08-r5` and `-r6` had local edits but no submitted result
  summary.

**Replay evidence surface:** Message request/result inventory, headless TUI
trace files, result summaries, workspace or artifact patch identity, and branch
records.

**Future fixed contract:** Typed reconstruction should classify each request
variant as completed, timed out, missing, locally mutated but unsubmitted, or
submitted. Later summaries should not infer completion from request presence or
from local workspace mutation alone.

## RF-04: Tool Proposal Lifecycle Ambiguity

**Bad behavior:** Tool events and proposal states can be mistaken for final
workspace effects.

Subcases:

- `ToolCallCompleted` can represent a staged proposal rather than an applied
  edit.
- Cargo compile failures can appear as completed cargo tool events with
  `ok=false`, not as `ToolFailed`.
- Malformed edit-tool arguments can appear in retained relay/debug evidence but
  not as normal proposal records.

**Observed examples:**

- Historical notes and this run both show `non_semantic_patch` completion can
  mean staged proposal, not final applied file state.
- `node-a87394840086768d-r7` had compile failures represented through completed
  cargo events.
- `node-81bd26e4b6222d08-r8` showed malformed `non_semantic_patch` calls
  missing required `reasoning` in retained relay evidence before proposal
  creation.
- `branch-dafe57f7e75214ad` had invalid `non_semantic_patch` / malformed diff
  evidence but no recorded applied proposal.

**Replay evidence surface:** Historical provider output replayed through the
real TUI session/tool loop, plus typed event/proposal records and next-request
tool response capture.

**Future fixed contract:** Replay should distinguish requested, staged,
accepted, applied, rejected, malformed, and failed tool states. The next model
request should receive the precise current tool result, not an overbroad
success marker.

## RF-05: Edit Composition and Same-File Repair Failure

**Bad behavior:** Repeated edits to the same file can produce stale, partial, or
structurally broken states before the system forces a clean batch/invalidation
boundary.

Subcases:

- Same-file repair loops can leave malformed Rust.
- Partial non-semantic patches can require multiple repair attempts, increasing
  the chance of stale anchors or duplicated fragments.
- A trace may eventually compile after churn, but the system lacks a strong
  invariant that prevents the bad intermediate states from becoming committed
  artifacts.

**Observed examples:**

- `node-a87394840086768d-r2` left malformed `fs.rs` with restore markers,
  missing declarations, and delimiter errors.
- `node-a87394840086768d-r7` repaired `fs.rs` after many proposal events before
  eventually passing visible checks.
- `node-81bd26e4b6222d08-r6` repaired a new `headless_runtime.rs` after duplicate
  definitions, an unclosed delimiter, and partial patch failure.
- `node-81bd26e4b6222d08-r3` had 16 proposal events around one module file and
  repeated compile failures.

**Replay evidence surface:** Headless TUI provider output, proposal records,
tool responses, file hash expectations, cargo tool events, and final workspace
patch projection.

**Future fixed contract:** Replay should show that repeated same-file edits are
either composed safely before apply, invalidated and retried with fresh state, or
rejected before a malformed candidate can be submitted or materialized.

## RF-06: Semantic Edit Addressing Gaps

**Bad behavior:** Semantic edit tools fail on crate root or module-root targets,
pushing the model into noisier non-semantic patch loops.

Subcases:

- Module-root targets such as `crate::core` and `crate::lib` are not resolved
  reliably.
- Failed semantic lookup can degrade into repeated non-semantic patch attempts
  instead of a clearer typed failure or safe alternate path.
- Tool feedback can be correct locally but insufficient for the model to choose
  a better target.

**Observed examples:**

- `node-81bd26e4b6222d08-r3` failed semantic edit resolution for
  `canon=crate::core`.
- `node-81bd26e4b6222d08-r9` failed semantic edit resolution for
  `canon=crate::lib`.
- `branch-dafe57f7e75214ad` had code lookup and apply failures before aborting
  with no proposal.

**Replay evidence surface:** Historical tool-call arguments decoded through
typed tool DTOs, semantic edit resolver responses, proposal records, and next
model-request tool response content.

**Future fixed contract:** Replay should demonstrate either successful
module-root semantic resolution or a deterministic typed refusal that steers the
session to a safe patch path without same-file churn.

## RF-07: Protected-Surface Target Drift

**Bad behavior:** The model repeatedly plans fixes against protected or
out-of-policy files, learning the boundary only through edit denials after
substantial exploration.

Subcases:

- Protected `ploke-eval` files are repeatedly targeted under a broad-harness
  policy whose writable surface excludes them.
- Manifest/dependency edits are attempted when dependency changes are not
  permitted.
- Repeated denials are not promoted into a candidate-level warning or planning
  constraint.

**Observed examples:**

- `node-a87394840086768d-r2` attempted to edit protected `ploke-eval` and
  `ploke-tree/Cargo.toml` surfaces.
- `node-81bd26e4b6222d08-r5`, `-r6`, and `-r7` repeatedly targeted protected
  `ploke-eval` files.
- `node-81bd26e4b6222d08-r9` attempted protected `crates/ploke-eval/src/runner.rs`.

**Replay evidence surface:** Headless TUI trace provider output, tool denials,
edit policy evidence, readable/writable root metadata, and proposal records.

**Future fixed contract:** Replay should show that protected-surface
constraints are available before planning or are escalated after the first
denial, preventing repeated protected edit attempts in the same candidate.

## RF-08: Read/Navigation Scope Mismatch

**Bad behavior:** The model spends turns trying to read campaign, node, or
outside-root artifacts through a tool surface that cannot access them.

Subcases:

- Outside-root reads fail when the model tries to inspect campaign-level
  artifacts from within a workspace-scoped headless TUI surface.
- Missing node artifacts are requested before they exist or from the wrong
  relative surface.
- File/directory and range-shape mistakes add navigation noise.

**Observed examples:**

- The published broad-harness request explicitly told the model to inspect
  campaign evidence, for example
  [`node-a87394840086768d.md`](</home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/messages/edit-harness-request/node-a87394840086768d.md>)
  says past benchmark results live under `prototype1/evaluations` and prior
  attempts live under `prototype1/nodes`. The matching typed request
  [`node-a87394840086768d.json`](</home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/messages/edit-harness-request/node-a87394840086768d.json>)
  contains `evidence_roots` for `history/blocks`, `evaluations`, `nodes`, and
  node-scoped `protocol-artifacts`. Despite that, the first recorded tool call
  in
  [`node-a87394840086768d.headless-tui.json`](</home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/messages/edit-harness-result/node-a87394840086768d.headless-tui.json>)
  attempted `list_dir` on the enclosing campaign `prototype1` directory and was
  rejected with `path outside configured roots`.
- The same prompt/root mismatch recurred after the run had more campaign state.
  [`node-81bd26e4b6222d08-r6.md`](</home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/messages/edit-harness-request/node-81bd26e4b6222d08-r6.md>)
  advertised the same evidence surface, while
  [`node-81bd26e4b6222d08-r6.headless-tui.json`](</home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/messages/edit-harness-result/node-81bd26e4b6222d08-r6.headless-tui.json>)
  recorded a failed `read_file` of
  `prototype1/campaign.json` as `path outside configured roots`. The current
  artifact tree does contain campaign metadata and run records around that
  path, including
  [`branches.json`](</home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/branches.json>),
  [`scheduler.json`](</home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/scheduler.json>),
  and
  [`transition-journal.jsonl`](</home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/transition-journal.jsonl>).
- [`node-81bd26e4b6222d08-r7.headless-tui.json`](</home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/messages/edit-harness-result/node-81bd26e4b6222d08-r7.headless-tui.json>)
  failed a `read_file` of `prototype1/branches.json` as outside-root even
  though the persisted
  [`branches.json`](</home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/branches.json>)
  is exactly the kind of read-only campaign navigation index agents need when
  selecting prior evidence.
- Missing-node reads were reported as raw file I/O misses rather than typed
  missing-artifact guidance. In
  [`node-81bd26e4b6222d08.headless-tui.json`](</home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/messages/edit-harness-result/node-81bd26e4b6222d08.headless-tui.json>),
  `read_file` requested
  `prototype1/nodes/node-a87394840086768d/runner-result.json`, but the persisted
  node directory only had
  [`node.json`](</home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/nodes/node-a87394840086768d/node.json>)
  and
  [`runner-request.json`](</home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/nodes/node-a87394840086768d/runner-request.json>).
  [`node-81bd26e4b6222d08-r9.headless-tui.json`](</home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/messages/edit-harness-result/node-81bd26e4b6222d08-r9.headless-tui.json>)
  similarly requested a not-yet-existing
  `nodes/node-81bd26e4b6222d08-r9/runner-result.json`.
- Navigation shape errors added avoidable churn around the same issue:
  [`node-a87394840086768d-r2.headless-tui.json`](</home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/messages/edit-harness-result/node-a87394840086768d-r2.headless-tui.json>)
  recorded `read_file` with `start_line=1050,end_line=200` and `list_dir` on
  the worktree `.git` file. These are model/tool-use errors, but the tool
  responses did not steer the session back to the intended evidence roots.
- The downstream agent-turn sidecars show the broader cost of this navigation
  mismatch: the treatment runs mostly spent turns on `list_dir`, `read_file`,
  and `request_code_context` before provider aborts. See the sidecar inventory
  in
  [`agent-turn-traces.md`](./deep-review/agent-turn-traces.md) and the raw
  treatment trace for
  [`branch-1e4da15f47e52f34`](</home/brasides/.ploke-eval/instances/prototype1/p1-smoke-broad-harness-1x3-20260519-1/treatments/branch-1e4da15f47e52f34/instances/BurntSushi__ripgrep-2209/runs/run-1779186538438-structured-current-policy-d5325c36/agent-turn-trace.json>).

**Code-path follow-up:** This is not simply `ploke-tui` blocking evidence
reads. `ploke-eval` publishes the request roots in
`harness_request.rs`, maps them with `tui_adapter.rs::evidence_read_roots`, and
passes them through
`runner.rs::setup_workspace_tui_runtime_with_read_roots`. `ploke-tui`
`SystemStatus::derive_path_policy` preserves those extra roots, and `ploke-io`
then enforces the configured roots. The problematic contract is that the prompt
advertises campaign evidence while the configured readable roots admit only
selected subdirectories (`history/blocks`, `evaluations`, `nodes`) and skip
attached reports and the enclosing `prototype1` navigation root. Agents should
be allowed to read the campaign evidence needed to navigate those records; the
read-only admission surface is too narrow or too imprecisely described.

**Replay evidence surface:** Tool request/response records, readable-root
metadata, missing-artifact responses, and request-code-context records.

**Future fixed contract:** Replay should show that the session receives
actionable scope metadata or typed missing-artifact errors early enough to avoid
repeated dead-end navigation.

## RF-09: Provider Abort and Empty Submission Dominance

**Bad behavior:** Evaluation can be dominated by provider aborts and empty
submissions rather than by the candidate code change being measured.

Subcases:

- Provider 429/rate-limit aborts produce empty treatment submissions.
- Empty submissions can still pass projection packaging checks.
- Evaluation outcomes can compare empty treatment patches against empty or
  nonempty baselines without surfacing provider-abort dominance prominently.

**Observed examples:**

- All six treatment agent-turn runs aborted and produced empty `fix_patch`.
- Every treatment trace showed provider-side 429/rate-limit evidence; the
  baseline did not.
- Generation 2 `keep` outcomes were relative to empty/no-patch baselines and
  treatments.

**Replay evidence surface:** Agent-turn records, provider error messages,
`record.json.gz`, packaging/submission records, and branch evaluation records.

**Future fixed contract:** Replay or reconstruction should classify
provider-aborted empty submissions as a distinct run-quality outcome. They
should not be summarized as candidate-quality wins or ordinary `keep` results.

## RF-10: Discovery and Context Churn

**Bad behavior:** The normal path for candidate generation spends excessive
turns on search/read/context before producing weak, indirect, or no edits.

Subcases:

- High `request_code_context`, `read_file`, and `list_dir` volume becomes the
  default behavior rather than an exception.
- Churn is not separated by cause: protected-surface targeting, missing
  artifact reads, semantic resolver misses, and genuine code understanding all
  appear as one "tool count" blob.
- Tiny or indirect final edits can be produced after large exploration budgets.

**Observed examples:**

- The 11 headless TUI traces recorded 807 tool requests.
- `node-81bd26e4b6222d08-r2` made 42 `request_code_context` calls for three
  markdown tool-text edits.
- Several no-edit timeout traces spent most events on read/search/context.

**Replay evidence surface:** Headless TUI traces, tool-event counts, per-tool
arguments/results, proposal count, and final change summary.

**Future fixed contract:** Replay reconstruction should classify churn by
cause, not just volume. Later tests can assert that improved planning metadata
or tool feedback reduces repeated dead-end categories without forbidding normal
exploration.

## RF-11: Candidate Quality Evidence Weakness

**Bad behavior:** A candidate can be syntactically valid but weak, indirect, or
directionally suspect relative to the benchmark objective.

Subcases:

- Compile-clean patches may lack measurement or benchmark relevance.
- Prompt/tool-text edits can pass checks while not touching the runtime path
  being scored.
- Reliability/backoff changes can look like fixes while possibly increasing
  latency.

**Observed examples:**

- `node-a87394840086768d` made a tiny `split_whitespace().join(" ")` cleanup.
- `node-81bd26e4b6222d08-r2` changed tool-description markdown only.
- `node-81bd26e4b6222d08-r9` increased BM25 timeout/backoff constants.
- `node-a87394840086768d-r7` added a plausible History block cache, but the
  performance benefit was unmeasured and the downstream treatment aborted.

**Replay evidence surface:** Headless TUI change summaries, validation records,
branch evaluations, submission artifacts, and any benchmark/evidence records
attached to the candidate.

**Future fixed contract:** Reconstruction should separate "valid patch",
"objective-relevant patch", and "measured improvement". A passing compile check
should not be enough to mark a performance candidate as improved.

## RF-12: Operator Projection and Lineage Drift

**Bad behavior:** Projection files can remain stale relative to authoritative
History, transition, runner, and active checkout state.

Subcases:

- `scheduler.json` can show a stale root frontier after the run has completed.
- `node.json` can say `running` after runner success and History successor
  selection.
- Closure/protocol readiness can be confused with finished run lineage.
- History selection from a rejected branch can be misread as benchmark approval
  instead of policy-permitted exploration.

**Observed examples:**

- `scheduler.json` still showed only the root planned/frontier.
- Root and selected generation-1 `node.json` records still said `running`.
- History selected `node-81bd26e4b6222d08` from a rejected generation-1 branch
  because policy allowed continuation from rejected branches.
- History head and active checkout showed the run finished at generation 2.

**Replay evidence surface:** History blocks and indexes, transition journal,
node records, runner results, evaluations, closure state, and active checkout
identity.

**Future fixed contract:** Projection reconstruction should prefer authoritative
History and transition records over stale convenience files, or explicitly label
stale/legacy projections so operator output cannot overclaim liveness or
success.

## RF-13: Baseline/Treatment Comparator Ambiguity

**Bad behavior:** Treatment results are interpreted without enough context about
the baseline surface they were compared against.

Subcases:

- Generation-1 treatments regressed from a nonempty baseline to empty
  submissions.
- Generation-2 `keep` outcomes compared empty treatment submissions against
  empty or already-aborted baselines.
- Tool-call failure deltas can drive reject/keep outcomes while patch
  nonemptiness remains empty for both arms.

**Observed examples:**

- Generation-1 rejected branches had baseline applied/nonempty and treatment
  no/empty.
- `branch-b1c602731878d8ff` and `branch-d68716f09d5982ae` were `keep` despite
  empty treatment patches.
- `branch-dafe57f7e75214ad` was rejected partly because treatment tool-call
  failures regressed from 0 to 4.

**Replay evidence surface:** Branch evaluation records, baseline/treatment
agent-turn records, packaging/submission artifacts, and comparison metrics.

**Future fixed contract:** Reconstruction should always report the baseline
patch state, treatment patch state, provider-abort state, and comparator reason
together. A `keep` without a nonempty treatment patch should remain a relative
comparison outcome, not a repair claim.

## RF-14: Authority Boundary Erasure in Summaries

**Bad behavior:** Submitted result summaries, trace reviews, or operator-facing
reports can imply admission, grant, child-plan, or History authority that the
artifact itself explicitly does not claim.

Subcases:

- Result summaries can contain useful return evidence while
  `authority_boundary.admission`, `grant`, and `child_plan` remain
  `not_claimed`.
- Run reviews can accidentally collapse trace-local, lifecycle, evaluation, and
  History facts into one narrative.
- Candidate identity can be discussed without making clear which artifact layer
  minted or selected that identity.

**Observed examples:**

- The `node-a873...` result summaries explicitly kept admission/grant/child-plan
  authority at `not_claimed`.
- The review had to separate headless TUI traces from agent-turn traces because
  a headless edit can compile locally while the later treatment aborts with an
  empty patch.
- Generation-1 selected successor evidence came from History, not the headless
  trace alone.

**Replay evidence surface:** Result summaries with authority-boundary fields,
child plans, History blocks, transition journal, evaluations, and agent-turn
records.

**Future fixed contract:** Any replay-derived summary should carry authority
boundary fields forward. The summary should make it impossible to infer
admission, grant, child-plan, evaluation, or History selection from a lower-layer
artifact unless the higher-layer record is present.

## Initial Grouping for Later Test Design

These families can be grouped later into test lanes without yet choosing code
owners:

| Lane | Families |
| --- | --- |
| Outcome semantics | RF-01, RF-11, RF-13, RF-14 |
| Candidate lifecycle gates | RF-02, RF-03 |
| Tool-loop replay behavior | RF-04, RF-05, RF-06 |
| Policy and navigation feedback | RF-07, RF-08, RF-10 |
| Provider/evaluation robustness | RF-09, RF-13 |
| Operator projection correctness | RF-12, RF-14 |

The next pass should map these families to responsible modules and pick one or
two replay fixtures per family. Historical provider output should be replayed
into the real session/tool loop when the desired proof is tool behavior; typed
record reconstruction is enough when the desired proof is artifact joining,
projection, or operator summary semantics.
