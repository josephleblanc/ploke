# Runtime Reliability, Telemetry, And Demo Readiness

## Status

Prototype 1 has enough persisted evidence to run a small bounded HyperAgents-like demo if the claim is modest: a parent runtime generates/evaluates a small child fanout, records child outcomes, projects timing/provider evidence, and stops under explicit generation/node limits. It is not yet strong enough to claim robust long-horizon self-improvement, because key runtime failures can still be indistinguishable from model or candidate quality failures.

The strongest current observability path is diagnostic, not authoritative. `PLOKE_PROTOTYPE1_TRACE_JSONL` enables structured observation JSONL under `~/.ploke-eval/logs`, with `ploke_exec`, `chat_http`, and `chat-loop` events written as JSON when configured in `crates/ploke-eval/src/tracing_setup.rs:73` and `crates/ploke-eval/src/tracing_setup.rs:123`. The monitor then joins scheduler/node state, run artifacts, stream logs, observation steps, and provider HTTP events into `prototype1-monitor-timing.v2` in `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:2380` and `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:2445`.

The authority boundary matters for the demo narrative. The module docs explicitly warn that mutable scheduler/report projections are not History authority, and that runtime self-report cannot become promotion without verification in `crates/ploke-eval/src/cli/prototype1_state/mod.rs:18` and `crates/ploke-eval/src/cli/prototype1_state/mod.rs:62`. A demo can say "we can observe and explain this bounded run"; it should not yet say "the loop is generally trustworthy over long horizons."

## Existing Pieces

Run and agent evidence:

- `RunRecord` persists run metadata, phases, conversation, coarse timing, and turn records in `record.json.gz`; the schema includes `timing` at `crates/ploke-eval/src/record.rs:149` and `crates/ploke-eval/src/record.rs:168`.
- Agent metadata records selected model, selected provider, and selected endpoint provenance in `crates/ploke-eval/src/record.rs:636`. Runtime metadata can carry max turns/tool calls/wall-clock timeout fields in `crates/ploke-eval/src/record.rs:666`, but the Prototype 1 timeout policy is not yet unified around that carrier.
- Turn records carry start/end, request/response, tool calls, outcome, and optional agent-turn artifact in `crates/ploke-eval/src/record.rs:807`.
- The runner allocates `agent-turn-trace.json`, `agent-turn-summary.json`, `llm-full-responses.jsonl`, and `record.json.gz` for agent runs in `crates/ploke-eval/src/runner.rs:2098`, records model/provider/endpoint provenance in `crates/ploke-eval/src/runner.rs:2108`, slices full-response traces in `crates/ploke-eval/src/runner.rs:2381`, finalizes timing in `crates/ploke-eval/src/runner.rs:2491`, and writes the compressed run record in `crates/ploke-eval/src/runner.rs:2526`.
- Agent-turn artifacts capture LLM responses, tool requested/completed/failed events with latency, and turn-finished records in `crates/ploke-eval/src/runner.rs:3462`.

Provider telemetry:

- `ploke-llm` has a structured `ProviderAttempt` record with request id, attempt/max attempts, phase timings, status, response bytes, outcome, failure phase, body failure, retry decision, and backoff in `crates/ploke-llm/src/manager/builders/attempt.rs:53`.
- Each finished provider attempt emits a `provider_attempt` tracing event with the serialized attempt payload in `crates/ploke-llm/src/manager/builders/attempt.rs:298`.
- The HTTP layer emits request start, headers, body, status error, retry scheduled, completion, request error, and retry suppressed events with request/attempt/status/timing fields in `crates/ploke-llm/src/manager/session.rs:484`.
- Prototype 1 installs runtime span context for parent/child chat requests, including role, phase, campaign, node, branch, generation, and runtime id in `crates/ploke-eval/src/cli/prototype1_state/telemetry.rs:55` and `crates/ploke-eval/src/cli/prototype1_state/telemetry.rs:70`.
- The monitor parses observation JSONL into `ProviderHttpEvent`, including span-attributed campaign/node/branch/generation/runtime fields and embedded `ProviderAttempt` payloads in `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:2047` and `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3946`.

Runtime/journal pieces:

