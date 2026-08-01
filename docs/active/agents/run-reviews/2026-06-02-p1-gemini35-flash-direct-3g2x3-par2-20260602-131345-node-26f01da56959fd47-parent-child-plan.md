# Prototype 1 parent patch-generation / child-planning run review

Verdict: parent planning/request publication is mechanically real, but descendant usefulness is not yet established. The live parent node has published the broad-harness request slots and materialized only the first two candidate workspaces. There is no submitted broad-harness result, no child self-eval completion, no runner result, and no protocol artifact set yet. Treat r3-r9 as request-only slots, not completed children.

Review scope: this is a point-in-time review of the parent patch-generation / child-planning turn for the active Prototype 1 campaign. I did not advance the loop and did not run doctor/step/observe commands. Evidence is from persisted artifacts and read-only process/git checks.

## Evidence roots

- Project root: `/home/brasides/code/ploke`
- Campaign id: `p1-gemini35-flash-direct-3g2x3-par2-20260602-131345`
- Campaign root: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345`
- Prototype root: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345/prototype1`
- Worktree/repo root: `/home/brasides/.ploke-eval/worktrees/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345`
- Parent node: `node-26f01da56959fd47`
- Task: `BurntSushi__ripgrep-2209`
- Model route: `google/gemini-3.5-flash` via direct Google (`campaign.json` uses `route_source: direct_google`; run profile uses `route_source = "direct-google"`)
- Parent node record: `prototype1/nodes/node-26f01da56959fd47/node.json`
- Runner request: `prototype1/nodes/node-26f01da56959fd47/runner-request.json`
- Transition journal: `prototype1/transition-journal.jsonl`
- Run profile: `prototype1/run-profile.toml`
- Broad-harness request dir: `prototype1/messages/edit-harness-request`
- Broad-harness workspace dir: `prototype1/workspaces/edit-harness`
- Eval run root: `/home/brasides/.ploke-eval/instances/prototype1/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345/BurntSushi__ripgrep-2209/runs/run-1780406339507-structured-current-policy-34e9980e`

## Exact execution path

The active runtime path is the Prototype 1 state-machine path, not a standalone benchmark-only eval report and not a completed child self-eval:

```text
./target/debug/ploke-eval loop prototype1-state --debug-tools
  -> run profile generation.source = broad-harness-request
  -> run_parent_target_selection(..., CandidateGenerationConfig::BroadHarnessRequest, child_budget)
  -> publish_broad_harness_child_plan_request(...)
  -> publish_broad_edit_harness_request(...) for each slot
  -> for attempted slots: run_broad_headless_tui_attempt_with_options(...)
       -> GitWorktreeBackend.prepare_broad_harness_workspace(...)
       -> read published prompt
       -> tui_adapter::run_headless_with_model_capture_responses(...)
       -> finish_broad_headless_tui_attempt(...) only if a terminal applied edit is reached
```

Evidence for this path:

