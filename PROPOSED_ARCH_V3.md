# ploke Project Architecture (WIP)

 ## 1. Project Vision
 <!-- TODO: Add a clear, concise statement of what ploke aims to achieve, its core 
 value proposition, and target users. -->

 ## 2. Overview
 This document serves as the working design document for the `ploke` project
 workspace.

 The ploke project is an RAG for code generation and refactoring. The project
 should parse a user's repository and (optionally) dependencies into a hybrid
 vector-graph database. The RAG processes user requests for code generation and
 refactoring, querying the database for relevant code snippets to include as
 context for the augmented query sent to the LLM.

 ## 3. Core Concepts
 <!-- TODO: Add definitions of key concepts and components (RAG, vector-graph
 database, etc.) to ensure all team members share the same understanding. -->

 ## 4. System Architecture

 ### 4.1 File System Structure

 ```
 ploke/
 ├── Cargo.toml             # Workspace configuration
 ├── crates/
 │   ├── core/         󱃜    # Core types and traits (NodeId ..)
 │   ├── error/        󱃜    # Cross-crate error types
 │   ├── ingest/            # Core processing Pipeline
 │   │   ├── parser/   🚀   # core traversal + parsing logic....syn
 │   │   ├── lsp/      💤   # LSP data processing
 │   │   ├── embed/    💤   # Vector embeddings.................cozo
 │   │   └── graph/    💤   # AST ➔ CozoDB transformations......cozo
 │   ├── io/           💤   # Input/Output pipeline
 │   │   ├── watcher/  💤   # watches for events (ide, file, lsp)
 │   │   └── writer/   💤   # write code, message ide, commands
 │   ├── database/     💤   # Query processing & ranking........cozo
 │   ├── context/      💤   # aggregate data for llm
 │   ├── llm/          💤   # Local LLM integration
 │   ├── ui/           💤   # CLI/GUI entrypoints...............egui
 │   └── analyze/      🚀   # Static analysis of parsed data
 ├── examples/              # Documentation examples
 └── benches/               # Performance benchmarks

 💤 Asynchronous (tokio)
 🚀 Multithreaded (rayon)
 🚀 <--> flume <--> 💤
 󱃜  Send + Sync (Not tied to tokio or rayon runtime)
 ```

 ### 4.2 Data Flow Diagrams
 <!-- TODO: Add formal data flow diagrams showing how information moves through th 
 system -->

 Current flow notes:
 ```
 ui -> parser
 watcher -> parser

 visit -> embed |use flume|
 visit -> graph |use flume|

 embed -> |write| database
 graph -> |write| database

 database -> |read| analyze
 database <- |write|analyze

 llm -> ui
 ```

 ### 4.3 Processing Pipeline

 1. `watcher` notices change that requires parsing:
   - file changes
   - user input
   - ide event

 2. `watcher` calls (messages?) `injest`

 3. `injest` processes data/file
   - `parser` handles rust source files
   - `lsp` processes lsp messages
   - `embed` handles:
     - code:
       - snippets obtained form parser go to `embed` for further processing
       if necessary, e.g. text pre-processing.
       - code embedding mostly handled by cozo for now, but could add more embeddi 
 options later
     - natural language:
       - encoded through interface with embedding llm. Potentially separate from
 graph database.
   - intermediate data types transformed into suitable form by `graph` using
   CozoScript or through cozo methods.
     - Intermediate types used in parsing (e.g. `TypeKind`, `RelationKind`), not
 sent directly.
       - Allows more flexibility for tokio inside parser.
     - Sent types are all `Send + Sync` cozo-native types.

 4. `database` receives pre-processed data ready for insertion to embedded cozo db 
   - enters new code data to database through either cozo methods or CozoScript

 ## 5. Component Details
 <!-- TODO: For each component, provide specific responsibilities, key algorithms
 techniques, external dependencies, and performance expectations -->

 ## 6. API Contracts
 <!-- TODO: Define the expected inputs, outputs, and error conditions for each
 module boundary -->

 ## 7. Concurrency Model

 ### Understanding Tokio vs Rayon

 **Understanding the Conflict**

 - **Tokio** is an asynchronous runtime for I/O-bound tasks. It excels at handling 
 many concurrent operations that spend time waiting (file I/O, network requests,
 etc.).

 - **Rayon** is designed for CPU-bound parallelism. It provides work-stealing thre 
 pools that efficiently distribute computational work across available cores.

 The conflict concerns come from their different concurrency models:
 - Tokio uses async/await (non-blocking concurrency)
 - Rayon uses threads (parallel execution)

 ### Divided Architecture

 Here's how we structure the system:

 1. **Clear boundary between I/O and computation domains**
    - I/O domain: File watching, database operations (Tokio), ..
    - Computation domain: Code parsing, analysis (Rayon)

 2. **For core data structures**:
    - Make them `Send + Sync` but don't tie them to either runtime
    - Use `Arc<RwLock<_>>` from `parking_lot` or standard library (not Tokio's
 locks)
    - Consider `dashmap` for concurrent hash maps

 3. **Processing pipeline architecture**:
    ```
    File Watcher (Tokio) → Parser Coordinator → Parallel Parsing (Rayon) → Databas 
 Writer (Tokio)
    ```

 4. **Channel-based communication**:
    - Use `flume` to communicate between domains
    - This allows clean separation between the async and parallel components

 Flume example: [`flume` across boundaries]

 ## 8. Cross-Cutting Concerns

 ### 8.1 Error Handling Strategy
 <!-- TODO: Document the project-wide approach to error handling -->

 ### 8.2 Logging and Observability
 <!-- TODO: Define logging standards and observability mechanisms -->

 ### 8.3 Testing Approach
 <!-- TODO: Outline the testing strategy for different components -->

 ### 8.4 Performance Considerations
 <!-- TODO: Document performance goals and optimization strategies -->

 ## 9. Decision Records
 <!-- TODO: Document key architectural decisions, alternatives considered, and
 rationale for choices made -->

 ## 10. Development Roadmap
 <!-- TODO: Include prioritization of components and a phased implementation plan
 -->

 ## 11. Examples
 <!-- TODO: Add concrete examples of how the system would process typical user
 requests from end to end -->

 [`flume` across
 boundaries]:/home/brasides/code/second_aider_dir/ploke/docs/design/concurrency/bo 
 dary_flume_example.md
