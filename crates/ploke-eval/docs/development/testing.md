# Testing

Use focused commands first, then broaden.

## Docs

Build this book:

```bash
mdbook build crates/ploke-eval
```

Command examples in this book should be copy/paste runnable when they are not
live-provider probes. Prefer concrete safe examples, or shell variables with
`${VAR:?explanation}` guards, over angle-bracket placeholders in `bash` blocks.
Live API examples should say so in nearby prose.


Serve it locally:

```bash
mdbook serve crates/ploke-eval --open
```

Serve on a specific host/port:

```bash
mdbook serve crates/ploke-eval --hostname 127.0.0.1 --port 4000
```

## Rust checks

Focused crate check:

```bash
cargo check -p ploke-eval
```

Focused tests:

```bash
cargo test -p ploke-eval
```

Workspace smoke check:

```bash
cargo check --workspace 2>&1 | rg -A 8 E0
```

Full workspace test run can be costly:

```bash
cargo test --workspace 2>&1 | rg -A 8 E0
```

## Fixture-backed work

Before costly fixture-backed tests, verify local assets:

```bash
./target/debug/xtask verify-fixtures 2>&1 | rg -A 8 E0
```

Do not loosen backup fixture import semantics to hide schema drift. Regenerate
or migrate fixtures when schema changes require it.
