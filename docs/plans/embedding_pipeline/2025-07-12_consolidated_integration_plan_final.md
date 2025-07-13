# Consolidated Integration Plan: Tying TUI, RAG, and DB Together

**Date:** 2025-07-12
**Version:** Final

> "The process of preparing programs for a digital computer is especially attractive, not only because it can be economically and scientifically rewarding, but also because it can be an aesthetic experience much like composing poetry or music." - Donald Knuth

> “In order to scale the mountain of rationality, we must first drive a piton into the raw rock face as we ascend. Only thus may we rest and consider our ascent without falling back into the darkness of insanity.” - J.L. 2025

## 1. Introduction: Finding the Path Forward

This project has reached a significant level of complexity. Core components like the code graph parser (`syn_parser`), the database layer (`ploke-db`), and the IO system (`ploke-io`) are functional but not yet fully integrated. The recent addition of vector embeddings (`ploke-embed`) has introduced a powerful new capability, but also new integration challenges.

The primary goal of this document is to serve as a clear, comprehensive, and authoritative roadmap for a single developer to navigate this complexity. It will guide us from the current state of disparate, maturing components to a cohesive, functional prototype that realizes the project's core vision: a TUI-based, RAG-powered assistant for Rust development.

This plan is the definitive source of truth for this phase of development, superseding all previous plans.

## 2. Core Principles

### 2.1. Documentation
As a developer tool, our documentation is a core feature. All new code should be accompanied by clear, concise, and idiomatic Rust documentation (`///`). Public APIs must be documented. Complex internal logic should have comments explaining the *why*.

### 2.2. Observability & Tracing
To ensure we can debug and monitor our concurrent system effectively, we will use the `tracing` crate. Key asynchronous functions, especially those running in spawned tasks, should be annotated with `#[instrument]` to provide structured, contextual logs.

### 2.3. Error Handling
Errors are expected events, not exceptions. We will use `Result<T, E>` extensively. Errors should be logged with context, and the UI should present them to the user gracefully without crashing. Long-running tasks must be resilient to partial failures.

## 3. Unified Architecture: The Full Picture

To understand how all the pieces fit together, we need a unified view of the system. The following diagram combines the TUI's concurrency model with the data flow for both indexing and RAG-powered chat queries.

```mermaid
flowchart TD
    subgraph UserInteraction["User Interaction (ploke-tui)"]
        direction TB
        UserInput[User Input] -->|Commands & Chat| App(App UI)
        App -->|Render| Terminal((Terminal))
        App <-->|State Reads/Writes| StateManager["State Manager<br/>Arc<Mutex<AppState>>"]
        App -->|Events| EventBus
    end

    subgraph AsyncRuntime["Tokio Runtime (Background Tasks)"]
        direction TB

        subgraph CoreServices["Core Services"]
            direction LR
            StateManager
            EventBus
        end

        subgraph Workers["Async Workers"]
            direction TB
            IndexerTask["Indexer Task"]
            RAGEngine["RAG Engine"]
            LLMManager["LLM Manager"]
            IoManager["I/O Manager"]
        end

        subgraph DataLayer["Data Layer (ploke-db)"]
            direction TB
            PlokeDb[("ploke-db<br/>(CozoDB)")]
        end
    end

    subgraph ExternalSystems["External Systems"]
        direction LR
        FileSystem[["📁 File System"]]
        LLM_API[["🤖 External LLM"]]
    end

    %% --- Connections ---

    %% Indexing Flow
    UserInput -- "/index start" --> App
    App -->|StateCommand::IndexWorkspace| StateManager
    StateManager -->|Spawns| IndexerTask
    IndexerTask -->|get_nodes_for_embedding()| PlokeDb
    PlokeDb -->|Vec<EmbeddingData>| IndexerTask
    IndexerTask -->|IoRequest::ReadSnippetBatch| IoManager
    IoManager -->|Reads Files| FileSystem
    IoManager -->|Vec<Result<String>>| IndexerTask
    IndexerTask -->|generate_embeddings()| RAGEngine
    RAGEngine -->|Vec<Embedding>| IndexerTask
    IndexerTask -->|update_embeddings()| PlokeDb
    PlokeDb -->|Creates HNSW Index| PlokeDb
    IndexerTask -->|IndexingStatus Events| EventBus
    EventBus -->|Updates UI| App

    %% Chat/RAG Flow
    UserInput -- "User Chat Message" --> App
    App -->|StateCommand::AddUserMessage| StateManager
    StateManager -->|Llm::Request Event| EventBus
    EventBus -->|Receives Request| RAGEngine
    RAGEngine -->|generate_embedding(query)| RAGEngine
    RAGEngine -->|query(vector, graph_traversal)| PlokeDb
    PlokeDb -->|Retrieved Nodes| RAGEngine
    RAGEngine -->|IoRequest::ReadSnippetBatch| IoManager
    IoManager -->|Retrieved Snippets| RAGEngine
    RAGEngine -->|Reranks & Builds Context| RAGEngine
    RAGEngine -->|Augmented Prompt| LLMManager
    LLMManager -->|API Call| LLM_API
    LLM_API -->|LLM Response| LLMManager
    LLMManager -->|StateCommand::UpdateMessage| StateManager
    StateManager -->|MessageUpdated Event| EventBus
    EventBus -->|Updates UI| App

    classDef default fill:#2d3748,stroke:#e2e8f0,stroke-width:1px,color:#e2e8f0;
    classDef subgraphStyle fill:#1a202c,stroke:#4a5568,stroke-width:2px;
    class UserInteraction,AsyncRuntime,ExternalSystems subgraphStyle;
```

