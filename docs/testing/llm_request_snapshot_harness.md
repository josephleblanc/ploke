# LLM Request Snapshot Harness (Minimal Files)

Purpose
- Provide a minimal, self-contained approach to snapshot the JSON payloads we send to providers.
- Avoids new dev-dependencies (e.g., insta) and network calls by snapshotting request bodies.

What’s covered
- Regular chat requests (no tools).
- Tool-call capable requests (tools[], tool_choice="auto", provider routing).

Minimal files involved
- crates/ploke-tui/src/llm/session.rs
  - Adds a small helper: build_openai_request(...) that constructs the OpenAI-style request.
  - Unit tests serialize the request via serde_json::to_string_pretty and compare to a checked-in string.
- crates/ploke-tui/src/llm/mod.rs
  - Defines the data structures used in the request (OpenAiRequest, RequestMessage, ToolDefinition, LLMParameters).
- crates/ploke-tui/src/user_config.rs
  - ProviderConfig definition and OPENROUTER_URL constant (used to construct a ProviderConfig in tests).

How to add more snapshot tests
1) In crates/ploke-tui/src/llm/session.rs under #[cfg(test)] mod tests, create a new #[test] function.
2) Build a ProviderConfig for the model/provider you want to exercise (e.g., "openai/gpt-4o" or "anthropic/claude-3.5-sonnet").
3) Prepare LLMParameters with stable values (temperature, max_tokens) to keep snapshots deterministic.
4) Build messages: include an optional system prompt and your user/assistant messages.
5) For tool-call tests: build a ToolDefinition with a small JSON schema and pass Some(vec![tool]) and use_tools=true.
6) Call build_openai_request(...) and serialize with serde_json::to_string_pretty(&payload).
7) Compare the resulting string to a literal "snapshot" you paste into the test (kept in-repo).

Why not insta?
- The workspace base Cargo.toml is read-only here; adding dev-deps would require broader changes.
- This harness keeps tests deterministic and dependency-light.

Limitations
- Snapshots live inline as string literals (not in separate .snap files).
- This harness validates the request payload only; it does not hit the network.

Next steps (optional)
- If you later allow adding dev-deps, migrate to insta and move snapshot strings into dedicated .snap files.
- Extract additional builder helpers (e.g., for provider preferences) as needed to improve reuse.
