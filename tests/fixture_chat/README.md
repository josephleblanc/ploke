# Token diagnostics fixture

This folder holds a trimmed excerpt from `crates/ploke-tui/logs/tokens_*.log` captured on 2025-12-21 with `PLOKE_LOG_TOKENS=1`. It contains only the `estimate_input` and `actual_usage` lines for three requests. The outbound request bodies and prompts were truncated to avoid leaking full content.

Notes:
- The `model` field shows `None` because we currently log the optional model key; the request itself was sent with the default `moonshotai/kimi-k2` model (visible in the `estimate_input` lines). Consider logging the resolved model name if you need it in fixtures.
- To regenerate a fresh fixture, run the TUI with `PLOKE_LOG_TOKENS=1`, then use `cargo xtask extract-tokens-log` to copy only the relevant lines into `tests/fixture_chat/tokens_sample.log`.
