# trace-01 — broad harness and agent-turn summary after R7→R8

## Question

Can the DB summarize each broad-harness attempt and its post-attempt agent/tool trace evidence?

## Query

- Query file: `../queries/trace-01__harness-agent-summary.cozo`
- Raw output: `trace-01__harness-agent-summary__step-09.json`
- Step: `09 / after R7->R8`

## Output summary

```text
row_count = 3
requests = node-ec383aa38762a3d4, node-ec383aa38762a3d4-r2, node-ec383aa38762a3d4-r3
all graph_nearest = 24
all child_min/child_max = 1/1
event_count = 34, 48, 51
attempts = 3, 5, 9
normalized tool_event count at this checkpoint = 133
```

## Answer

Partial/Yes. The DB has one harness request, one diagnostic, and one agent-turn row per broad-harness attempt, plus normalized tool-event rows. It is enough to summarize attempt counts, terminal metadata, event counts, model/provider route, and artifact refs.

## Helpfulness

Useful for broad attempt inventory and tool-loop health. It showed three attempts ran in parallel and all produced post-attempt agent-turn bundles.

## Schema/query design notes

Helpful: request, diagnostic, agent-turn, and tool-event relations make attempt-level DB review possible without opening every trace file.

Gaps: `eval_model_exchange` count remained 0 even though provider HTTP activity is visible in logs and agent-turn files exist. If model-exchange normalization is expected for this path, this is a schema/write-path gap to investigate. Full provider responses also appear as zero-byte `llm-full-responses.jsonl` files for these attempts, while trace/summary JSON contains substantial event data.
