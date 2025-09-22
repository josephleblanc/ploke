# Implementation log 009 — M0 hardening: typed persist params and serde JSON conversions (2025-08-19)

Summary
- Introduced typed parameter bundles for observability DB writes:
  - ToolRequestPersistParams and ToolDonePersistParams in observability.rs.
- Replaced ad-hoc "stringy" JSON helpers with serde_json serialization at the boundary.
  - Removed canonical_json/json_string; use serde_json::to_string for Value/String conversions.
- Kept lightweight FNV-1a 64-bit hashing for args_sha256 as a temporary stand-in; to be replaced with true SHA-256 later.

Changes
- crates/ploke-tui/src/observability.rs
  - New structs ToolRequestPersistParams and ToolDonePersistParams.
  - LlmTool and SystemEvent tool-call handlers now assemble typed params and call persist_* with those.
  - persist_tool_requested/persist_tool_done now accept the typed params and perform serde_json conversion only where the ploke-db API still requires String.
  - Removed manual JSON helpers; added a note to prefer type-safe patterns and push string conversions to the edge.
- crates/ploke-tui/docs/feature/agent-system/decisions_required.md
  - Added item 11) about ploke-db ObservabilityStore accepting typed JSON values to avoid String round-trips.

Rationale
- Moves toward type-safe programming by constraining conversions to a single boundary.
- Keeps ploke-db API stable for M0 while preparing for more robust types in M1.

Notes and next steps
- Replace FNV placeholder with SHA-256 once dependency changes are staged.
- Track latency_ms by correlating Requested → Done timestamps (requires start-time lookup).
- Consider ObservabilityStore methods that accept serde_json::Value directly (see decisions_required item 11).
- Longer-term: central newtypes for Validity timestamps and Json payloads to reduce accidental misuse.

Acceptance alignment
- Continues M0 plan: SSoT event flow intact; persistence code more robust and testable.
- No external API changes; internal refactor only.
