# Command Flow Analysis for Decision Tree Implementation

## Current "/index" Command Flow

```
User types "/index" + Enter
    ↓
[UI Layer] `app/mod.rs` captures key event
    ↓
[Parser] `app/commands/parser.rs::parse()`
    - Strips "/" prefix
    - No match for "index" in structured Command enum
    - Returns `Command::Raw("index")`
    ↓
[Executor] `app/commands/exec.rs::execute()`
    - Matches `Command::Raw(cmd_str)`
    - Calls `execute_legacy(app, cmd_str)`
    ↓
[Legacy Executor] `execute_legacy()` - string matching:
    - "index start [path]" → `StateCommand::IndexTargetDir`
    - "index pause" → `StateCommand::PauseIndexing`
    - "index resume" → `StateCommand::ResumeIndexing`
    - "index cancel" → `StateCommand::CancelIndexing`
    - "index" alone → NOT HANDLED (would fall through)
    ↓
[State Manager] `app_state/dispatcher.rs::state_manager()`
    - Receives `StateCommand` via mpsc channel
    - Dispatches to handlers:
      - `handlers::indexing::index_workspace()`
      - `handlers::indexing::pause/resume/cancel()`
    ↓
[Handlers] `app_state/handlers/indexing.rs`
    - Performs actual indexing work
    - Emits events via `event_bus.send(AppEvent::...)`
```

## Key Files and Their Roles

| File | Purpose | Decision Tree Relevance |
|------|---------|------------------------|
| `app/commands/parser.rs` | Parses input into `Command` enum | Needs structured `Command::Index` variant |
| `app/commands/exec.rs` | Executes parsed commands | Needs decision tree logic for validation |
| `app_state/commands.rs` | Defines `StateCommand` enum | May need new command types |
| `app_state/dispatcher.rs` | Dispatches `StateCommand` to handlers | Handler selection logic |
| `event_bus/mod.rs` | Event broadcasting system | Test will subscribe to output events |
| `lib.rs` | `AppEvent` enum definition | Need new event types for command validation |
| `app_state/events.rs` | `SystemEvent` enum definition | May need `CommandDispatchEvent` |

## Current vs. Desired Behavior

### Current: "/index" (no args) at workspace root with no db
```rust
// Parser returns: Command::Raw("index")
// Legacy executor: NO MATCH (only "index start", "index pause", etc.)
// Result: Command silently ignored or falls through
```

### Desired (per Decision Tree):
```rust
// Parser should return: Command::Index { scope: Workspace, target: None }
// Executor should:
//   1. Check state: no db loaded ✓
//   2. Check pwd: workspace root ✓
//   3. Emit event: CommandDispatchEvent::StartIndexing { scope: Workspace }
//   4. Or emit: CommandDispatchEvent::Error { 
//        kind: AmbiguousIndexTarget, 
//        suggestion: "Use /index workspace or /index crate <name>" 
//      }
```

## Event System Architecture

### Current Event Flow
```rust
// Handlers emit events via EventBus
EventBus::send(AppEvent::IndexingStarted)
EventBus::send(AppEvent::IndexingCompleted)
EventBus::send(AppEvent::System(SystemEvent::BackupDb { ... }))
```

### Needed for Decision Tree Testing
```rust
// NEW: Command dispatch events emitted BEFORE execution
pub enum CommandDispatchEvent {
    ValidationError {
        command: String,
        error_kind: ValidationErrorKind,
        recovery_suggestion: Option<String>,
    },
    ValidationSuccess {
        command: String,
        validated_action: IndexAction,
    },
    StateTransition {
        from: DbState,
        to: DbState,
        trigger: String,
    },
}

pub enum ValidationErrorKind {
    NoDatabaseLoaded { command: String },
    NotWorkspaceMember { crate_name: String },
    UnsavedChanges { target: String },
    AlreadyLoaded { crate_name: String },
    NotInRegistry { ref_type: String, ref_name: String },
    // ... per decision tree
}
```

## Testing Architecture (Step 2.5)

### Test Setup
```rust
// Create EventBus
let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));

// Subscribe to command dispatch events
let mut dispatch_rx = event_bus.subscribe_to_dispatch_events();

// Create minimal command dispatcher (no heavy actors)
let dispatcher = CommandDispatcher::new(event_bus.clone(), /* minimal state */);

// Send command
dispatcher.dispatch(Command::Index { scope: Workspace, target: None }).await;

// Assert on event output
let event = dispatch_rx.recv().await.unwrap();
assert_matches!(event, CommandDispatchEvent::ValidationSuccess { ... });
```

### What Makes This Lightweight

