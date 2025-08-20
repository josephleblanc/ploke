# M1 Readiness Assessment for ploke-db
Date: 2025-08-20

Source of truth
- Spec: crates/ploke-tui/docs/feature/agent-system/ploke_db_requests.md (M1 — Editing proposals)

Scope of this report
- Assess the readiness of ploke-db’s M1 surfaces (editing proposals) against the spec.
- Identify gaps, risks, and recommended follow-ups.

Summary
- Status: Partially Ready (feature-complete API and schema present; lifecycle/idempotency/validation need tightening)
- The core relation and APIs exist and generally align with the spec. Some safeguards and tests should be added to reach “production-ready” for M1.

Spec vs Implementation

1) Relation: code_edit_proposal (time-travel)
- Spec: code_edit_proposal(request_id: Uuid, at: Validity => diffs_json: Json, confidence: Float?, status: String, decided_at_ms: Int?, applied_at_ms: Int?, commit_hash: String?)
- Impl: crates/ploke-db/src/observability.rs → ensure_observability_schema()
  - Matches spec fields and adds results_json: Json? (extra field).
  - Uses time-travel (Validity) and :put with at = 'ASSERT' to maintain consistent transaction timestamps for decisions.

Readiness:
- Ready with caveats:
  - Schema contains results_json (not in spec). This is useful but should be reconciled with the spec (documented in agent_observability_impl_log.md).
  - diffs_json is declared as non-null Json in schema. Current code sets DataValue::Null if JSON parsing fails (record_edit_proposed), which may violate schema expectations at runtime. Input should be validated before write, or the schema should allow Json?.

2) API: record_edit_proposed(req_id, diffs_json, confidence?)
- Impl present: record_edit_proposed(...)
- Behavior: Asserts a new “proposed” state with ASSERT timestamp; does not check for duplicates or existing proposals.

Readiness:
- Ready with caveats:
  - No idempotency: multiple identical proposals will accumulate states.
  - Validation: if diffs_json is invalid JSON, it is stored as Null (see caveat above).

3) API: record_edit_decision(req_id, status: "approved" | "denied")
- Impl present: record_edit_decision(...)
- Behavior: Sets status via ASSERT and stamps decided_at_ms = to_int(at). Does not validate prior state or prevent changing decisions later.

Readiness:
- Needs work:
  - Lifecycle guards are absent. For example, “approved” → “denied” later is currently allowed but should be rejected per acceptance guidance.
  - Idempotency not enforced (repeating the same decision should be a no-op).

4) API: record_edit_applied(req_id, results_json, applied_at_ms, commit_hash?)
- Impl present: record_edit_applied(...)
- Behavior: Unconditionally transitions to status="applied", accepts applied_at_ms from caller, captures results_json and commit_hash.

Readiness:
- Needs work:
  - Lifecycle guard missing: currently allows applied from any prior status (including “denied”). Spec implies allowed from “approved” only.
  - Idempotency not enforced (repeating same apply should be a no-op).
  - Validation: results_json is parsed; invalid JSON is stored as Null.

5) API: get_edit_proposal(req_id)
- Impl present: get_edit_proposal(...)
- Behavior: Reads latest snapshot @ 'NOW', returns JSON fields as strings via dump_json, and validity metadata (created_at).

Readiness:
- Ready:
  - Matches spec intent to retrieve current proposal state.
  - Returns ergonomic stringified JSON.

6) Privacy/Redaction policy
- Spec: Allow redaction (store hashes) when disabled.
- Impl: Accepts pre-redacted JSON; does not enforce redaction or toggle.

Readiness:
- Needs work:
  - No crate-level redaction policy or toggle. Caller must handle redaction before storing.

Behavioral alignment highlights
- Timestamps:
  - Decisions use ASSERT timestamp to derive decided_at_ms → matches spec.
  - Applied timestamp is taken from caller → matches spec.
- Status values:
  - Supports “proposed”, “approved”, “denied”, “applied”.
- JSON handling:
  - Stored as Cozo Json; fetch uses dump_json to return strings → ergonomic.

Testing status
- Tool call lifecycle tests exist and pass.
- No acceptance tests included for M1 edit proposals (proposed → approved → applied; invalid transitions; idempotency).

Readiness matrix
- Relation availability: Ready with caveats (results_json extension; diffs_json nullability/validation).
- API surface presence: Ready.
- Lifecycle enforcement: Needs work.
- Idempotency: Needs work.
- Privacy/redaction: Needs work.
- Tests: Needs work.

Risks and recommendations
1) Lifecycle enforcement
- Enforce allowed transitions:
  - proposed → approved | denied
  - approved → applied
- Reject:
  - approved → denied (and vice versa)
  - denied → applied
  - applied → any
- Implement checks in record_edit_decision and record_edit_applied by reading current status @ 'NOW' and returning InvalidLifecycle on invalid transitions.

2) Idempotency semantics
- record_edit_proposed: If latest state is already “proposed” with identical diffs_json and confidence, return Ok(()) without asserting a new state.
- record_edit_decision: No-op if the same decision has already been recorded (same status).
- record_edit_applied: No-op if identical results_json and applied_at_ms have already been recorded (or at least if status is already “applied”).

3) Validation and schema alignment
- Ensure diffs_json is valid JSON before writing; if not, return DbError::QueryConstruction or similar, or make schema Json? and allow nulls explicitly.
- Decide whether results_json should remain within code_edit_proposal or move to a separate relation; if kept, update the spec doc.

4) Privacy/redaction
- Add a crate-level toggle or runtime flag to enforce redaction policy (e.g., store only hashes for diffs/results when enabled).
- Document expected caller behavior if policy is left to ploke-io.

5) Tests to add (acceptance)
- proposed → approved → applied, verifying timestamps and outputs.
- approved → denied should be rejected.
- denied → applied should be rejected.
- Double-apply with identical payload is a no-op.
- JSON roundtrip for diffs_json and results_json (validity and dump_json behavior).
- Redaction scenario: verify storing of redacted payloads (if toggle implemented).

Conclusion
- M1 is close: the essential schema and API endpoints are in place and functional.
- To reach “production-ready,” add lifecycle guards, idempotency, validation for JSON, and targeted acceptance tests.
- Spec alignment: either update the spec to include results_json or move apply results to another relation.

Artifacts referenced
- Implementation: crates/ploke-db/src/observability.rs
- Public API exports: crates/ploke-db/src/lib.rs (re-exports ObservabilityStore, CodeEditProposal, etc.)
- Implementation log: crates/ploke-db/docs/agent_observability_impl_log.md
