//! Associated [`MethodNode`] paranoid tests for the `fixture_nodes` crate (modules from
//! `fixture_nodes/src/lib.rs` only).
//!
//! See `methods.rs` for exhaustive coverage of every `MethodNode` emitted for `impls.rs` and
//! `traits.rs`. Inner `impl`s under `imports.rs` are not surfaced as graph items by the current
//! visitor (see `fixture_assoc_method_node_total_matches_graph`). Impl blocks are keyed by span;
//! module paths and [`AssocParanoidArgs`](crate::common::AssocParanoidArgs) are fixture-coupled.

mod methods;
