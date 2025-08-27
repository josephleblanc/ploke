# Technical Debt

This is a list of known fixes that I will want to make but are not terribly urgent.

## Nodes
 * [ ] Change `VariantNode`'s field `discriminant` from `String` to a number.
    *  [ ] Change [node_definition](/crates/ingest/syn_parser/src/parser/nodes/enums.rs)
    *  [ ] Update change in [db transform](crates/ploke-transform/src/transform/variants.rs)
 * [ ] Add attribute tracking to impl (if applicable, look into it)
 * [ ] Add `unsafe` toggle for all relevant nodes (probably most) but specifically:
    * All `Union`s are unsafe.
 * [ ] Refactor `CodeGraph.defined_types` to be a vec of ids (possibly a new typed id), and move all nodes into their own fields. 
    * For `StructNode`, `UnionNode`, `EnumNode` and `TypeAliasNode`
 * [ ] Add more logging to tests in ploke-transform
 * [ ] Implement proper error handling in ploke-transform
 * [ ] Change `ModuleNode` to handle `imports` and `exports` through variant `ModuleKind`
 * [ ] Add tracking_hash to `ImportNode`
 * [ ] Add attribute tracking to `ImportNode`
 * [ ] Change `CodeGraph.use_statements` to `CodeGraph.imports`
 * [ ] Add the canonical path as a string to each `*Node` item for now, primarily as a debugging tool but also possibly just to make the results more human-readable.
 * [ ] Add new relation between file-level module and all nodes within the file.
 * [ ] Make `TypeNode` `related_types` an option
 * [ ] Refactor `TypeKind`
    * change `Array` size from `String` to `i64`
 * [ ] Add tests for TrackingHash
    * [ ] Remove Remove `?` from the database transforms for `TrackingHash`
 * [ ] Consider adding an ID to `SyntacticRelation`s
 * [x] Implement a `Database` type to wrap the cozo database, probably in `ploke-core`
  - NOTE: This is in `ploke-db`
 * [ ] `CrateContext` Only added some of the fields, could possibly also add better file processing or a list of the Uuids of the modules/primary node types here.

## TUI
* [ ] Clean up main app rendering loop (extract into functions)
* [ ] Untangle Event system
* [ ] Add proper error Event handling
* [ ] Revisit `add_msg_immediate`, it is over-used
* [ ] Separate File operations from `SystemEvent` and into `FileEvent` as initially planned
* [ ] Consider Refactor of `MessageUpdateEvent` command flow to use `oneshot`
      after initial message creation to wait on update to the message.
* [ ] Need to add more feedback
    * [ ] Refactor `FileManager` or `AppEvent` to handle response with update
          on file save state completion.
* [ ] Introduce a reusable trait for scrollable overlays/panes (dynamic placement left/right/top/bottom, adjustable coverage like splits, with consistent scroll state management); adopt in Model Browser and new Context/Approvals overlays.
* [ ] Add input autocomplete for commands and keywords; show suggestions inline.
* [ ] In Slash/Command mode, detect known commands pre-submit and change input box color (visual affordance).
* [ ] SysInfo verbosity: introduce On/Off toggle, later add levels and auto‑aging of transient system messages to reduce noise.

## OpenRouter Types
* [ ] Consolidate OpenRouter types into a single module with strong typing (serde derives), remove ad‑hoc conversions; add micro validation layer.

## Database
* [ ] Change the Uuid type in the cozo database to be Bytes instead.
  * This is because the Uuid type within cozo is basically useless (e.g. can't sort by Uuid).

## Tests

### Test Database
* [ ] Tests for loading database from config file
  * [ ] Graceful failures

## Longer term/larger refactor
* [ ] Expand tracked types to handle the following potentially missing types
  Missing Rust types:
  1. **Closure types** (e.g., `|| -> i32 { ... }`)
  2. **Projection types** (e.g., `<T as Trait>::AssocType`)
  3. **Bound variables** (for generic parameters)
  4. **Inferred tuple structs** (e.g., `struct Foo(_)`)
  5. **Type parameters** (generic placeholders like `T`)
  6. **Const generics** (e.g., `[T; N]` where N is a const generic)
  7. **Placeholder types** (like `_` in more contexts)