- `runner-request.json` requests `loop prototype1-state --repo-root /home/brasides/.ploke-eval/worktrees/...` and targets `.ploke/prototype1/parent_identity.json`.
- The live process check found PID `1856868` active with command `./target/debug/ploke-eval loop prototype1-state --debug-tools`, cwd `/home/brasides/.ploke-eval/worktrees/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345`, and exe `.../target/debug/ploke-eval`.
- `run-profile.toml` sets `[generation] source = "broad-harness-request"`, child budget min 2/max 3, and `parallel_targets = 2`.
- Source-side path witness in the current checkout: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs` dispatches `CandidateGenerationConfig::BroadHarnessRequest` to `publish_broad_harness_child_plan_request`; that function allocates `child_budget.max * BROAD_TUI_FRESH_ATTEMPTS_PER_CHILD` slots, publishes each request, and carries the first published request into `Parent<AwaitingHarnessPlan>`. The later attempt path prepares the workspace, reads the prompt, and runs `tui_adapter::run_headless_with_model_capture_responses`.

## Parent identity and durable state

Parent identity is internally consistent across the worktree, journal, and node records:

- `.ploke/prototype1/parent_identity.json` records parent/node `node-26f01da56959fd47`, generation 0, instance `BurntSushi__ripgrep-2209`, branch `prototype1-parent-p1-gemini35-flash-direct-3g2x3-par2-20260602-131345-gen0`, created at `2026-06-02T13:14:59.399744914+00:00`.
- `prototype1/transition-journal.jsonl` has `parent_started` for the same identity, repo root, and PID `1856868`, plus a measured cargo target resource event.
- `node.json` records the same node id, generation, instance, branch, and `status: running` with updated_at `2026-06-02T13:22:36.992730554+00:00`.
- `scheduler.json` still lists the same node in `frontier_node_ids` and `status: planned`. This is a projection gap or stale scheduler view relative to `node.json` and the live process, not evidence of child completion.

The worktree itself is on branch `prototype1-parent-p1-gemini35-flash-direct-3g2x3-par2-20260602-131345-gen0` at commit `8f478c66e30224adf234439c8c6395bf521a6373` and had no read-only git status output.

## Closure, eval, patch, and protocol state

Closure/eval state is mechanically complete for the benchmark run, but protocol is missing and this does not mean broad-harness descendants completed.

- `closure-state.json` at the campaign root reports registry complete: expected 1, mapped 1, missing 0.
- Eval is complete: expected 1, complete 1, failed 0, missing 0; last transition at `2026-06-02T13:22:20.502856569+00:00`.
- Protocol is missing: expected 1, full 0, missing 1. All required procedures are missing: `tool-call-intent-segments`, `tool-call-review`, and `tool-call-segment-review`.
- The eval run root exists and contains `record.json.gz`, `agent-turn-trace.json`, `agent-turn-summary.json`, `llm-full-responses.jsonl`, `validation-audit.json`, `multi-swe-bench-submission.jsonl`, and `benchmark-patch-projection.json`.
- `benchmark-patch-projection.json` reports `check.status: passed` and an exported non-empty `multi-swe-bench-submission.jsonl` from the recorded checkout cwd.
- `validation-audit.json` records changed paths `crates/printer/src/util.rs` and `crates/printer/src/standard.rs`; the final cargo call was a workspace `cargo test` covering changed files and `ok: true`. It also warns that no formatting check evidence was recorded.

Benchmark patch output is therefore present, but the active report target is the later parent planning/broad-harness publication state. Do not conflate benchmark eval completion with a completed broad-harness child result.

## Broad-harness request publication state

Nine request slots were published in `prototype1/messages/edit-harness-request`: the parent slot plus `r2` through `r9`. This matches the run profile max child budget of 3 multiplied by the broad TUI fresh-attempt fanout of 3 slots per child.

| Slot | Request id | Request/prompt | Workspace | Submitted result |
| --- | --- | --- | --- | --- |
| parent | `broad-harness-request:node-26f01da56959fd47` | json + md present | present | absent |
| r2 | `broad-harness-request:node-26f01da56959fd47:r2` | json + md present | present | absent |
| r3 | `broad-harness-request:node-26f01da56959fd47:r3` | json + md present | absent | absent |
| r4 | `broad-harness-request:node-26f01da56959fd47:r4` | json + md present | absent | absent |
| r5 | `broad-harness-request:node-26f01da56959fd47:r5` | json + md present | absent | absent |
| r6 | `broad-harness-request:node-26f01da56959fd47:r6` | json + md present | absent | absent |
| r7 | `broad-harness-request:node-26f01da56959fd47:r7` | json + md present | absent | absent |
| r8 | `broad-harness-request:node-26f01da56959fd47:r8` | json + md present | absent | absent |
| r9 | `broad-harness-request:node-26f01da56959fd47:r9` | json + md present | absent | absent |

Important verified claim: r3-r9 are request-only. Their JSON and markdown prompts point to candidate workspace paths, but `os.path.isdir` checks found no corresponding workspace directories under `prototype1/workspaces/edit-harness`, and `prototype1/messages/edit-harness-result` does not exist. They must not be classified as completed self-evals.

Request content is consistent across slots:

- `parent_node_id` is `node-26f01da56959fd47`.
- Candidate workspaces are slot-scoped, e.g. parent slot `.../workspaces/edit-harness/node-26f01da56959fd47`, r2 `.../node-26f01da56959fd47-r2`, r3 `.../node-26f01da56959fd47-r3`.
- Each slot has `child_budget: { min_children: 1, max_children: 1 }`; these are individual patch-attempt slots, not proof that a child node was admitted.
- Each request points to the same admission target artifact `artifact:git-commit:8f478c66e30224adf234439c8c6395bf521a6373` and policy `workspace except ploke-eval`.
- Each prompt begins with the instruction to modify that slot’s candidate checkout to improve `Prototype 1 descendant performance` while staying outside protected core.
- The prompt files are present and readable; I did not find a separate first-class persisted prompt-preflight record. Workspace materialization for the parent slot and r2 is the durable sign that those two slots progressed past request publication into attempt setup.

## Workspace and edit-attempt observations

The parent slot and r2 slot have materialized workspaces:

- `workspaces/edit-harness/node-26f01da56959fd47` exists, contains `Cargo.toml`, and has `.ploke/prototype1/parent_identity.json`. It is not a standalone `.git` directory, but `git -C` works via repository linkage. Read-only `git status --short` showed three modified files:
  - `crates/ploke-core/src/compilation_unit.rs`
  - `crates/ploke-core/src/lib.rs`
  - `crates/ploke-core/src/workspace.rs`
- Read-only `git diff --stat` for the parent slot showed 3 files changed, 35 insertions, 5 deletions.
- `workspaces/edit-harness/node-26f01da56959fd47-r2` exists, contains `Cargo.toml`, and has `.ploke/prototype1/parent_identity.json`. Read-only `git status --short` showed no changed files.
- No workspace directory exists for r3-r9.

This is a high-signal distinction: the parent slot appears to have an in-progress or unsubmitted edit in its candidate workspace, but the absence of `messages/edit-harness-result/node-26f01da56959fd47.json` means the harness has not accepted or published that attempt as a submitted broad-harness result. r2 is materialized but not yet dirty/submitted. r3-r9 have not even reached materialized workspace state.

## Concrete trace reconstruction

Concrete chain for the current parent planning turn:

```text
parent_started journal record + parent_identity.json
  -> node runner enters prototype1-state for generation 0 parent
  -> closure eval completes for BurntSushi__ripgrep-2209 and writes eval run artifacts
  -> run-profile generation.source selects broad-harness-request path
  -> publish_broad_harness_child_plan_request writes 9 request JSON/MD prompt pairs
  -> first published request becomes Parent<AwaitingHarnessPlan> identity anchor
  -> attempt setup materializes parent-slot workspace and r2 workspace
  -> parent-slot workspace receives unsubmitted file changes
  -> no submitted_result JSON exists; no runner-result.json exists; r3-r9 remain request-only
