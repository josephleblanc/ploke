# Prototype 1 Timing Projection Handoff

Recorded: 2026-05-02 16:44 UTC

Status: restart handoff after orienting around a better Prototype 1 timing
projection. No implementation changes were made in this pass.

## Current Goal

Build an operator-facing timing projection that joins node phase timings with
turn, tool, provider, eval, and History/Crown timing evidence.

The practical question is:

```text
Why does one selected Prototype 1 generation take about 800-1000s?
```

The projection must remain a read-only view over persisted evidence. It must
not turn scheduler JSON, runner JSON, trace files, or CLI output into sealed
History authority.

## Causal Frame

```text
Surface Request:
  join node phase timings with turn/tool/provider timing

Causal Chain:
  Parent/child/successor runtime
  -> persisted observation steps, invocations, streams, turn traces,
     run records, evaluations, sealed History

Concern:
  identify where generation wall time is going without weakening
  Runtime/Artifact/History boundaries

Evidence Surface:
  typed persisted records and summarized trace/run metadata

Existing Algebra:
  Prototype 1 scheduler/journal/history preview/metrics/timing projections,
  RunRecord/agent-turn traces, History block records

Missing Structure:
  stable join key across node/runtime/turn/provider and normalized provider
  attempt timing

Transformation:
  load, join, fold, and project timing evidence

Projection:
  CLI timing table/JSON view, possibly with a TUI-like turn summary later

Preservation Check:
  projection must not turn mutable JSON or trace artifacts into History
  authority
```

## Observed Campaign

Command run:

```bash
./target/debug/ploke-eval loop prototype1-monitor \
  --campaign p1-5gen-observed-20260502-7 \
  --repo-root /home/brasides/.ploke-eval/worktrees/p1-5gen-observed-20260502-7 \
  timing
```

Important output:

- Campaign: `p1-5gen-observed-20260502-7`
- Scheduler updated: `2026-05-02T15:06:27Z`
- Final selected gen5 node: `node-399b70ad955f22f4`
- Final selected branch: `branch-223ba2074cec9c93`
- Gen5 child runtime: `b6cbf7c2-d702-4ca8-a004-206b2f4bd49a`
- Gen5 child duration: about `843.959s`
- Gen5 child observation wait: about `843.970s`
- Gen5 run summary from `inspect conversations`:
  - wall time: `747.449s`
  - turns: `1`
  - tool calls: `18`
  - failed tool calls: `5`
  - token usage from raw response sidecar: `prompt:329398 completion:27905 total:357303`
  - total tool latency from current trace summary: under `1s`

Interpretation:

The selected gen5 child wait is dominated by the chat/provider/model/prompt
loop, not local tool execution. Build/check costs are stable and visible
around `68s` total per selected child, but they are not the main opaque cost.

## Current Timing Surfaces

Existing `prototype1-monitor timing` already reads or summarizes:

- `scheduler.json`
  Node list, generation, parent node, branch id, status, created/updated times.
- `nodes/<node>/node.json`
  Scheduler-owned node mirror.
- `nodes/<node>/runner-result.json`
  Latest runner result projection, including recorded time and disposition.
- `nodes/<node>/streams/<runtime>/stderr.log`
  Parsed for stderr-derived branch eval timing, HTTP body timeouts, chat retry
  backoff, and LLM error count.
- `~/.ploke-eval/logs/prototype1_observation_*.jsonl`
  Observation spans such as child build/check, child wait, History seal/append,
  successor spawn, ready wait, and startup predecessor validation.
- `~/.ploke-eval/instances/prototype1/<campaign>/.../agent-turn-trace.json`
  Event counts and tool latency totals/max.

Useful data exists but is not joined into `prototype1-monitor timing`:

- `record.json.gz`
  `RunRecord.timing`, turn start/end, tool calls, tool latencies, turn outcomes.
- `llm-full-responses.jsonl`
  Response count, token usage, finish reasons, provider/model response payloads.
- Branch evaluation reports
  Carry or lead to treatment/baseline run record paths.
