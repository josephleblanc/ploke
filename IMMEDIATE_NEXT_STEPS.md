## IMMEDIATE_NEXT_STEPS

A document to set and track immediate goals for both human and AI pair programmer.

-----

### **TASKS:**

- [ ] test new CI/CD pipeline
- [ ] **Prioritize Detailed Type System Design:** Define how concrete types will be represented and managed in the `CodeGraph`.
- [ ] **Investigate `crossbeam`:** Evaluate `crossbeam` for implementing the producer-consumer pattern and replacing some of the `std::sync` primitives.
- [ ] **Define a High-Level Error Taxonomy:** Establish a clear error taxonomy before diving into implementation.
- [ ] **Defer Lock-Free Structures:** Defer the decision on lock-free data structures until performance profiling reveals contention as a bottleneck.
- [ ] **Detailed Design of Producer-Consumer:** Flesh out the design of the producer-consumer pattern, including data flow, error handling, and resource management.
- [ ] **Apply patch message to core_design_document.**
    - We are currently on a branch I made specifically to nail down a core design document to facilitate AI assistance.
    - I should have a workable version of the core design before closing branch
    - It will help avoid confusion with AI assistance to have a document I can quickly reference with project design.
      - For now, add a section that LOUDLY says the `syn_parser` is undergoing active revision.
      - Later we can stabilize the design doc V2 with what we've covered, and it can stay stable for a while.


-----


### REASONING:

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
