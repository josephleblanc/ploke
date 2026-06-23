# Provider Routing

`ploke-eval` supports model/provider selection through model registry state,
provider preferences, per-command overrides, and admitted run profiles.

## Common OpenRouter flow

Inspect providers. This may call the live OpenRouter endpoints API for
OpenRouter-routed models:

```bash
cargo run -p ploke-eval -- model providers moonshotai/kimi-k2
```

Source excerpt for provider-route behavior:

```rust,ignore
{{#include ../../src/cli/handlers/model.rs:model_providers_route_behavior}}
```

Persist an OpenRouter provider preference:

```bash
cargo run -p ploke-eval -- model provider set chutes --model-id moonshotai/kimi-k2
```

Run with an explicit provider. This is a live provider/model API run:

```bash
cargo run -p ploke-eval -- run single agent \
  --instance BurntSushi__ripgrep-2209 \
  --provider chutes
```

Provider preferences live under:

```text
~/.ploke-eval/models/provider-preferences.json
```

## Direct Google caveat

Prototype 1 currently uses an asymmetric setup-time encoding for direct Google.
A run profile may contain:

```toml
[model]
id = "google/gemini-3.5-flash"
route_source = "direct-google"
provider = "google"
```

That `provider = "google"` value is a setup-time sentinel only. A correctly
admitted direct Google campaign manifest should serialize route source as
`direct_google` and should omit `provider_slug` or serialize it as null.

Do **not** add `"provider_slug": "google"` to `campaign.json` to make the fields
look symmetric. OpenRouter provider slugs belong only to `route_source =
"openrouter"` campaigns.

Source excerpts for direct-Google normalization:

```rust,ignore
{{#include ../../src/campaign.rs:campaign_direct_google_provider_normalization}}
{{#include ../../src/campaign.rs:campaign_resolve_direct_google_provider}}
```

## Debugging route issues

Use:

```bash
./target/debug/ploke-eval loop prototype1-doctor --repo-root . --live-protocol-preflight
```

only when a tiny live provider request is acceptable.
