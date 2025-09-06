# UI Event Flow Diagram - Proposal System Approval/Denial Workflow

## Complete Event Flow Between Actors

```mermaid
sequenceDiagram
    participant User
    participant TUI_KeyHandler as TUI Key Handler<br/>(handle_overlay_key)
    participant CommandParser as Command Parser<br/>(App::send_cmd)
    participant StateDispatcher as State Dispatcher<br/>(state_manager)
    participant EditingHandlers as Editing Handlers<br/>(approve_edits/deny_edits)
    participant IoManager as IoManager<br/>(write_snippets_batch)
    participant EventBus as Event Bus<br/>(SystemEvent)
    participant TUI_Renderer as TUI Renderer<br/>(render_approvals_overlay)
    participant ProposalStore as Proposal Store<br/>(AppState.proposals)

    Note over User, ProposalStore: Initial Setup - Tool Call Creates Proposal
    
    User->>+TUI_KeyHandler: Press 'e' (show approvals overlay)
    TUI_KeyHandler->>+TUI_Renderer: Show approvals overlay
    TUI_Renderer->>+ProposalStore: Read proposals (blocking_read())
    ProposalStore-->>-TUI_Renderer: Vec<EditProposal> with Pending status
    TUI_Renderer-->>-TUI_KeyHandler: Display proposal list + preview
    TUI_KeyHandler-->>-User: Show overlay with keyboard shortcuts

    Note over User, ProposalStore: User Decision - Approve Flow
    
    User->>+TUI_KeyHandler: Press Enter or 'y' (approve)
    TUI_KeyHandler->>+CommandParser: tokio::spawn async closure
    CommandParser->>+StateDispatcher: StateCommand::ApproveEdits { request_id }
    StateDispatcher->>+EditingHandlers: approve_edits(&state, &event_bus, request_id)
    
    EditingHandlers->>+ProposalStore: Read proposal by request_id
    ProposalStore-->>-EditingHandlers: EditProposal (check status == Pending)
    
    alt Status is Pending
        EditingHandlers->>+ProposalStore: Update status to Approved
        EditingHandlers->>+IoManager: write_snippets_batch(proposal.edits)
        
        alt Write Success
            IoManager-->>-EditingHandlers: Ok(results)
            EditingHandlers->>+ProposalStore: Update status to Applied
            EditingHandlers->>+EventBus: SystemEvent::ToolCallCompleted
            EventBus-->>-TUI_Renderer: Trigger UI refresh
        else Write Failure  
            IoManager-->>-EditingHandlers: Err(error)
            EditingHandlers->>+ProposalStore: Update status to Failed(error)
            EditingHandlers->>+EventBus: SystemEvent::ToolCallFailed
            EventBus-->>-TUI_Renderer: Trigger UI refresh
        end
    else Status is Already Applied
        EditingHandlers-->>StateDispatcher: Return early (idempotent)
    end
    
    EditingHandlers-->>-StateDispatcher: Complete
    StateDispatcher-->>-CommandParser: Complete
    CommandParser-->>-TUI_KeyHandler: Complete
    TUI_KeyHandler-->>User: UI updates automatically

    Note over User, ProposalStore: User Decision - Deny Flow
    
    User->>+TUI_KeyHandler: Press 'n' or 'd' (deny)
    TUI_KeyHandler->>+CommandParser: tokio::spawn async closure  
    CommandParser->>+StateDispatcher: StateCommand::DenyEdits { request_id }
    StateDispatcher->>+EditingHandlers: deny_edits(&state, &event_bus, request_id)
    
    EditingHandlers->>+ProposalStore: Read proposal by request_id
    ProposalStore-->>-EditingHandlers: EditProposal (check status == Pending)
    
    alt Status is Pending
        EditingHandlers->>+ProposalStore: Update status to Denied
        EditingHandlers->>+EventBus: SystemEvent::ToolCallFailed
        EventBus-->>-TUI_Renderer: Trigger UI refresh
    else Status is Already Processed
        EditingHandlers-->>StateDispatcher: Return early (idempotent)
    end
    
    EditingHandlers-->>-StateDispatcher: Complete
    StateDispatcher-->>-CommandParser: Complete
    CommandParser-->>-TUI_KeyHandler: Complete
    TUI_KeyHandler-->>User: UI updates automatically

    Note over User, ProposalStore: Additional User Actions
    
    User->>+TUI_KeyHandler: Press 'o' (open in editor)
    TUI_KeyHandler->>+CommandParser: open_editor_with_proposal(proposal)
    CommandParser->>CommandParser: Launch external editor process
    CommandParser-->>-TUI_KeyHandler: Editor launched
    TUI_KeyHandler-->>-User: External editor opens
    
    User->>+TUI_KeyHandler: Press Esc or 'q' (close overlay)
    TUI_KeyHandler-->>-User: Close overlay, return to main UI
```

## Actor Responsibilities and Key Functions

### 1. **TUI Key Handler** (`crates/ploke-tui/src/app/mod.rs`)
**Primary Function**: `handle_overlay_key(key: KeyEvent) -> bool`
- **Events Handled**: KeyCode::Enter, KeyCode::Char('y'/'n'/'d'/'o'/'e')
- **Events Emitted**: StateCommand via tokio::spawn + cmd_tx.try_send()
- **State**: ApprovalsState (selected_index, UI positioning)