| Current Test (Comprehensive) | New Test (Event-Based) |
|------------------------------|------------------------|
| Full `AppHarness` with all actors | Minimal command dispatcher only |
| Spawns indexing tasks | No indexing - validates dispatch decision |
| Polls chat state for messages | Subscribes to events via broadcast channel |
| String matching on UI messages | Pattern matching on event enums |
| ~44 seconds for 72 tests | Should be <1 second |
| Tests end-to-end behavior | Tests command dispatch contract |

## Decision Tree Implementation Points

### 1. Command Parser Extension
```rust
// In app/commands/parser.rs
pub enum Command {
    // ... existing variants
    
    // NEW: Structured index commands
    Index {
        scope: IndexScope,  // Workspace, Crate, Auto
        target: Option<String>, // crate name or path
    },
    Load {
        kind: LoadKind,     // Crate, Workspace
        name: String,
        force: bool,        // --force flag
    },
    Save {
        kind: SaveKind,     // Db, History, etc.
    },
    Update {
        scope: UpdateScope, // Focused, All, Auto
    },
}
```

### 2. Decision Tree Validator
```rust
// NEW: app/commands/validator.rs
pub struct CommandValidator {
    state: Arc<AppState>, // minimal read-only state
}

impl CommandValidator {
    pub fn validate_index(&self, cmd: &IndexCommand) -> Result<IndexAction, ValidationError> {
        match self.get_db_state() {
            DbState::NoDb => self.validate_index_no_db(cmd),
            DbState::SingleCrateStandalone => self.validate_index_standalone(cmd),
            DbState::SingleCrateWorkspaceMember => self.validate_index_workspace_member(cmd),
            DbState::MultipleCratesWorkspace => self.validate_index_multi_workspace(cmd),
        }
    }
    
    fn validate_index_no_db(&self, cmd: &IndexCommand) -> Result<IndexAction, ValidationError> {
        let pwd_context = self.get_pwd_context();
        match (cmd.scope, pwd_context) {
            (IndexScope::Auto, PwdContext::WorkspaceRoot) => {
                Ok(IndexAction::IndexWorkspace { path: pwd_context.workspace_root() })
            }
            (IndexScope::Auto, PwdContext::CrateRoot) => {
                Ok(IndexAction::IndexCrate { path: pwd_context.crate_root() })
            }
            (IndexScope::Workspace, PwdContext::CrateRoot { is_member: false }) => {
                Err(ValidationError::NotWorkspaceMember)
            }
            // ... per decision tree
        }
    }
}
```

### 3. Event Types for Testing
```rust
// In app_state/events.rs or new app_state/command_events.rs

/// Emitted by command dispatcher during validation/decision
#[derive(Clone, Debug)]
pub enum CommandDispatchEvent {
    /// Command passed validation, will proceed
    Accepted {
        command: String,
        action: ValidatedAction,
    },
    
    /// Command failed validation
    Rejected {
        command: String,
        reason: RejectionReason,
        recovery: Option<String>,
    },
    
    /// State transition required user confirmation
    ConfirmationRequired {
        command: String,
        transition: StateTransition,
        warning: String,
    },
}

#[derive(Clone, Debug)]
pub enum ValidatedAction {
    Index(IndexAction),
    Load(LoadAction),
    Save(SaveAction),
    Update(UpdateAction),
}

#[derive(Clone, Debug)]
pub enum RejectionReason {
    NoDatabaseLoaded,
    AlreadyLoaded { name: String },
    NotInRegistry { ref_type: String, name: String },
    NotWorkspaceMember { name: String },
    AmbiguousTarget { options: Vec<String> },
}
```

## Next Steps

1. **Define Event Types**: Create `CommandDispatchEvent` enum for testable output
2. **Build Validator**: Implement decision tree logic in `CommandValidator`
3. **Refactor Parser**: Add structured `Command::Index/Load/Save/Update` variants
4. **Create Test**: Subscribe to events, validate dispatch decisions
5. **Implement Handler**: Wire validator to actual execution

## Test Coverage Requirements

The event-based test must cover all decision tree branches:
- **Section 1**: pwd workspace root, no db (12 cases)
- **Section 2**: pwd crate root, no db (8 cases)
- **Section 3**: Single crate + workspace loaded (9 cases)
- **Section 4**: Standalone crate loaded (8 cases)
- **Section 5**: Multiple crates + workspace (10 cases)
- **Section 6**: pwd crate, db loaded (8 cases)
- **Section 7**: Transition/unsaved changes (4 cases)
- **Section 8**: `/workspace` subcommands (6 cases)

**Total: ~65 test cases** validating event output, not state changes.
