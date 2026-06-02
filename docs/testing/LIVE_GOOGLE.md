# Live Google Testing

Direct Google live tests use Vertex AI's OpenAI-compatible chat completions
route with Google application default credentials (ADC). They do not use
`GOOGLE_API_KEY` for the supported direct-Google route.

## Required Setup

Authenticate ADC once on the machine:

```bash
gcloud auth application-default login
```

For a service account, set `GOOGLE_APPLICATION_CREDENTIALS` to the JSON
credential file before running the tests. Do not commit the credential file or
print its contents in logs.

Set the route and live-test gate in the same shell that will run `cargo test`:

```bash
export GOOGLE_PROJECT_ID=cs-poc-gtxw7jmtfuwfsiauziui9yx
export GOOGLE_REGION=global
export PLOKE_RUN_LIVE_TESTS=1
export PLOKE_LIVE_GOOGLE_CHAT_MODEL=google/gemini-2.5-flash
```

`PLOKE_EVAL_LIVE_GOOGLE_MODEL_ID` and
`PLOKE_EVAL_HEADLESS_TUI_GOOGLE_MODEL_ID` are accepted by eval-specific
surfaces, but `PLOKE_LIVE_GOOGLE_CHAT_MODEL` is the shared chat canary model.

Local setup hints are opt-in. To have live-Google setup failures print the
preflight command, set:

```bash
export PLOKE_LOCAL_GOOGLE_AUTH_HINTS=1
```

This is intentionally off unless the environment opts in, so merged code does
not point unrelated machines at a local setup command by default.

## Preflight

Run the auth preflight from the same shell before live tests:

```bash
cargo xtask auth google --strict-live
```

The preflight verifies:

- `GOOGLE_PROJECT_ID` and `GOOGLE_REGION` route config.
- ADC config availability.
- ADC bearer-token resolution through the same `ploke-llm` Google router path
  used by live requests.
- selected model id parsing.
- `PLOKE_RUN_LIVE_TESTS` when `--strict-live` is passed.

For diagnostics without failing the command:

```bash
cargo xtask auth google --strict-live --report-only
cargo xtask --format json auth google --strict-live --report-only
```

The report intentionally does not print bearer tokens, API keys, or credential
file contents.

## Focused Canaries

After preflight passes, these canaries exercise the supported route:

```bash
cargo test -p ploke-llm --features live_api_tests live_google_chat_step_forced_tool_call_success_or_quota -- --ignored --nocapture
cargo test -p ploke-tui --features live_api_tests live_google_chat_session_executes_list_dir_tool_call_success_or_quota -- --ignored --nocapture
cargo test -p ploke-tui --features live_api_tests live_google_harness_router_command_runs_list_dir_through_llm_manager -- --ignored --nocapture
```

The first proves direct Google `chat_step` forced tool calls. The second proves
`ChatSession<Google>` tool-call handling. The third proves the TUI command
harness path through `/model router google`, `/model use`, the LLM manager, and
event-bus tool dispatch.

## Notes

- Keep these tests ignored and `live_api_tests`-gated. They are not part of the
  default workspace gate.
- Treat a skipped live path as not validated. With `PLOKE_RUN_LIVE_TESTS=1`,
  missing route config or ADC should fail the run.
- `gcloud auth print-access-token` can mint a token for manual debugging, but
  exporting it as `GOOGLE_API_KEY` is not the current supported route.
