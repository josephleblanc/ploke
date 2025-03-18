
-----

**NOTE: This is a foundational design document currently under review**
This file is speculative and actively being edited as the proposed
structure for the project. It will continue to be edited as we work on the
proposed project structure and does not accurately reflect the current state of
the project.

This is a planning document **only*** and will be archived once a design
decision is chosen. The only part of this project that is at MVP status so far
is the `syn_parser`, which is the parser for the project.

-----

**Here's a refined plan based on your suggestion:**

1.  **Minimal `syn_parser` Rework:** Focus on adapting the existing `syn_parser` to output data directly compatible with CozoDB, *without* fundamentally altering its data flow or concurrency model. This means:
    *   **Type Alignment:** Change the types used within `syn_parser` to align with the CozoDB schema (e.g., using `Bytes` for content hashes, appropriate numeric types for embeddings).
    *   **Send + Sync:** Ensure all publicly exposed types are `Send + Sync`. This is a good practice regardless and will prepare the codebase for future concurrency improvements.
    *   **Remove `CodeGraph` (or significantly reduce its scope):** You're right to question the value of the `CodeGraph` as an intermediary. It adds complexity without necessarily providing significant benefits. We can likely stream data directly from the `syn` AST to CozoDB.
2.  **Separate `graphdb` Crate:** Keep the graph database interaction logic in a separate crate. This promotes modularity and allows us to experiment with different database backends without modifying the parser. The workspace structure already supports this.
3.  **Concurrent Downstream Components:** Focus on building the `ingest`, `context`, and `reasoning` crates with concurrency in mind from the start. Use techniques like `rayon`, `crossbeam`, and asynchronous programming to parallelize data processing and database interactions.
4.  **Iterative Refinement:** Once the core pipeline is working, we can revisit the `syn_parser` and consider more significant concurrency improvements if necessary.

**Addressing your questions:**

*   **Difficulty of keeping `graphdb` separate:** No, there shouldn't be any extra difficulty. They're already in the same workspace, allowing for easy dependency management and code sharing.
*   **Is this a good compromise?** Absolutely. It balances the need for concurrency with the reality of existing tech debt and the risks of a large-scale rewrite.

**Benefits of this approach:**

*   **Faster Time to Market:** We can get a working prototype up and running more quickly.
*   **Reduced Risk:** Minimizes the risk of introducing bugs and instability through a major rewrite.
*   **Clearer Separation of Concerns:** Keeps the parser focused on parsing and the graph database interaction logic in a separate crate.
*   **Flexibility:** Allows us to experiment with different database backends and concurrency models without modifying the parser.

I strongly support this revised plan. It's a pragmatic and sensible way to move forward. Let's focus on making the `syn_parser` a reliable and efficient data source for the rest of the system, and then build the concurrent pipeline on top of that.