- Search bounds exist: default `max_generations=1`, `max_total_nodes=32`, `min_children=2`, `max_children=6`, `require_keep_for_continuation=true`, and `explore_from_rejected=true` are exposed in `crates/ploke-eval/src/cli.rs:900` and mirrored in the scheduler default in `crates/ploke-eval/src/intervention/scheduler.rs:15`.
- The transition journal is append-only JSONL at `prototype1/transition-journal.jsonl`, with entries for parent start, resources, materialize/build/spawn/child/ready/observe/successor in `crates/ploke-eval/src/cli/prototype1_state/journal.rs:332`. Appends use `OpenOptions::append`, one serialized JSON line, and `sync_data` in `crates/ploke-eval/src/cli/prototype1_state/journal.rs:655`.
- Child lifecycle records distinguish built/spawned/acknowledged/terminated and observed terminal success/failure in `crates/ploke-eval/src/cli/prototype1_state/event.rs:56`.
- Child ready is bounded by `READY_TIMEOUT = 10s`; the parent checks the typed channel, the journal, and `try_wait` before failing the node on exit or timeout in `crates/ploke-eval/src/cli/prototype1_state/c3.rs:63` and `crates/ploke-eval/src/cli/prototype1_state/c3.rs:764`.
- Attempt-scoped channels exist as a role-indexed file transport with typed messages, runtime id, message id, body hash, and direction validation in `crates/ploke-eval/src/cli/prototype1_state/channel.rs:257`. Child-to-parent messages include `Ready`, `Evaluating`, `ResultWritten`, `Failed`, and `Exited` in `crates/ploke-eval/src/cli/prototype1_state/channel.rs:373`.

## Gaps

Timeout and retry semantics are split across layers and partly contradictory:

- `ploke-llm` implements retryable send/body/status handling with backoff in `crates/ploke-llm/src/manager/session.rs:143`, `crates/ploke-llm/src/manager/session.rs:203`, `crates/ploke-llm/src/manager/session.rs:295`, and `crates/ploke-llm/src/manager/session.rs:385`; default retry tuning includes send/body timeout/read retry and common retryable statuses in `crates/ploke-llm/src/registry/calibration.rs:91`.
- The current TUI agent session path resolves provider timing, then forces `attempt_timeout` to the outer HTTP timeout and `max_attempts = 1` before calling `chat_step_with_attempts` in `crates/ploke-tui/src/llm/manager/session.rs:708`. A provider-exhausted attempt then prevents the outer chat-step retry path in `crates/ploke-tui/src/llm/manager/session.rs:780`. The test `provider_retry_exhaustion_does_not_outer_retry_chat_step` asserts one request and one exhausted attempt in `crates/ploke-tui/src/llm/manager/session.rs:2351`.
- Older docs still describe nested `3 HTTP attempts * 120s` behavior in `docs/workflow/evalnomicon/drafts/runtime/child.md:406`, but current code defaults are `LLM_TIMEOUT_SECS = 300` and `ChatHttpConfig::default().max_attempts = 1` in `crates/ploke-llm/src/lib.rs:50` and `crates/ploke-llm/src/manager/session.rs:60`. This is an implemented/documented mismatch that can mislead demo planning.
- Parent-side child observation after `Child<Ready>` has no visible deadline. `ObserveChild::transition` loops until a result path appears, sleeping every 100ms in `crates/ploke-eval/src/cli/prototype1_state/c4.rs:359` and `crates/ploke-eval/src/cli/prototype1_state/c4.rs:493`. The runtime draft records the same gap: no post-ready observation deadline and no child `try_wait` check were visible in the result wait loop in `docs/workflow/evalnomicon/drafts/runtime/child.md:254`.

Fanout/concurrency risks remain:

- `run_child_fanout` runs planned children in `spawn_blocking` tasks and joins them with `JoinSet` in `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5442`. It uses `child_budget.min` as the concurrent fanout width, not `max`; `max` truncates the candidate set earlier in `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5933`.
- Each planned child gets its own `PrototypeJournal` handle to the same transition journal in `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5260`. Appends are synchronized to disk, but there is no repository-level or file-lock protocol proving multi-writer JSONL append order under concurrent children.
- Scheduler and node state are mutable JSON files written by load/modify/write helpers, not atomic transaction records. `save_scheduler_state` writes the whole scheduler with `fs::write` in `crates/ploke-eval/src/intervention/scheduler.rs:413`; concurrent `update_node_status` calls can race if multiple children finish together in `crates/ploke-eval/src/intervention/scheduler.rs:556`.
- The parent/child-channel design draft identifies the current fanout race surfaces directly: shared `transition-journal.jsonl`, `scheduler.json`, `branches.json`, and node mirrors in `docs/workflow/evalnomicon/drafts/runtime/parent-child-channel.md:125`.
- The file channel transport validates schema/direction/campaign/node/runtime/body hash on read, but append does not call `sync_data` and has no sequence/ack policy beyond cursor offsets in `crates/ploke-eval/src/cli/prototype1_state/channel.rs:692` and `crates/ploke-eval/src/cli/prototype1_state/channel.rs:720`.