- Sealed History files under `prototype1/history/*`
  Authority-bearing records. Current timing should only expose History timing
  spans unless explicitly building a History inspection view.

## Source/Binary Drift

Important: the checked source and the current binary disagree.

The checked source in
`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs` shows a
`prototype1-monitor-timing.v1` implementation that renders `streams`.

The current `./target/debug/ploke-eval` binary renders
`prototype1-monitor-timing.v2` and includes `attempts` with child/successor
runtime ids, invocation paths, ready/observed times, result paths, and evidence
labels.

Observed facts:

- `git status --short` was clean.
- `git diff -- crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs` was
  empty.
- `stat` showed `target/debug/ploke-eval` older than `cli_facing.rs`.
- `strings ./target/debug/ploke-eval` found `prototype1-monitor-timing.v2`.

Implication:

Before rebuilding, recover or recreate the v2 attempt-join behavior in source.
Otherwise a rebuild will likely regress the monitor back to the committed v1
shape.

## Code Pointers

Command and projection wiring:

- `crates/ploke-eval/src/cli.rs:438`
  `Prototype1MonitorCommand` and subcommands.
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1492`
  `Prototype1MonitorCommand::run` dispatch.
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1913`
  Current checked-source timing structs/load/render. This is v1 in source.

Timing producers:

- `crates/ploke-eval/src/cli/prototype1_state/observe.rs:1`
  Observation telemetry helper.
- `crates/ploke-eval/src/cli/prototype1_state/c2.rs:390`
  Child build/check/promote timing producers.
- `crates/ploke-eval/src/cli/prototype1_state/c4.rs:268`
  Child observe/result/evaluation timing producers.
- `crates/ploke-eval/src/cli/prototype1_process.rs:1080`
  History/Crown/successor timing producers.
- `crates/ploke-eval/src/cli/prototype1_state/parent.rs:440`
  Parent startup timing producers.

Run/turn/tool/provider data:

- `crates/ploke-eval/src/record.rs:149`
  `RunRecord`, including coarse `timing`.
- `crates/ploke-eval/src/record.rs:807`
  `TurnRecord`, including start/end, tool calls, and agent turn artifact.
- `crates/ploke-eval/src/record.rs:1227`
  `RawFullResponseRecord`.
- `crates/ploke-eval/src/record.rs:1240`
  `ToolExecutionRecord`.
- `crates/ploke-eval/src/runner.rs:586`
  `finalize_run_timing`.
- `crates/ploke-eval/src/runner.rs:3448`
  Capture of LLM events, tool requests/completions/failures, and turn finish
  events into agent turn artifacts.
- `crates/ploke-llm/src/types/meta.rs:9`
  `LLMMetadata`, including processing time, cost, and performance metrics.

Existing inspection commands:

- `ploke-eval loop prototype1-monitor timing`
  Node-oriented timing projection.
- `ploke-eval history metrics`
  Read-only metric projection.
- `ploke-eval history preview`
  History-shaped preview over current evidence, not sealed authority.
- `ploke-eval conversations --record <record.json.gz>`
  Run summary with turns, token usage, and wall time.
- `ploke-eval inspect operational --record <record.json.gz>`
  Compact operational metrics.
- `ploke-eval inspect tool-calls --record <record.json.gz>`
  Tool call list/details. JSON can be very large.
- `ploke-eval inspect turn --record <record.json.gz> 1`
  Turn-level inspection.

## Missing Shape

Data exists but is not joined/shown:

- RunRecord wall time for each node attempt.
- Turn start/end and duration.
- Tool call counts and failure counts from `record.json.gz`.
- Tool latency from `record.json.gz`, not only trace summaries.
- Full-response sidecar response count, token totals, finish reasons, and model.
- A dedicated History/Crown timing section grouped from observation spans.

Data not persisted cleanly yet:

- Normalized per-provider request latency/attempt timing with stable causal ids.
- Explicit response/turn/provider join ids across `RunRecord`,
  `agent-turn-trace.json`, `llm-full-responses.jsonl`, and node/runtime records.
