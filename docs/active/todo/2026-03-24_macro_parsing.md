# Todo for macro_rules parsing

Trying to parse the `serde` workspace in `tests/fixture_github_clones` has
revealed that we are missing structural elements in our module tree
construction and parsing that are being left out by our macro parsing,
specifically macro_rules parsing. To expand functionality to cover targets like
`serde`, at least to the point where we can create a useful (if not
guarenteed to be correct since we still aren't handling proc macros or build.rs
yet) that is at least parsing most of the crate correctly, and does not result
in errors like expected vs. actual pruned counts in syn_parser.

Therefore, we need to implement a new ploke workspace crate to handle some
macro rules parsing, but are limiting the scope to handling macro_rules that is
targeted at crate and module structure as opposed to full Expr parsing and
expansion.

## New crate ploke-mbe

We will copy and credit the permissively licensed `mbe` crate from rustc and
rust-analyzer as the base, then adapt as needed to our data types and parsing
model.

While I'm tempted to use RA's version as the basis, I'll probably start from
rustc's version since it is better documented, adapt it to our parsing process,
and introduce improvements I can pick up from RA.

## Likely areas to change

### fixtures and tests

We will want to add fixtures to provide targets to evaluate our parsing on in
regard to macro_rules that influence module structure, like this example from
serde (the file-paths in the comments are from `tests/fixture_github_clones`):
```rust
// in serde/serde/src/lib.rs
#[cfg(docsrs)]
#[macro_use]
#[path = "core/crate_root.rs"]
mod crate_root;

macro_rules! crate_root {
    () => {
        mod lib {
            mod core {
                #[cfg(not(feature = "std"))]
                pub use core::*;
                #[cfg(feature = "std")]
                pub use std::*;
            }

            pub use self::core::{f32, f64};
            pub use self::core::{ptr, str};

            // more here
        }

        // Used by generated code and doc tests. Not public API.
        #[doc(hidden)]
        mod private;

        include!(concat!(env!("OUT_DIR"), "/private.rs"));
    };
}

crate_root!();
```

```toml
# in serde/serde/Cargo.toml
[package]
name = "serde"
# other fields

[dependencies]
serde_core = { version = "=1.0.228", path = "../serde_core", default-features = false, features = ["result"] }
```

```rust
// in serde/serde_core/src/lib.rs

#[cfg(feature = "alloc")]
extern crate alloc;

#[macro_use]
mod crate_root;
#[macro_use]
mod macros;

crate_root!();
```

```rust
// in serde/serde_core/src/crate_root.rs
macro_rules! crate_root {
    () => {
        mod lib {
            mod core {
                #[cfg(not(feature = "std"))]
                pub use core::*;
                #[cfg(feature = "std")]
                pub use std::*;
            }

        #[cfg_attr(all(docsrs, if_docsrs_then_no_serde_core), path = "core/de/mod.rs")]
        pub mod de;
        #[cfg_attr(all(docsrs, if_docsrs_then_no_serde_core), path = "core/ser/mod.rs")]
        pub mod ser;

        #[cfg_attr(all(docsrs, if_docsrs_then_no_serde_core), path = "core/format.rs")]
        mod format;

        #[doc(inline)]
        pub use crate::de::{Deserialize, Deserializer};
        #[doc(inline)]
        pub use crate::ser::{Serialize, Serializer};

        // Used by generated code. Not public API.
        #[doc(hidden)]
        #[cfg_attr(
            all(docsrs, if_docsrs_then_no_serde_core),
            path = "core/private/mod.rs"
        )]
        mod private;

        // Used by declarative macro generated code. Not public API.
        #[doc(hidden)]
        pub mod __private {
            #[doc(hidden)]
            pub use crate::private::doc;
            #[doc(hidden)]
            pub use core::result::Result;
        }

        include!(concat!(env!("OUT_DIR"), "/private.rs"));

        #[cfg(all(not(feature = "std"), no_core_error))]
        #[cfg_attr(all(docsrs, if_docsrs_then_no_serde_core), path = "core/std_error.rs")]
        mod std_error;
    };
}
```

The issues we ran into during parsing was that we attempted to parse the
`tests/fixture_github_clones/serde` workspace with `syn_parser::parse_workspace`:

1. during the discovery phase we explore the filesystem and find, among others:
```bash
serde/serde/src/lib.rs
serde/serde/src/core/crate_root.rs
serde/serde/src/core/lib.rs
serde/serde/src/core/de/mod.rs
serde/serde/src/core/ser/mod.rs
serde/serde/src/integer128.rs
# ..others

```
Then we create partial code graphs for the contents of each file-level module,
and then merge the partial graphs in our `syn_parser` method,
`ParsedCodeGraph::merge_new`.


2. then during parsing with `analyze_files_parallel` in `analyze_files_phase2` we traverse the AST parsed with `syn`, using `CodeVisitor::visit_item_*` methods

However, we do not handle `macro_rules!` parsing or invocation, except to save the string contents in a `MacroNode`, in `CodeVisitor::visit_item_macro`

Therefore, while parsing the contents of `serde/serde/src/core/crate_root.rs`, we did not expand the macro rule found there, which contained the module declarations for `lib`, `format`, `ser`, `de`:
```rust
macro_rules! crate_root {
    () => {

        mod lib {
            mod core {
                #[cfg(not(feature = "std"))]
                pub use core::*;
                #[cfg(feature = "std")]
                pub use std::*;
            }

        // elliding `pub use` statements

        #[cfg_attr(all(docsrs, if_docsrs_then_no_serde_core), path = "core/de/mod.rs")]
        pub mod de;
        #[cfg_attr(all(docsrs, if_docsrs_then_no_serde_core), path = "core/ser/mod.rs")]
        pub mod ser;

        #[cfg_attr(all(docsrs, if_docsrs_then_no_serde_core), path = "core/format.rs")]
        mod format;

        // `mod private` is also here, but that is another bag of worms due to `include!`
    }
}
```

Therefore, as far as our parser is concerned, the file-level `ModuleNode` for
`crate_root` that only contains a single `MacroNode` and no module
declarations.

3. We merge the graphs in `ParsedCodeGraph::build_tree_and_prune` and handle `#[path = "..."]` linking,

While parsing `serde/serde/src/lib.rs`, we found
```rust
#[cfg(docsrs)]
#[macro_use]
#[path = "core/crate_root.rs"]
mod crate_root;
```
and link this in `ParsedCodeGraph::build_tree`, creating a `CustomPath` link
between the `mod crate_root` module declaration and the file-level module for
`core/crate_root.rs`, which we would parse as having the module path
`crate::crate_root`, and containing only a `MacroNode`, again not expanding or
evaluating the module declarations for `ser`, `de`, etc contained in the
macro_rule.

At this point, as far as our data model can tell, most of the files in step 1, such as 
```bash
serde/serde/src/core/de/mod.rs
serde/serde/src/core/ser/mod.rs
```
are file-level `ModuleNode` nodes that do not have a link to a module
declaration via `ResolvesToDefinition` or `CustomPath`, and are therefore
considered modules that are not linked to the module tree. They are therefore
pruned, along with their children.

NOTE: This already represents a deviation from desired behavior, though not an
error, since we are missing the structural layout of the target crate, and
therefore the ingestion pipeline of which syn_parser is a component is missing
most of the content and relationships between code items, which is needed to
form a code graph that can provide accurate code items for search and retrieval
in `ploke-rag` and `ploke-db` for the LLM chat in `ploke-tui`.

4. 
