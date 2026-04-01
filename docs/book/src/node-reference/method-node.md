# Overview

Status: Draft

## Purpose

Walks through how `MethodNode` is created, stored, and retrieved across our pipelines.

## Scope

`MethodNode` across all crates, calling out notable differences from how other nodes are handled.

## Parsing (syn_parser)

1. Construction happens in the visitor.

- In `visit_item_impl`, methods are created inside impls and then linked back to the parent impl.

```rust,noplayground
{{#rustdoc_include ../../../../crates/ingest/syn_parser/src/parser/visitor/code_visitor.rs:method_from_impl_node}}
```

```rust,noplayground
{{#rustdoc_include ../../../../crates/ingest/syn_parser/src/parser/visitor/code_visitor.rs:impl_method_relations}}
```

- `visit_item_trait` does the same for trait default methods.

```rust,noplayground
{{#rustdoc_include ../../../../crates/ingest/syn_parser/src/parser/visitor/code_visitor.rs:method_from_trait_node}}
```

```rust,noplayground
{{#rustdoc_include ../../../../crates/ingest/syn_parser/src/parser/visitor/code_visitor.rs:trait_method_relations}}
```

- `MethodNodeId` is only an associated-item ID; there is no direct `graph.get_method(...)` path.

```rust,noplayground
{{#rustdoc_include ../../../../crates/ingest/syn_parser/src/parser/nodes/ids/internal.rs:method_node_id_marker}}
```

2. Merge just appends the parent nodes and relations.

- `ParsedCodeGraph` just appends buckets during merge.

```rust,noplayground
{{#rustdoc_include ../../../../crates/ingest/syn_parser/src/parser/graph/parsed_graph.rs:parsed_graph_append_all}}
```

- `CodeGraph` does the same at the lower level.

```rust,noplayground
{{#rustdoc_include ../../../../crates/ingest/syn_parser/src/parser/graph/code_graph.rs:code_graph_append_all}}
```

3. Linking is mostly not method-aware.

- `build_module_tree_from_root_module` copies relations, resolves modules, prunes unlinked modules, and then links definitions to import sites.

```rust,noplayground
{{#rustdoc_include ../../../../crates/ingest/syn_parser/src/parser/graph/parsed_graph.rs:build_module_tree_relations_and_links}}
```

- `ensure_definition_index` only indexes primary items that appear in `module.items()`, and `link_definition_imports` wires imports back to those definitions.

```rust,noplayground
{{#rustdoc_include ../../../../crates/ingest/syn_parser/src/resolve/module_tree.rs:definition_index_primary_nodes}}
```

```rust,noplayground
{{#rustdoc_include ../../../../crates/ingest/syn_parser/src/resolve/module_tree.rs:link_definition_imports}}
```

- Lookups find methods through their parents rather than as standalone primary nodes.

```rust,noplayground
{{#rustdoc_include ../../../../crates/ingest/syn_parser/src/parser/graph/mod.rs:find_methods_in_module_lookup}}
```

```rust,noplayground
{{#rustdoc_include ../../../../crates/ingest/syn_parser/src/parser/graph/mod.rs:find_any_node_lookup}}
```

4. Pruning removes methods only indirectly.

- Methods are excluded from the direct prune count, because they live under impls/traits.

```rust,noplayground
{{#rustdoc_include ../../../../crates/ingest/syn_parser/src/parser/graph/parsed_graph.rs:prune_methods_and_retain}}
```

## Transform (ploke-transform)

MethodNode is handled as an associated-node record, not as a top-level primary node.

- `transform_parsed_graph` routes methods through `transform_traits` and `transform_impls`, then writes relations later.

```rust,noplayground
{{#rustdoc_include ../../../../crates/ingest/ploke-transform/src/transform/mod.rs:transform_parsed_graph_methods}}
```

- `transform_impls` stores each impl method into `MethodNodeSchema` and sets `owner_id` to the impl.

```rust,noplayground
{{#rustdoc_include ../../../../crates/ingest/ploke-transform/src/transform/impls.rs:transform_impls_methods}}
```

- `transform_traits` does the same for trait default methods.

```rust,noplayground
{{#rustdoc_include ../../../../crates/ingest/ploke-transform/src/transform/traits.rs:transform_traits_methods}}
```

- The method schema is `assoc_nodes::MethodNodeSchema`.

```rust,noplayground
{{#rustdoc_include ../../../../crates/ingest/ploke-transform/src/schema/assoc_nodes_multi.rs:method_node_schema}}
```

- The impl/trait-to-method edge is stored as a generic syntax edge.

```rust,noplayground
{{#rustdoc_include ../../../../crates/ingest/ploke-transform/src/schema/edges.rs:impl_trait_associated_edges}}
```

## Embedding

MethodNode is not part of the embedding path.

- `ploke-embed` iterates `NodeType::primary_nodes()` in `next_batch`, so it never asks for method rows.

```rust,noplayground
{{#rustdoc_include ../../../../crates/ingest/ploke-embed/src/indexer/mod.rs:next_batch_primary_nodes}}
```

- `ploke-db`’s embedding helpers are primary-node only too.

```rust,noplayground
{{#rustdoc_include ../../../../crates/ploke-db/src/get_by_id/mod.rs:common_fields_embedded_primary_nodes}}
```

```rust,noplayground
{{#rustdoc_include ../../../../crates/ploke-db/src/database.rs:get_unembedded_node_data_primary_nodes}}
```

```rust,noplayground
{{#rustdoc_include ../../../../crates/ploke-db/src/multi_embedding/db_ext.rs:get_nodes_ordered_for_set_primary_nodes}}
```

```rust,noplayground
{{#rustdoc_include ../../../../crates/ploke-db/src/multi_embedding/db_ext.rs:get_rel_with_cursor_primary_nodes}}
```

- So `MethodNode` rows exist in the DB, but they are not embedded or returned by the standard primary-node hydration paths.

Last refreshed: 2026-03-30

## Notes

TODO: Add key references and links.
