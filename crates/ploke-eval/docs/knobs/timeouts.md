# ploke-eval timeout knobs

This is a practical map of the timeout and polling surfaces currently found in
`crates/ploke-eval`. It focuses on values that can stop, delay, or repeatedly
poll an operator-visible path.

Search basis for this pass:

- `timeout`, `deadline`, `TimedOut`, `ReadyTimedOut`
- `Duration::from_secs`, `Duration::from_millis`
- `tokio::time::timeout`, `sleep`, `thread::sleep`
- `wall_clock_secs`, `llm_timeout_secs`, `tool_call_timeout_secs`, `bm25_timeout_ms`

Pure elapsed-time reporting and fixture-only latency fields are intentionally
left out.

## Benchmark eval runs

| Surface | Default | Operator control | Enforcement |
| --- | ---: | --- | --- |
| Eval run wall clock | `1800s` | `--wall-clock-secs` on prepare commands, or `eval.budget.wall_clock_secs` in campaign config | `runner.rs::run_benchmark_turn` stops with `PrepareError::Timeout { phase: "benchmark_turn" }` |
| Eval LLM request timeout | `ploke_llm::LLM_TIMEOUT_SECS` (`300s` today, owned by `ploke-llm`) | Not exposed by `ploke-eval` CLI; configured in runtime setup | `runner.rs::configure_eval_model_runtime` writes `RuntimeConfig.llm_timeout_secs` |
| Benchmark tool-call timeout | `60s` | Not exposed by `ploke-eval` CLI | `runner.rs::benchmark_chat_policy` sets `tool_call_timeout_secs = 60` |
| Benchmark timeout retry backoff | base `5s`, attempts `3` | Not exposed by `ploke-eval` CLI | `runner.rs::benchmark_chat_policy` sets `ChatTimeoutStrategy::Backoff { attempts: Some(3) }` |

Relevant files:

- `crates/ploke-eval/src/spec.rs`
  `EvalBudget` defaults to `max_turns = 40`, `max_tool_calls = 200`, and
  `wall_clock_secs = 1800`.
- `crates/ploke-eval/src/cli.rs`
  The MSB prepare and batch commands expose `--wall-clock-secs`, defaulting to
  `1800`.
- `crates/ploke-eval/src/campaign.rs`
  `EvalCampaignPolicy.budget` carries the same `EvalBudget` into campaign
  config.
- `crates/ploke-eval/src/runner.rs`
  `run_benchmark_turn` builds the wall-clock deadline from the prepared run
  budget.

## Setup and indexing waits

| Surface | Value | Operator control | Behavior |
| --- | ---: | --- | --- |
| Generic phase/index completion timeout | `300s` | Hard-coded | `runner.rs::wait_for_indexing_completion` times out `indexing_completed` and may write a failure DB snapshot if debug snapshots are enabled |
| Indexing heartbeat | `10s` | Hard-coded | Logs periodic "waiting for indexing completion" messages while waiting |
| Indexing select poll cap | `250ms` | Hard-coded | Caps each wait arm inside the indexing event loop |
| BM25 ready timeout | `60s` | Hard-coded | `runner.rs::wait_for_bm25_ready` waits for BM25 `Ready { docs > 0 }` |
| BM25 ready poll | `100ms` | Hard-coded | Sleep between BM25 status checks |
| Headless TUI BM25 service timeout | `10_000ms` | Hard-coded | Passed to `TestRuntime::new_with_embedding_processor_and_bm25_timeout` and copied into `cfg.rag.bm25_timeout_ms` |

Relevant files:

- `crates/ploke-eval/src/runner.rs`
  Constants near the top define `DEFAULT_PHASE_TIMEOUT_SECS`,
  `WAIT_HEARTBEAT_SECS`, `BM25_READY_TIMEOUT_SECS`, and
  `HEADLESS_TUI_BM25_TIMEOUT_MS`.

## Headless TUI broad-harness attempts

