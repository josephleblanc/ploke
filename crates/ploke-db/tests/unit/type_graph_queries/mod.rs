#![cfg(feature = "typed_type_graph")]
//! Type graph query contracts for graphRAG-oriented Rust code traversal.
//!
//! This directory is intentionally a capability map, not just a regression
//! suite for the next small implementation slice.
//!
//! The non-ignored tests are executable anchors: they pin behavior that already
//! exists at the DB layer. The ignored wishlist tests are long-horizon
//! contracts. They should read like examples of searches we currently perform
//! with `rg`, but want to answer structurally through:
//!
//! code owner -> root type use -> structural type containment
//! -> terminal named/trait source -> resolved definition
//! -> other owners with related exact/root/terminal type usage.

mod common;
mod containment;
mod corpus_contracts;
mod direct_roots;
mod reachability;
mod related_owners;
mod wishlist_contracts;
