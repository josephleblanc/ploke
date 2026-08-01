# Walk Trace Show Fails On Tool Argument Newtype

Status: source repaired and regression verified; preserved server restart needed for live revalidation
Discovered: 2026-07-14

## Summary

`ploke-eval loop walk trace list` found the completed Stage 5 baseline run, but
`trace show` could not decode the server response in either table or JSON mode:

```text
failed to serialize manifest: invalid type: newtype struct, expected any valid JSON value
```

The server had loaded the sealed evaluation trace and emitted a valid
4,961,796-byte JSON frame. The failure occurred when the typed client decoded
that frame back into `WalkResponse::EvaluationTrace`.

## Evidence

Campaign and run:

```text
p1-stage5-observe-g35f-orembed-3g1x3-p3-20260714-213222
run-1784090747347-structured-current-policy-0991511b
```

Both the sealed `agent-turn-summary.json` and the compressed `RunRecord`
contain provider tool calls whose argument payload is carried by
`ToolArgumentsJson`. Removing either source alone did not make the response
decodable; removing both did.

One focal persisted call is:

```text
call_id = function-call-4ab08ea8-ecc1-4fe9-81dc-243b586654d1
tool = request_code_context
arguments = {"search_term":"replacement multiline printer pcre2"}
```

## Root Cause

`EvaluationTraceState` is an internally tagged Serde enum. Deserializing such
an enum buffers its variant content before decoding the selected variant.
`ToolArgumentsJson::deserialize` asked that buffered content deserializer for
`Box<serde_json::value::RawValue>`, which is represented to Serde as a newtype.
Serde's content deserializer cannot supply that raw JSON newtype and produced
the observed error.

This was not a trace-loader, source-hash, frame-size, or server serialization
failure.

## Broken Contract

Persisted tool argument carriers must remain decodable through every typed
container used by replay, records, UI snapshots, and private walk IPC. Current
canonical string-form arguments must preserve their exact content. Legacy
object/array/scalar shapes may be compact-normalized for compatibility, but
must not prevent a completed trace from crossing the typed response boundary.

## Fix

`ToolArgumentsJson` now deserializes through ordinary `serde_json::Value`.
String values retain their exact inner text; legacy non-string values retain
the existing compact JSON compatibility behavior. The trace protocol and
`EvaluationTraceState` tagging are unchanged.

Coverage includes:

- exact canonical-string preservation inside an internally tagged carrier;
- legacy object compatibility;
- a completed evaluation trace containing a tool request round-tripped through
  the production framed walk IPC helpers.

## Remaining Verification

Rebuild the client while the preserved Stage 5 server remains available and
rerun the exact `trace show` command. The command must render the completed
trace and retain the focal call id, tool name, and argument text above. A future
fixture import should retain the full sealed Stage 5 artifact for a durable
historical replay in addition to the focused IPC regression.

The focused tagged-carrier tests and completed-trace production IPC regression
pass. A new client could not complete live revalidation against the preserved
old server because trace requests were reset while ordinary status/audit
requests still succeeded. Preserve that observation and repeat after the
recovery-required old server is explicitly retired and restarted from the fixed
binary; do not reinterpret a connection reset as proof of the decoder fix.
