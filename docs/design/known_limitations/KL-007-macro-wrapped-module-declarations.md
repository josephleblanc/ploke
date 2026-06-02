# KL-007 Macro-wrapped module declarations are not discovered as modules

## Description

`syn_parser` currently discovers module declarations from the ordinary
pre-expansion Rust item tree. If a crate hides `mod` items inside a macro
invocation, those modules are not discovered as real `ModuleNode`s and their
files are not part of the reachable parsed graph unless another ordinary module
path reaches them.

The concrete corpus case is `hyperium/hyper`:

```rust
cfg_feature! {
    #![feature = "client"]

    pub mod client;
}
```

and, inside `src/client/mod.rs`:

```rust
cfg_feature! {
    #![any(feature = "http1", feature = "http2")]

    pub mod conn;
    pub(super) mod dispatch;
}
```

The source file `src/client/dispatch.rs` exists and contains
`channel<T, U>() -> (Sender<T, U>, Receiver<T, U>)`, but the current parser does
not expand `cfg_feature!`, so `client::dispatch` is not represented in the DB
graph produced by the live corpus parse.

## Crate-level summary

See [syn_parser known limitations — L6](../syn_parser_known_limitations.md).

## Distinction from [KL-002](KL-002-proc-macro-pre-expansion-syntax.md)

KL-002 covers source that is accepted by rustc only after proc-macro expansion
but fails during `syn::parse_file`.

KL-007 covers source that can still parse as a macro invocation, but whose
semantic module declarations are hidden inside that macro invocation. The file
parses, but the module declarations are not visible to the visitor as
`syn::ItemMod` values.

## Distinction from [KL-003](KL-003-cfg-disjoint-duplicate-inline-mod.md)

KL-003 is about duplicate paths when multiple cfg branches are visible to the
syntactic graph at once.

KL-007 is about the opposite failure mode: module declarations behind a macro
are not visible at all unless the macro is expanded or handled by a targeted
preprocessor.

## Repro / observed test behavior

The typed type graph corpus contract originally targeted Hyper's
`src/client/dispatch.rs::channel` tuple return. That function exists on disk,
but it was absent from the transformed DB function relation under the current
live corpus parse.

The test was retargeted to the reachable ordinary module
`src/common/watch.rs::channel`, which preserves the same type-graph tuple-return
behavior while avoiding a macro-hidden module path.

## Possible future resolution paths

1. **Expansion pipeline:** parse post-expansion Rust so macro-generated or
   macro-wrapped modules become ordinary items.
2. **Targeted macro preprocessor:** teach discovery about known declaration
   wrapper macros such as Hyper's `cfg_feature!`.
3. **Feature-aware corpus parse mode:** combine macro handling with explicit
   Cargo feature selection so gated module declarations are represented for a
   chosen feature set.
4. **Document-only policy:** keep using reachable ordinary module surfaces for
   corpus tests and record macro-hidden modules as unsupported.

## Current policy

- Do not silently synthesize modules hidden inside arbitrary macro invocations.
- Do not pretend feature-gated macro-wrapped modules are present in corpus DB
  tests unless the parser actually discovers them.
- Prefer precise tests against the reachable parsed graph until a real expansion
  or macro-specific module discovery policy exists.
