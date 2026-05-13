# ploke-eval -> ploke-tui Adapter Boundary Review

Date: 2026-05-12

Scope: read-only review of the current bounded surface edit boundary between
`ploke-eval` and `ploke-tui`. No code changes were made.

## 2026-05-13 Update

Commit `bd00056b Add broad harness request fanout` supersedes the parts of this
review that describe complete mode as stopping at `PendingBroadHarnessRequest`.
The live broad path now has an eval-owned headless `ploke-tui` continuation:
one request slot per child-budget slot, request-bound submitted results, backend
admission against the live `EditSurfaceAdmission`, and child-plan sealing from
admitted broad transactions.

The boundary caveat still matters: this is the current runnable adapter path,
not yet the final production `Harness` trait implementation described below.

## Verdict

If the broad-harness continuation blocker is handled, the parent runtime can
publish a broad harness request and later admit a submitted child Artifact. That
does not yet mean the parent has a live, production-grade trait adapter that
calls `ploke-tui` to edit a child Artifact.

Current state:

- The eval-owned operation boundary exists as a skeleton trait:
  `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness.rs:7`.
- The real `ploke-tui` lowering/check/apply evidence is mostly implemented as
  eval-owned wrappers in `edit_surface/tui.rs`, not as a concrete trait
  implementation.
- The live Router/`ploke-tui` path is proven in an integration-style test, not
  wired into complete-mode child planning.
- The broad harness prompt is currently a filesystem request to an external
  agent/workspace, not a direct call into `ploke-tui`.

## Interfaces That Exist

### Generic harness trait

`Harness` is defined at
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness.rs:7`:

```text
graph() -> Graph
propose(Input) -> (Proposal, Run)
apply_checked(Proposal, surface::Check) -> Applied
```

The key authority boundary is that `apply_checked` requires a
`surface::Check`, which is minted by `ploke-eval`, not by the harness:
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness.rs:16`.

The live implementation is only `harness::Mock`:
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness.rs:114`.
Call sites are tests and test helpers, for example
`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:10910` and
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs:733`.

### Eval-owned TUI boundary wrappers

`edit_surface/tui.rs` explicitly says it wraps lower TUI request/proposal/material
shapes without giving them authority:
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs:3`.

Important carriers:

- `GeneratorSurfaceVersion`: captures projection id/hash, bounds digest, and
  source identity at
  `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs:141`.
- `Bounds`: binds a TUI projection to graph bounds at
  `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs:339`.
- `Bounds::touch`: checks Artifact identity, expected file hash, and surface
  containment before minting `surface::Touch` at
  `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs:382`.
- `LowerRequest` / `LowerEdit`: lower `ploke_tui::rag::utils::ApplyCodeEditRequest`
  into eval-owned shape at
  `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs:591`.
- `Proposal::stage`: rejects auto-apply and binds touches/projection/base at
  `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs:675`.
- `Apply::from_results`: admits apply evidence only when write count and touched
  spans match at `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs:766`.
- `Apply::validate`: checks the after Artifact and touched file hashes before
  exposing `ArtifactDelta` at
  `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs:829`.

This is close to the planned adapter, but it is not yet a concrete
`Harness for PlokeTuiAdapter`.

## Who Calls What Today

### Deterministic TUI-tools path

Complete mode still only admits deterministic TUI-tools child planning for live
complete runs:
`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:745`.

The deterministic path:

1. Generates checked candidates under `Prototype1EditSurface::PlokeTuiTools` at
   `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1136`.
2. Writes treatment/evaluation projections at
   `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1177`.
3. Produces `ChildFiles` from checked edit evidence at
   `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1188`.
4. Locks and receives the child plan at
   `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1209`.

### Broad harness request path

Broad harness request mode publishes a request and then fails closed as pending:

- `run_parent_target_selection` maps `BroadHarnessRequest` to
  `publish_broad_harness_child_plan_request` at
  `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:970`.
- Complete mode rejects broad-harness request child planning until a typed
  request-to-child-plan receipt exists at
  `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:750`.
- If a request is published during preparation, complete mode returns
  `PendingBroadHarnessRequest` with request/prompt/workspace paths at
  `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6226`.

The request publication writes both JSON and Markdown prompt files:
`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1273`.

### Live `ploke-tui` scout path

The live Router path exists in a test:
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs:1644`.

