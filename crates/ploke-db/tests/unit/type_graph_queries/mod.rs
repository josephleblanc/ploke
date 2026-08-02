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
//! - a shared corpus-backed `TypeShapeCase` matrix that names each modeled
//!   structure as positive traversal, DB-only coordinate coverage, no-target
//!   fallback, or absent-in-current-corpus fallback
//! - ordinary named type sources resolving to ordinary targets
//! - trait sources resolving to trait targets
//! - selected structural containment edges, including named arguments,
//!   references, slices, arrays, tuples, raw pointers, qualified projections,
//!   trait objects, impl trait, trait bounds, associated type bounds, and trait
//!   supers
//! - exact root vs nested terminal ranking for related owners
//! - selected type generic parameter resolution and shadowing behavior
//! - selected alias, trait-impl, associated type bound, and real-corpus
//!   graphRAG contracts
//!
//! Where-clause behavior is modeled across these layers:
//!
//! - Parser fixture ingestion supplies source coordinates for each where
//!   subject and bound.
//! - `setup_typed_fixture_db` reparses the focused fixture and runs the
//!   transform before each DB assertion, so `direct_roots`, `reachability`,
//!   `fixed_rules`, and `invariants` are parser -> transform -> database
//!   contracts, not hand-inserted DB fixtures.
//! - `direct_roots` pins the persisted DB coordinate model:
//!   `WherePredicateSubject { predicate_index }`,
//!   `WherePredicateBound { predicate_index, bound_index }`, and
//!   `WhereGenericParamBound { containing_owner_id, predicate_index,
//!   bound_index }`.
//! - Direct generic-param subjects intentionally get both containing-owner
//!   roots and generic-param-owned roots. Composite subjects such as `Vec<T>`
//!   remain only containing-owner subject/bound roots and must not be collapsed
//!   into `T: Bound`.
//! - `reachability` checks that exact where coordinates flow through
//!   `type_contains` and `type_relation` to the expected terminal targets.
//! - `corpus_contracts::where_clause_bounds` pins real-corpus backup behavior
//!   for local trait bounds, repeated-subject predicates, multi-predicate and
//!   multi-bound coordinates, nested local terminals inside external bounds,
//!   and composite where subjects.
//! - `matrix` provides corpus-backup rows consumed by DB, RAG, and TUI tests;
//!   `context_expansion_wishlist` keeps longer-horizon expansion behavior.
//!
//! Known typed-id expansion points not yet exercised at the DB layer:
//!
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
//! - ignored live model/tool matrix assertions. Direct TUI tool execution is
//!   covered for materializable matrix rows; live tests should assert
//!   `ToolCallRequested`/`ToolCallCompleted` payloads rather than model wording.
//! - broad negative/exclusion contracts that invalid family crossings do not
//!   appear in DB query results. Where-composite-to-generic-param collapse is
//!   covered as a focused negative case.
//! - semantic non-node targets such as `Self`, primitives/builtins, and
//!   external dependencies, once those have an explicit DB representation
//!
//! Recursive containment is intentionally bounded by fixture examples and the
//! fixed-rule traversal depth. Tests assert the deepest nesting present in the
//! focused and corpus fixtures rather than trying to enumerate an unbounded
//! recursive type grammar.

mod common;
mod containment;
mod context_expansion_wishlist;
mod corpus_contracts;
mod direct_roots;
mod endpoint_families;
mod fixed_rules;
mod invariants;
mod matrix;
mod reachability;
mod related_owners;
mod wishlist_contracts;
