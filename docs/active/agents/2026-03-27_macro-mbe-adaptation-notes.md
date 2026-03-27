# date
2026-03-27

# task title
Adapting rust-analyzer `mbe` ideas to `syn_parser`

# task description
Survey `~/clones/rust-analyzer/crates/mbe`, compare it to `crates/ingest/syn_parser`, and sketch a design that can reuse the strong parts of rust-analyzer's declarative macro machinery without forcing `syn_parser` into rust-analyzer's architecture.

# related planning files
- `docs/active/todo/2026-03-24_macro_parsing.md`

## Summary

`rust-analyzer`'s `mbe` crate is worth borrowing from, but not worth porting wholesale.

The reusable core is:
- a standalone declarative-macro IR
- a parser that lowers macro rules into that IR
- a matcher that handles nested repetitions through structured bindings
- a transcriber that turns bindings back into tokens

The parts that do not fit `syn_parser` directly are:
- rust-analyzer's token-tree stack (`tt`, `span`, `syntax-bridge`, hygiene/syntax-context handling)
- its incremental database integration (`salsa`)
- its assumption that macro expansion participates in a larger parse/expand/HIR pipeline rather than a file-graph plus module-tree pipeline

For `syn_parser`, the right adaptation is a small, explicit, structural-expansion subsystem focused on `macro_rules!` that can produce additional `syn::Item`s needed for module-tree construction, not a full rust-analyzer-style macro engine embedded in every parser path.

## What rust-analyzer `mbe` is doing well

Relevant local references:
- [`crates/mbe/src/lib.rs`](/home/brasides/clones/rust-analyzer/crates/mbe/src/lib.rs)
- [`crates/mbe/src/parser.rs`](/home/brasides/clones/rust-analyzer/crates/mbe/src/parser.rs)
- [`crates/mbe/src/expander/matcher.rs`](/home/brasides/clones/rust-analyzer/crates/mbe/src/expander/matcher.rs)
- [`crates/mbe/src/expander/transcriber.rs`](/home/brasides/clones/rust-analyzer/crates/mbe/src/expander/transcriber.rs)

The useful design choices are:

- `DeclarativeMacro` is a compact compiled representation of a macro definition rather than raw source text.
- `MetaTemplate` + `Op` separates parsing of macro syntax from expansion.
- The matcher handles repetitions with a nested binding structure instead of ad hoc recursion.
- The transcriber is independent from the matcher, which makes expansion failures easier to report.
- The crate is mostly isolated from the rest of rust-analyzer's semantics. That isolation is exactly the part worth copying.

The strongest portable idea is the three-stage shape:

1. Parse macro definition into a declarative IR.
2. Match invocation tokens against rule patterns.
3. Transcribe the chosen rule into output tokens.

## Where `syn_parser` is different

Relevant local references:
- [`crates/ingest/syn_parser/src/parser/visitor/code_visitor.rs`](/home/brasides/code/ploke/crates/ingest/syn_parser/src/parser/visitor/code_visitor.rs)
- [`crates/ingest/syn_parser/src/parser/nodes/macros.rs`](/home/brasides/code/ploke/crates/ingest/syn_parser/src/parser/nodes/macros.rs)
- [`crates/ingest/syn_parser/src/parser/visitor/mod.rs`](/home/brasides/code/ploke/crates/ingest/syn_parser/src/parser/visitor/mod.rs)
- [`crates/ingest/syn_parser/src/parser/graph/code_graph.rs`](/home/brasides/code/ploke/crates/ingest/syn_parser/src/parser/graph/code_graph.rs)
- [`docs/active/todo/2026-03-24_macro_parsing.md`](/home/brasides/code/ploke/docs/active/todo/2026-03-24_macro_parsing.md)

Current `syn_parser` behavior is AST-first and file-oriented:

- each file is parsed once with `syn`
- `CodeVisitor` records nodes and relations directly into a `CodeGraph`
- `visit_item_macro` stores a `MacroNode` with a string body, but does not parse or expand it
- module-tree construction happens after file graphs are merged
- pruning depends on explicit module declarations having been discovered before pruning

That means macro expansion matters here for one specific reason: macro-generated `mod`, `use`, and related structural items can affect tree construction and prevent valid files from being pruned.

This is narrower than rust-analyzer's goal. `syn_parser` does not need full semantic macro expansion to get immediate value. It needs reliable structural expansion.

## Recommended target design

Create a new crate, likely `crates/ingest/ploke-mbe`, with a deliberately narrow contract:

- input: declarative macro definitions plus invocation token streams
- output: expanded token streams, plus structured errors/provenance
- no direct dependency on `syn_parser` graph types
- no direct dependency on `salsa`
- no hygiene-heavy architecture in the first version

Use rust-analyzer's shape, but replace the surrounding ecosystem with ploke-local types.

### Internal layers

1. `ploke_mbe::tt`
- define a very small token-tree model backed by `proc_macro2`
- keep delimiter, ident, punct, literal, and subtree
- preserve spans only as far as needed for diagnostics and tracking

2. `ploke_mbe::ir`
- port the conceptual structure of `DeclarativeMacro`, `Rule`, `MetaTemplate`, `Op`, `RepeatKind`, and metavar kinds
- keep this independent from `syn`