### 2. **Command Parser** (`crates/ploke-tui/src/app/mod.rs`)
**Primary Function**: `send_cmd(command: StateCommand)`
- **Events Received**: Raw StateCommand structs
- **Events Emitted**: StateCommand via mpsc channel to StateDispatcher
- **Integration**: Async command queuing with error handling

### 3. **State Dispatcher** (`crates/ploke-tui/src/app_state/dispatcher.rs`)
**Primary Function**: `state_manager(state, cmd_rx, event_bus, context_tx)`
- **Events Received**: StateCommand::{ApproveEdits, DenyEdits}
- **Events Emitted**: Calls to editing handler functions
- **Pattern Matching**:
  ```rust
  StateCommand::ApproveEdits { request_id } => {
      rag::editing::approve_edits(&state, &event_bus, request_id).await;
  }
  StateCommand::DenyEdits { request_id } => {
      rag::editing::deny_edits(&state, &event_bus, request_id).await;
  }
  ```

### 4. **Editing Handlers** (`crates/ploke-tui/src/rag/editing.rs`)
**Primary Functions**: 
- `approve_edits(state: &Arc<AppState>, event_bus: &Arc<EventBus>, request_id: Uuid)`
- `deny_edits(state: &Arc<AppState>, event_bus: &Arc<EventBus>, request_id: Uuid)`

**State Transitions**:
- **Approve Path**: Pending → Approved → Applied (success) | Failed (error)
- **Deny Path**: Pending → Denied
- **Idempotency**: Early return if already processed

**Events Emitted**:
- `SystemEvent::ToolCallCompleted` (on successful apply)
- `SystemEvent::ToolCallFailed` (on deny or apply failure)

### 5. **IoManager** (`ploke-io` crate)
**Primary Function**: `write_snippets_batch(edits: Vec<WriteSnippetData>)`
- **Safety Features**: Atomic file operations, hash verification
- **Integration**: Called from approve_edits() for file writing
- **Error Handling**: Returns Result<Vec<WriteResult>, IoError>

### 6. **Event Bus** (`crates/ploke-tui/src/app_state/events.rs`)
**Event Types**:
- `SystemEvent::ToolCallCompleted { request_id, parent_id, call_id, content }`
- `SystemEvent::ToolCallFailed { request_id, parent_id, call_id, error }`

### 7. **TUI Renderer** (`crates/ploke-tui/src/app/view/components/approvals.rs`)
**Primary Function**: `render_approvals_overlay(frame, area, state, ui) -> Option<Uuid>`
- **Rendering Logic**: Proposal list + preview pane split layout
- **Status Display**: "Pending", "Applied", "Failed(error)", "Denied"
- **Interactive Elements**: Scrollable list with keyboard navigation

### 8. **Proposal Store** (`crates/ploke-tui/src/app_state/core.rs`)
**Data Structure**: `Arc<RwLock<HashMap<Uuid, EditProposal>>>`
- **Thread Safety**: RwLock for concurrent read/write access
- **Persistence**: Proposals maintained in memory during session
- **Status Tracking**: Real-time status updates via EditProposalStatus enum

## Current Implementation Status

### ✅ **Fully Implemented Components**

1. **Complete UI/UX Workflow**
   - Overlay-based proposal viewer with keyboard shortcuts
   - Real-time status updates and visual feedback
   - External editor integration ('o' key)
   - Intuitive navigation and interaction patterns

2. **Event-Driven Architecture** 
   - Async command processing via tokio::spawn
   - Event bus integration for UI updates
   - Clean separation between UI, state management, and business logic

3. **State Machine Implementation**
   - 5-state finite state machine with proper transitions
   - Idempotent operations (safe to call approve/deny multiple times)
   - Error state handling with descriptive failure messages

4. **File Operations Integration**
   - IoManager coordination for atomic writes
   - Hash verification and safety-first editing
   - Batch write operations for efficiency

5. **Comprehensive Test Coverage**
   - UI interaction tests in `crates/ploke-tui/src/tests/`
   - State machine transition testing
   - Error handling and edge case coverage

### 🔄 **Areas for Enhancement**

1. **Persistence Beyond Session**
   - Currently proposals are memory-only
   - Could benefit from database persistence for audit trails

2. **Batch Operations**
   - UI supports single proposal approval/denial
   - Could add "approve all" / "deny all" functionality

3. **Advanced Preview Features**
   - Current diff preview is comprehensive
   - Could add syntax highlighting or side-by-side comparison

4. **Notification System**
   - Status changes trigger UI updates
   - Could add optional desktop notifications or sound alerts

### 🎯 **Key Strengths of Current Implementation**

1. **Type Safety**: Strong typing throughout the event flow with no stringly-typed interfaces
2. **Performance**: Async architecture with non-blocking UI interactions  
3. **Reliability**: Idempotent operations and comprehensive error handling
4. **Usability**: Intuitive keyboard shortcuts and clear visual feedback
5. **Extensibility**: Clean actor separation allows easy addition of new features
6. **Safety**: Hash-verified atomic file operations prevent data corruption

The implementation represents a production-ready human-in-the-loop approval system with excellent separation of concerns and robust error handling.