## 4. Phased Implementation Plan

We will tackle the integration in three distinct, sequential phases. Each phase delivers tangible value and builds the foundation for the next.

### Phase 1: TUI Polish & User Feedback (The "Make it Usable" Phase)

**Goal:** Address immediate UI shortcomings to create a stable and informative user experience. This makes further development and testing much more pleasant.

**Tasks:**

1.  **Implement Informational Messages:**
    *   **Goal:** Provide non-intrusive feedback to the user within the chat window, distinct from LLM prompts and responses.
    *   **Implementation:**
        1.  Refactor the `Message` struct in `app_state.rs`. Instead of overloading `Role`, we will introduce a new enum: `pub enum MessageKind { User, Assistant, SystemInfo(String) }`. The `Message` struct will contain this `MessageKind` instead of separate `role` and `content` fields.
        2.  In the UI rendering logic, create a distinct visual style for `MessageKind::System` (e.g., different color, italicized).
        3.  When the `/index start` command is issued, the `StateManager` will add a `System` message.
        4.  **Workspace Path Handling:** The `/index` command will be updated to accept an optional path argument (e.g., `/index .`). If no path is provided, it will default to the current working directory.
        5.  Ensure the logic that prepares context for the LLM explicitly ignores `MessageKind::System` messages.
    *   **Error Handling:** If reading the current directory for the index command fails, a `System` message will be dispatched to the UI explaining the failure (e.g., "Error: Could not read directory permissions denied").
    *   **Testing:** Verify `System` messages appear with a distinct style and are excluded from LLM context.

2.  **Create Indexing Progress Bar:**
    *   **Goal:** Give the user clear, real-time feedback on the long-running indexing process.
    *   **Analysis:** The backend logic is mostly in place. We just need to connect it to the UI.
    *   **Implementation:**
        1.  The `App`'s main event loop will use `try_recv()` on the `IndexingStatus` broadcast receiver on every tick. This is non-blocking and prevents UI hangs if the channel lags.
        2.  On receiving an `IndexingStatus`, the `App` will update the `AppState.indexing_state`.
        3.  The UI rendering logic will render a `Gauge` based on the `indexing_state`.
    *   **Error Handling:** If the `IndexerTask` panics or sends an `IndexingStatus::Error`, the `indexing_state` will reflect this. The UI will hide the progress bar and can display an error icon or message in its place.
    *   **Testing:** Verify the progress bar appears, updates, and disappears correctly on completion or error.

