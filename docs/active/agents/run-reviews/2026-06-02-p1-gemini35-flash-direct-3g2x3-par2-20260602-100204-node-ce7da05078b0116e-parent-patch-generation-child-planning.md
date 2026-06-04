# Prototype 1 parent patch-generation / child-planning run review

Status: durable run review for `node-ce7da05078b0116e`. This is a read-only review of the parent patch-generation / child-planning turn for campaign `p1-gemini35-flash-direct-3g2x3-par2-20260602-100204`.

## Short verdict

The harness turn is mechanically real, but it did not produce a benchmark-useful parent patch or a persisted child-plan result. The run ended with terminal provider failure (`provider_unavailable` / HTTP 429), `patch_artifact.applied=false`, no submitted-result JSON, and no child-plan artifact in the checked result surface.

The only suspiciously "successful" local result is a focused `cargo check` / `cargo test` sequence on `crates/ingest/syn_parser/Cargo.toml` inside the turn-live trace. That is a real diagnostic signal, but it is not proof of descendant-benchmark progress because it is crate-local and the run never produced an admitted patch or child-plan outcome.

## Evidence roots

- Campaign id: `p1-gemini35-flash-direct-3g2x3-par2-20260602-100204`
- Campaign root: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-100204`
- Prototype root: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-100204/prototype1`
- Worktree: `/home/brasides/.ploke-eval/worktrees/p1-gemini35-flash-direct-3g2x3-par2-20260602-100204`
- Parent node record: `prototype1/nodes/node-ce7da05078b0116e/node.json`
- Runner request: `prototype1/nodes/node-ce7da05078b0116e/runner-request.json`
- Transition journal: `prototype1/transition-journal.jsonl`
- Run profile: `prototype1/run-profile.toml`
- Campaign manifest: `campaign.json`
- Headless diagnostics: `prototype1/messages/edit-harness-result/node-ce7da05078b0116e.headless-tui.json`
- Turn-live summary: `prototype1/messages/edit-harness-result/node-ce7da05078b0116e.turn-live/agent-turn-summary.json`
- Turn-live trace: `prototype1/messages/edit-harness-result/node-ce7da05078b0116e.turn-live/agent-turn-trace.json`
- Result directory checked for a submitted result: `prototype1/messages/edit-harness-result/`

## Exact execution path

The observed path is the Prototype 1 broad-harness parent patch-generation path, with request publication configured but no admitted child result:

```text
loop prototype1-state
  -> run profile generation.source = broad-harness-request
  -> publish_broad_harness_child_plan_request / publish_broad_edit_harness_request
  -> run_broad_headless_tui_attempt_with_options(slot)
       -> GitWorktreeBackend.prepare_broad_harness_workspace
       -> read published prompt
       -> tui_adapter::run_headless_with_model_capture_responses
       -> write_broad_headless_tui_diagnostics
       -> write_broad_headless_tui_turn_live_bundle
       -> finish_broad_headless_tui_attempt
```

The route/model provenance for this run is direct Google Gemini 3.5 Flash:

- `campaign.json` sets `model_id = "google/gemini-3.5-flash"` and `route_source = "direct_google"`
- `prototype1/run-profile.toml` sets `model.id = "google/gemini-3.5-flash"` and `route_source = "direct-google"`
- `agent-turn-summary.json` reports `selected_model = "google/gemini-3.5-flash"`

## Scheduler / node / transition state

The persisted scheduler and node surfaces are intentionally small for this run:

- `scheduler.json` contains only the root parent node `node-ce7da05078b0116e` at generation 0, `branch_id = prototype1-parent-p1-gemini35-flash-direct-3g2x3-par2-20260602-100204-gen0`, and `status = planned`.
- `node.json` and `runner-request.json` agree on the same parent identity, generation 0, and repository root.
- `transition-journal.jsonl` contains only the parent startup record plus a measured cargo target resource record.

That means the checked scheduler surface does not show any staged child nodes or downstream child-admission state for this turn.

## Child-plan / branch-registry surface

There is no persisted child-plan artifact in the checked result surface for this node.

Verified absence checks:

- no `prototype1/messages/edit-harness-result/node-ce7da05078b0116e.json`
- no `*child*` file under `prototype1/messages/edit-harness-result/`
- no submitted-result JSON anywhere under the checked result tree

