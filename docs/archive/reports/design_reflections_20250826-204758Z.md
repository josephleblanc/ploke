Design Reflections — 2025-08-26 20:47:58Z

Stylistic Preferences Observed
- Strong modular separation: tui, rag, db, io, error crates with clear boundaries.
- Event-driven architecture in tui with AppEvent priority split (Realtime/Background).
- Tool-calling wired via a central llm manager and dispatcher; typed tool payloads.
- Favoring explicit logs/tracing and diagnostic artifacts for LLM sessions (good for triage).

Strengths
- Edit safety: atomic io with hash verification; layered staging/approval flow; previews.
- RAG orchestration: hybrid capability with strict/lenient bm25; token budgeting; graceful fallbacks.
- Model/provider registry with capability cache and enforcement knob; model browser overlay.
- Observability groundwork in db: conversation turns, tool lifecycle with latency/idempotency.

Anti‑Patterns / Risks
- Overuse of sysinfo messages for state vs structured events; consider separating channel concerns or typed UI notifications.
- Heuristic tests (token trimming) brittle across environments; promote invariant‑driven tests.
- Scattered OpenRouter conversions; must consolidate around strong typed interfaces and spec docs.
- Long, complex tests without env gating can hang CI; need consistent patterns (added now).

Neutral/Preference Patterns
- Free functions vs traits in several modules; scale okay, but trait boundaries might help for UI panes (scrolling) and git helper injection.
- Mixed sync/async boundaries (e.g., db helpers and async rag): acceptable, but be mindful of blocking.

Recommendations
- Adopt a ScrollableOverlay trait and extract UI components (Model Browser, Approvals, Context Items) to standardize interactions and tests.
- Introduce a ploke-git helper with minimal API over gix; enable DI for unit testing.
- Centralize OpenRouter SDK types; strongly typed f64/u32 fields; serde derives; add a micro validation layer.
- Add a “session trace” index JSON with links for per-request results; drive the trace overlay from it.
- Tighten error taxonomies across crates; unify UX-facing messages.
- Continue pushing tool handlers toward typed arguments and robust error returns; keep sysinfo messages as a secondary signal.

SysInfo Verbosity and Lifecycle
- Add tiered SysInfo verbosity with a simple On/Off toggle first; later introduce levels and auto‑aging (messages that disappear as the next stage begins: “Embedding user message” → “Sending to LLM” → “Waiting for response” → “Response received”). Track as TECH_DEBT and wire into plan.

OpenRouter Type Consolidation
- Create a report and strategy to consolidate scattered OpenRouter types into a single module with strong typing and serde derives; remove ad‑hoc conversions.
