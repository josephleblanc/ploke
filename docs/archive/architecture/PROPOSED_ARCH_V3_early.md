# ploke Project Architecture (WIP)
This document serves as the working design document for the `ploke` project workspace.

The ploke project is an RAG for code generation and refactoring. The project
should parse a user's repository and (optionally) dependencies into a hybrid
vector-graph database. The RAG processes user requests for code generation and
refactoring, querying the database for relevant code snippets to include as
context for the augmented query sent to the LLM. 

## File System

You're touching on an important architectural consideration. Let me help clarify the distinction between Tokio and Rayon and suggest how to structure your system.


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
Flume example: [`flume` across boundaries]

1. **Clear boundary between I/O and computation domains**
   - I/O domain: File watching, database operations (Tokio), ..
   - Computation domain: Code parsing, analysis (Rayon)

2. **For your core data structures**:
   - Make them `Send + Sync` but don't tie them to either runtime
   - Use `Arc<RwLock<_>>` from `parking_lot` or standard library (not Tokio's locks)
   - Consider `dashmap` for concurrent hash maps

3. **Processing pipeline architecture**:
   ```
   File Watcher (Tokio) → Parser Coordinator → Parallel Parsing (Rayon) → Database Writer (Tokio)
   ```

4. **Channel-based communication**:
   - Use `tokio::sync::mpsc` or `crossbeam::channel` to communicate between domains
   - This allows clean separation between the async and parallel components

ui -> parser
watcher -> parser

visit -> embed |use flume|
visit -> graph |use flume|

embed -> |write| database
graph -> |write| database

database -> |read| analyze
database <- |write|analyze

llm -> ui
// make mermdai diagram AI!


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
      - code embedding mostly handled by cozo for now, but could add more embedding options later
    - natural language:
      - encoded through interface with embedding llm. Potentially separate from graph database.
  - intermediate data types transformed into suitable form by `graph` using
  CozoScript or through cozo methods.
    - Intermediate types used in parsing (e.g. `TypeKind`, `RelationKind`), not sent directly.
      - Allows more flexibility for tokio inside parser.
    - Sent types are all `Send + Sync` cozo-native types.

4. `database` receives pre-processed data ready for insertion to embedded cozo db
  - enters new code data to database through either cozo methods or CozoScript



### Understanding Tokio vs Rayon

**Understanding the Conflict**

- **Tokio** is an asynchronous runtime for I/O-bound tasks. It excels at handling many concurrent operations that spend time waiting (file I/O, network requests, etc.).

- **Rayon** is designed for CPU-bound parallelism. It provides work-stealing thread pools that efficiently distribute computational work across available cores.

The conflict concerns come from their different concurrency models:
- Tokio uses async/await (non-blocking concurrency)
- Rayon uses threads (parallel execution)

### Divided Architecture

Here's how I structure the system:

1. **Create a clear boundary between I/O and computation domains**
   - I/O domain: File watching, database operations (Tokio)
   - Computation domain: Code parsing, analysis (Rayon)

2. **For your core data structures**:
   - Make them `Send + Sync` but don't tie them to either runtime
   - Use `Arc<RwLock<_>>` from `parking_lot` or standard library (not Tokio's locks)
   - Consider `dashmap` for concurrent hash maps

3. **Processing pipeline architecture**:
   ```
   File Watcher (Tokio) → Parser Coordinator → Parallel Parsing (Rayon) → Database Writer (Tokio)
   ```

4. **Channel-based communication**:
   - Use `tokio::sync::mpsc` or `crossbeam::channel` to communicate between domains
   - This allows clean separation between the async and parallel components


This approach gives you the best of both worlds: Tokio for watching files and database I/O, Rayon for parallel parsing work. The key is creating clear boundaries and using channels to communicate between the different concurrency domains.

Does this approach make sense for your project structure?

[`flume` across boundaries]:/home/brasides/code/second_aider_dir/ploke/docs/design/concurrency/boundary_flume_example.md