The surrounding workspace does contain source/docs about child-plan machinery and branch provenance, but those are implementation/context documents, not a run-scoped child-plan message for this turn. For this run, the durable evidence stops at request publication and diagnostics; there is no child-plan payload to join to a submitted result.

## Concrete trace chain

One concrete trace chain from the turn-live bundle is:

```text
read `crates/ingest/syn_parser/benches/type_resolution.rs`
  -> read `crates/ingest/syn_parser/benches/type_resolution_corpus.rs`
  -> run `cargo check` (focused on `crates/ingest/syn_parser/Cargo.toml`) and it succeeds
  -> run `cargo test` on the same focused manifest and it exits 101
  -> continue exploration with code-graph reads / searches around `ModuleTree`, `ancestor`, and `get_code_edges`
  -> terminal ends as `provider_unavailable` with HTTP 429 RESOURCE_EXHAUSTED
```

This chain matters because it shows real tool use and real diagnostic feedback, but not a patch-worthy or benchmark-grade result. The validation commands were focused on `crates/ingest/syn_parser/Cargo.toml`, not on the requested root-level `cargo check` / `cargo test` contract or a descendant-benchmark success path.

## Verified suspicious result

The suspicious-looking result is the successful `cargo check` entry in `agent-turn-summary.json` / `headless-tui.json`.

Verification:

- `validations[0]` reports `ok: true`, `command: "check"`, `exit_code: 0`, `manifest_path: "/home/brasides/.ploke-eval/campaigns/.../prototype1/workspaces/edit-harness/node-ce7da05078b0116e/crates/ingest/syn_parser/Cargo.toml"`
- `validations[1]` reports `cargo test` against the same focused manifest and `exit_code: 101`
- `patch_artifact.applied` is `false`
- `terminal_record.outcome` is `aborted` with HTTP 429 / `RESOURCE_EXHAUSTED`

So the check/test pair is real, but it is local crate validation, not proof of an admissible patch, not proof of child planning, and not proof of benchmark usefulness.

## Mechanical completion vs benchmark usefulness

Mechanical evidence present:

- the parent runtime started
- the runner request exists
- the headless TUI attempt produced a diagnostics JSON and turn-live bundle
- the turn made real tool calls and recorded real validation output

Benchmark-usefulness missing:

- no edit proposal was applied
- no submitted-result JSON exists
- no child-plan artifact exists in the result surface
- no descendant benchmark measurement exists
- no root-level request-contract validation is recorded

## LLM / tool behavior and provider failure

- `selected_model`: `google/gemini-3.5-flash`
- `terminal_record.outcome`: `aborted`
- `terminal_record.summary`: HTTP 429 `RESOURCE_EXHAUSTED` from Vertex / Google AI Platform
- `patch_artifact.applied`: `false`
- `patch_artifact.edit_proposals`: `[]`
- `patch_artifact.create_proposals`: `[]`
- `attempts` in `headless-tui.json`: no successful edit proposals, no applied patch, and a provider-unavailable terminal result

The provider failure is the reason the turn does not reach a durable patch or child-plan submission. It is distinct from a model-quality failure: the trace shows the model did real retrieval/validation work before the provider aborted.

## What is working

- The scheduler/node/runner-request spine is internally consistent for the root parent node.
- The turn-live bundle preserves enough of the internal trace to reconstruct real tool usage.
- The diagnostics record distinguishes focused validation from provider failure.
- The run profile clearly records model and route provenance.

## What is not working yet

- No submitted-result JSON was written.
- No child-plan artifact was persisted in the checked result surface.
- No applied patch was produced.
- The final outcome is provider exhaustion, not benchmark progress.
- The validations that did run are crate-local diagnostics, not the requested benchmark-contract proof.

## Action items

1. Persist a first-class child-plan / no-child-plan outcome even when the provider aborts before submission. Right now the result surface only shows request/trace/diagnostics, which makes "nothing happened" look too similar to "child planning was intentionally empty."
2. Keep focused crate validations labeled as local diagnostics unless they satisfy the request contract. A passing `cargo check` on `syn_parser` should not be over-read as benchmark proof.
3. Preserve provider-failure classification separately from patch success or child admission. `provider_unavailable` should remain its own terminal reason so it is not scored as a model/patch failure.
4. Add a compact join from scheduler state to child-plan/request publication when the result surface is empty. The current checked state makes it hard to tell whether the run never attempted child planning or simply never persisted it.