3. `ploke_mbe::parse`
- lower `macro_rules!` definitions from token trees into the IR
- fail closed on unsupported syntax rather than silently accepting it

4. `ploke_mbe::match` and `ploke_mbe::transcribe`
- keep rust-analyzer's nested-binding idea
- preserve explicit expansion errors

5. `ploke_mbe::structural`
- parse expanded tokens back into `syn` items
- expose a narrow API for "expand this invocation into top-level items"

## Recommended integration point in `syn_parser`

Do not run expansion inside [`code_visitor.rs`](/home/brasides/code/ploke/crates/ingest/syn_parser/src/parser/visitor/code_visitor.rs) while visiting a single file.

That is too early for cases like:
- `#[macro_use] mod crate_root;`
- `crate_root!();`

in which the defining `macro_rules!` lives in another file that only becomes reachable after explicit module linking.

The better shape is an iterative crate-level stage:

1. Phase 1: discovery as today.
2. Phase 2: parse files as today, recording:
- ordinary graph nodes
- macro definitions
- top-level macro invocations with their tokens and module context
3. Phase 3a: build the initial module tree from explicit `mod` declarations and `#[path]` links only.
4. Phase 3b: build a macro registry from reachable declarative macros.
5. Phase 3c: expand only structural macro invocations and reparse the resulting items.
6. Phase 3d: merge generated structural items into the graph and rebuild/augment the module tree.
7. Repeat until no new structural items appear, then prune as today.

This breaks the current cycle:
- module links are needed to reach macro definitions in other files
- macro expansion is needed to discover more module links

An explicit fixed-point stage is a better fit than trying to make Phase 2 omniscient.

## Narrow first scope

The first supported output should be restricted to items that affect crate structure:

- `mod`
- `use`
- `pub use`
- nested modules
- `extern crate`
- possibly `type`, `const`, `static`, `fn`, `struct`, `enum`, `trait`, `impl` if reparsing them is cheap

But the success criterion should be structural completeness, not full macro fidelity.

For the serde-style cases in the existing todo note, the priority subset is:

- `macro_rules!` definition parsing
- invocation expansion for local declarative macros
- reparsing expanded items as `syn::Item`
- honoring `#[path]` and `cfg_attr(path = ... )` on generated `mod` items

## Things to keep intentionally different from rust-analyzer

### 1. No full rust-analyzer dependency stack

Porting `mbe` together with `tt`, `span`, `intern`, `syntax-bridge`, and `salsa` would import a second parser architecture into this workspace. That is likely more expensive than writing a thinner compatibility layer around `proc_macro2`.

### 2. Structural expansion before semantic expansion

`syn_parser` is building a graph and module tree, not an IDE-grade fully expanded HIR. Structural expansion gets the immediate win without pretending proc macros, hygiene, or name resolution are solved.

### 3. Fail closed on unsupported cases

This is important for current correctness guardrails.

If expansion depends on unsupported behavior such as:
- `include!`
- builtin macros like `concat!` or `env!`
- non-local macro resolution that has not been modeled yet
- fragment kinds that the first implementation cannot faithfully transcribe

then the parser should record an explicit unresolved/blocked expansion result. It should not silently drop generated items or invent permissive fallback behavior.

## Concrete graph/API additions that seem justified

`syn_parser` likely needs a distinction it does not currently have:

- macro definitions
- macro invocations
- macro-generated items

Recommended additions:

- add a `MacroInvocationNode` or equivalent phase-local record
- add provenance on generated items:
  - source invocation id
  - source macro definition id if known
  - expansion round
- add an unresolved expansion record with a reason enum

That allows the module-tree stage to say:
- this module declaration was generated by macro expansion
- this invocation was reachable but intentionally not expanded

This is more robust than overloading `MacroNode.body: Option<String>`.

## Minimal implementation plan

1. Introduce a small TT adapter crate or module using `proc_macro2`.
2. Port the declarative IR shape from rust-analyzer, not its whole dependency graph.
3. Teach Phase 2 to capture top-level macro invocations in addition to definitions.
4. Add a post-merge structural expansion stage before final prune.
5. Reparse expanded output with `syn::parse2::<syn::File>` or `syn::parse2::<syn::Item>`-level helpers.
6. Record generated items with provenance and explicit unsupported-expansion errors.
7. Start with fixture coverage modeled on the serde examples in the todo note.

## Suggested first milestone

A good first milestone is:

- handle `macro_rules!` defined in one reachable module file
- invoked at crate root or module top level
- expanding into `mod` declarations and `use` items
- with no `include!` support yet

If that works, the serde-style pruning failure should turn into an explicit smaller set of unresolved cases rather than a wholesale loss of module structure.

## Bottom line

Borrow rust-analyzer's declarative-macro core, not its surrounding architecture.

For `syn_parser`, the right design is:
- standalone ploke-local MBE crate
- restricted structural expansion
- iterative crate-level integration after initial explicit module linking
- strict unsupported-case reporting

That gives you the part of `mbe` that is mature and valuable while staying compatible with `syn_parser`'s current graph-first, module-tree-first design.
