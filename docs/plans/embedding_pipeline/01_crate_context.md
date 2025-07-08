## Immediate next steps

I think I'm realizing why these tests are failing. It is a problem of the crate namespace. For testing we have been using the default crate namespace, but that isn't what is actually being used when we generate the crate namespace. We have been using a `PROJECT_NAMESPACE_UUID` (defined in `ploke-core`) in the `ploke-io` crate, but that isn't actually how the namespace is derived in `derive_crate_namespace` in `discovery.rs`.

We actually aren't storing the `CrateContext` anywhere in the graph right now. We are already parsing this information within `syn_parser`, so we don't need to add anything more to the parsing information, but we will need to add a schema to the `ploke-transform` crate, along with a new method that will be similar to the other `transform_*` methods, and turn the file context into a node. We probably also will want to add an edge between the file context and all of the other nodes that are being added into the database that come from that crate. This edge will be defined in `syn_parser` in `relations.rs`.

Then we will need to update the database query for the snippets in `ploke-db` to retrieve the crate context, probably separately from the other items since it has a very different structure from the rest of the nodes we are handling. Then, when we need to do read the snippets from the files in `ploke-io`, we will want to have a way to incorporate that into the crate namespace. Maybe we actually just compute the crate namespace once for the whole batch? But that could be a problem later when we are handling multiple crates in the same workspace. We can punt on that for now though. For now we can go with handling a single crate correctly. So we will need to make sure we have a way to calculate the crate namespace from either within the query for the next group of nodes or shortly afterwards, and then maybe we can add the crate namespace to each `EmbeddingData` for now, since the crate namespace is `Copy`.

Finally we will want to make sure we update our `process_file` in `ploke-io` to use the new namespace, as well as make sure the data is being passed correctly through `handle_read_snippet_batch`, `handle_request`, `run`, and make sure to add documentation on this updated change so we don't forget in the future.

Below I am including an example of what I would like the plan you generate to look like:

## Generated Plan (Revised)

This plan has been revised to keep the `CodeGraph` as a pure syntactic representation. The `ploke-transform` crate will be responsible for handling the `CrateContext` metadata as a special case, creating its database entry and the link to the syntactic graph.

### Files to Read

- `crates/ingest/syn_parser/src/discovery.rs`: To understand the structure of `CrateContext`.
- `crates/ploke-core/src/lib.rs`: To see the definition of `EmbeddingData` and `PROJECT_NAMESPACE_UUID`.
- `crates/ingest/ploke-transform/src/schema/mod.rs`: To understand how schemas are defined and registered.
- `crates/ingest/ploke-transform/src/transform/mod.rs`: To see how different nodes are transformed and integrated.
- `crates/ingest/syn_parser/src/parser/graph.rs`: To understand the `ParsedCodeGraph` struct which contains both `CrateContext` and `CodeGraph`.
- `crates/ploke-db/src/database.rs`: To find the query (`get_unembedded_data_single`) that needs updating.
- `crates/ploke-io/src/lib.rs`: To understand how `EmbeddingData` is used and where the `TrackingHash` is generated (`process_file`).

### Files Requiring Changes

- `crates/ingest/ploke-transform/src/schema/crate_node.rs` (new file)
- `crates/ingest/ploke-transform/src/schema/edges.rs` (for the new `CrateContainsModule` edge schema)
- `crates/ingest/ploke-transform/src/schema/mod.rs`
- `crates/ingest/ploke-transform/src/transform/crate_context.rs` (new file)
- `crates/ingest/ploke-transform/src/transform/mod.rs`
- `crates/ploke-core/src/lib.rs`
- `crates/ploke-db/src/database.rs`
- `crates/ploke-io/src/lib.rs`

---

### Step 1: Define Database Schema for Crate Context and its Relation

This step focuses on creating the necessary Cozo schemas to store the crate's metadata and the edge linking it to its modules.

**Crates to modify:** `ploke-transform`

**Files to create/modify:**
- `crates/ingest/ploke-transform/src/schema/crate_node.rs`: (New file) Define the `CrateNodeSchema` for Cozo.
- `crates/ingest/ploke-transform/src/schema/edges.rs`: Add a `CrateContainsModuleSchema`.
- `crates/ingest/ploke-transform/src/schema/mod.rs`: Register the new schemas.

**Plan:**
1.  **In `schema/crate_node.rs`:**
    -   Create a new file and use the `define_schema!` macro to define `CrateNodeSchema`.
    -   The schema should include fields like `id: "Uuid"`, `name: "String"`, `version: "String"`, and `namespace: "Uuid"`.
2.  **In `schema/edges.rs`:**
    -   Add a new schema definition for the edge: `define_schema!(CrateContainsModuleSchema { "crate_contains_module", source: "Uuid", target: "Uuid" });`
3.  **In `schema/mod.rs`:**
    -   Add `pub mod crate_node;`.
    -   In `create_schema_all`, add calls to `CrateNodeSchema::create_and_insert_schema(db)?;` and `CrateContainsModuleSchema::create_and_insert_schema(db)?;`.

---

### Step 2: Create Transformation Logic for Crate Context

This step implements the logic to take the `CrateContext` from a `ParsedCodeGraph` and write it, along with its connecting edge, to the database.

**Crates to modify:** `ploke-transform`

