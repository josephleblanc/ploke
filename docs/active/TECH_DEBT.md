# Technical Debt

This is a list of known fixes that I will want to make but are not terribly urgent.

## Nodes
 * [ ] Change `VariantNode`'s field `discriminant` from `String` to a number.
    *  [ ] Change [node_definition](/crates/ingest/syn_parser/src/parser/nodes/enums.rs)
    *  [ ] Update change in [db transform](crates/ploke-transform/src/transform/variants.rs)
 * [ ] Add attribute tracking to impl (if applicable, look into it)
