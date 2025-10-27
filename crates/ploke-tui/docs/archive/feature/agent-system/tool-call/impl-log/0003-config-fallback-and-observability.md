# Impl Log 0003 — Configurable 404 tool fallback and Observability enrichment

Date: 2025-08-24

Summary
- Added a configuration-driven policy for 404 “tool unsupported” fallbacks. When provider_registry.require_tool_support is true, the session will not retry without tools and surfaces guidance immediately; when false (default), it retries once without tools.
- Enriched observability with model and provider_slug fields for tool call lifecycle. The DB schema, request records, and queries now persist and expose these fields.

Changes
- ploke-tui
  - llm/session.rs: RequestSession now accepts fallback_on_404 flag; 404 handling respects this policy.
  - llm/mod.rs: Wire fallback_on_404 = !require_tools from ProviderRegistry into RequestSession::new.
  - observability.rs: Persist ToolCallReq with model and provider_slug from active provider.
- ploke-db
  - observability.rs:
    - Schema: :create tool_call now includes model: String? and provider_slug: String?.
    - ToolCallReq struct carries model and provider_slug.
    - record_tool_call_requested inserts model/provider_slug; record_tool_call_done carries them forward.
    - get_tool_call and list_tool_calls_by_parent select and return model/provider_slug.

Rationale
- Avoid silent behavioral shifts when users enforce tool-capable models/providers.
- Persisting model and provider_slug enables richer analytics, debugging, and cost/perf correlation.

Notes and compatibility
- New columns are created on fresh DBs; existing persisted DBs would need a migration (not included here). Test environments use in-memory DBs, so schema is applied fresh.
- ToolEvent remains unchanged; observability derives provider info from active config at event time.

Next steps
- Add a UI toggle/command to flip provider_registry.require_tool_support at runtime and persist it.
- Extend observability to categorize error kinds more granularly.
- Consider adding a version field to typed tool results for forward compatibility.

Files touched
- crates/ploke-tui/src/llm/session.rs
- crates/ploke-tui/src/llm/mod.rs
- crates/ploke-tui/src/observability.rs
- crates/ploke-db/src/observability.rs