It uses `ploke_tui::app::commands::harness::TestRuntime`, spawns file manager,
state manager, event bus, LLM manager, and observability at
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs:1661`.

The test sends a prompt as a user message and asks the model to call
`apply_code_edit` exactly once:
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs:1709`.

Then it watches:

- `ToolCallRequested` at
  `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs:1767`;
- `ToolCallCompleted` at
  `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs:1771`;
- `ToolCallFailed` at
  `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs:1775`;
- `ChatTurnFinished` at
  `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs:1778`.

It then lowers the staged `ploke-tui` proposal into checked eval touches at
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs:1866`, binds
a request-policy receipt at
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs:1902`, checks
the grant at `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs:1957`,
and validates apply evidence into `ArtifactDelta` at
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs:1969`.

## Input Object / Prompt

There are two current prompt/input shapes.

### Broad harness prompt

The broad request carrier is `request::Request<request::Broad, request::Published>`,
currently exposed through the compatibility alias
`PublishedBroadHarnessRequest` at
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:29`.

It carries:

- request id/hash/path/prompt/submitted-result path at
  `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:212`;
- admission binding at
  `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:219`;
- child budget and workspace/edit-policy data through `BroadHarnessRequest` at
  `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:16`.

The Markdown prompt includes:

- source repository and mutable candidate workspace:
  `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:933`;
- edit policy and child budget:
  `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:942`;
- evaluation scope/selection/guidance:
  `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:948`;
- protected core:
  `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:959`;
- evidence roots:
  `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:963`;
- return evidence contract:
  `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:974`;
- instructions:
  `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:984`.

Evidence roots include History blocks, evaluations, node records, protocol
artifacts, and oracle report summaries:
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:866`.

The instruction to write typed result evidence is explicit:
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:762`.

### TUI tool-call input

`ploke-tui` exposes `ApplyCodeEditRequest`:
`crates/ploke-tui/src/rag/utils.rs:15`.

It contains `edits: Vec<Edit>` and optional confidence. `Edit` supports:

- canonical semantic edits:
  `crates/ploke-tui/src/rag/utils.rs:25`;
- byte splice edits:
  `crates/ploke-tui/src/rag/utils.rs:33`;
- patch mode:
  `crates/ploke-tui/src/rag/utils.rs:46`.

The current live scout prompt uses only canonical mode with explicit JSON:
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs:1709`.

## Output Object

### `ploke-tui` output

`apply_code_edit_tool` returns an optional proposal id and emits realtime
events:
`crates/ploke-tui/src/rag/tools.rs:626`.

On success, staging inserts `ploke_tui::app_state::core::EditProposal` into the
proposal store with `status: Pending`:
`crates/ploke-tui/src/rag/tools.rs:217`.

The proposal shape includes request id, parent id, call id, concrete
`WriteSnippetData`, files, preview, status, and semantic flag:
`crates/ploke-tui/src/app_state/core.rs:359`.

The tool emits `ToolCallCompleted` with serialized `ApplyCodeEditResult`:
`crates/ploke-tui/src/rag/tools.rs:306`.
Failures emit `ToolCallFailed` through `ToolCallParams`:
`crates/ploke-tui/src/rag/utils.rs:232`.

### `ploke-eval` output

The eval lowering path converts `WriteSnippetData` into `EditProposal` via
`proposal_from_resolved_writes`:
`crates/ploke-eval/src/cli/prototype1_state/backend.rs:293`.

`validate_edit_surface_candidate` returns `CheckedSurfaceEdit`:
`crates/ploke-eval/src/cli/prototype1_state/backend.rs:1074`.

`CheckedSurfaceEdit` exposes child-surface evidence through
`surface_evidence`:
`crates/ploke-eval/src/cli/prototype1_state/backend.rs:430`.

For broad harness submissions, `SubmittedBroadHarnessResult` is the typed
submitted record at
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_result.rs:11`.
Backend admission returns `AdmittedBroadHarnessResult` after request binding,
workspace isolation, clean source, base-head, surface, and persisted Artifact
checks:
`crates/ploke-eval/src/cli/prototype1_state/backend.rs:1338`.

## Patch Validation And Multi-Patch Handling

`ploke-tui` can stage multiple edits in one `ApplyCodeEditRequest`: the request
has `Vec<Edit>` at `crates/ploke-tui/src/rag/utils.rs:16`, and staging groups
edits by file at `crates/ploke-tui/src/rag/tools.rs:119`.

