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
//!
//! Current exercised surface:
//!
//! - ordinary named type sources resolving to ordinary targets
//! - trait sources resolving to trait targets
//! - selected structural containment edges, including named arguments,
//!   references, tuples, function pointers, and trait objects
//! - exact root vs nested terminal ranking for related owners
//! - selected type generic parameter resolution and shadowing behavior
//! - selected alias, trait-impl, and real-corpus graphRAG contracts
//!
//! Known typed-id expansion points not yet exercised at the DB layer:
//!
//! - full structural `AnyTypeId` coverage for slices, arrays, raw pointers,
//!   `impl Trait`, parens, never, inferred, macro, and unknown type vertices
//! - explicit contracts that container type-use roots are queryable as roots
//!   while only named/trait-bound descendants become resolution sources
//! - exhaustive `type_use` root-role coverage for method params/returns,
//!   fields, consts, statics, impl self, impl trait, trait super, alias
//!   targets, and function params/returns
//! - exhaustive ordinary target coverage for structs, enums, unions,
//!   type aliases, and type generic params
//! - lifetime and const generic parameter families, including their exclusion
//!   from ordinary type-resolution target sets
//! - generic parameter owner scopes across functions, defined types, traits,
//!   impls, methods, and type aliases
//! - first-class generic parameter relations such as type bounds, type
//!   defaults, and const parameter types
//! - negative/exclusion contracts that invalid family crossings do not appear
//!   in DB query results
//! - semantic non-node targets such as `Self`, primitives/builtins, and
//!   external dependencies, once those have an explicit DB representation

mod common;
mod containment;
mod context_expansion_wishlist;
mod corpus_contracts;
mod direct_roots;
mod reachability;
mod related_owners;
mod wishlist_contracts;
