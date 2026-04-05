// setup: add one harness per expected initial db state
// - db not loaded
// - db loaded:
//  - no workspace
//      - single crate (use TEST_APP_NODES_CANNONICAL_FRESH)
//  - workspace
//      - single crate (needs fixture setup in fixture db registry)
//      - multiple crates (needs fixture setup in fixture db registry)

use std::sync::Arc;

use lazy_static::lazy_static;
use ploke_test_utils::{FIXTURE_NODES_CANONICAL, fresh_backup_fixture_db};
use tokio::sync::Mutex;

use crate::app::commands::exec::execute;
use crate::app::commands::parser::Command;
use crate::app::{App, commands::unit_tests::harness::setup_test_app_from_db};
use crate::user_config::CommandStyle;

lazy_static! {
    /// A test-only accessible App instance for tests, wrapped in Arc<Mutex<...>>.
    /// This app has a DB loaded with fixture_nodes_canonical data.
    static ref TEST_APP_NODES_CANNONICAL_FRESH: Arc<Mutex<App>> = {
        // This stays registry-backed, but it is intentionally not sourced from the shared immutable
        // fixture cache because TEST_APP_NODES_CANNONICAL_FRESH wires the
        // database into mutable runtime services.
        let fixture_db = Arc::new(
                fresh_backup_fixture_db(&FIXTURE_NODES_CANONICAL)
                    .expect("load fixture_nodes_canonical fresh test db")
            );

        // helper builder
        setup_test_app_from_db(&fixture_db)
    };
}

mod decision_tree;
mod harness;

// ============================================================================
// TEST CASE 1: /index with no db loaded at workspace root
// ============================================================================
// Decision tree path: "pwd is workspace root" → "no db loaded" → "/index"
//
// Current behavior:
//   Parser returns: Command::Index { mode: Auto, target: None }
//   Executor: forwards StateCommand::Index
//   Result: state layer handles indexing effects
//
// NOTE: This test now documents the UPDATED behavior after parser/executor changes.

#[tokio::test]
async fn test_index_no_db_workspace_root_current_behavior() {
    let mut app = TEST_APP_NODES_CANNONICAL_FRESH.lock().await;

    // Set up input and parse command
    app.input_buffer = "/index".to_string();
    let style = CommandStyle::Slash;
    let command = crate::app::commands::parser::parse(&app, "/index", style);

    // UPDATED: Command is now parsed as Command::Index
    match &command {
        Command::Index { mode, target } => {
            assert!(matches!(mode, crate::app_state::commands::IndexMode::Auto));
            assert!(target.is_none(), "Expected target=None for bare /index");
            println!(
                "Command parsed as Command::Index {{ mode: {:?}, target: {:?} }}",
                mode, target
            );
        }
        _ => panic!("Unexpected command variant: {:?}", command),
    }

    // Execute command - now runs update/indexing logic
    execute(&mut app, command);

    // The executor emits AddMessageImmediate with scanning message
    // Full validation is done in the decision_tree test suite
}

#[tokio::test]
async fn test_index_workspace_dot_normalizes_to_current_workspace() {
    let app = TEST_APP_NODES_CANNONICAL_FRESH.lock().await;
    let style = CommandStyle::Slash;
    let command = crate::app::commands::parser::parse(&app, "/index workspace .", style);

    match &command {
        Command::Index { mode, target } => {
            assert!(matches!(
                mode,
                crate::app_state::commands::IndexMode::Workspace
            ));
            assert!(
                target.is_none(),
                "Expected target=None for `/index workspace .`"
            );
        }
        _ => panic!("Unexpected command variant: {:?}", command),
    }
}

#[tokio::test]
async fn test_index_pause_stays_on_legacy_feedback_path() {
    let app = TEST_APP_NODES_CANNONICAL_FRESH.lock().await;
    let style = CommandStyle::Slash;
    let command = crate::app::commands::parser::parse(&app, "/index pause", style);

    match &command {
        Command::Raw(raw) => {
            assert_eq!(raw, "index pause");
        }
        _ => panic!("Unexpected command variant: {:?}", command),
    }
}

// ============================================================================
// TEST CASE 2: /save db with no db loaded (should error)
// ============================================================================
// Decision tree path: "no db loaded" → "/save db" → error
//
// Expected: Error event emitted: "No crate/workspace in db to save"
// Current: Silently succeeds or creates empty backup

#[tokio::test]
async fn test_save_db_no_db_loaded_should_error() {
    // TODO: Use TEST_APP_NO_DB harness
    let mut app = TEST_APP_NODES_CANNONICAL_FRESH.lock().await;

    // TODO: Subscribe to events
    // let mut event_rx = app.subscribe(EventPriority::Realtime);

    // Parse and execute
    let style = CommandStyle::Slash;
    let command = crate::app::commands::parser::parse(&app, "/save db", style);

    match &command {
        Command::Save { kind } => {
            assert!(matches!(kind, crate::app::commands::parser::SaveKind::Db))
        }
        _ => panic!("Unexpected command variant: {:?}", command),
    }

    execute(&mut app, command);

    // TODO: Assert error event received
    // Expected: AppEvent::Error(ErrorEvent {
    //     message: "No crate/workspace in db to save",
    //     severity: ErrorSeverity::Error,
    // })
}

// ============================================================================
// TEST CASE 3: /load crate <nonexistent> should suggest /index
// ============================================================================
// Decision tree path: "no db loaded" → "/load crate X" → not in registry → suggest index

