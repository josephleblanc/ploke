# Deps tech debt
Audit
```
cargo tree | less or cargo tree -e features (to see which features pull what).

cargo geiger to check unsafe usage in dependencies.

cargo udeps for unused dependencies.
```

Turn off default features aggressively.
```
Example: reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }.
```

This alone can cut dozens of transitive crates.

Replace big crates with small targeted ones.

Instead of reqwest, sometimes plain hyper + serde_json is enough.

Instead of pulling anyhow + thiserror, you might just use one.

For OpenRouter, you may not need a full OpenAPI-generated client, just reqwest + a couple of structs.

Split workspaces.
If Ploke is a workspace, you can isolate heavy stuff (e.g. graph visualization with petgraph, eframe, d3) in its own crate, so your core logic compiles faster.

Watch for codegen crates.
Anything pulling prost, tonic, or bindgen will balloon compile time and disk use. Sometimes you can generate once and commit the result.
