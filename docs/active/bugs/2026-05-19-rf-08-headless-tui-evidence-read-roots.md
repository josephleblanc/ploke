# RF-08 Headless TUI Evidence Read Roots Mismatch

## Summary

Prototype 1 broad-harness prompts tell headless TUI agents to inspect campaign
evidence such as `prototype1/evaluations` and `prototype1/nodes`, but the
readable roots handed to the TUI tool layer are narrower than the navigation
surface agents need. As a result, valid campaign-level navigation reads such as
`prototype1`, `prototype1/campaign.json`, and `prototype1/branches.json` are
rejected as outside configured roots, while missing node artifacts are reported
as raw file I/O misses instead of typed missing-artifact guidance.

This is not a desired denial. Agents should be able to read the campaign
evidence that the broad-harness request advertises, while still being prevented
from writing to protected or output-owned paths.

## Affected Surface

- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs`
- `crates/ploke-eval/src/runner.rs`
- `crates/ploke-tui/src/app_state/core.rs`
- `crates/ploke-io/src/path_policy.rs`
- Prototype 1 broad-harness headless TUI requests and traces

## Observed Examples

- [`node-a87394840086768d.md`](</home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/messages/edit-harness-request/node-a87394840086768d.md>)
  told the agent that benchmark results live under `prototype1/evaluations` and
  prior attempts under `prototype1/nodes`. The matching
  [`node-a87394840086768d.json`](</home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/messages/edit-harness-request/node-a87394840086768d.json>)
  carried typed `evidence_roots` for `history/blocks`, `evaluations`, `nodes`,
  and node-scoped protocol artifacts. The first tool call in
  [`node-a87394840086768d.headless-tui.json`](</home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/messages/edit-harness-result/node-a87394840086768d.headless-tui.json>)
  nevertheless tried to `list_dir` the enclosing `prototype1` directory and was
  rejected as outside configured roots.
- [`node-81bd26e4b6222d08-r6.headless-tui.json`](</home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/messages/edit-harness-result/node-81bd26e4b6222d08-r6.headless-tui.json>)
  failed `read_file` on `prototype1/campaign.json` as outside configured roots.
  That file is campaign metadata, not a protected source edit target, and the
  agent was already asked to inspect campaign evidence.
- [`node-81bd26e4b6222d08-r7.headless-tui.json`](</home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/messages/edit-harness-result/node-81bd26e4b6222d08-r7.headless-tui.json>)
  failed `read_file` on `prototype1/branches.json` as outside configured roots,
  even though the persisted
  [`branches.json`](</home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/branches.json>)
  is a read-only navigation index for branch/run evidence.
- [`node-81bd26e4b6222d08.headless-tui.json`](</home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/messages/edit-harness-result/node-81bd26e4b6222d08.headless-tui.json>)
  tried to read
  `prototype1/nodes/node-a87394840086768d/runner-result.json`. The node
  directory had
  [`node.json`](</home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/nodes/node-a87394840086768d/node.json>)
  and
  [`runner-request.json`](</home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/nodes/node-a87394840086768d/runner-request.json>),
  but no runner result. The response was a raw file I/O miss, not a typed
  "artifact not yet available" response.
- [`node-81bd26e4b6222d08-r9.headless-tui.json`](</home/brasides/.ploke-eval/campaigns/p1-smoke-broad-harness-1x3-20260519-1/prototype1/messages/edit-harness-result/node-81bd26e4b6222d08-r9.headless-tui.json>)
  repeated the missing-node-artifact pattern for
  `nodes/node-81bd26e4b6222d08-r9/runner-result.json`.
- 2026-05-25 recurrence:
  [`node-57e8487f70ce4abc.headless-tui.json`](</home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-fixed-20260525-123824/prototype1/messages/edit-harness-result/node-57e8487f70ce4abc.headless-tui.json>)
  rejected `list_dir` on the enclosing
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-fixed-20260525-123824/prototype1`
  directory as outside configured roots, even though the prompt told the agent
  to inspect `/prototype1/nodes` for prior attempt history.

## Current Code Path

`BroadHarnessRequest::prototype1_workspace` publishes evidence roots for:

- submitted result output;
- `history/blocks`;
- `evaluations`;
- `nodes`;
- node-scoped `protocol-artifacts`;
- an attached `final_report.json` report when present.

`BroadHarnessRequest::render_prompt` advertises the evaluations and nodes paths
to the model. `tui_adapter::run_headless_with_model` then derives
`extra_read_roots` with `evidence_read_roots` and passes them to
`runner::setup_workspace_tui_runtime_with_read_roots`.

`runner::prepare_sparse_workspace` stores those roots in `SystemStatus` and
calls `derive_path_policy`. `ploke-tui` preserves the extra read roots in its
tool path policy, and `ploke-io` enforces that policy.

The mismatch is at the contract boundary:

- `EvidenceRootLocation::Directory` contributes only the exact directory path.
- `EvidenceRootLocation::NodeScopedDirectory` contributes only `nodes_root`.
- `EvidenceRootLocation::AttachedReport` contributes no root.
- `SubmittedResultOutput` is correctly excluded from read roots.
- The enclosing `prototype1` navigation root is not added for real published
  requests, even though agents naturally need to inspect campaign metadata and
  list available evidence directories.

There is already test coverage showing file evidence roots contribute their
parent directory, but the actual oracle evidence root is an `AttachedReport`,
not a `File`, so that test does not prove the current published request can
read the campaign root.

## Expected Behavior

Broad-harness headless agents should be able to read the campaign evidence
advertised in the request, including the navigation metadata needed to discover
which node, branch, evaluation, or History artifacts exist.

Acceptable fixed behavior includes:

1. add the enclosing `prototype1` directory as an explicit read-only evidence
   root when the request advertises child evidence directories;
2. render prompt text that names the exact readable roots and warns that parent
   paths are unavailable if the narrower policy is intentional;
3. provide typed missing-artifact responses for known Prototype 1 artifacts such
   as node runner results, rather than raw file I/O misses;
4. keep submitted-result output and protected source/config paths write-denied.

The fix must preserve broad model autonomy: agents can inspect campaign records
and use evidence, but cannot write result bookkeeping files or protected core
surfaces.

## Future Regression Coverage

Replay a published broad-harness request through the real headless TUI setup and
assert that:

- prompt-listed evidence paths are included in the TUI read policy;
- the agent can `list_dir` the enclosing campaign evidence root or receives an
  explicit typed explanation of the narrower allowed roots;
- `read_file` on existing campaign records such as `branches.json` succeeds;
- reads of missing node artifacts return a typed missing-artifact error;
- writes to protected paths and submitted-result output remain denied.

## Related Review Artifacts

- [`replay-regression-failure-taxonomy.md`](../agents/run-reviews/2026-05-19-p1-smoke-broad-harness-1x3-20260519-1/replay-regression-failure-taxonomy.md)
- [`trace-review.md`](../agents/run-reviews/2026-05-19-p1-smoke-broad-harness-1x3-20260519-1/trace-review.md)
- [`deep-review/node-a873-traces.md`](../agents/run-reviews/2026-05-19-p1-smoke-broad-harness-1x3-20260519-1/deep-review/node-a873-traces.md)
- [`deep-review/node-81bd-applied-traces.md`](../agents/run-reviews/2026-05-19-p1-smoke-broad-harness-1x3-20260519-1/deep-review/node-81bd-applied-traces.md)
- [`deep-review/node-81bd-timeout-traces.md`](../agents/run-reviews/2026-05-19-p1-smoke-broad-harness-1x3-20260519-1/deep-review/node-81bd-timeout-traces.md)
