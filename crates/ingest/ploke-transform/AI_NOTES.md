# ploke-transform AI Developer Notes

This document provides a dense, technical overview of the `ploke-transform` crate for AI-assisted development.

## 1. Core Functionality

`ploke-transform` converts a `syn_parser::ParsedCodeGraph` into a set of relations within a `cozo` in-memory database. It defines the database schema and handles the data transformation logic.

## 2. Schema: The `define_schema!` Macro

The single source of truth for the database schema is the `define_schema!` macro in `src/schema/mod.rs`.

-   **Usage**: `define_schema!(SchemaStructName { "relation_name", field1: "CozoType", ... });`
-   **Generates**:
    -   `SchemaStructName::SCHEMA`: A static instance of the schema definition.
    -   `schema.script_create() -> String`: Returns the CozoScript to create the relation.
    -   `schema.script_put(&params) -> String`: Returns the CozoScript to insert data.
    -   `SchemaStructName::create_and_insert_schema(db)`: A helper to create the relation in the DB.
-   **Key Convention**: Fields listed in `ID_KEYWORDS` (`id`, `owner_id`, etc.) are treated as primary/foreign keys. The generated `script_create` places them before the `=>` in a CozoDB relation definition.

## 3. Transformation Pipeline

1.  **Entry Point**: `transform::transform_parsed_graph(&db, parsed_graph, &tree)` is the main function.
2.  **Orchestration**: It calls specialized `transform_*` functions for each node type (e.g., `transform_functions`, `transform_structs`).
3.  **Node Processing**:
    -   Each `transform_*` function iterates through a `Vec` of nodes from the `ParsedCodeGraph`.
    -   It uses the `CommonFields` trait (via `cozo_btree()`) to get a base `BTreeMap` of common fields.
    -   It adds node-specific fields to the `BTreeMap`.
    -   It calls `schema.script_put()` to generate the insertion script.
    -   It runs the script against the database using `db.run_script()`.

## 4. Key Data Structures and Traits

-   **`macro_traits::CommonFields`**: Implemented for all primary nodes via the `common_fields!` macro.
    -   Provides `cozo_btree()` which returns a `BTreeMap<String, DataValue>` of fields common to all nodes (ID, name, span, visibility, etc.). This is the standard way to begin transforming a node for DB insertion.
-   **`schema` module**: Contains all schema definitions, categorized by node type.
    -   `primary_nodes`: `function`, `struct`, `enum`, etc.
    -   `secondary_nodes`: `field`, `param`, `variant`, `attribute`.
    -   `assoc_nodes`: `method`.
    -   `types`: `named_type`, `reference_type`, etc.
    -   `edges`: `syntax_edge`.
    -   `meta`: `crate_context`, `bm25_doc_meta`.

## 5. Important File Locations

-   **All Schemas**: `src/schema/` directory. The `mod.rs` file contains the `define_schema!` macro and the `create_schema_all` function.
-   **Transformation Logic**: `src/transform/` directory. The `mod.rs` file contains the main `transform_parsed_graph` entry point.
-   **Standardized Field Access**: `src/macro_traits.rs` contains the `CommonFields` trait and `common_fields!` macro.

## 6. IDs and Data Types

-   All node identifiers (`id`, `owner_id`, `type_id`, etc.) are `Uuid`s, represented as `DataValue::Uuid`.
-   `tracking_hash` is also a `Uuid`, used for incremental processing and change detection.
-   Spans are `[Int; 2]`.
-   Visibility is stored in two fields on each primary node's relation: `vis_kind: String` and `vis_path: [String]?`.

## 7. How to Interact with the Schema from Other Crates

-   To initialize a database, call `ploke_transform::schema::create_schema_all(&db)`.
-   When constructing queries in other crates (like `ploke-db`), refer to the `define_schema!` invocations in `ploke-transform/src/schema/` to know the exact relation and field names.
-   For example, to query functions, you will query the `function` relation. The fields available are defined in `FunctionNodeSchema`.
