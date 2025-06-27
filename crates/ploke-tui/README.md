# ploke-tui

#### Concurrency Model Analysis (Mermaid Diagram)

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


### **Pending Implementation Work**

1. **EventBus Upgrade (2 hours)**
   - Add error channel to `EventBus`
   - Implement backpressure strategy for background tasks

2. **Persistence Layer (1 hour)**
   - Complete Markdown writer in `chat_history.rs`
   - Add atomic write via tempfile + rename

**Phase 2: User Experience**

3. **Error Handling Pipeline (1.5 hours)**
   - Visual toast system
   - Error serialization to log files

4. **Performance Optimization (Ongoing)**
   - Frame timing instrumentation
   - Render fallback on over-budget frames

**Phase 3: Integration**

5. **LLM Worker Completion (3 hours)**
   - Governor rate limiter integration
   - Streaming API response handling

---

### **Concerns Requiring Final Decisions**
1. **Frame Budget Allocation**
   - Hard threshold for frame rendering (8ms for 120fps?)
   - Degradation strategy (skip effects vs lower quality)

2. **Error Visual Hierarchy**
   - Distinction between transient errors vs persistent failures
   - Top-bar annuity vs ephemeral toasts

3. **Persistence Triggers**
   - Auto-save interval configuration
   - Manual save shortcuts (Ctrl+S)

### Message control flow

```mermaid
%%{init: {'theme': 'base', 'themeVariables': {
    'darkMode': true,
    'background': '#0f172a',
    'primaryColor': '#1e293b',
    'primaryBorderColor': '#334155',
    'secondaryColor': '#1e293b',
    'lineColor': '#64748b',
    'textColor': '#e2e8f0',
    'actorBorder': '#94a3b8',
    'actorBkg': '#1e293b',
    'actorTextColor': '#f8fafc',
    'actorLineColor': '#64748b',
    'noteBkgColor': '#1e293b',
    'noteTextColor': '#e2e8f0',
    'noteBorderColor': '#334155'
}}}%%

sequenceDiagram
    actor User
    participant App as App (UI)
    participant StateManager as State Manager
    participant AppState as AppState (RwLock)
    participant EventBus
    participant LLMManager as LLM Manager

    User->>App: Types message & presses Enter
    App->>StateManager: send_cmd(AddUserMessage { content })
    App->>App: clear_input_buffer()

    activate StateManager
    StateManager->>StateManager: Receives AddUserMessage
    StateManager->>AppState: write().lock()
    activate AppState
    StateManager->>AppState: add_message(content, Role::User)
    AppState-->>StateManager: Ok(user_message_id)
    StateManager->>AppState: update_current(user_message_id)
    AppState-->>StateManager: 
    deactivate AppState

    StateManager->>EventBus: send(MessageUpdatedEvent)
    StateManager->>EventBus: send(Llm::Request)
    deactivate StateManager

    Note over App,EventBus: UI Update
    EventBus->>App: event_rx.recv()
    activate App
    App->>AppState: read().lock()
    activate AppState
    App->>AppState: get_full_path()
    AppState-->>App: returns new path
    deactivate AppState
    App->>App: sync_list_selection() & re-render
    deactivate App

    Note over LLMManager,EventBus: Async LLM Mock
    EventBus->>LLMManager: event_rx.recv()
    activate LLMManager
    LLMManager->>StateManager: send_cmd(AddMessage { role: Assistant, content: "" })
    
    activate StateManager
    StateManager->>AppState: write().lock()
    activate AppState
    StateManager->>AppState: add_child() → placeholder message
    AppState-->>StateManager: 
    deactivate AppState
    StateManager->>EventBus: send(MessageUpdatedEvent)
    deactivate StateManager
    
    LLMManager->>LLMManager: tokio::time::sleep(0.1s)
    LLMManager->>StateManager: send_cmd(UpdateMessage { content: "mock response", status: Completed })
    
    activate StateManager
    StateManager->>AppState: write().lock()
    activate AppState
    StateManager->>AppState: try_update() → final message
    AppState-->>StateManager: 
    deactivate AppState
    StateManager->>EventBus: send(MessageUpdatedEvent)
    deactivate StateManager
    
    deactivate LLMManager
```

