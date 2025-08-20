# Implementation Log 009 — Observability Store: fix evaluation errors and improve idempotency semantics

Date: 2025-08-20

Summary
- Addressed Cozo "Evaluation of expression failed" errors observed in tests during:
  - Second call to record_tool_call_requested (expected idempotent no-op).
  - First call to record_tool_call_done.
- Changes focus on avoiding brittle in-query dependencies and ignoring volatile timestamps for idempotency checks.

Hypotheses for failures
- Idempotency check for requested used full struct equality, including started_at Validity timestamp. Since Cozo assigns a transaction-stable "ASSERT" timestamp, the stored started_at differs from the caller-provided one, causing unnecessary re-assertions.
- The done script carried forward metadata via an in-query read: `*tool_call{ ... @ 'NOW' }`. Within the same transaction, this can be fragile because:
  - 'NOW' equals the timestamp used by 'ASSERT', and evaluation ordering might surface multiple facts or lead to ambiguity.
  - Failing subexpressions may result in the generic "Evaluation of expression failed" error.

What changed
- record_tool_call_requested:
  - Idempotency comparison now ignores started_at; we compare only the semantic fields (ids, vendor, tool_name, args_sha256, arguments_json).
- record_tool_call_done:
  - Removed the in-script read of `*tool_call @ 'NOW'`.
  - We now look up the existing request first (via get_tool_call) and pass its metadata explicitly as parameters into the insert script.
  - This avoids timing/order pitfalls inside a single script and keeps lifecycle logic clearly in Rust.
- No changes to schema or external API signatures.

Open questions and requests for context
- Please confirm Database::run_script parameter semantics. If you have its signature/impl (crates/ploke-db/src/database.rs), add it to the chat so we can instrument better diagnostics or switch to raw_query_mut for certain cases.
- If Cozo provides more specific error messages beyond "Evaluation of expression failed", can we enable verbose logs/tracing? Any guidance on the engine's eval errors would help.
- Are there constraints on using 'ASSERT' multiple times for the same (request_id, call_id) at the same timestamp within one transaction? We currently rely on separate calls/transactions from the application layer.

Next steps (pending confirmation from test output)
- If errors persist, introduce structured tracing around failing scripts, echoing the script and key params for faster diagnosis.
- Progress toward stronger type-safety (serde_json::Value for Json fields; ConversationKind enum) once behavior stabilizes.

Note
- This log does not assert that tests pass yet. Please run `cargo test -p ploke-db` and share output so we can iterate.
