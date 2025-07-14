# Known Limitations

## Fixture Crates Require Complete Cargo.toml

When using fixture crates for testing, each crate must have a complete `Cargo.toml` file that properly defines the crate type. For example:

```toml
[package]
name = "fixture_nodes"
version = "0.1.0"
edition = "2021"

# This section is REQUIRED for discovery
[lib]
path = "src/lib.rs"

[dependencies]
serde = { version = "1.0", features = ["derive"] }
```

### Why this is required
The discovery phase in our parser pipeline uses `cargo metadata` to identify crate roots and establish crate context. Without a proper `[lib]` or `[[bin]]` section in `Cargo.toml`:
1. The crate won't be recognized as a valid Rust target
2. No `CrateContext` will be created during discovery
3. Module tree construction will fail with "Crate context is missing"

### Impacted Areas
This affects any test that exercises the full parser pipeline, including:
- Module tree construction
- Path resolution
- Any functionality requiring crate-level metadata

### Workaround
Always include target definitions in fixture `Cargo.toml` files. For library crates use:
```toml
[lib]
path = "src/lib.rs"
```
For binary crates use:
```toml
[[bin]]
name = "my_bin"
path = "src/main.rs"
```
