Date: 20-05-26

Verified surface: `cargo test -p ploke-llm google` passed 7 focused unit tests; this was a code survey plus official-doc check, not a live Google request, native TUI run, or `ploke-eval` run.

Update 2026-05-20: `Google` now implements `HasModels` through a Google OpenAI-compatible model-list adapter in `ploke-llm`. Verified with `cargo test -p ploke-llm google` and `cargo check -p ploke-tui`; this was not a live Google request.

Verdict: Google is partially integrated in `ploke-llm` as an OpenAI-compatible request/response shape, but it is not yet at OpenRouter parity for the TUI tool loop or eval runner.

**Already Done**
- `ploke-llm` has a `Google` router type with OpenAI-compatible constants and `GEMINI_API_KEY` auth: [google/mod.rs](/home/brasides/code/ploke/crates/ploke-llm/src/router_only/google/mod.rs:159).
- Google request serialization is wired through generic `ChatCompRequest<Google>`, including model-id conversion from `google/gemini-*` to API-facing `gemini-*`: [google/mod.rs](/home/brasides/code/ploke/crates/ploke-llm/src/router_only/google/mod.rs:135), [router_only/mod.rs](/home/brasides/code/ploke/crates/ploke-llm/src/router_only/mod.rs:426).
- Google `extra_body.google.thinking_config` and `cached_content` structs exist and are unit-tested: [google/mod.rs](/home/brasides/code/ploke/crates/ploke-llm/src/router_only/google/mod.rs:14), [google_chat_completion.rs](/home/brasides/code/ploke/crates/ploke-llm/src/request/tests/google_chat_completion.rs:15).
- The shared parser can accept Google’s OpenAI-compatible success response shape: [shape_tests.rs](/home/brasides/code/ploke/crates/ploke-llm/src/response/shape_tests.rs:6).
- The generic HTTP sender `chat_step<R: Router>` can technically send Google requests because it uses `R::COMPLETION_URL` and `R::resolve_api_key`: [session.rs](/home/brasides/code/ploke/crates/ploke-llm/src/manager/session.rs:281).
- `Google` implements `HasModels` with its own OpenAI-compatible model-list response types and an adapter into the existing shared model registry item shape: [google/mod.rs](/home/brasides/code/ploke/crates/ploke-llm/src/router_only/google/mod.rs:21).

**Still Missing For Parity**
- `Google` does not implement `HasEndpoint`; OpenRouter endpoint metadata drives provider/tool support, but Google has no equivalent implementation: [openrouter/mod.rs](/home/brasides/code/ploke/crates/ploke-llm/src/router_only/openrouter/mod.rs:76).
- `Google` does not implement `RouterCalibration`, so it cannot be used by the TUI session loop as written. `run_chat_session` requires `R: Router + RouterCalibration`: [session.rs](/home/brasides/code/ploke/crates/ploke-tui/src/llm/manager/session.rs:779). Only OpenRouter has a calibration impl: [calibration.rs](/home/brasides/code/ploke/crates/ploke-llm/src/registry/calibration.rs:174).
- `ploke-tui` hardcodes OpenRouter when building live chat requests: [manager/mod.rs](/home/brasides/code/ploke/crates/ploke-tui/src/llm/manager/mod.rs:584), [manager/mod.rs](/home/brasides/code/ploke/crates/ploke-tui/src/llm/manager/mod.rs:616).
- TUI model search, provider listing, API-key checks, and provider pinning are OpenRouter-specific: [exec.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app/commands/exec.rs:517), [exec.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app/commands/exec.rs:560).
- `ploke-eval` is OpenRouter-specific for model registry, tool-capable provider resolution, and headless runtime setup: [model_registry.rs](/home/brasides/code/ploke/crates/ploke-eval/src/model_registry.rs:61), [runner.rs](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:2000), [runner.rs](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:2244).
- Tool-call request shape is likely compatible, but not proven live. Official Gemini OpenAI compatibility docs show `tools` plus `tool_choice="auto"` are supported, and the same docs show the `/v1beta/openai/chat/completions` and `/v1beta/openai/models` endpoints with bearer auth. See Google docs: https://ai.google.dev/gemini-api/docs/openai.

**Work To Reach Parity**
1. Done: add Google model-list response types and an adapter from Google `/openai/models` into the project’s model registry shape.
2. Done: add `impl HasModels for Google`.
3. Decide what replaces OpenRouter endpoint/provider metadata for Google. Direct Google likely has no provider endpoint selection, so model/tool capability should be modeled as direct-provider capability, not fake OpenRouter endpoints.
4. Add `impl RouterCalibration for Google`.
5. Generalize `RuntimeConfig` and TUI request construction so active router is selected explicitly, not inferred as OpenRouter.
6. Add Google-aware API-key diagnostics and model commands.
7. Add live ignored tests for Google tool calls through `chat_step`, then through `ploke-tui` recorded/live tool loop, then through `ploke-eval` headless adapter.

The right next implementation slice is small: add `RouterCalibration for Google`, a direct Google model-list adapter, and a Google `chat_step` live ignored tool-call test. That proves the provider boundary before touching TUI/eval routing.
