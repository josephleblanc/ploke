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

#### Single Workspace Member Loaded 
(Section 3 in decision_tree.rs)
- single crate + workspace member: `/index` re-indexes focused crate
- single crate + workspace member: `/index workspace` re-indexes entire workspace
- single crate + workspace member: `/index crate <focused>` re-indexes focused
- single crate + workspace member: `/index crate <other member>` switches focus + indexes
- single crate + workspace member: `/index crate <not member>` error + guidance


##### Standalone Crate Loaded 
(Section 4 in decision_tree.rs)
- standalone crate: `/index` re-indexes loaded crate
- standalone crate: `/index crate <loaded>` re-indexes
- standalone crate: `/index crate <different>` error "use `/load crate`"
- standalone crate: `/index workspace` error "not a workspace"

#### Full Workspace Loaded 
(Section 5 in decision_tree.rs)
- multi crate + workspace: `/index` re-indexes all members
- multi crate + workspace: `/index crate <member>` indexes that member
- multi crate + workspace: `/index crate <not member>` error + guidance
- multi crate + workspace: `/index workspace` re-indexes all (same as `/index`)
- multi crate + workspace: `/index path/to/crate` indexes if within workspace
- multi crate + workspace: `/index path/to/crate` error if outside workspace

### Desired: "/index" (no args) at workspace root with no db

TODO: fill out the current behavior

#### Single Workspace Member Loaded 
(Section 3 in decision_tree.rs)
- single crate + workspace member: `/index`
- single crate + workspace member: `/index workspace`
- single crate + workspace member: `/index crate <focused>`
- single crate + workspace member: `/index crate <other member>`
- single crate + workspace member: `/index crate <not member>`

##### Standalone Crate Loaded 
(Section 4 in decision_tree.rs)
- standalone crate: `/index`
- standalone crate: `/index crate <loaded>`
- standalone crate: `/index crate <different>`
- standalone crate: `/index workspace`

#### Full Workspace Loaded 
(Section 5 in decision_tree.rs)
- multi crate + workspace: `/index`
- multi crate + workspace: `/index crate <member>`
- multi crate + workspace: `/index crate <not member>`
- multi crate + workspace: `/index workspace`
- multi crate + workspace: `/index path/to/crate`
- multi crate + workspace: `/index path/to/crate`

---

## Current "X" Command Flow

```
TODO
```

## Key Files and Their Roles

| File | Purpose | Decision Tree Relevance |
|------|---------|------------------------|
TODO

## Current vs. Desired Behavior
TODO

### Current: "/index" (no args) at workspace root with no db
TODO
