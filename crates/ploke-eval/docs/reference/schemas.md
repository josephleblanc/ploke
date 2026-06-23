# Schemas

Most `ploke-eval` schemas are Rust-owned rather than separately versioned public
schemas. Treat source types as the canonical schema when docs and code differ.

| Surface | Schema/source owner |
| --- | --- |
| CLI args | `src/cli/args/` |
| Run manifests and artifacts | `src/runner/`, `src/run_history.rs`, `src/record.rs` |
| Campaign manifests | `src/campaign.rs`, `src/cli/args/campaign.rs` |
| Closure state | `src/closure.rs` |
| Model registry/preferences | `src/model_registry.rs`, `src/provider_prefs.rs` |
| Protocol artifacts/reports | `src/protocol*.rs` |
| Prototype 1 run profile | `src/cli/prototype1_state/profile.rs` |
| Prototype 1 parent identity | `src/cli/prototype1_state/identity.rs` |
| Prototype 1 child-plan message | `src/cli/prototype1_state/parent.rs` |
| Prototype 1 invocation/ready/completion | `src/cli/prototype1_state/invocation.rs` |
| Prototype 1 journal | `src/cli/prototype1_state/journal.rs` |
| Prototype 1 History | `src/cli/prototype1_state/history/` |
| Prototype 1 scheduler/node records | `src/intervention/scheduler.rs` |
| Prototype 1 branch registry | `src/intervention/branch_registry.rs` |

## Documentation rule

Reference pages should describe current fields and invariants, but schema truth
belongs to the owning Rust type and its serialization tests. Do not loosen import
or validation behavior just to load older artifacts without explicit approval.
