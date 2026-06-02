# `protocol run` latency and UX

## What the wait is doing

- The CLI wrapper in [`crates/ploke-eval/src/cli.rs`](crates/ploke-eval/src/cli.rs) computes `before` state, awaits one protocol step, then recomputes `after` state. See `protocol_state_for_run` and the `protocol run` branch around lines 3448-3503.
- That means the user-visible pause includes:
  - record decode and artifact directory scans before the step
  - the actual protocol procedure
  - another full state recompute after the step
- For intent segmentation, the procedure is mostly one LLM call plus local transforms.
- For tool-call review and segment review, the procedure is three LLM calls, but they are serialized by nested `FanOut` composition, not parallelized.

## Likely latency drivers

- LLM time is the dominant cost when the next step is review or segmentation. The LLM call path is `JsonAdjudicator -> chat_step -> reqwest`.
- Protocol-local CPU work is moderate:
  - `crates/ploke-protocol/src/tool_calls/segment.rs:468-503` runs contextualize -> LLM -> normalize.
  - `crates/ploke-protocol/src/tool_calls/review.rs:562-685` runs contextualize -> 3 branch LLM judgments -> assemble.
  - `crates/ploke-protocol/src/procedure.rs:87-178` shows `Sequence` and `FanOut` are awaited serially.
- IO is non-trivial but probably secondary:
  - `crates/ploke-eval/src/protocol_artifacts.rs:133-191` writes the artifact, then syncs registration state.
  - `crates/ploke-eval/src/run_registry.rs:174-196` rescans artifacts and may reload the aggregate on every write.
  - `protocol_state_for_run` and `load_latest_segmented_sequence` rescan the artifact directory and deserialize JSON again.

## Missing operator feedback

- `protocol run` prints nothing until the awaited step finishes, so there is no visible "started", "running segmentation", "running review branch", or "writing artifact" feedback.
- The console tracer is WARN-only in [`crates/ploke-eval/src/tracing_setup.rs`](crates/ploke-eval/src/tracing_setup.rs:87-103), so the useful `info!` traces are file-only.
- The actual timing hooks already exist but are not surfaced live:
  - `crates/ploke-llm/src/manager/session.rs:57-157` emits `chat_http_request_start`, `chat_http_response_headers`, and request/body error events on the `chat_http` target.
  - The full request/response payloads go to the `api_json` target in `log_api_request_json`, `log_api_raw_response`, and `log_api_parsed_json_response`.
- The CLI also hides the sub-step identity; users only learn the final `executed:` label after completion.

## What to inspect next

- [`crates/ploke-eval/src/cli.rs`](crates/ploke-eval/src/cli.rs:3440-3503, 4232-4585): add phase prints or elapsed timers around `protocol_state_for_run`, each `execute_protocol_*_quiet` call, and the post-run recompute.
- [`crates/ploke-protocol/src/tool_calls/segment.rs`](crates/ploke-protocol/src/tool_calls/segment.rs:468-503, 505-620): verify whether segmentation prompt size or normalization is material.
- [`crates/ploke-protocol/src/tool_calls/review.rs`](crates/ploke-protocol/src/tool_calls/review.rs:562-685, 341-413): confirm the three branch prompts are the main wall-time source.
- [`crates/ploke-protocol/src/procedure.rs`](crates/ploke-protocol/src/procedure.rs:87-178): consider parallelizing `FanOut` if ordering does not matter.
- [`crates/ploke-llm/src/manager/session.rs`](crates/ploke-llm/src/manager/session.rs:57-220): attach wall-time fields to the existing `chat_http` logs.
- [`crates/ploke-eval/src/protocol_artifacts.rs`](crates/ploke-eval/src/protocol_artifacts.rs:133-191) and [`crates/ploke-eval/src/run_registry.rs`](crates/ploke-eval/src/run_registry.rs:174-196): measure artifact write/sync overhead.
