## esm-map-tui-boundary

Read-only scout report. No files changed.

Target guidance: `docs/workflow/evalnomicon/drafts/edit-surface/model.md`.
Stale broad transition plan ignored as target guidance.

Harness/projection facts:

- `crates/ploke-tui/src/app_state/core.rs:360` stores staged
  `EditProposal`; this is harness-local proposal state, not authority.
- `crates/ploke-tui/src/tools/code_edit.rs:93` turns typed tool input into the
  legacy staging path; `:131` projects typed result from proposal registry.
- `crates/ploke-tui/src/rag/tools.rs:626` semantic `apply_code_edit` entrypoint
  resolves first, then stages.
- `crates/ploke-tui/src/rag/tools.rs:666` resolves `ApplyCodeEditRequest` to
  `Vec<WriteSnippetData>` through path scoping and `ploke_db` graph resolution.
- `crates/ploke-tui/src/rag/tools.rs:70` stages semantic `EditProposal` into
  `AppState.proposals`, computes preview/diff, persists it, and may spawn
  approval.
- `crates/ploke-tui/src/rag/tools.rs:905` stages non-semantic patch proposals
  with `NsWriteSnippetData`.
- `crates/ploke-tui/src/rag/editing.rs:20` dispatches semantic vs
  non-semantic apply.
- `crates/ploke-tui/src/rag/editing.rs:338` semantic apply calls
  `write_snippets_batch`.
- `crates/ploke-tui/src/rag/editing.rs:82` non-semantic apply calls
  `write_batch_ns`.

Authority facts:

- `docs/workflow/evalnomicon/drafts/edit-surface/model.md:38` says
  `EditProposal` is intermediate procedure state, not Artifact transition or
  History authority.
- `docs/workflow/evalnomicon/drafts/edit-surface/model.md:144` separates
  TUI/tool harness as executor/projection from Parent/History authority.
- `docs/workflow/evalnomicon/drafts/edit-surface/model.md:399` identifies
  `SurfaceGrant` as authority-bearing.
- `docs/workflow/evalnomicon/drafts/edit-surface/model.md:844` states
  `AppState.proposals`, proposal status, chat/tool events, and `edit approve`
  are not authority.
- `docs/workflow/evalnomicon/drafts/edit-surface/model.md:935` says checked
  apply must require eval-owned surface checks.
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs:3` says the
  module lowers/wraps TUI shapes without giving them authority.
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs:331` contains
  eval-owned material-span lowering and touch checks.
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs:559`
  defines lower carriers for TUI request/proposal evidence; scout reports no
  external consumers in current tree.
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs:675`
  contains `Proposal::stage`, `Apply::from_results`, and `Apply::validate`,
  enforcing base/projection/check/write/after-artifact consistency before
  `ArtifactDelta`.
- `crates/ploke-eval/src/cli/prototype1_state/backend.rs:955` is the live eval
  authority path for checked edit-surface candidates.
- `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1193` computes
  generator-surface fingerprint evidence, not authority.

Smallest verification commands:

```bash
rg -n "apply_code_edit_tool|resolve_code_edit_request|stage_semantic_edit_proposal" crates/ploke-tui/src/rag/tools.rs
rg -n "approve_edits|apply_semantic_edit|apply_ns_edit" crates/ploke-tui/src/rag/editing.rs
rg -n "generator_bounds|touches\\(|LowerRequest|LowerProposal|Proposal::stage|Apply::from_results|validate\\(" crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs
rg -n "validate_edit_surface_candidate|generator_surface_for_proposed_touches|grant\\.check|Apply::from_results|validate\\(&after_artifact\\)" crates/ploke-eval/src/cli/prototype1_state/backend.rs
rg -n "EditProposal|SurfaceGrant|TUI/tool harness|not authority|ploke-eval owns the authority checks" docs/workflow/evalnomicon/drafts/edit-surface/model.md
```
