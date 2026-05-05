# Prototype 1 Persistence Map: Tracing, Timing, Logs

Worker: 5  
Date: 2026-05-03  
Scope: live Prototype 1 loop plus child eval paths. This file maps persisted tracing/timing/log evidence only; transition journals and scheduler state are referenced when they provide join keys.

## Causal Position

Tracing and timing evidence is mostly diagnostic. The authoritative transition surface remains the Prototype 1 journal, node records, runner invocation/result records, and run records. The timing monitor intentionally projects over several evidence families:

- observation JSONL under `~/.ploke-eval/logs`
- child runtime stdout/stderr streams under campaign node directories
- child eval `record.json.gz`, `agent-turn-trace.json`, and `llm-full-responses.jsonl`
- scheduler/node records for node/branch lineage

The current join story is usable but uneven: node, branch, runtime, and assistant-message IDs are present in most durable records, while provider HTTP request IDs are local process counters and only become node/branch-joinable when captured inside Prototype 1 tracing spans.

## Items

### Prototype 1 Observation JSONL

- Path pattern: `~/.ploke-eval/logs/prototype1_observation_<run_id>.jsonl` by default, or an explicit path from `PLOKE_PROTOTYPE1_TRACE_JSONL`.
- Enablement: `PLOKE_PROTOTYPE1_TRACE_JSONL=1|true|auto|default` writes the default file; any other nonempty value is used as the exact path; unset/empty disables it.
- Schema/event shape: tracing-subscriber JSON with flattened event fields, `span`, `spans`, target/level/file/line/current span. Prototype 1 observation steps require `duration_ms`; provider HTTP events require `request_id` and `event`.
- Writer: `crates/ploke-eval/src/tracing_setup.rs:73` creates the optional writer; `:129`-`:143` configures JSON output for `ploke_exec` and `chat_http`; `:206`-`:220` maps the env var to the path.
- Step producers: `crates/ploke-eval/src/cli/prototype1_state/observe.rs:27`-`:89` emits `duration_ms` step completion/failure events; `:92`-`:176` wraps command and IO steps; `:178`-`:229` wraps sync/async result steps.
- Reader/CLI: `ploke-eval loop prototype1-monitor timing --campaign <campaign> [--format json] [--node <node-id>] [--depth node|phase|turn|call] [--show-paths]`. Reader source is `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3495`-`:3557`.
- Key IDs for joins: `campaign_id`/`campaign`, `node_id`, `branch_id`, `generation`, span name, `duration_ms`; for chat events also `request_id`, `attempt`, `max_attempts`.
- Classification: diagnostic observation log/projection evidence. It is not transition authority.
- Safe bounded inspection:

```bash
rg -n '"campaign_id":"<campaign>"|"campaign":"<campaign>"' ~/.ploke-eval/logs/prototype1_observation_*.jsonl | head -n 20
jq -c 'select((.campaign_id? // .span.campaign_id? // (.spans[-1].campaign_id?)) == "<campaign>") | {ts:.timestamp,target:.target,event:.event,node_id:(.node_id? // .span.node_id? // .spans[-1].node_id?),branch_id:(.branch_id? // .span.branch_id? // .spans[-1].branch_id?),duration_ms}' ~/.ploke-eval/logs/prototype1_observation_*.jsonl | head -n 20
```

### `chat_http` Provider Attempt Traces

