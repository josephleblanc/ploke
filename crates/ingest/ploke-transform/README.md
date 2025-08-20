# ploke-transform: The Graph Transformation Layer

## Purpose and Role in the ploke Project

The `ploke-transform` crate serves as the critical bridge between the parsed Rust code representation (`syn_parser`) and the hybrid vector-graph database (`ploke-db`). Its primary responsibilities are:

1.  **Schema Definition**: Defining the CozoDB schema that represents Rust code semantics. This is primarily handled by the `define_schema!` macro.
2.  **Data Transformation**: Converting parsed AST nodes from `syn_parser` into database relations. Each node type has a corresponding `transform_*` function.
3.  **Type System Mapping**: Preserving Rust's rich type system in the database by mapping `syn_parser::TypeNode` and `syn_parser::TypeKind` to a set of detailed type relations.
4.  **Relationship Mapping**: Translating syntactic relationships (e.g., containment, imports) into explicit `syntax_edge` relations in the graph.

`ploke-transform` operates in the "Graph Transformer" stage, receiving a `ParsedCodeGraph` from `syn_parser` and populating a `cozo` database instance, making the code structure queryable for `ploke-db`.

## Core Concepts

### Schema Definition with `define_schema!`

The entire database schema is defined using the `define_schema!` macro found in `src/schema/mod.rs`. This macro provides a single source of truth for each relation's structure.

```rust
// Example from src/schema/primary_nodes.rs
define_schema!(FunctionNodeSchema {
    "function",
    id: "Uuid",
    name: "String",
    docstring: "String?",
    vis_kind: "String",
    vis_path: "[String]?",
    span: "[Int; 2]",
    tracking_hash: "Uuid",
    cfgs: "[String]",
    return_type_id: "Uuid?",
    body: "String?",
    module_id: "Uuid",
    embedding: "<F32; 384>?"
});
```

This macro generates:
- A struct (`FunctionNodeSchema`) holding the relation name and field definitions.
- Accessor methods for each field name (e.g., `schema.id()`, `schema.name()`).
- `script_create()`: Generates the CozoScript `CREATE` statement.
- `script_put()`: Generates a CozoScript `PUT` statement for inserting data.
- `create_and_insert_schema()`: A helper to run the creation script against a DB instance.

### Data Transformation

The `transform` module contains functions like `transform_functions`, `transform_structs`, etc. The main entry point is `transform_parsed_graph`, which orchestrates the transformation of an entire `ParsedCodeGraph`.

These functions iterate over the nodes provided by `syn_parser`, convert them into a `BTreeMap<String, DataValue>` matching the schema, and execute the `put` script.

### The `CommonFields` Trait

To standardize the conversion of node data, the `CommonFields` trait (in `src/macro_traits.rs`) is implemented for all primary node types via the `common_fields!` macro. It provides methods to extract common data points like ID, name, documentation, span, and visibility.

```rust
pub trait CommonFields
where
    Self: HasAnyNodeId,
{
    fn cozo_id(&self) -> DataValue;
    fn cozo_name(&self) -> DataValue;
    // ... and others
    fn cozo_btree(&self) -> BTreeMap<String, DataValue>;
}
```
The `cozo_btree()` method is the primary mechanism for preparing a node's common fields for database insertion.

## Schema Overview

The database schema is organized into several categories of relations:

-   **Primary Nodes**: Core code items like `function`, `struct`, `enum`, `trait`, `impl`, `module`. These are defined in `src/schema/primary_nodes.rs`.
-   **Secondary Nodes**: Items that are part of a primary node, such as `field` (for structs/unions), `variant` (for enums), `param` (for functions), and `attribute`. Defined in `src/schema/secondary_nodes.rs`.
-   **Associated Nodes**: Items defined within `impl` or `trait` blocks, like `method`. Defined in `src/schema/assoc_nodes.rs`.
-   **Type Nodes**: A detailed breakdown of Rust's type system. Relations like `named_type`, `reference_type`, `slice_type`, etc., capture the structure of each `TypeNode`. Defined in `src/schema/types.rs`.
-   **Edge Relations**: The `syntax_edge` relation captures direct syntactic relationships between nodes (e.g., `Contains`, `ResolvesToDefinition`). Defined in `src/schema/edges.rs`.
-   **Metadata**:
    -   `crate_context`: Stores metadata about the crate being processed (name, version, namespace).
    -   `bm25_doc_meta`: Stores metadata for the BM25 full-text search index.

### Visibility and Spans

-   **Visibility**: is not stored in a separate table. Instead, each primary node relation includes `vis_kind: String` and `vis_path: [String]?` fields, directly matching Rust's visibility model.
-   **Spans**: are stored as a `[Int; 2]` list, representing the start and end byte offsets of the node in its source file.

## Usage

To use this crate, you typically perform two main steps:

1.  **Initialize Schema**: Call `ploke_transform::schema::create_schema_all(&db)` once on a new database instance. This creates all necessary tables and indices.
2.  **Transform Data**: After parsing code with `syn_parser`, call `ploke_transform::transform::transform_parsed_graph(&db, parsed_graph, &tree)` to populate the database.