| Surface | Default | Operator control | Enforcement |
| --- | ---: | --- | --- |
| Published attempt turn timeout | `900s` | In the published broad-harness request contract | Consumed as the default whole-run headless TUI timeout |
| Published per-tool timeout | `180s` | In the published broad-harness request contract | Recorded in the contract, but this pass found no separate `ploke-eval` enforcement of `tool_seconds` |
| Minimum implicit attempt timeout | `60s` | Hard-coded floor when no CLI override is given | `effective_broad_tui_timeout_secs` uses `contract.attempt.timeout.turn_seconds.max(60)` |
| CLI attempt override | none | `prototype1-harness attempt --timeout-secs` and `prototype1-harness sweep --timeout-secs` | Overrides the published default directly; `0` is rejected by `tui_adapter::Budget::new` |
| Headless TUI outer timeout | effective attempt timeout | Derived from published contract or CLI override | `tui_adapter::run_headless_with_model` wraps the whole attempt loop in `tokio::time::timeout` |
| Headless TUI LLM/tool timeout | `180s` | Hard-coded for sparse strict headless runtime | `runner.rs::configure_sparse_strict_rag` sets `llm_timeout_secs`, `tool_call_timeout_secs`, and timeout backoff base to `180s` |
| Headless TUI timeout retries | attempts `10`, base `180s` | Hard-coded | `configure_sparse_strict_rag` sets `ChatTimeoutStrategy::Backoff { attempts: Some(10) }` |
| Post-apply proposal status wait | `120s` | Hard-coded | `tui_adapter.rs::wait_for_selected` waits for selected proposals/creations to become terminal |
| Post-apply proposal poll | `100ms` | Hard-coded | Sleep between proposal status checks |
| Post-apply index completion wait | `180s` | Hard-coded | `tui_adapter.rs::wait_for_index_output` waits for indexing output after applying a proposal |
| Post-apply index start grace | `2_000ms` | Hard-coded | If no index output appears and indexing is not required, the adapter continues after this grace period |
| Post-apply index event poll cap | `250ms` | Hard-coded | Per-event timeout while waiting for indexing output |

Relevant files:

- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs`
  Defines the published broad-harness contract defaults:
  `max_attempts = 4`, `turn_seconds = 900`, `tool_seconds = 180`.
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
  Resolves effective broad TUI attempt timeouts and builds
  `tui_adapter::Budget`.
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs`
  Enforces the outer timeout and post-apply waits.
- `crates/ploke-eval/src/runner.rs`
  Configures the sparse strict headless TUI runtime.

## Replay live probes

| Surface | Default | Operator control | Enforcement |
| --- | ---: | --- | --- |
| `run replay turn-live` timeout | `300s` | `--timeout-secs` | Converted to `replay::probe::ProbeBudget`, then to `tui_adapter::Budget` |
| `run replay self-edit-live` timeout | `300s` | `--timeout-secs` | Converted directly to `tui_adapter::Budget` |
| Replay probe default budget | `300s`, `max_attempts = 1` | CLI command defaults can override before construction | `replay/probe.rs::ProbeBudget::default` |

Relevant files:

- `crates/ploke-eval/src/cli.rs`
  `ReplayTurnLiveCommand` and `ReplaySelfEditLiveCommand` define the
  `--timeout-secs` defaults.
- `crates/ploke-eval/src/replay/probe.rs`
  Owns the default replay probe budget and adapter conversion.
- `crates/ploke-eval/src/replay/self_edit.rs`
  Reports headless TUI timeout terminals as `timed_out secs=<n>`.

## Protocol and intervention LLM calls

| Surface | Value | Operator control | Enforcement |
| --- | ---: | --- | --- |
| Tool-call review timeout | `ploke_llm::LLM_TIMEOUT_SECS` (`300s` today) | Not exposed on the command surface | `TOOL_CALL_REVIEW_TIMEOUT_SECS` feeds `protocol_llm_config` |
| Intent segmentation timeout | `120s` | Not exposed on the command surface | Several protocol segmentation paths call `protocol_llm_config(..., 120, ...)` |
| Tool-call segment review timeout | `120s` | Not exposed on the command surface | Segment review first runs segmentation with the same `120s` config |
| Generic intervention synthesis timeout | `120s` | Not exposed on the command surface | `intervention_synthesis` CLI path calls `protocol_llm_config(..., 120, ...)` |
| Protocol HTTP max attempts | `1` | Hard-coded | `PROTOCOL_HTTP_MAX_ATTEMPTS` is passed with all of the above |

Relevant files:

- `crates/ploke-eval/src/cli.rs`
  Defines `TOOL_CALL_REVIEW_TIMEOUT_SECS`, `PROTOCOL_HTTP_MAX_ATTEMPTS`, and
  the protocol/intervention paths that pass `120` or the tool-review constant
  into `protocol_llm_config`.