```

The last point where the parent had enough information to act was after the eval closure/patch projection was available and the broad-harness profile selected the request-publication path. It did act by publishing request slots. The next missing point is child response/admission: no slot has produced a submitted result to admit into a child plan, so there is not yet a descendant benchmark-usefulness signal.

## Mechanical completion vs benchmark usefulness

Mechanical successes:

- Parent identity and node records exist.
- Eval closure completed for the initial benchmark run.
- A benchmark patch projection and Multi-SWE-bench submission were written.
- Broad-harness request JSON/MD files were published for all nine slots.
- Parent and r2 candidate workspaces were materialized.

Not yet benchmark-useful for descendants:

- No broad-harness submitted result exists.
- No child plan was received/admitted from these broad-harness requests.
- No child node, child self-eval, successor-ready record, or successor-completion record exists for r3-r9.
- Protocol artifacts for the eval run are missing, so protocol cannot yet judge the benchmark trace.
- The dirty parent-slot workspace may be useful, but without a submitted result and admission artifact it is only an in-progress workspace state.

## Protocol review and blind spots

Protocol is missing, not merely semantically weak. `closure-state.json` reports missing protocol for all three required procedures. Therefore no protocol judgment should be cited for this campaign turn.

The current artifact layout also forces manual joins:

- Request files are present, but there is no compact persisted slot ledger with per-slot publication/materialization/submission/admission status.
- Prompt files are present and readable, but I found no first-class prompt-preflight artifact separate from the attempt path.
- Workspace materialization has to be verified by checking directories and git state.
- The scheduler projection still says the parent is planned while `node.json` and PID evidence say running.

These are observability/playback gaps, not necessarily runtime blockers.

## What is working

- The parent identity spine is durable and cross-checks through worktree identity, journal, node JSON, runner request, branch name, and PID/cwd.
- The eval run artifact set is complete enough to know a benchmark patch projection exists and validation covered changed files.
- Broad-harness request publication produced deterministic, slot-scoped JSON and markdown prompt files with request hashes and submitted result paths.
- Candidate workspaces are isolated by slot name and can be inspected independently.
- The published requests correctly keep authority/admission/grant/child-plan return evidence as `not_claimed`, which helps avoid overclaiming before submitted results exist.

## What is not working yet / pending

- The live node is still running; `runner-result.json` is absent.
- No submitted broad-harness result has been written for parent or r2, and the result directory itself is absent.
- r3-r9 have request/prompt files only; no materialized workspace, result, or child evidence.
- Protocol is missing for the eval run.
- Scheduler and node status projections disagree (`scheduler.json` planned/frontier vs `node.json` running and active PID).
- Parent-slot workspace has unsubmitted changes, so a reviewer could mistakenly read dirty git state as a completed child unless they join it to the absent submitted result path.

## Action items

1. Persist a per-slot broad-harness ledger. The artifact gap is manual joining across request JSON, prompt MD, workspace directories, result paths, and git status. The ledger should label each slot as published, prompt-read/preflighted, workspace-materialized, submitted, admitted/rejected, or pending.

2. Add a first-class prompt/workspace preflight record. Current evidence proves prompt files are present/readable and workspaces exist for parent/r2, but there is no separate durable preflight result for all slots. This would prevent conflating request publication with attempt execution.

3. Reconcile scheduler vs node/process status projection. `scheduler.json` still says planned/frontier while `node.json` says running and PID 1856868 is active. Add or expose a live projection that makes this join explicit.

4. Keep request-only slots out of child self-eval accounting. r3-r9 should remain pending/request-only until their workspace and submitted result artifacts appear.

5. Run or repair protocol only after preserving this current state. Closure says protocol missing for all required procedures; protocol follow-up is needed before using protocol claims as selection evidence.

6. For the parent dirty workspace, wait for the submitted result/admission artifact before classifying the edit. If the process later times out or rejects the attempt, the dirty files should be described as unsubmitted workspace state, not a child patch.