3.  **Implement Chat Scrollbar & Text Wrapping:**
    *   **Goal:** Improve readability and navigation of long conversations.
    *   **Implementation (Wrapping):**
        1.  Locate the `Paragraph` widget that renders chat message content.
        2.  Configure it with `wrap: Wrap { trim: false }`.
    *   **Implementation (Scrollbar):**
        1.  Add a `ScrollbarState` to the `AppState` for the chat panel.
        2.  In the UI rendering, wrap the chat panel's `Rect` in a `Scrollbar` widget, passing it the `ScrollbarState`.
        3.  Update the input handling logic. When the chat panel is focused, up/down arrow keys (or mouse wheel events) should modify the scroll position in `ScrollbarState` and the vertical scroll of the `Paragraph` widget. This must not conflict with the existing message selection in `Normal` mode. A new `InputMode::ChatScrolling` might be necessary to disambiguate controls.
    *   **Error Handling:** Not applicable for this purely visual task.
    *   **Testing:** Generate a long LLM response. Verify text wraps correctly. Verify the scrollbar appears. Verify you can scroll up and down through the history without changing the selected message in `Normal` mode.

### Phase 2: Activating Vector Search (The "Unlock the RAG" Phase)

**Goal:** Make the stored vector embeddings searchable by creating the necessary database index and exposing a query interface.

**Tasks:**

1.  **Create HNSW Index in `ploke-db`:**
    *   **Goal:** Build the HNSW index in CozoDB to enable fast approximate nearest neighbor search.
    *   **Analysis:** Based on CozoDB documentation, HNSW indexes are not incremental and must be rebuilt after new data is added. Our approach of rebuilding after each full indexing run is correct.
    *   **Implementation:**
        1.  In `ploke-db/src/lib.rs`, create a new public async function `build_vector_index()`.
        2.  Annotate it with `#[instrument(skip_all, err)]` for tracing.
        3.  This function will construct the datalog command: `"::hnsw create embeddings_hnsw ON nodes(embedding) WITH dim=384, ef_construction=200, m=16"`. The parameters should be configurable later, but can be hardcoded for now.
        4.  Execute this command using `db.run()`.
        5.  In `ploke-embed/src/indexer.rs`, after the `update_embeddings` call successfully completes, the `IndexerTask` will call this new `db.build_vector_index()` function.
        6.  Add comprehensive doc comments to the new function.
    *   **Error Handling:** The function will return a `Result`. If the `::hnsw create` command fails, the error will be propagated up to the `IndexerTask`, which will log it and send an `IndexingStatus::Error` event to the UI.
    *   **Testing:** After a successful `/index` run, connect to the CozoDB instance directly. Manually run a query to verify the `embeddings_hnsw` index exists and is populated.

2.  **Expose a Vector Search Query:**
    *   **Goal:** Create a simple, high-level API in `ploke-db` for performing semantic search.
    *   **Implementation:**
        1.  In `ploke-db/src/lib.rs`, create a new public async function `search_similar_nodes(embedding: Vec<f32>, k: usize, distance_threshold: f32) -> Result<Vec<(NodeId, f32)>>`.
        2.  Add the `distance_threshold` to the query to filter out low-similarity results at the DB level.
        3.  Annotate with `#[instrument(skip(embedding), err)]`.
    *   **Error Handling:** If the Cozo query fails, the error is returned. The `RAGEngine` must handle the `Err` case, likely by logging the error and proceeding to the LLM without augmented context.
    *   **Testing:** Create a new integration test in `ploke-db`. The test will: insert a few nodes with known embeddings, build the index, and then call `search_similar_nodes` with a test vector. Assert that the returned nodes and distances are correct and in the expected order.

### Phase 3: Implementing the RAG Pipeline (The "Bring it to Life" Phase)

**Goal:** Build out the currently stubbed `ploke-rag` crate to perform a full, end-to-end retrieval-augmented generation cycle.

**Tasks:**

