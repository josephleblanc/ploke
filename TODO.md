# Immediate TODO

This document holds the immediate todo items, which will change frequently.

## Immediate Objectives

### Refactor `TypeKind` transform to cozo



### Basic Relation Storage

Add the explicit edge relations `SyntacticRelation` to the cozo database. This should always be done within the transform of the source node to ensure the entry of the node and its edges remains tightly coupled. As the node is being added to the database there should be a verification step to ensure that no orphaned nodes or dangling edges are added to the database.

1.  [ ] Add `ModuleNode` relations:
      - [ ] `Contains`: module -> primary node
      - [ ] `ResolvesToDefinition`: module decl -> file-level module definition
      - [ ] `CustomPath`: module decl -> file-level module definition
      - [ ] `ModuleImports`: module -> import
1.  [ ] Add `ImportNode` relations:
      - [ ] `ReExports`: import -> primary node
1.  [ ] Add `StructNode` relations:
      - [ ] `StructField` struct -> field
1.  [ ] Add `UnionNode` relations:
      - [ ] `UnionField` union -> field
1.  [ ] Add `VariantNode` relations:
      - [ ] `VariantField` variant -> field
1.  [ ] Add `EnumNode` relations:
      - [ ] `EnumVariant` enum -> variant



### Someday Relations (noted here for recording elsewhere, relations not yet implemented)
1.  [ ] Add `ImplNode` relations:
      - [ ] `ImplAssociatedItem`

[primary schema]:crates/ingest/ploke-transform/src/schema/primary_nodes.rs