- Durable chat-loop phase timing finer than stderr-derived retries/timeouts and
  response-sidecar timestamps.
- A path-independent node/runtime/turn/provider join record.

## Recommended Next Slice

Do not start by adding new instrumentation. First make the existing evidence
visible.

1. Restore or recreate the `prototype1-monitor-timing.v2` attempt model in
   source.
2. Add a run-artifact join to the timing projection:
   - from node/branch/evaluation evidence to treatment `record.json.gz`;
   - from run dir to `llm-full-responses.jsonl`;
   - preserve paths as evidence refs, not authority.
3. Add a compact per-attempt `agent_run` section:

```text
agent_run:
  record: <path>
  wall_time: 747.449s
  turns: 1
  responses: 19
  tokens: prompt=329398 completion=27905 total=357303
  tools: total=18 failed=5 latency_total=821ms max=131ms
  finish_reasons: tool_calls=18 stop=1
  unexplained_or_provider_time: wall_time - tool_latency - known local timing
```

4. Keep JSON output first-class and derive table output from the same struct.
5. Only after this projection is useful, add missing durable provider timing
   records with stable join keys.

## Safe Verification Commands

Bounded timing summary:

```bash
./target/debug/ploke-eval loop prototype1-monitor \
  --campaign p1-5gen-observed-20260502-7 \
  --repo-root /home/brasides/.ploke-eval/worktrees/p1-5gen-observed-20260502-7 \
  timing --format json \
| jq '{schema_version,campaign_id,nodes:(.nodes|map({node_id,generation,branch_id,status,attempts:(.attempts // [] | length),traces:(.traces|length),observation_steps:(.observation_steps|length)}))}'
```

Gen5 attempt timing:

```bash
./target/debug/ploke-eval loop prototype1-monitor \
  --campaign p1-5gen-observed-20260502-7 \
  --repo-root /home/brasides/.ploke-eval/worktrees/p1-5gen-observed-20260502-7 \
  timing --node node-399b70ad955f22f4 --format json \
| jq '.nodes[0] | {node_id,generation,branch_id,status,attempts,observation_steps,traces}'
```

RunRecord timing summary for gen5 selected branch:

```bash
gzip -cd /home/brasides/.ploke-eval/instances/prototype1/p1-5gen-observed-20260502-7/treatments/branch-223ba2074cec9c93/instances/BurntSushi__ripgrep-2209/runs/run-1777733544527-structured-current-policy-3f6b970d/record.json.gz \
| jq '{timing, turns:(.phases.agent_turns|map({turn_number,started_at,ended_at,tool_count:(.tool_calls|length),tool_latency_ms:([.tool_calls[].latency_ms]|add // 0)}))}'
```

Full response sidecar summary:

```bash
jq -s '{responses:length, usage_total:{prompt:(map(.response.usage.prompt_tokens // 0)|add), completion:(map(.response.usage.completion_tokens // 0)|add), total:(map(.response.usage.total_tokens // 0)|add)}, finishes:(map(.response.choices[0].finish_reason // "unknown")|group_by(.)|map({finish:.[0],count:length}))}' \
  /home/brasides/.ploke-eval/instances/prototype1/p1-5gen-observed-20260502-7/treatments/branch-223ba2074cec9c93/instances/BurntSushi__ripgrep-2209/runs/run-1777733544527-structured-current-policy-3f6b970d/llm-full-responses.jsonl
```

## Cautions For The Next Agent

- Do not run `prototype1-state` from Codex. That is live runtime execution.
- Do not dump raw run records, full traces, prompts, responses, patches, or
  large tool payloads into context.
- Do not parse human-rendered CLI output if a typed record or JSON output is
  available.
- Do not treat scheduler, node, runner-result, trace, or monitor output as
  sealed History authority.
- Keep role/state structure explicit. Avoid adding flattened helper names when
  a small carrier or projection type would carry the relation cleanly.