- Path pattern: primarily same observation JSONL when `PLOKE_PROTOTYPE1_TRACE_JSONL` is enabled; also formatted into normal eval logs. If `PLOKE_PROTOCOL_DEBUG` is truthy, compact JSON lines are also written to process stderr, which for child runtimes lands in `prototype1/nodes/<node-id>/streams/<runtime-id>/stderr.log`.
- Schema/event shape: `chat_http_request_start`, `chat_http_response_headers`, `chat_http_response_body`, `chat_http_response_error_status`, `chat_http_retry_scheduled`, `chat_http_request_completed`, `chat_http_request_error`, `chat_http_retry_suppressed`. Fields include `request_id`, `attempt`, `max_attempts`, `url`, `model`, `timeout_secs`, `status`, `elapsed_ms`, `backoff_ms`, `retry_after_ms`, `request_bytes`, `response_bytes`, `phase`, `failure`, `receive_phase`, `body_failure`, `is_timeout`, `raw_error`.
- Writer: `crates/ploke-llm/src/manager/session.rs:88`-`:320` wraps chat HTTP attempts; event writers are at `:342`-`:620`. `PLOKE_PROTOCOL_DEBUG` stderr emission is controlled at `:71`-`:85`.
- Reader/CLI: `prototype1-monitor timing` parses structured observation JSONL into `ProviderHttpEvent` at `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3560`-`:3588`, then groups attempts by `(source log path, request_id)` around `:3124`-`:3158`. Stderr fallback counters are parsed at `:3338`-`:3419`.
- Key IDs for joins: `request_id` is only process-local; `attempt` joins attempt events within a request. Reliable campaign/node/branch joins require tracing span fields (`campaign_id`, `node_id`, `branch_id`, `generation`) from the surrounding Prototype 1 span.
- Classification: diagnostic provider transport evidence. Useful for retry/timeout/backoff proof; not authoritative loop state.
- Safe bounded inspection:

```bash
jq -c 'select((.target? == "chat_http") and ((.campaign_id? // .span.campaign_id? // (.spans[-1].campaign_id?)) == "<campaign>")) | {ts:.timestamp,event,request_id,attempt,max_attempts,status,phase,elapsed_ms,backoff_ms,node_id:(.node_id? // .span.node_id? // .spans[-1].node_id?),branch_id:(.branch_id? // .span.branch_id? // .spans[-1].branch_id?)}' ~/.ploke-eval/logs/prototype1_observation_*.jsonl | head -n 40
rg -n 'chat_http_(request_error|retry_scheduled|retry_suppressed|request_completed)' ~/.ploke-eval/campaigns/<campaign>/prototype1/nodes/<node-id>/streams/<runtime-id>/stderr.log | head -n 20
```

### Timing Spans From `TimingTrace`

- Path pattern: child runtime stderr stream, `~/.ploke-eval/campaigns/<campaign>/prototype1/nodes/<node-id>/streams/<runtime-id>/stderr.log`.
- Schema/event shape: plain stderr lines, not JSON. Shape is `<HH:MM:SS> <label>.start` and `<HH:MM:SS> <label>.end +<seconds>s`.
- Writer: `crates/ploke-eval/src/cli.rs:1176`-`:1213` defines `TimingTrace`; Prototype 1 uses it for loop and branch/eval phases in `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:704`, `:731`, `:748`, `:758`, `:788`, `:796`, `:834`, `:962`, and in child branch evaluation at `crates/ploke-eval/src/cli/prototype1_process.rs:1797`.
- Reader/CLI: `prototype1-monitor timing` parses branch-eval stderr with regexes at `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3338`-`:3419`.
- Key IDs for joins: `runtime_id` from the stream directory, `node_id` from path, and branch ID embedded in labels such as `loop.prototype1_branch.evaluate.<branch-id>`.
- Classification: diagnostic stderr projection/cache. It is bounded and useful, but not structured authority.
- Safe bounded inspection:

```bash
rg -n 'loop\.prototype1(_branch)?\..*\.(start|end)' ~/.ploke-eval/campaigns/<campaign>/prototype1/nodes/<node-id>/streams/<runtime-id>/stderr.log | head -n 40
```

### Monitor Timing Projection

