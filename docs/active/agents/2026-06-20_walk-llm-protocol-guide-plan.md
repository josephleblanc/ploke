# 2026-06-20 Walk LLM protocol surfacing and operator guide plan

## Goal

Prepare Prototype 1 for more live runs by making `loop walk llm` a better review surface for completed tool-calling loops and by documenting how operators should use `loop walk` for both live driving and historical debugging.

## Scope

### 1. Protocol review surfaced in `loop walk llm`

Add read-only protocol inspection for nested LLM/tool-loop checkpoints:

- New command:
  - `ploke-eval loop walk llm protocol`
  - `ploke-eval loop walk llm protocol --json`
  - lane/session selection should match existing `lanes/timeline/show/tool/prompt` behavior.
- Session-level output should summarize whether protocol artifacts are present, and if present show:
  - protocol run identity / artifact source;
  - coverage counts;
  - aggregate call review verdict counts;
  - segment review verdict counts;
  - missing call/segment review counts;
  - protocol issues by call where available.
- Existing drill-downs should be enriched when protocol data is present:
  - `walk llm show --step N` should mention protocol status for tool calls in that checkpoint.
  - `walk llm tool --step N` should include per-tool-call protocol review details for the selected historical call.
- Mapping rule:
  - enumerate persisted `tool_requests` in checkpoint order across the selected session;
  - use that zero-based/one-based call sequence to join to `ProtocolCallReviewRow.focal_call_index` conservatively;
  - keep output explicit when no matching row exists.
- If no protocol data is present, say so plainly:
  - `protocol: not present for this tool-loop session`

### 2. User/LLM guide for `loop walk`

Add a guide at:

```text
crates/ploke-eval/docs/guides/loop-walk.md
```

The guide should explain:

- which binary to use:
  - local dev binary when testing local code changes;
  - target worktree binary for authority-bearing live parent execution;
- read-only historical review commands:
  - `walk summary`, `walk replay/back/forward`, `walk llm lanes/focus/timeline/show/prompt/tool/protocol`;
- live driving commands:
  - `walk start`, `walk step`, `walk step --watch`, `walk step --allow git-changes`;
- effectful nested debugger commands:
  - `walk llm step`, `walk llm finish` and their required `--allow workspace-mutation` / `--watch` gates;
- how to review tool-loop evidence and protocol output before another live run;
- why the walk server should be restarted after protocol/format changes.

## Guardrails

- Keep `walk llm protocol` read-only.
- Do not weaken protected-write enforcement.
- Do not treat protocol review output as authority; it is debugging/review evidence.
- Do not silently infer protocol data when artifacts are absent.
- Avoid changing canonical child-plan admission semantics.

## Initial GitNexus impact snapshot

Checked before implementation:

- `Prototype1StateWalkLlmSubcommand`: LOW, no impacted processes.
- `WalkRequestBody`: LOW, no impacted processes.
- `crates/ploke-eval/src/cli/prototype1_state/walk/client.rs::run`: LOW, no impacted processes.
- `WalkController::llm_tool_report`: LOW, no impacted processes.
- `WalkController::render_llm_tool`: LOW, one direct test impact.
- `WalkController::llm_report`: LOW, two direct test impacts.

## Implementation plan

1. Add CLI/protocol/client/server plumbing for `walk llm protocol`.
2. Add controller protocol loading and session-to-call-index mapping helpers.
3. Render human and JSON protocol reports.
4. Enrich `show` and `tool` with protocol snippets when protocol data is present.
5. Add controller and CLI parser tests.
6. Write `crates/ploke-eval/docs/guides/loop-walk.md`.
7. Run formatting, focused tests, `cargo check -p ploke-eval --all-targets`, and `git diff --check`.
8. Update this plan with completed work and remaining next steps.

## Completion update

Completed in this slice:

- Added read-only `ploke-eval loop walk llm protocol` and `--json` plumbing through CLI args, walk IPC protocol, client, server, and controller.
- Added protocol artifact discovery for selected tool-loop sessions. The loader searches the session debug directory, result-adjacent protocol artifact directories, and published `ProtocolArtifacts` evidence roots when available.
- Added session tool-call enumeration across persisted checkpoint steps and a conservative zero-based mapping to protocol `focal_call_index` values.
- Added human protocol output with:
  - explicit absent state: `protocol: not present for this tool-loop session`;
  - artifact counts by procedure;
  - call-review coverage and missing indices;
  - segment and segment-review counts when present;
  - per-call review feedback when present.
- Added JSON output for `walk llm protocol --json` with `kind=llm_protocol`, searched dirs, artifact summaries, session call mapping, call reviews, missing call indices, and segment counts.
- Enriched `walk llm show --step N` with protocol status and step-local tool-call feedback.
- Enriched `walk llm tool --step N` with selected-call protocol feedback and artifact path when review data exists.
- Added regression coverage:
  - `llm_protocol_render_shows_call_feedback_and_json`;
  - `loop_walk_llm_protocol_command_parses`.
- Wrote the operator/LLM guide at `crates/ploke-eval/docs/guides/loop-walk.md`.
- Restarted the walk server with the rebuilt local dev binary and manually checked the live-lane historical session:
  - `walk llm protocol` reports protocol absence plainly;
  - `walk llm protocol --json` is pipeable through `jq`;
  - `walk llm show --step 11` and `walk llm tool --step 11` include protocol absence lines.

Validation completed:

```bash
cargo fmt --all
cargo test -p ploke-eval walk::controller::tests::llm_protocol_render_shows_call_feedback_and_json -- --nocapture
cargo test -p ploke-eval loop_walk_llm_protocol_command_parses -- --nocapture
cargo test -p ploke-eval walk::controller::tests -- --nocapture
cargo check -p ploke-eval --all-targets
cargo build -p ploke-eval
./target/debug/ploke-eval loop walk llm protocol --help
./target/debug/ploke-eval loop walk llm --repo-root /home/brasides/.ploke-eval/worktrees/p1-live-lanes-g25p-20260619-123437 protocol
./target/debug/ploke-eval loop walk llm --repo-root /home/brasides/.ploke-eval/worktrees/p1-live-lanes-g25p-20260619-123437 protocol --json | jq '{kind, present, total_calls, missing: (.missing_call_indices|length)}'
```

Notes and remaining work:

- Existing historical session `2c30ef18-9191-4025-a05e-c34c4a06347a` has no persisted protocol artifacts, so current manual output correctly reports absence.
- The command currently scans known likely artifact locations; future runs that persist exact protocol artifacts under those locations will be surfaced without changing checkpoint history.
- Exact request tool definitions are still not persisted in historical `ChatDebugStep`/tool-loop records; `walk llm tool` remains a current-renderer schema view.
- Full typed reuse of `ProtocolAggregate` remains future work for run-record-backed protocol data. This slice reuses existing stored protocol artifact loading/summaries and keeps identity inference conservative for tool-loop checkpoints that do not have compressed `RunRecord` identity.
- Higher-risk prompt cleanup for the leaked child prompt planning sentence was intentionally left out of this slice after GitNexus reported `render_prompt` as HIGH blast radius; handle that as a separate reviewed change.
- Future improvements: persist protocol artifacts directly under the tool-loop session directory, add richer per-segment display, and add typed JSON for full step/timeline records.
