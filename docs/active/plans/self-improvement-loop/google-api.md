Date: 20-05-26

Verified surface: `cargo test -p ploke-llm google` passed 11 focused unit tests; `cargo test -p ploke-llm --features live_api_tests --no-run live_google_chat_step_forced_tool_call_success_or_quota` compiled the ignored live Google tool-call test; `cargo check -p ploke-tui` and `cargo check -p ploke-eval` passed. This was not a live Google request or native TUI run.

Update 2026-05-20: `Google` now implements `HasModels` through a Google OpenAI-compatible model-list adapter in `ploke-llm`. Verified with `cargo test -p ploke-llm google` and `cargo check -p ploke-tui`; this was not a live Google request.

Update 2026-05-20: `Google` now implements `RouterCalibration` with a direct model-key calibration id, and `ploke-llm` has an ignored live `chat_step` forced-tool-call test for Google. The direct Google provider path should not fake OpenRouter endpoint/provider metadata; until TUI/eval routing is generalized, Google tool capability is carried by the Google model-list adapter. Verified by the surface above.

Verdict: Google is partially integrated in `ploke-llm` as an OpenAI-compatible request/response shape, but it is not yet at OpenRouter parity for the TUI tool loop or eval runner.

**Already Done**
- `ploke-llm` has a `Google` router type with OpenAI-compatible constants and `GEMINI_API_KEY` auth: [google/mod.rs](/home/brasides/code/ploke/crates/ploke-llm/src/router_only/google/mod.rs:159).
- Google request serialization is wired through generic `ChatCompRequest<Google>`, including model-id conversion from `google/gemini-*` to API-facing `gemini-*`: [google/mod.rs](/home/brasides/code/ploke/crates/ploke-llm/src/router_only/google/mod.rs:135), [router_only/mod.rs](/home/brasides/code/ploke/crates/ploke-llm/src/router_only/mod.rs:426).
- Google `extra_body.google.thinking_config` and `cached_content` structs exist and are unit-tested: [google/mod.rs](/home/brasides/code/ploke/crates/ploke-llm/src/router_only/google/mod.rs:14), [google_chat_completion.rs](/home/brasides/code/ploke/crates/ploke-llm/src/request/tests/google_chat_completion.rs:15).
- The shared parser can accept Google’s OpenAI-compatible success response shape: [shape_tests.rs](/home/brasides/code/ploke/crates/ploke-llm/src/response/shape_tests.rs:6).
- The generic HTTP sender `chat_step<R: Router>` can technically send Google requests because it uses `R::COMPLETION_URL` and `R::resolve_api_key`: [session.rs](/home/brasides/code/ploke/crates/ploke-llm/src/manager/session.rs:281).
- `Google` implements `HasModels` with its own OpenAI-compatible model-list response types and an adapter into the existing shared model registry item shape: [google/mod.rs](/home/brasides/code/ploke/crates/ploke-llm/src/router_only/google/mod.rs:21).
- `Google` implements `RouterCalibration` without provider preferences or endpoint routing, using the direct model key as the calibration key.
- `ploke-llm` has an ignored live Google `chat_step` forced-tool-call test. It requires `--features live_api_tests -- --ignored` plus `GEMINI_API_KEY`.

**Still Missing For Parity**
- `Google` still does not implement `HasEndpoint`, intentionally. OpenRouter endpoint metadata drives provider/tool support, but direct Google has no provider endpoint selection. TUI/eval code that requires endpoint metadata must be generalized rather than fed fake Google endpoints.
- `ploke-tui` hardcodes OpenRouter when building live chat requests: [manager/mod.rs](/home/brasides/code/ploke/crates/ploke-tui/src/llm/manager/mod.rs:584), [manager/mod.rs](/home/brasides/code/ploke/crates/ploke-tui/src/llm/manager/mod.rs:616).
- TUI model search, provider listing, API-key checks, and provider pinning are OpenRouter-specific: [exec.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app/commands/exec.rs:517), [exec.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app/commands/exec.rs:560).
- `ploke-eval` is OpenRouter-specific for model registry, tool-capable provider resolution, and headless runtime setup: [model_registry.rs](/home/brasides/code/ploke/crates/ploke-eval/src/model_registry.rs:61), [runner.rs](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:2000), [runner.rs](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:2244).
- Tool-call request shape is likely compatible, but not proven live. Official Gemini OpenAI compatibility docs show `tools` plus `tool_choice="auto"` are supported, and the same docs show the `/v1beta/openai/chat/completions` and `/v1beta/openai/models` endpoints with bearer auth. See Google docs: https://ai.google.dev/gemini-api/docs/openai.

**Work To Reach Parity**
1. Done: add Google model-list response types and an adapter from Google `/openai/models` into the project’s model registry shape.
2. Done: add `impl HasModels for Google`.
3. Done: decide what replaces OpenRouter endpoint/provider metadata for Google. Direct Google has no provider endpoint selection in this integration; model/tool capability is modeled as direct-provider model capability, not fake OpenRouter endpoints.
4. Done: add `impl RouterCalibration for Google`.
5. Generalize `RuntimeConfig` and TUI request construction so active router is selected explicitly, not inferred as OpenRouter.
6. Add Google-aware API-key diagnostics and model commands.
7. Partially done: add live ignored tests for Google tool calls through `chat_step`, then through `ploke-tui` recorded/live tool loop, then through `ploke-eval` headless adapter.

The next implementation slice is TUI routing: generalize `RuntimeConfig` and live request construction so the active router can be selected explicitly instead of assuming OpenRouter.
