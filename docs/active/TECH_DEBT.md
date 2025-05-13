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