## Registry, embedding, and provider setup

| Surface | Value | Operator control | Behavior |
| --- | ---: | --- | --- |
| OpenRouter model registry fetch | `30s` | Hard-coded | `model_registry.rs::resolve_model_for_run` applies a per-request reqwest timeout |
| Embedding model registry refresh | `15s` | Hard-coded | `runner.rs::load_eval_embedding_registry` refreshes the embedding registry, then falls back to the cached snapshot on fetch failure |
| Auto embedding model resolution | `15s` | Hard-coded | `runner.rs::resolve_eval_embedding_selection` probes the default embedding model when neither model nor provider is pinned |
| Eval embedding backend request timeout | `30s` | Hard-coded in eval embedding config | `runner.rs::eval_embedding_config` sets `timeout_secs = 30` |
| Eval embedding backoff | initial `250ms`, max `10_000ms`, attempts `3` | Hard-coded in eval embedding config | Applies to the OpenRouter embedding backend created for eval |

Relevant files:

- `crates/ploke-eval/src/model_registry.rs`
- `crates/ploke-eval/src/runner.rs`

## Prototype 1 parent/child handoff waits

| Surface | Value | Operator control | Behavior |
| --- | ---: | --- | --- |
| Child ready wait in `c3` | `10s` | Hard-coded | Polls child/channel every `50ms`; records `ReadyTimedOut` if the child does not send `Ready` |
| Successor ready wait | `10s` | Hard-coded | Polls child/channel every `50ms`; kills and waits the child on timeout; records successor `TimedOut` |
| Child result observation in `c4` | unbounded | None | Polls result files every `100ms`; no timeout was found in this loop |

Relevant files:

- `crates/ploke-eval/src/cli/prototype1_state/c3.rs`
- `crates/ploke-eval/src/cli/prototype1_state/c4.rs`
- `crates/ploke-eval/src/cli/prototype1_process.rs`
- `crates/ploke-eval/src/cli/prototype1_state/successor.rs`

## Monitor and watch polling

These are operator polling intervals, not failure timeouts.

| Surface | Default | Operator control | Behavior |
| --- | ---: | --- | --- |
| `just-watch` poll interval | `1000ms` | `--interval-ms` | CLI command field in `cli.rs`; polling implementation is in `cli_facing.rs` |
| Prototype 1 timing watch interval | `1000ms` | `--interval-ms` | Clamped to at least `1ms`; stops when a terminal loop state is observed |
| Prototype 1 file/location watch interval | `50ms` | `--interval-ms` | Clamped to at least `1ms`; stops when a terminal loop state is observed |
| Quiet snapshot terminal heuristic | `2000ms` | Hard-coded | `stalled_materialization_state` and `exited_parent_state` require no monitor-file changes for 2 seconds |

Relevant files:

- `crates/ploke-eval/src/cli.rs`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`

## Persisted timeout records

These are record surfaces, not timers by themselves.

- `crates/ploke-eval/src/spec.rs`
  `PrepareError::Timeout { phase, secs }` is the common setup/turn timeout
  error shape.
- `crates/ploke-eval/src/record.rs`
  `RuntimeMetadata.wall_clock_timeout_secs` records runtime parameter metadata,
  and `TurnOutcome::Timeout { elapsed_secs }` records timed-out turns.
- `crates/ploke-eval/src/operational_metrics.rs`
  Counts `TurnOutcome::Timeout` together with error outcomes.

## Test-only and canary guardrails

These are useful when a test or ignored canary appears stuck, but they are not
production operator knobs.

| Surface | Value |
| --- | ---: |
| `src/tests/replay.rs` proposal-terminal guard | `10s`, poll `50ms` |
| `src/tests/replay.rs` historical `ToolCallFailed` guard | `2s` |
| `edit_surface/tests.rs` live staging guard | `120s`, plus `30s` post-stage deadline and `50ms` polls |
| `tui_adapter.rs` prompt-event canary loops | `15s` loop deadline, `100ms` event timeout |
| `tui_adapter.rs` ignored prompt construction canaries | `30s` |
| `tui_adapter.rs` ignored BM25 status canary | `5s` |
| `tui_adapter.rs` ignored live canary budget | `600s` |
| `runner.rs` post-terminal drain unit tests | `250ms` grace periods |

If an operator-visible timeout changes, update this file and the local
`docs/knobs/README.md` entry if the scope changes beyond timeouts.
