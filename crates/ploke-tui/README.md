# Ploke TUI

Ploke TUI is a terminal-based AI assistant designed for software engineers, with a special focus on the Rust programming language. It provides a conversational interface to an LLM that is augmented with context from your local codebase. This allows for more accurate, relevant, and helpful interactions, such as code generation, explanation, and refactoring suggestions.

The application is built with a robust, concurrent architecture to handle background tasks like code parsing and indexing without blocking the user interface, ensuring a smooth and responsive experience.

## Core Concepts

The application is designed around a few central components that manage state, communication, and background processing.

-   **`AppState`**: The single source of truth for all shared application data. It is wrapped in an `Arc` and uses `RwLock` and `Mutex` for thread-safe access from multiple concurrent tasks.
-   **`StateManager`**: An actor that is the sole mutator of `AppState`. It processes `StateCommand`s from a channel, ensuring that all state changes are serialized and atomic. This follows a CQRS-like pattern.
-   **`EventBus`**: A central broadcast system for communicating events between different parts of the application. It uses `tokio::sync::broadcast` channels to decouple components. Events are prioritized into `Realtime` (for UI updates) and `Background` categories.
-   **Actors**: Long-running, independent tasks that handle specific responsibilities, such as `LlmManager` (communicating with LLM APIs), `IndexerTask` (embedding code), `FileManager` (disk I/O), and `ContextManager` (assembling prompts).

## System Architecture

At a high level, the application consists of a responsive frontend TUI that communicates with a set of backend actors via commands and events. These actors, in turn, interact with external systems like the file system, a database, and LLM APIs.

```mermaid
graph TD
    User -- Interacts with --> TUI[TUI App]

    subgraph Frontend
        TUI -- StateCommand --> StateManager
        EventBus -- AppEvent --> TUI
    end

    subgraph Backend Actors
        StateManager -- Manages --> AppState
        StateManager -- Dispatches to --> LlmManager
        StateManager -- Dispatches to --> Indexer
        StateManager -- Dispatches to --> ContextManager
        StateManager -- Dispatches to --> FileManager
    end

    subgraph External Systems
        LlmManager --> LlmApi[LLM API]
        Indexer --> SourceCode[File System]
        Indexer --> Database
        FileManager --> SourceCode
        ContextManager --> Database
    end

    LlmManager -- AppEvent --> EventBus
    Indexer -- AppEvent --> EventBus
    ContextManager -- AppEvent --> EventBus
    FileManager -- AppEvent --> EventBus
    StateManager -- AppEvent --> EventBus

    style AppState fill:#f9f,stroke:#333,stroke-width:2px
```

## Processing Pipelines

Ploke TUI supports several key workflows, each implemented as a data processing pipeline that flows through the system's actors.

### Code Indexing Pipeline

This pipeline is responsible for parsing a Rust workspace, storing its structure in a database, and generating vector embeddings for semantic search. It is typically initiated by the user with the `/index start` command.

```mermaid
sequenceDiagram
    actor User
    participant App
    participant StateManager
    participant Parser
    participant IndexerTask
    participant Database

    User->>App: Enters command: `/index start .`
    App->>StateManager: Sends StateCommand::IndexWorkspace
    StateManager->>Parser: run_parse()
    activate Parser
    Parser->>Database: Writes parsed code graph
    Parser-->>StateManager: Returns Ok
    deactivate Parser
    StateManager->>IndexerTask: Spawns indexing task
    activate IndexerTask
    IndexerTask->>Database: get_unembedded_node_data()
    Database-->>IndexerTask: Returns nodes without embeddings
    IndexerTask->>IndexerTask: Generates embeddings for nodes
    IndexerTask->>Database: update_embeddings_batch()
    Database-->>IndexerTask: Confirms update
    IndexerTask-->>StateManager: Task completes (via EventBus)
    deactivate IndexerTask
    StateManager->>App: Sends AppEvent::IndexingCompleted
```

### Chat and RAG Pipeline

This is the primary user interaction pipeline. It takes a user's chat message, finds relevant code context using Retrieval-Augmented Generation (RAG), sends it to the LLM, and displays the response.

```mermaid
sequenceDiagram
    actor User
    participant App
    participant StateManager
    participant Embedder
    participant Database
    participant ContextManager
    participant LlmManager

    User->>App: Enters chat message
    App->>StateManager: StateCommand::AddUserMessage
    App->>StateManager: StateCommand::EmbedMessage

    StateManager->>StateManager: Adds user message to ChatHistory
    StateManager->>Embedder: generate_embeddings(user_message)
    Embedder-->>StateManager: Returns message embedding

    StateManager->>Database: search_similar(embedding)
    Database-->>StateManager: Returns similar code nodes

    StateManager->>ContextManager: RagEvent::ContextSnippets(nodes)
    StateManager->>ContextManager: RagEvent::UserMessages(history)

    activate ContextManager
    ContextManager->>ContextManager: Constructs final prompt
    ContextManager->>LlmManager: AppEvent::Llm(PromptConstructed)
    deactivate ContextManager

    activate LlmManager
    LlmManager->>LlmManager: Sends prompt to LLM API
    LlmManager-->>LlmManager: Receives LLM response
    LlmManager->>StateManager: StateCommand::UpdateMessage(response)
    deactivate LlmManager

    StateManager->>StateManager: Updates assistant message in ChatHistory
    StateManager->>App: AppEvent::MessageUpdated
    App->>User: Displays LLM response
```

### File-based Query Pipeline

This pipeline allows developers to run raw Datalog queries against the code graph database from a file. This is a powerful debugging and inspection tool.

```mermaid
sequenceDiagram
    actor User
    participant App
    participant StateManager
    participant FileManager
    participant Database

    User->>App: Enters command: `/query load default default.dl`
    App->>StateManager: StateCommand::ReadQuery
    StateManager->>FileManager: AppEvent::System(ReadQuery)
    
    activate FileManager
    FileManager->>FileManager: Reads 'default.dl' from disk
    FileManager->>App: AppEvent::System(WriteQuery)
    deactivate FileManager

    App->>StateManager: StateCommand::WriteQuery
    activate StateManager
    StateManager->>Database: raw_query_mut(query_content)
    Database-->>StateManager: Returns query result
    StateManager->>FileManager: Writes result to 'output.md'
    deactivate StateManager
```

## Getting Started

To use the application, run it from your terminal. You can interact with it using a multi-modal, vim-like interface.

-   Press `i` to enter **Insert Mode** to type your messages.
-   Press `Esc` to return to **Normal Mode** for navigation.
-   In Normal Mode, press `:` to enter **Command Mode** to issue commands like `/index start` or `/model list`.

For a full list of commands and keyboard shortcuts, use the `/help` command.
