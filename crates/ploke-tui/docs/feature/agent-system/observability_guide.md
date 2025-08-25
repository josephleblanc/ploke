# Observability Guide — Auditing conversations and tool calls

Overview
- This guide shows how to follow a tool call end-to-end and inspect conversation history using Cozo queries.
- Timestamps use Validity; treat to_int(at) as epoch millis and to_bool(at) for assert flag.
- Logs are written to logs/ploke.log (daily rotation).

List recent conversation turns
```
?[id, kind, content, at_ms] :=
    *conversation_turn{ id, at, kind, content @ 'NOW' },
    at_ms = to_int(at)
    :sort -at_ms
    :limit 50
```

Fetch a tool call by (request_id, call_id)
```
?[request_id, call_id, parent_id, vendor, tool_name, args_sha256, arguments_json_s, status, ended_at_ms, latency_ms, outcome_json_s, error_kind, error_msg, at_ms, at_valid] :=
    *tool_call{
        request_id, call_id, at, parent_id, vendor, tool_name, args_sha256, arguments_json, status, ended_at_ms, latency_ms, outcome_json, error_kind, error_msg
        @ 'NOW'
    },
    request_id = $request_id,
    call_id = $call_id,
    arguments_json_s = if(is_null(arguments_json), null, dump_json(arguments_json)),
    outcome_json_s = if(is_null(outcome_json), null, dump_json(outcome_json)),
    at_ms = to_int(at),
    at_valid = to_bool(at)
```

List recent tool calls (most recent first)
```
?[request_id, call_id, vendor, tool_name, status, at_ms] :=
    *tool_call{ request_id, call_id, at, vendor, tool_name, status @ 'NOW' },
    at_ms = to_int(at)
    :sort -at_ms
    :limit 100
```

Interpretation notes
- Lifecycle is modeled with time-travel: later asserted facts supersede earlier ones when querying '@ NOW'.
- Terminal statuses are immutable: once completed/failed, attempts to change terminal status are rejected.
- arguments_json/outcome_json are stored as cozo Json when enabled; use dump_json() to render strings.

Troubleshooting
- If you observe channel lag warnings, they will be rate-limited to avoid log spam.
- Ensure you subscribe to broadcast channels before sending events in tests to avoid race conditions; use EventBusStarted where provided.

Security posture
- In M0, JSON payloads are stored to speed development; plan to add redaction toggles before prod-ready.
