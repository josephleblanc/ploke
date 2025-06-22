# ploke-tui

## Message Event Flow
```mermaid
flowchart TD
    %% Grouping the main components side-by-side
    subgraph System Architecture
        direction LR

        %% UI Thread remains on the left
        subgraph MainThread["Main Thread (UI Rendering)"]
            direction TB
            A[Input Poller] -->|Key/Mouse Events| B[Event Transformer]
            B -->|UI Events| C(EventBus)
            C -->|Triggers Render| D[TUI Renderer]
            D -->|Reads| G[AppState Snapshot]
            D -->|Frame Updates| H((Terminal))
        end

        %% All async workers and services on the right
        subgraph TokioRuntime["Tokio Runtime (Async Workers)"]
            direction TB

            %% Central Hub for shared services
            subgraph CoreServices["Core Services Hub"]
                direction LR
                C --> StateManager["State Manager <br> Arc&ltMutex&ltAppState>>"]
                C --> Concurrency["Concurrency Primitives"]
            end

            %% Worker modules that perform specific tasks
            subgraph WorkerModules["Async Worker Modules"]
                direction TB
                LLMManager["LLM Manager"]
                RAGEngine["RAG Engine"]
                FileManager["File Manager"]
                AgentSystem["Agent System<br>[Placeholder]"]
            end

            %% Define connections from the hub to the workers
            C -->|Events| LLMManager
            C -->|Events| RAGEngine
            C -->|Events| FileManager
            C -->|Events| AgentSystem

            StateManager -->|Provides State To| AgentSystem
            StateManager <-->|Reads/Writes<br>via Arc&ltMutex>| LLMManager
            StateManager -->|Provides Snapshot| G
            
            
            %% Define connections between workers
            RAGEngine -->|Provides Context| LLMManager
            FileManager -->|File Updates| RAGEngine
        end
    end

    %% Define connections to external systems for clarity
    subgraph ExternalSystems["External Systems & I/O"]
        direction LR
        style ExternalSystems fill:#f0f0f0,stroke:#ccc,stroke-dasharray: 5 5

        LLMManager <-->|API Calls| EXLLM[[fa:fa-robot External LLM]]
        RAGEngine <-->|Reads/Writes| VDB[[fa:fa-database Vector DB]]
        FileManager <-->|Reads/Writes| FS[[fa:fa-folder-open File System]]
        H -.->|User Input| A
    end

    %% Style definitions for a cleaner look
    classDef default fill:#fff,stroke:#333,stroke-width:2px,font-family:Inter,font-size:12px;
    classDef subgraphStyle fill:#f9f9f9,stroke:#ddd,stroke-width:1px;
    class MainThread,TokioRuntime,CoreServices,WorkerModules,ExternalSystems subgraphStyle;
```
