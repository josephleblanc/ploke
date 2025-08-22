# OpenRouter Model Endpoints — Working Notes

Purpose
Track observations about `/api/v1/models/:author/:slug/endpoints`, refine our typed schema, and capture diagnostic artifacts from live tests.

Key Observations
- Pricing fields are strings:
  - The endpoints JSON returns prices as strings (e.g., "0.0000000666396"), not numbers.
  - Our initial `Pricing` struct used `f64` and failed deserialization with: invalid type: string "…", expected f64.
  - Action: store pricing as `String` in our typed model to accept the canonical API shape.
- Tool support signaling:
  - An endpoint advertises tool support when `"supported_parameters"` contains `"tools"`.
  - This is the signal we should use for tools-only enforcement and provider selection UIs.
- Context length types:
  - Most observed values are integers (e.g., 128000). Docs sometimes show floats in examples; we keep `u64`.
- Provider identification:
  - Endpoints list includes a human-readable `name` (e.g., "OpenAI") but routing often needs a provider slug.
  - We build a name→slug map from `/providers`. If no match, we derive a slug-ish fallback by lowercasing and replacing spaces with dashes.

Diagnostic Artifacts
- Test `openrouter_endpoints_live_smoke` now writes the raw JSON body into `logs/openrouter_endpoints_<timestamp>.json`.
- Provider preference experiment and tools smoke tests also persist raw bodies for later inspection.
- For convenience, a stable alias is also written as `logs/<prefix>_latest.json` (e.g., `logs/tools_success_matrix_latest.json`).

Logging Visibility
- Tests capture stdout/stderr by default and print it when a test fails. We now emit logs to stderr via tracing so failure diagnostics appear without --nocapture.
- To see logs even on success: run with `RUST_LOG=info cargo test -p ploke-tui -- --nocapture`.

Hypotheses and Status
- H1: Provider preferences on chat/completions
  - `provider: { "order": ["<slug>"] }` is accepted/respected; `allow` may 400 on chat/completions.
  - Status: We log outcomes for A/B/C payloads; inspect logs to confirm per model/provider.
- H2: Some endpoints support tools while others do not
  - Status: Use `/models/:author/:slug/endpoints` and check `supported_parameters` for "tools".
- H3: require_tool_support should block downgrade
  - Status: Enforce via our registry layer; ensure we never silently drop tools when policy=ON.

Next Steps
- Integrate actual tool schema in live tools tests (avoid fake tools).
- Tighten typed models only when we’re sure fields are stable; otherwise prefer permissive types + adapters.
- Capture headers/latency in spans for better visibility.

Learnings From Live Smoke Tests
- Endpoints schema parsed successfully end-to-end after switching pricing to a flexible deserializer (accept string or number into f64).
- Global tracing initialization across tests caused a SetGlobalDefaultError previously; using try_init resolved duplicate init across tests.
- Provider preference experiment (omitted vs. provider.order vs. provider.allow) did not crash the client; responses were captured for inspection. Exact routing behavior still needs deeper analysis across providers/models.
- Headers and bodies are now persisted to logs/ for offline inspection. This has already helped confirm shape differences and error payloads.
- Timeouts (45s) on reqwest eliminate “silent hangs,” surfacing actionable error states for flaky providers.

What We Still Don’t Know
- Provider routing guarantees per model: when provider.order is honored vs. ignored.
- Tool support variance across endpoints for the same model: which providers actually surface tool_calls reliably.
- Impact of instructions: how system prompt wording influences tool selection frequency.

Planned Work
- Add a matrix-style live test to measure tool_call frequency across combinations of system prompts, user tasks, tool_choice settings, and provider preferences.
- Replace synthetic examples with our real tool schemas once the registry export is wired in (pending).
- Codify minimal acceptance thresholds (locally, not CI) for tool-call rates on commonly used models and refine prompts accordingly.