#[tokio::test]
async fn test_load_crate_nonexistent_should_suggest_index() {
    // TODO: Use TEST_APP_NO_DB harness
    let mut app = TEST_APP_NODES_CANNONICAL_FRESH.lock().await;

    let style = CommandStyle::Slash;
    let command = crate::app::commands::parser::parse(&app, "/load crate nonexistent_xyz", style);

    match &command {
        Command::Load { kind, name, force } => {
            assert!(matches!(
                kind,
                crate::app::commands::parser::LoadKind::Crate
            ));
            assert_eq!(name.as_deref(), Some("nonexistent_xyz"));
            assert!(!force);
        }
        _ => panic!("Unexpected command variant: {:?}", command),
    }

    execute(&mut app, command);

    // TODO: Assert error event with recovery suggestion
    // Expected: AppEvent::Error(ErrorEvent {
    //     message: "Crate 'nonexistent_xyz' not found in registry",
    //     severity: ErrorSeverity::Error,
    // })
    // Plus a message suggesting: "Use `/index crate <path>` to index it first"
}

// ============================================================================
// MISSING INFRASTRUCTURE - Implementation Checklist
// ============================================================================

// [ ] 1. EXTEND Command ENUM (app/commands/parser.rs)
//     Add variants:
//     - Command::Index { mode: IndexMode, target: Option<String> }
//       where IndexMode = Auto | Workspace | Crate
//     - Command::Load { kind: LoadKind, name: String, force: bool }
//       where LoadKind = Crate | Workspace
//     - Command::Save { kind: SaveKind }
//       where SaveKind = Db | History | Config
//     - Command::Update { scope: UpdateScope }
//       where UpdateScope = Auto | Focused | All

// [ ] 2. ADD PARSER RULES (app/commands/parser.rs)
//     Match patterns:
//     - "index" → Command::Index { scope: Auto, target: None }
//     - "index workspace" → Command::Index { scope: Workspace, target: None }
//     - "index crate <name>" → Command::Index { scope: Crate, target: Some(name) }
//     - "load crate <name>" → Command::Load { kind: Crate, name, force: false }
//     - "load crate <name> --force" → Command::Load { kind: Crate, name, force: true }
//     - etc.

// [ ] 3. CREATE CommandValidator (NEW: app/commands/validator.rs)
//     - Takes (&Command, &AppStateSnapshot) -> ValidationResult
//     - Implements decision tree logic from spec
//     - Returns ValidationResult::Success | ValidationResult::Error { reason, recovery }
//     - Does NOT execute expensive operations (just validates)

// [ ] 4. EXTEND App FOR TEST ACCESS (app/mod.rs)
//     Add method:
//     - pub fn subscribe(&self, priority: EventPriority) -> broadcast::Receiver<AppEvent>
//       Returns clone of event_rx or new subscription from event_bus
//     - Or make event_bus accessible via AppState

// [ ] 5. CREATE TEST HARNESS VARIANTS (unit_tests/harness.rs)
//     Add lazy_static refs:
//     - TEST_APP_NO_DB: App with no loaded crates/workspace
//       (empty system_state, no db backup loaded)
//     - TEST_APP_STANDALONE: Single crate loaded, NOT workspace member
//     - TEST_APP_WORKSPACE_SINGLE: Workspace with 1 member
//     - TEST_APP_WORKSPACE_MULTI: Workspace with 2+ members

// [ ] 6. PWD CONTEXT DETECTION (utility function)
//     - fn detect_pwd_context() -> PwdContext
//     - PwdContext::WorkspaceRoot { path, members }
//     - PwdContext::CrateRoot { path, parent_workspace: Option<PathBuf> }
//     - PwdContext::Other { path }
//     - Uses syn_parser::discovery to detect Cargo.toml structure

// [ ] 7. DECISION TREE IMPLEMENTATION (validator.rs)
//     For each decision tree branch:
//     - Check current state (loaded_crates, loaded_workspace)
//     - Check pwd context if needed
//     - Check registry if needed
//     - Check unsaved changes if needed
//     - Return appropriate ValidationResult

// ============================================================================
// DECISION TREE COVERAGE MAP
// ============================================================================
// Section 1: pwd workspace root, no db (12 cases)
//   - /index → IndexWorkspace
//   - /index workspace → IndexWorkspace
//   - /index crate <member> → IndexCrate
//   - /index crate → ListMembers
//   - /load crate <exists> → LoadCrate
//   - /load crate <not exists> → Error + suggest index
//   - /load workspace → Error + suggest index
//   - /save db → Error (no db)
//   - /update → Error (no db)
//   - /index start/pause/resume/cancel → Control indexing
//
// Section 2-8: [additional sections similar...]
// Total: ~65 test cases

// ============================================================================
// TEST PATTERN TEMPLATE
// ============================================================================
// async fn test_<scenario>() {
//     // 1. Setup app in required state
//     let mut app = TEST_APP_<STATE>.lock().await;
//
//     // 2. Subscribe to events
//     let mut event_rx = app.subscribe(EventPriority::Realtime);
//
//     // 3. Parse command
//     let command = parser::parse(&app, "/command args", CommandStyle::Slash);
//
//     // 4. Validate (if testing validation)
//     let validation = validator::validate(&command, &app.state_snapshot());
//
//     // 5. Execute (if testing execution)
//     execute(&mut app, command);
//
//     // 6. Assert on events
//     let event = timeout(Duration::from_millis(100), event_rx.recv()).await;
//     assert_matches!(event, Ok(Ok(AppEvent::...)));
// }