Within `ploke-tui`, file edits are sorted in reverse byte order before preview
materialization, so same-file multiple spans can be folded without shifting later
byte offsets:
`crates/ploke-tui/src/rag/tools.rs:136`.

Across the eval boundary:

- `Bounds::touches` requires one target per write and rejects target/write count
  mismatch at
  `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs:407`.
- `validate_edit_surface_candidate` currently rejects multi-file proposals:
  `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1098`.
- It sorts touches by `(start, end)` and validates spans before folding:
  `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1127`.
- It requires every touch expected hash to match the current source content hash:
  `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1130`.
- `Apply::from_results` rejects missing, extra, mismatched, or failed writes:
  `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs:781`.
- `Apply::validate` checks the after Artifact reference and touched after hashes:
  `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs:844`.

So multiple touches are supported for a single file. Multiple files are not yet
admitted by `validate_edit_surface_candidate`.

## Failure Handling

Current `ploke-tui` failure signals:

- Empty edit request returns `ToolCallFailed`:
  `crates/ploke-tui/src/rag/tools.rs:640`.
- Parse failure also returns `ToolCallFailed`:
  `crates/ploke-tui/src/rag/tools.rs:633`.
- Resolver errors return typed `ToolError` through `ToolCallFailed`:
  `crates/ploke-tui/src/rag/tools.rs:653`.
- Tool events are shaped as `ToolCallCompleted` / `ToolCallFailed` /
  `ChatTurnFinished`:
  `crates/ploke-tui/src/app_state/events.rs:87`.

Timeouts:

- Default tool-call timeout is 30 seconds:
  `crates/ploke-tui/src/llm/manager/session.rs:145`.
- The chat session sets HTTP attempt timeout from `llm_timeout_secs`:
  `crates/ploke-tui/src/llm/manager/session.rs:647`.
- Tool waiters time out per call and return a structured timeout error:
  `crates/ploke-tui/src/llm/manager/session.rs:1540`.
- Tool requests are emitted only after the dispatcher is live:
  `crates/ploke-tui/src/llm/manager/session.rs:1622`.
- `ChatTurnFinished` carries outcome, error id, summary, and attempts:
  `crates/ploke-tui/src/llm/manager/mod.rs:247`.

If the sub-agent/model returns without making an edit and no tool error is
emitted, the current live scout test detects it by waiting for either a staged
proposal or a terminal turn, then panicking if terminal arrives without a
proposal:
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs:1751`.

That guard is not yet a production adapter outcome. The production boundary
needs a typed `HarnessRun` outcome such as `NoProposal`, `ToolFailed`,
`TimedOut`, `CompletedWithoutEdit`, or `StagedProposal`.

## Oracle And Child Validation Visibility

The broad prompt exposes oracle summaries when present as an evidence root:
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:903`.

It also exposes evaluations, node records, protocol artifacts, and History
blocks:
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:874`.

This gives the sub-agent filesystem paths, not a typed oracle API. There is no
current adapter method for "latest oracle values" or "child validation status".
The prompt asks for suggested checks, but the harness submission remains evidence
only:
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:911`.

The planning docs expect the parent planning layer to consume typed History,
mechanical evaluation stats, protocol judgments, score decomposition,
build/test/oracle outcomes, provider failures, and prior surface evidence:
`docs/workflow/evalnomicon/drafts/edit-surface/harness-adapter-plan.md:1599`.

## Sequence Diagram

```text
Parent<Ready>
  -> ploke-eval: choose candidate generation mode
  -> BroadHarnessRequest mode:
       publish request JSON + Markdown prompt
       Parent<AwaitingHarnessPlan<Broad>>
       stop with PendingBroadHarnessRequest

External agent / future ploke-tui adapter
  -> reads prompt + evidence roots
  -> edits candidate workspace or stages apply_code_edit proposal
  -> writes SubmittedBroadHarnessResult or emits TUI proposal/events

ploke-eval admission
  -> verify request identity/hash/admission binding
  -> verify clean source repo + isolated workspace + base HEAD
  -> verify changed paths inside surface
  -> persist candidate files as derived Artifact
  -> mint admitted broad result / child plan continuation

For TUI semantic adapter path
  -> ploke-tui ApplyCodeEditRequest
  -> ploke-tui EditProposal + ToolCallCompleted/Failed + ChatTurnFinished
  -> ploke-eval LowerRequest/LowerProposal/Bounds
  -> surface Grant -> Check
  -> Apply::from_results -> validate(after Artifact)
  -> ArtifactDelta / CheckedSurfaceEdit / SurfaceEvidence
```

