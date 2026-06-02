## esm-map-eval-code

Read-only scout report. No files changed.

Larger object from `model.md`:

```text
OperationCoordinate -> SurfaceGrant -> Proposal -> Check -> ArtifactDelta -> History evidence
```

Current eval-owned code has fragments of the chain, but proposal/check is split
across carriers and History evidence collapses grant/coordinate.

Carrier map:

- `SurfaceGrant / Grant`: `crates/ploke-eval/src/cli/prototype1_state/edit_surface/surface.rs:132`
  through `:196`. Current `surface::Grant` carries `artifact`, `graph`,
  `write`, and `forbidden`, then emits `surface::Check`.
- `Proposal / EditProposal`: `crates/ploke-eval/src/cli/prototype1_state/backend.rs:234`
  through `:302` for `backend::EditProposal`; staged TUI-side carrier is
  `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs:656`
  through `:706`.
- `Check / CheckedProposal`: `crates/ploke-eval/src/cli/prototype1_state/edit_surface/surface.rs:205`
  through `:229`; current checked-proposal equivalent is
  `crates/ploke-eval/src/cli/prototype1_state/backend.rs:317` through `:430`,
  produced by `validate_edit_surface_candidate` at `:955`.
- `ArtifactDelta`: `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness.rs:23`
  through `:52`; checked/apply projection is in
  `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs:733`
  through `:885`.
- `SurfaceEvidence`: `crates/ploke-eval/src/cli/prototype1_state/history.rs:2871`
  through `:3090`; attached to `CandidateArtifact` at `:3106`.
- `surface_attempt`: `crates/ploke-eval/src/cli/prototype1_state/history.rs:2897`
  through `:2958`; selection payload assembly consumes it at
  `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:7242`.
- `OperationCoordinate`: `crates/ploke-eval/src/loop_graph.rs:125` through
  `:155`; History-side reduced coordinate is `CandidateCoordinate` at
  `crates/ploke-eval/src/cli/prototype1_state/history.rs:2628`.

Drift points:

- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:23`
  and `:487` make prompt and child-plan file paths part of published request
  identity/hash; this is file-address authority rather than grant/check
  authority.
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:854` and `:1227`
  surface broad-harness handoff as request file + prompt file + child-plan file.
- `crates/ploke-eval/src/cli/prototype1_state/parent.rs:107`,
  `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1299`, and
  `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1875` validate and
  consume `ChildPlanFiles` mainly by receiver/generation/contained-child
  membership.
- `SurfaceEvidence` and `CandidateArtifact` persist proposal/apply evidence but
  not full `SurfaceGrant` or full operation coordinate.
- `CandidateCoordinate` collapses `(generator Runtime, target Artifact)` into
  node/generation/runtime metadata.

Smallest verification commands:

```bash
rg -n "struct Grant|struct Check" crates/ploke-eval/src/cli/prototype1_state/edit_surface/surface.rs
rg -n "struct EditProposal|struct CheckedSurfaceEdit|fn validate_edit_surface_candidate" crates/ploke-eval/src/cli/prototype1_state/backend.rs
rg -n "struct Proposal|fn stage|struct Apply|fn from_results|fn delta" crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs
rg -n "struct ArtifactDelta" crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness.rs
rg -n "struct SurfaceEvidence|mod surface_attempt|struct CandidateCoordinate|struct CandidateArtifact" crates/ploke-eval/src/cli/prototype1_state/history.rs
rg -n "enum OperationTarget|struct Coordinate" crates/ploke-eval/src/loop_graph.rs
rg -n "prompt_path|child_plan_path|validate_and_write_tui_child_plan|validate_received_child_plan|validate_child_plan" crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs crates/ploke-eval/src/cli/prototype1_state/parent.rs
```
