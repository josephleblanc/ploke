# Setup

This page is the minimum preflight before running eval commands.

## 1. Build the binary

```bash
cargo build -p ploke-eval
```

Expected:

```text
target/debug/ploke-eval
```

## 2. Check local setup

```bash
cargo run -p ploke-eval -- doctor
```

Use this before blaming run logic: it catches missing environment, registry, or
local configuration problems.

## 3. List dataset keys

```bash
cargo run -p ploke-eval -- run datasets list
```

Use one of the listed keys with `run repo fetch`, `run prepare instance`, or
`run prepare batch`.

## 4. Inspect model/provider state

```bash
cargo run -p ploke-eval -- model current
cargo run -p ploke-eval -- model providers
```

`model providers` may call the live OpenRouter endpoints API unless the current
cached model route is direct Google.

For provider setup and route caveats, see [Providers](providers.md) and
[Provider Routing](../reference/provider-routing.md).

## State touched

| Command | Safety | State touched |
| --- | --- | --- |
| `cargo build -p ploke-eval` | `BUILD` | `target/` |
| `ploke-eval doctor` | `RO` | read-only diagnosis |
| `run datasets list` | `RO` | read-only built-in registry view |
| `model current` | `RO` | reads model selection |
| `model providers` | `NET?` | may query the OpenRouter endpoints API depending on cached/current route |