1.  **Orchestrate the RAG Flow:**
    *   **Goal:** Implement the core logic loop within the `RAGEngine`.
    *   **Implementation:**
        1.  The `RAGEngine`'s main loop will listen for `Llm::Request` events on the `EventBus`.
        2.  Upon receiving a request, it will spawn a new Tokio task to handle the RAG pipeline for that single request, to avoid blocking the main `RAGEngine` loop. This task's main function will be annotated with `#[instrument(skip_all, fields(message_id = %request.id))]`.
    *   **Error Handling:** The spawned task will be wrapped in an `instrument` span. If any step in the pipeline fails, the task will log the error at its specific step, and then fall back to sending the original, un-augmented prompt to the `LLMManager`. It can also dispatch a `System` message to the UI (e.g., "Context retrieval failed.").

2.  **The RAG Pipeline Steps (within the spawned task):**
    *   **Step 1: Generate Query Embedding:** The task will use the embedding model (from `ploke-embed`) to convert the user's chat message into a query vector.
    *   **Step 2: Query the Database:** Call `ploke_db::search_similar_nodes` with the query vector to get the top `k` semantically similar nodes.
    *   **Step 3: Retrieve Code Snippets:** Take the `Vec<NodeId>` from the DB. For each ID, query `ploke-db` to get its file path and byte offsets. Send a single `IoRequest::ReadSnippetBatch` to the `IoManager` to get the code snippets.
    *   **Step 4: Rerank and Build Context:**
        *   **MVP Reranking:** For the MVP, we will use a simple strategy: the order returned by the vector search is sufficient. No complex reranking is needed yet.
        *   **Context Formatting:** Format the retrieved snippets into a single string. Each snippet should be clearly demarcated, e.g.:
            ```
            --- File: src/main.rs ---
            fn main() {
                println!("Hello, world!");
            }
            ```
    *   **Step 5: Augment and Delegate:** Create the final augmented prompt by prepending the context block to the user's original message. Send this prompt to the `LLMManager`.

3.  **Testing:**
    *   Create a suite of integration tests for the `ploke-rag` crate.
    *   Mock the `ploke-db` and `ploke-io` dependencies.
    *   **Test 1 (Unit):** Test the prompt formatting logic.
    *   **Test 2 (Integration):** Test the full pipeline. Provide a user message, mock the DB/IO responses, and assert that the final augmented prompt sent to the (mocked) `LLMManager` is correctly formatted and contains the expected context.
    *   **Test 3 (Failure):** Test multiple failure paths (e.g., DB error, IO error) to ensure the fallback logic works correctly and the original prompt is sent.

## 5. Open Questions & Design Decisions

1.  **Error Handling for Partial Indexing:**
    *   **Decision:** Log the error, send a `System` message to the UI, and continue with the rest of the batch. The `IndexerTask` will report a final `IndexingStatus::Completed` or `IndexingStatus::Error` with details. This is sufficient for the MVP.
2.  **Vector Index Management:**
    *   **Decision:** Completely rebuild the index at the end of every `/index start` run. As confirmed by CozoDB documentation, this is the required approach as indexes are not incremental.
3.  **Frame Budget & Performance:**
    *   **Decision:** Defer deep optimization. However, we will implement a simple, toggleable FPS counter. This can be a small widget in the corner of the UI, enabled via a config setting or hotkey, to provide a baseline for future performance work.
4.  **Persistence Triggers:**
    *   **Decision:** Defer. This is out of scope for this plan.

## 6. Next Steps (Post-Plan)

Once this plan is complete, the following areas will be the next logical focus:

1.  **Advanced Reranking:** Implement more sophisticated reranking algorithms like Maximal Marginal Relevance (MMR) to improve context diversity.
2.  **Graph-Aware RAG:** Enhance the DB query to not only fetch semantically similar nodes but also traverse the code graph to include syntactically related context (e.g., function definitions called by the retrieved code).
3.  **UI/UX Refinements:** Add features like syntax highlighting for code blocks, conversation branching, and a more robust command palette.
4.  **Remote API for Embeddings:** Abstract the embedding generation to allow for using remote APIs instead of a local model, for users with different hardware constraints.
