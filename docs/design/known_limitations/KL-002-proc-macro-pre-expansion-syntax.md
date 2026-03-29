# KL-002 Proc-macro pre-expansion syntax

## Description

`syn_parser` currently requires each source file to parse successfully through
`syn::parse_file` before any visitor logic or item-skipping policy can apply.

This means some crates that are valid under the Rust toolchain after proc-macro
expansion are still unsupported by `syn_parser` when their raw source is not
parseable as ordinary pre-expansion Rust syntax.

The current reproduced case is `#[duplicate_item(...)]` placeholder syntax from
`FuelLabs/sway`, captured by:

- [`duplicate_item::repro_duplicate_item_placeholder_trait_signatures`](/home/brasides/code/ploke/crates/ingest/syn_parser/tests/repro/duplicate_item.rs)

That source compiles when the `duplicate` attribute proc macro expands it, but
the raw file fails during `syn::parse_file(...)` with parse errors such as
`expected ','`.

## Why macro skipping does not help

Skipping macro bodies in the visitor is too late for this class of failure.

The parse pipeline currently does:

1. read file contents
2. parse the whole file with `syn::parse_file`
3. visit parsed items and apply skip/traversal policy

If step 2 fails, the visitor never runs. This limitation is therefore about
pre-expansion file parsing, not about item traversal inside already-parsed macro
nodes.

## Current policy

- fail closed on these files/crates with an explicit parse error
- do not synthesize placeholder unresolved nodes for the unparsed file
- do not silently drop the file and continue as if the model were complete

This preserves graph correctness and avoids introducing hidden blank spots that
would later corrupt type resolution or import semantics.

## Possible future resolution paths

1. integrate with a real Rust expansion pipeline and parse expanded output
2. use tooling that exposes post-expansion source, then feed that to `syn`
3. add narrowly scoped preprocessing for specific proc macros, if explicitly
   accepted as a targeted workaround

The first two are architecturally stronger but much larger in scope. The third
is cheaper but brittle and macro-specific.

## Actions taken

- documented the limitation instead of weakening parser/model invariants
- kept the repro test failing as an explicit unsupported case marker
- chose explicit unsupported-crate/file errors over unresolved placeholder
  modeling