- Path pattern read set: campaign `prototype1/scheduler.json`, node records under `prototype1/nodes/<node-id>/node.json`, stream logs under `prototype1/nodes/<node-id>/streams/<runtime-id>/stderr.log`, observation logs under `~/.ploke-eval/logs/prototype1_observation_*.jsonl`, run artifacts under `~/.ploke-eval/instances/prototype1/<campaign>/treatments/<branch-id>/**`.
- Persisted schema: no new persistence; CLI projection schema is `prototype1-monitor-timing.v2`.
- Reader/CLI: command definition at `crates/ploke-eval/src/cli.rs:449`-`:565`; loader at `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:2019`-`:2115`.
- Projection shape: per-node `NodeEvidence` with `runs`, `streams`, `traces`, `observation_steps`, `provider_http` (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1917`-`:2017`).
- Key IDs for joins: `campaign_id`, `node_id`, `branch_id`, `runtime_id`, `record_path`, `assistant_message_id`, provider `request_id`.
- Classification: projection only. It should not be used as source authority.
- Safe bounded inspection:

```bash
ploke-eval loop prototype1-monitor --campaign <campaign> --repo-root <active-parent-worktree> timing --format json --node <node-id> | jq '{schema_version,campaign_id,nodes:[.nodes[] | {node_id,branch_id,status,streams:.streams|length,traces:.traces|length,provider_http:.provider_http|length}]}'
```

### Child Runtime Streams

- Path pattern: `~/.ploke-eval/campaigns/<campaign>/prototype1/nodes/<node-id>/streams/<runtime-id>/{stdout.log,stderr.log}`.
- Schema/event shape: raw stdout/stderr files. `stderr.log` can contain TimingTrace lines, formatted tracing console output, `PLOKE_PROTOCOL_DEBUG` chat HTTP JSON lines, and warnings/errors.
- Writer: child spawn redirects stdout/stderr at `crates/ploke-eval/src/cli/prototype1_state/c3.rs:565`-`:572`; stream paths are `:123`-`:131` and open/create code is `:135`-`:153`. Successor spawn uses equivalent stream paths at `crates/ploke-eval/src/cli/prototype1_process.rs:1024`-`:1057`.
- Reader/CLI: `prototype1-monitor timing` loads `stderr.log` at `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3306`-`:3335`; `prototype1-monitor peek` can show bounded excerpts (`:3699`-`:3727`, `:4329`-`:4350`).
- Key IDs for joins: `node_id` and `runtime_id` from path; some lines also contain `branch_id`, provider `request_id`, or span context.
- Classification: diagnostic log. Good for operator forensics and current timing fallback, weak as durable schema.
- Safe bounded inspection:

```bash
tail -n 80 ~/.ploke-eval/campaigns/<campaign>/prototype1/nodes/<node-id>/streams/<runtime-id>/stderr.log
rg -n 'elapsed_ms=|retry_delay_secs=|chat_http_|loop\.prototype1' ~/.ploke-eval/campaigns/<campaign>/prototype1/nodes/<node-id>/streams/<runtime-id>/stderr.log | head -n 40
```

### Agent Turn Trace

- Path pattern: `~/.ploke-eval/instances/prototype1/<campaign>/treatments/<branch-id>/**/agent-turn-trace.json`.
- Schema/event shape: `AgentTurnArtifact` with `events: Vec<ObservedTurnEvent>`. Relevant variants include `LlmResponse`, `ToolRequested`, `ToolCompleted`, `ToolFailed`, `MessageUpdated`, and `TurnFinished`; tool completion/failure records carry `latency_ms`.
- Writer: path is allocated at `crates/ploke-eval/src/runner.rs:2098`; `AgentTurnArtifact` and event types are defined at `:730`-`:796` and `:829`-`:834`; events are populated at `:3462`-`:3623`; the trace file is written at `:3380` and opportunistically during post-terminal drain at `:3442`-`:3444`.
- Reader/CLI: `prototype1-monitor timing` discovers these files at `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3422`-`:3439` and parses event counts/tool latencies at `:3441`-`:3487`.
- Key IDs for joins: treatment path contains `campaign_id` and `branch_id`; event records include `request_id`, `parent_id`, `call_id`, `assistant_message_id` in terminal records, and model/usage in `LlmResponse`.
- Classification: run/eval evidence. More structured than stderr; still a child run artifact, not parent transition authority.
- Safe bounded inspection:

```bash
jq '{event_counts: (.events | map(keys[0]) | group_by(.) | map({kind:.[0], count:length})), terminal_record}' ~/.ploke-eval/instances/prototype1/<campaign>/treatments/<branch-id>/**/agent-turn-trace.json
```

### Run Record Timing And Usage

- Path pattern: `~/.ploke-eval/instances/prototype1/<campaign>/treatments/<branch-id>/**/record.json.gz`.
- Schema/event shape: `RunRecord` (`run-record.v1`) with `timing: Option<RunTimingSummary>`, `phases.agent_turns`, `db_time_travel_index`, conversation, token usage in structured LLM records. `RunTimingSummary` fields are `started_at`, `ended_at`, `total_wall_clock_secs`, `setup_wall_clock_secs`, `agent_wall_clock_secs`.
- Writer: `RunTimingSummary` type at `crates/ploke-eval/src/record.rs:1467`-`:1486`; `finalize_run_timing` populates it at `crates/ploke-eval/src/runner.rs:586`-`:600`; agent run finalization calls it at `:2491`-`:2497`; compressed record is written at `:2526`-`:2534`.
- Reader/CLI: general inspect commands read compressed records; monitor timing loads run artifacts at `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3183`-`:3234` and uses timing around `:2905`-`:2924`.
- Key IDs for joins: `manifest_id`/task ID, run arm, model/provider metadata, run path, branch path, turn numbers, assistant message IDs through turn artifacts.
- Classification: authoritative run/eval evidence for that child attempt; parent loop authority still comes from invocation/result/journal records.
- Safe bounded inspection:

```bash
gzip -dc ~/.ploke-eval/instances/prototype1/<campaign>/treatments/<branch-id>/**/record.json.gz | jq '{schema_version,manifest_id,timing,turns:(.phases.agent_turns|length),run_arm:.metadata.run_arm}'
```

### Full-Response Trace And Usage Sidecar

- Path patterns:
  - global source log: `~/.ploke-eval/logs/llm_full_response_<run_id>.log`
  - run-local sidecar: `~/.ploke-eval/instances/prototype1/<campaign>/treatments/<branch-id>/**/llm-full-responses.jsonl`
- Schema/event shape: one JSON object per raw full response: `RawFullResponseRecord { assistant_message_id, response_index, response: OpenAiResponse }`. Usage is in `response.usage` when provider supplies it.
- Writer: full-response tracing target is `llm-full-response`; eval tracing writes the global source log at `crates/ploke-eval/src/tracing_setup.rs:67`-`:72` and filters the target at `:85`-`:95`. TUI session emits records at `crates/ploke-tui/src/llm/manager/session.rs:1126`-`:1135`. Agent runs slice the global log into the run-local sidecar using offsets at `crates/ploke-eval/src/runner.rs:2100`-`:2104`, persist it at `:2381`-`:2400`, and append/write at `:3739`-`:3804` via `append_jsonl_blob` at `:3711`-`:3726`.
- Reader/CLI: `ploke-eval inspect turn <n> --show responses [--format json]` loads by `assistant_message_id` at `crates/ploke-eval/src/cli.rs:6880`-`:6925` and `:9861`-`:9913`; usage aggregation fallback is at `:9939`-`:9974`. Monitor timing also loads sidecars at `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3210`-`:3255`.
- Key IDs for joins: `assistant_message_id` + `response_index`; can join to `TurnFinishedRecord.assistant_message_id` in `agent-turn-trace.json` and `RunRecord` turn artifacts. Provider response IDs and token usage are inside `response`.
- Classification: structured diagnostic/run evidence for provider envelopes and usage. It is not complete authority for cost; source comments note the sidecar may undercount if a final stop response is missed.
- Safe bounded inspection:

```bash
jq -c '{assistant_message_id,response_index,response_id:.response.id,finish:(.response.choices[0].finish_reason? // null),usage:.response.usage}' ~/.ploke-eval/instances/prototype1/<campaign>/treatments/<branch-id>/**/llm-full-responses.jsonl | head -n 20
```

### General Eval Logs

- Path pattern: `~/.ploke-eval/logs/ploke_eval_<run_id>.log`.
- Schema/event shape: formatted tracing log with target/level/file/line, no timestamp. It receives broad eval tracing controlled by `RUST_LOG`/default filter.
- Writer: `crates/ploke-eval/src/tracing_setup.rs:37`-`:83` initializes the main file layer. `--debug-tools` adds the execution debug target at `:40`-`:46`; console debug filtering is at `:105`-`:113`.
- Reader/CLI: no dedicated structured reader found for Prototype 1 timing joins. Operators use `rg`, `tail`, or `prototype1-monitor peek`.
- Key IDs for joins: only whatever fields appear in formatted lines; weak for machine joins.
- Classification: diagnostic log.
- Safe bounded inspection:

```bash
rg -n '<campaign>|<node-id>|chat_http_|prototype1 step' ~/.ploke-eval/logs/ploke_eval_*.log | head -n 80
```

## Environment And Feature Switches

- `PLOKE_PROTOTYPE1_TRACE_JSONL`: turns on structured Prototype 1 observation JSONL; default path under `~/.ploke-eval/logs`.
- `PLOKE_PROTOCOL_DEBUG`: truthy value causes `chat_http` compact JSON events to be emitted to stderr in addition to tracing.
- `--debug-tools`: enables `ploke_exec=debug` in the eval tracing filter and debug console visibility.
- `RUST_LOG`: honored by the eval tracing `EnvFilter`; default is `info,embed-pipeline=trace`.
- Cargo feature `demo`: suppresses `TimingTrace` stderr start/end lines and changes some console filtering paths.

## Current CLI Readers

- `ploke-eval loop prototype1-monitor ... list`: prints expected output locations and volatility.
- `ploke-eval loop prototype1-monitor ... report --format table|json`: read-only campaign report.
- `ploke-eval loop prototype1-monitor ... timing --format table|json [--node <id>] [--depth ...] [--show-paths]`: joins scheduler/node/run/stream/observation/full-response evidence into timing projection.
- `ploke-eval loop prototype1-monitor ... peek --lines <n> --bytes <n>`: bounded excerpts from expected files.
- `ploke-eval loop prototype1-monitor ... watch --print-initial --interval-ms <n>`: file-change monitor plus journal summaries.
- `ploke-eval loop prototype1-monitor ... history-metrics` and `history-preview`: read current persisted evidence as projections.
- `ploke-eval history metrics|preview`: similar History-shaped read-only projections.
- `ploke-eval inspect turn <n> --show responses --format table|json`: reads `llm-full-responses.jsonl` by `assistant_message_id`.

## Join Assessment

- Strong joins: `campaign_id`, `node_id`, `branch_id`, `runtime_id`, `assistant_message_id`, `call_id`, `record_path`.
- Medium joins: provider `request_id` + `attempt`, when preserved with tracing span fields in observation JSONL.
- Weak joins: stderr TimingTrace labels and formatted eval log lines; these rely on path context or regex parsing.
- Best current timing join surface: `prototype1-monitor timing --format json`, because it keeps the raw evidence paths while projecting per-node spans.

## Gaps / Unknowns

- Provider HTTP attempt evidence has no durable typed attempt record outside tracing/logs. `request_id` is a process-local counter, not globally unique.
- `PLOKE_PROTOCOL_DEBUG` stderr JSON is useful but duplicates tracing and is not read as structured JSON by the monitor; current stderr reader mostly counts regex patterns.
- TimingTrace is plain stderr text. It should be replaced or supplemented by a typed timing record with `campaign_id`, `node_id`, `branch_id`, `runtime_id`, phase name, start/end/duration, and outcome.
- Full-response sidecar usage is structured but known to be a stopgap; usage totals can undercount if final responses are missed.
- General eval logs are not suitable for joins beyond bounded human diagnostics.
- Monitor timing is a projection, not persistence. It should not become the authority for timings unless its component evidence is normalized into durable records.
