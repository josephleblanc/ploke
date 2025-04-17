Okay, here is a summary of the key points from the CozoDB documentation, focusing specifically on aspects most relevant to using CozoDB as an **embedded, in-memory database within your Rust project (`ploke-db`)**:

**1. Core Interaction Model (Rust API):**

*   **Primary Interface:** Interaction will primarily occur through the Rust API provided by the `cozo` (or `cozo-core`) crate. You'll hold a `Db<MemStorage>` instance.
*   **Executing Queries:** The main method will likely be `db.run_script(script_string, parameters, mutability)`. You'll construct CozoScript queries as strings within your Rust code.
*   **Parameters:** Dynamic values (like specific IDs or names to search for) are passed into scripts via a `BTreeMap<String, DataValue>` (or similar map type provided by the Rust API).
*   **Results:** Query results are returned, typically in a structure like `NamedRows` containing headers (column names) and rows (vectors of `DataValue`). Your Rust code will need to parse these results back into meaningful Ploke structures.
*   **Mutability:** Queries that modify data (using `:put`, `:rm`, `:create`, etc.) require `ScriptMutability::Mutable`. Read-only queries use `ScriptMutability::Immutable`.

**2. Data Modeling and Schema:**

*   **Relations:** Data is organized into relations (conceptually tables). You'll define these relations using `:create` or `:replace` within CozoScript executed via the Rust API.
*   **Schema Definition:** Schemas define column names, types (`Int`, `String`, `Uuid`, `Json`, `List`, etc.), and keys (`{ key1, key2 => value1 }`). This structure is necessary even for the in-memory backend.
*   **Type Mapping:** You need a clear mapping between your Rust types (e.g., `NodeId`, `VisibilityKind`, structs from `syn_parser`) and Cozo's `DataValue` enum variants (`Uuid`, `String`, `Int`, `List`, `Json`, etc.). The `Uuid` type is directly supported. Complex nested Rust structures might be stored as `Json` or serialized strings.
*   **Set Semantics:** Remember Cozo relations enforce set semantics (no duplicate rows).

**3. Data Ingestion (Loading the Graph):**

*   **CozoScript Mutations:** You can generate individual `:put` or `:insert` statements within CozoScript strings for each node or relation. This offers fine-grained control but can be slow for large graphs.
*   **Batch Import API (`import_relations`):** The documentation highlights a non-script API (likely available in the Rust API) for bulk data import (`import_relations`). This takes data structured similarly to the export format (e.g., `HashMap<String, RelationData>`). This is likely the **most efficient way** to load the graph data parsed by `syn_parser` into CozoDB from your Rust structures.
    *   **Important Caveat:** The documentation explicitly warns that **triggers are *not* run** for data loaded via `import_relations`. If you plan to use triggers later, this is a critical consideration.
*   **Upsert vs. Insert:** `:put` provides upsert semantics (replace if key exists), while `:insert` fails if the key exists. Choose based on your loading strategy.

**4. Query Language Features (CozoScript):**

*   **Datalog Foundation:** Leverage Datalog rules (`:=`) for complex joins and graph traversals. Break complex logic into smaller, named rules for clarity and potential performance benefits (due to semi-naïve evaluation and potential parallelism).
*   **Functions & Expressions:** Use built-in functions (`functions.rst`) within your queries for data manipulation, type conversion, filtering, etc.
*   **Graph Algorithms (`Algo.*`):** Access powerful graph algorithms directly using fixed rules (`<~`). You'll construct the input relations (e.g., `edges[from, to]`) using standard Datalog queries on your base data.
*   **Aggregations:** Use aggregations (`count`, `sum`, `min`, `max`, `collect`, etc.) in rule heads (`?[group_key, count(item)] := ...`) for summarizing results. Remember the distinction between semi-lattice (recursive-safe) and ordinary aggregations.

**5. Performance & Execution (In-Memory Context):**

*   **Deterministic Execution:** Cozo aims for predictable performance based on how queries are written.
*   **Semi-Naïve Evaluation & Magic Sets:** These optimizations happen internally to avoid redundant computations, especially in recursive queries.
*   **Atom Ordering:** Filters are applied early. Ordering binding atoms (joins) matters – place more restrictive joins first. Index usage is key.
*   **Indices (`::index`):** Crucial for performance, even in-memory. Define indices on columns frequently used for lookups or joins (e.g., `NodeId`, `name`). Cozo may use them automatically for simple cases, but explicit querying of the index relation (`*my_rel:my_index{...}`) guarantees usage.
*   **RAM Bound:** All data and intermediate results reside in RAM. Performance is CPU/RAM bound. Large intermediate results in complex queries can consume significant memory.

**6. Transactions & Concurrency:**

*   **Implicit Transactions:** Each `run_script` call is typically executed within its own transaction, ensuring atomicity for that script.
*   **Multi-Statement Transactions (Rust API):** The documentation mentions support for explicit multi-statement transactions in the hosting language API (including Rust). This is essential for ensuring atomicity across multiple `run_script` calls (e.g., creating multiple relations, loading data in stages). You'll need to find the specific Rust API calls for `begin_transaction`, `commit`, `rollback`.
*   **Concurrency (In-Memory):** The `MemStorage` backend has limitations on *write* concurrency (often single-writer). Read concurrency is generally better. This is usually acceptable for embedded use cases but important to be aware of if Ploke uses multiple threads interacting with the *same* `Db` instance.

**7. Features Less Relevant for Embedded In-Memory:**

*   **Time Travel:** The `Validity` type and `@` syntax are not needed unless you specifically model history.
*   **Persistence Details:** Specifics of RocksDB (BlobDB) or SQLite storage are irrelevant.
*   **Backup/Restore (File-based):** Less critical, though `import_relations` might conceptually load from an in-memory representation derived from a file.
*   **System Ops:** `::compact` is not applicable.

**Bridging to `cozo-core` Analysis:**

This summary highlights the *documented* features and how they apply conceptually to your Rust/in-memory scenario. When we look at `cozo-core`:

*   We'll look for the concrete Rust types corresponding to `DataValue`, `NamedRows`, etc.
*   We'll identify the exact functions for `run_script`, parameter passing, transaction management (`begin`, `commit`, `rollback`), and the `import_relations` equivalent.
*   We'll see how errors are represented (`cozo::Error` likely).
*   We might find lower-level APIs or configuration options not exposed in the general documentation that could be relevant for embedded use.
*   We can verify how features like indices or algorithms are invoked programmatically.

This summary should serve as a good baseline understanding derived from the general documentation as we transition to analyzing the specific `cozo-core` Rust implementation details.
