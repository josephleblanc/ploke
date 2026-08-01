# Providers

Use this page to inspect model/provider routing before a run.

## Show current model

```bash
cargo run -p ploke-eval -- model current
```

## List providers for the current model

This may call the live OpenRouter endpoints API unless the current cached model
route is direct Google.

```bash
cargo run -p ploke-eval -- model providers
```

Or inspect a specific model; this has the same live-API caveat:

```bash
cargo run -p ploke-eval -- model providers moonshotai/kimi-k2
```

## Persist a provider preference

```bash
cargo run -p ploke-eval -- model provider set chutes --model-id moonshotai/kimi-k2
```

Show or clear it:

```bash
cargo run -p ploke-eval -- model provider current --model-id moonshotai/kimi-k2
cargo run -p ploke-eval -- model provider clear --model-id moonshotai/kimi-k2
```

Provider preferences are stored under:

```text
~/.ploke-eval/models/provider-preferences.json
```

## Parent patcher model

Prototype 1 broad-harness parent patch generation can use a separate persisted
selection:

```bash
cargo run -p ploke-eval -- model parent-patcher set minimax/minimax-m2.5
cargo run -p ploke-eval -- model parent-patcher current
```

For direct Google route caveats, see
[Provider Routing](../reference/provider-routing.md).