**Files to create/modify:**
- `crates/ingest/ploke-transform/src/transform/crate_context.rs`: (New file) Create the function to transform `CrateContext`.
- `crates/ingest/ploke-transform/src/transform/mod.rs`: Integrate the new transformation function.

**Plan:**
1.  **In `transform/crate_context.rs`:**
    -   Create a new file with a function `pub(super) fn transform_crate_context(db: &Db<MemStorage>, parsed_graph: &ParsedCodeGraph) -> Result<(), TransformError>`.
    -   Inside this function:
        a.  Generate a stable `crate_id` (a `Uuid`) based on the `crate_context.namespace`.
        b.  Create a `BTreeMap` of parameters for the `CrateNode` and use `CrateNodeSchema` to write it to the database.
        c.  Find the `ModuleNodeId` of the root module in the `parsed_graph.graph.modules` (the one with path `["crate"]`).
        d.  Create a `BTreeMap` for the `CrateContainsModule` relation, linking the `crate_id` to the root `ModuleNodeId`.
        e.  Use `CrateContainsModuleSchema` to write the edge to the database.
2.  **In `transform/mod.rs`:**
    -   In `transform_code_graph`, change its signature to accept `parsed_graph: ParsedCodeGraph` instead of just `code_graph: CodeGraph`.
    -   At the beginning of the function, call `transform_crate_context(db, &parsed_graph)?;`.
    -   Update all calls to `transform_code_graph` throughout the file to pass the relevant parts of `parsed_graph`.
3.  **Testing:**
    -   Add a unit test in `transform/crate_context.rs` to verify the transformation logic.
    -   Update the integration test in `transform/mod.rs` (`test_insert_all`) to pass the full `ParsedCodeGraph` and ensure the new schemas and transforms work correctly.

---

### Step 3: Update Database Query to Fetch Crate Namespace

Now, we'll modify the database query to retrieve the newly stored crate namespace along with the other data needed for embeddings.

**Crates to modify:** `ploke-core`, `ploke-db`

**Files to modify:**
- `crates/ploke-core/src/lib.rs`: Update `EmbeddingData` struct.
- `crates/ploke-db/src/database.rs`: Modify the Cozo query.

**Plan:**
1.  **In `ploke-core/src/lib.rs`:**
    -   Add `pub crate_namespace: Uuid,` to the `EmbeddingData` struct.
2.  **In `ploke-db/src/database.rs`:**
    -   Locate the `get_unembedded_data_single` function.
    -   Modify the CozoScript query inside it.
    -   The query already finds the root module ancestor. Add a step to join from the root module to the `crate` relation using the `crate_contains_module` edge.
    -   The query should look something like this (conceptual):
        ```datalog
        // ... existing query to find root_module ...
        
        // Add this part
        has_crate[mod_id, crate_ns] := *crate_contains_module{source: crate_id, target: mod_id}, *crate{id: crate_id, namespace: crate_ns}

        // Modify the final projection
        batch[id, name, file_path, hash, span, crate_ns] := 
            needs_embedding[id, name, hash, span],
            ancestor[id, mod_id],
            is_root_module[mod_id],
            *file_mod { owner_id: mod_id, file_path },
            has_crate[mod_id, crate_ns] // Join with the new relation

        // Update the returned columns
        ?[id, name, file_path, hash, span, crate_ns] := ...
        ```
    -   Update the `QueryResult::to_embedding_nodes` method (or wherever the conversion happens) to extract the `crate_ns` from the query result and populate the new `crate_namespace` field in `EmbeddingData`.

---

### Step 4: Integrate Crate Namespace into I/O Pipeline

Finally, use the fetched `crate_namespace` in the I/O actor to ensure file content hashes are generated correctly.

**Crates to modify:** `ploke-io`

**Files to modify:**
- `crates/ploke-io/src/lib.rs`: Update the `process_file` function.

**Plan:**
1.  **In `ploke-io/src/lib.rs`:**
    -   Locate the `process_file` function.
    -   Find the line where `TrackingHash::generate` is called:
        ```rust
        let actual_tracking_hash = TrackingHash::generate(
            ploke_core::PROJECT_NAMESPACE_UUID, // <-- THIS IS THE LINE TO CHANGE
            &file_path,
            &file_tokens,
        );
        ```
    -   The `requests` vector passed into this function now contains `EmbeddingData` with the correct `crate_namespace`.
    -   Change the call to use the namespace from the request:
        ```rust
        // All requests in this function are for the same file, so they share the same context.
        let expected_namespace = requests[0].request.crate_namespace;
        let actual_tracking_hash = TrackingHash::generate(
            expected_namespace, // <-- Use the correct namespace
            &file_path,
            &file_tokens,
        );
        ```
    -   Verify that the `file_tracking_hash` in the request now matches this `actual_tracking_hash`.

---

### Step 5: Final Verification and Documentation

1.  **Run All Tests:** Execute the full test suite for the workspace (`cargo test --workspace`) to ensure the changes haven't introduced regressions. Pay close attention to tests in `ploke-io` and `ploke-db`.
2.  **Update Documentation:**
    -   Add doc comments to the new `CrateNodeSchema` and the `transform_crate_context` function.
    -   Update the documentation for `EmbeddingData` to explain the `crate_namespace` field.
    -   Add a comment to the Cozo query in `ploke-db` explaining the new join logic.
3.  **Manual Check:** If possible, run the embedding pipeline on a test crate and verify that the correct namespaces are being used and that embeddings are generated successfully.
