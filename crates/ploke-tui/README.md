# ploke-tui

Let's break this down:

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
**Phase 1: Critical Foundations**
1. **State Segmentation (2-3 hours)**
   - Implement granular `RwLock` for `AppState` components
   - Verify lock-free UI rendering paths

2. **EventBus Upgrade (2 hours)**
   - Add error channel to `EventBus`
   - Implement backpressure strategy for background tasks

3. **Persistence Layer (1 hour)**
   - Complete Markdown writer in `chat_history.rs`
   - Add atomic write via tempfile + rename

**Phase 2: User Experience**
4. **Error Handling Pipeline (1.5 hours)**
   - Visual toast system
   - Error serialization to log files

5. **Performance Optimization (Ongoing)**
   - Frame timing instrumentation
   - Render fallback on over-budget frames

**Phase 3: Integration**
6. **LLM Worker Completion (3 hours)**
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