## Relation To Planned Invariants

The plan says `ploke-eval` owns grants, checks, candidate admission, and History
evidence, while `ploke-tui` is one possible harness behind an operation boundary:
`docs/workflow/evalnomicon/drafts/edit-surface/model.md:836`.

The plan also says TUI proposal state is not authority:
`docs/workflow/evalnomicon/drafts/edit-surface/model.md:844`.

The hyper-agent posture says filesystem evidence roots should be preferred over
large prompt summaries, and TUI proposal state or CLI text must not become
authority:
`docs/design/drafts/edit-surface/hyperagents-prompt-posture.md:72` and
`docs/design/drafts/edit-surface/hyperagents-prompt-posture.md:87`.

The current implementation mostly follows this direction, but the live TUI call
is still test-scout wiring rather than the complete-loop adapter.

## Missing Pieces

1. Concrete `ploke-tui` adapter implementation of the eval-owned harness
   boundary.

   The existing `Harness` trait and `tui` wrappers need a production carrier that
   can run a TUI/Router turn, capture `HarnessRun` provenance, and return either
   a staged proposal or a typed terminal failure.

2. Request-bound continuation for broad harness results.

   Historical pre-`bd00056b` gap: complete mode needed to consume
   `AdmittedBroadHarnessResult` and mint the next typed state. Current complete
   mode has that first continuation path through broad request-batch fanout and
   child-plan sealing. The remaining work is making the durable evidence and
   formal surface-check spine as strong as the long-term boundary requires.

3. Durable run/error record for harness outcomes.

   `ToolCallFailed`, timeouts, and completed-without-edit need to be recorded as
   parent-readable typed evidence, not just realtime events or test assertions.

4. Full outbound request/response capture.

   The live scout explicitly records unknown request/response payload hashes:
   `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs:1898`.

5. Multi-file policy decision.

   `ploke-tui` can stage multiple files, but `validate_edit_surface_candidate`
   rejects multi-file edits. The first production adapter should either preserve
   that single-file invariant structurally or add a typed multi-file grant/check
   path.

6. Oracle/validation projection for agents.

   Current prompts expose evidence roots. They do not expose a typed "latest
   oracle values" or "child validation status" query surface. If the agent needs
   that, `ploke-eval` should write a small typed evidence projection and include
   it as an evidence root.

## Wireframe Implementation Plan

1. Define the production adapter outcome algebra.

   Keep active authority in `ploke-eval`: `HarnessRun`, `Proposal`, `NoProposal`,
   `ToolFailed`, `TimedOut`, and `CompletedWithoutEdit` should be durable
   projections of a bounded adapter run, not TUI session internals.

2. Implement a concrete TUI harness behind `edit_surface::harness::Harness`.

   Input should include the parent Artifact, `surface::Grant`, graph projection,
   objective text/evidence roots, run id, model policy, and timeout budget. Output
   should be staged proposal plus run evidence.

3. Preserve the two-step authority boundary.

   `ploke-tui` may resolve and stage edits. `ploke-eval` must convert writes to
   touches through `tui::Bounds`, call `Grant::check`, then call adapter
   `apply_checked` only with the `surface::Check`.

4. Add broad-harness continuation.

   After `admit_submitted_broad_harness_result`, mint a typed request-bound
   continuation that complete mode can consume: either a child plan or an admitted
   candidate Artifact list tied to `request::Reference<Broad, Published>`.

5. Add failure evidence.

   Store typed records for TUI timeout, tool failure, model completed-without-edit,
   malformed result, out-of-bounds proposal, stale hash, and insufficient checked
   candidates. Feed those into the next parent planning window.

6. Add the agent evidence projection.

   Write a compact typed summary of latest tests/oracle outcomes, previous
   validation failures, and relevant protocol diagnostics. Include that projection
   in the prompt evidence roots instead of expanding prompt prose.

7. Keep first production route narrow.

   Start with single-file bounded semantic edits on the `ploke-tui-tools` surface.
   Treat multi-file as a later typestate/grant extension unless the first route
   needs it immediately.