Observability is useful but not yet a durable causal record:

- Provider attempt evidence is persisted mainly through tracing JSONL/logs, not as a typed child attempt artifact or History-admissible event. The persistence map calls this out explicitly in `docs/workflow/evalnomicon/drafts/prototype1-persistence-map-2026-05-03/05-tracing-timing-logs.md:179`.
- `request_id` is process-local; it is only reliably joinable to campaign/node/branch when trace span fields are present. The current parser still has a temporary bridge that can misattribute unscoped provider attempts when one log file contains multiple campaigns in `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3927`.
- Full-response sidecars preserve provider envelopes and usage in `RawFullResponseRecord` at `crates/ploke-eval/src/record.rs:1222`, but the persistence map still treats them as a stopgap that can undercount final responses in `docs/workflow/evalnomicon/drafts/prototype1-persistence-map-2026-05-03/05-tracing-timing-logs.md:123`.

Demo versus long-horizon blockers:

- Small bounded demo blockers: enable structured observation JSONL by default for the demo command; make provider attempts visible in the monitor; add a child-result deadline or explicit "still evaluating" heartbeat; avoid concurrent fanout above `1` until scheduler/journal writes have a lock/transaction story; print the exact timeout/retry budget in setup/monitor output.
- Long-horizon blockers: typed durable attempt/failure records across provider, agent session, child result, and parent observation; coherent timeout policy across parent wait, child eval, TUI retry, low-level HTTP, tool timeout, and campaign budget; idempotent recovery/replay after crash; locked or transactionally appended shared state; History admission that imports telemetry as evidence without making logs authority.

## Recommended Next Slice

The smallest reliability/observability slice before claiming meaningful self-improvement is a bounded child-attempt health record, not archive-aware selection yet.

Implement one durable per-runtime attempt summary written under the node attempt, for example `nodes/<node-id>/results/<runtime-id>.health.json`, and have the parent fold it into `ObserveChild`/monitor output. It should include:

- identity: `campaign_id`, `node_id`, `branch_id`, `generation`, `runtime_id`, parent node id, run record path, runner result path;
- bounds: configured `child_ready_timeout`, `child_result_timeout`, `eval_run_timeout`, `llm_http_attempt_timeout`, `llm_http_max_attempts`, `chat_step_retry_limit`, `tool_call_timeout`, `tool_chain_limit`;
- provider summary: request count, attempt count, timeout count, retry decisions, exhausted attempts, total provider wall time, max attempt time, status/body-failure histogram;
- agent summary: turn count, tool count, failed tool count, token usage if complete, final turn outcome, final error id/class;
- terminal classification: `succeeded`, `failed`, `timed_out`, `child_exited_without_result`, `observation_timeout`, `provider_exhausted`, or `incomplete`;
- evidence refs: transition journal offsets or entry ids if available, observation JSONL path(s), run record path, full-response sidecar path, stream paths.

Then wire one explicit post-ready child result deadline into `ObserveChild::transition`. On deadline or child exit, write a terminal failed runner result and a journal `ObserveChild` after-entry instead of waiting forever. This should be paired with a demo command/profile that sets `max_generations=1..3`, `min_children=1`, `max_children=1`, fixed provider/model, `PLOKE_PROTOTYPE1_TRACE_JSONL=auto`, and an explicit total campaign wall-clock budget.

After that slice, the demo claim can be: "Prototype 1 can run a bounded archive-loop step and distinguish candidate failure, provider/runtime failure, and observation timeout well enough to interpret local self-improvement evidence." Without that slice, the honest claim is weaker: "Prototype 1 can run and project useful telemetry, but runtime uncertainty can still contaminate self-improvement interpretation."
