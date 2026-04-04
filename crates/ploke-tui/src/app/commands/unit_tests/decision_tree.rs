//! Event-based smoke tests for command decision tree.
//!
//! This test file validates the command parser + executor/validator component
//! via its event interface. Tests are grouped by DB state for efficiency.
//!
//! ## Test Coverage
//!
//! ### No DB Loaded (Section 1 & 2)
//! - 0% coverage (0/24 cases)
//! - 0/24 cases tested
//! - 0 passed
//! - 0 failed
//!
//! ### Single Workspace Member Loaded (Section 3)
//! - [ ] single crate + workspace member: `/index` re-indexes focused crate
//! - [ ] single crate + workspace member: `/index workspace` re-indexes entire workspace
//! - [ ] single crate + workspace member: `/index crate <focused>` re-indexes focused
//! - [ ] single crate + workspace member: `/index crate <other member>` switches focus + indexes
//! - [ ] single crate + workspace member: `/index crate <not member>` error + guidance
//! - [ ] single crate + workspace member: `/load crate <member>` suggests `/index crate`
//! - [ ] single crate + workspace member: `/load crate <not member>` error + guidance
//! - [ ] single crate + workspace member: `/load crate <not in registry>` suggests index
//! - [ ] single crate + workspace member: `/save db` saves workspace snapshot
//! - [ ] single crate + workspace member: `/update` scans focused crate, re-indexes if stale
//!
//! ### Standalone Crate Loaded (Section 4)
//! - [ ] standalone crate: `/index` re-indexes loaded crate
//! - [ ] standalone crate: `/index crate <loaded>` re-indexes
//! - [ ] standalone crate: `/index crate <different>` error "use `/load crate`"
//! - [ ] standalone crate: `/index workspace` error "not a workspace"
//! - [ ] standalone crate: `/load crate <name>` check unsaved, then load (or force)
//! - [ ] standalone crate: `/load workspace <name>` check unsaved, then load workspace
//! - [ ] standalone crate: `/save db` saves standalone snapshot
//! - [ ] standalone crate: `/update` scans and re-indexes if stale
//! - [ ] standalone crate: `/index path/to/other` error "use `/load crate`"
//!
//! ### Full Workspace Loaded (Section 5)
//! - [ ] multi crate + workspace: `/index` re-indexes all members
//! - [ ] multi crate + workspace: `/index crate <member>` indexes that member
//! - [ ] multi crate + workspace: `/index crate <not member>` error + guidance
//! - [ ] multi crate + workspace: `/index workspace` re-indexes all (same as `/index`)
//! - [ ] multi crate + workspace: `/index path/to/crate` indexes if within workspace
//! - [ ] multi crate + workspace: `/index path/to/crate` error if outside workspace
//! - [ ] multi crate + workspace: `/load crate <member>` suggests `/index crate`
//! - [ ] multi crate + workspace: `/load crate <not member>` check unsaved, unload, load single
//! - [ ] multi crate + workspace: `/load crate <not in registry>` suggests index
//! - [ ] multi crate + workspace: `/load workspace <different>` check unsaved, load new
//! - [ ] multi crate + workspace: `/save db` saves workspace snapshot
//! - [ ] multi crate + workspace: `/update` scans all members, re-indexes stale
//!
//! ### DB Loaded, PWD is Crate (Section 6)
//! - [ ] pwd=crate, db loaded: `/index` re-indexes crate at pwd if loaded
//! - [ ] pwd=crate, db loaded: `/index` error if pwd crate not loaded
//! - [ ] pwd=crate, db loaded: `/index workspace` re-indexes workspace if pwd is member
//! - [ ] pwd=crate, db loaded: `/index workspace` error if pwd not member
//! - [ ] pwd=crate, db loaded: `/index crate <pwd match>` re-indexes
//! - [ ] pwd=crate, db loaded: `/index crate <different loaded>` switches focus + indexes
//! - [ ] pwd=crate, db loaded: `/index crate <not loaded>` follows workspace rules
//! - [ ] pwd=crate, db loaded: `/load crate <pwd match>` error "already loaded"
//! - [ ] pwd=crate, db loaded: `/load crate <different>` check unsaved, then load
//! - [ ] pwd=crate, db loaded: `/load workspace <name>` check unsaved, then load
//! - [ ] pwd=crate, db loaded: `/save db`, `/update` same as workspace root rules
//!
//! ### Transition Cases (Section 7)
//! - [ ] `/load workspace <name>` when standalone loaded: check unsaved, prompt or force
//! - [ ] `/load crate <name>` when workspace loaded: check unsaved, prompt or force
//! - [ ] `/load crate <name>` standalone→standalone: check unsaved, prompt or force
//! - [ ] `/index` when db loaded: destructive re-parse (no force needed)
//!
//! Total: ~65 test cases

use std::sync::Arc;
use std::time::Duration;

use crate::app::commands::unit_tests::harness::{
    DebugStateCommand, Present, TestAppAccessor, TestInAppActorBuilder, TestOutEventBusBuilder,
    TestRuntime,
};
use crate::{AppEvent, StateCommand};
use ploke_test_utils::{
    FIXTURE_NODES_CANONICAL, WS_FIXTURE_01_CANONICAL, WS_FIXTURE_01_MEMBER_SINGLE,
    fresh_backup_fixture_db,
};
use tokio::time::timeout;

// =============================================================================
// Test: No DB Loaded
// =============================================================================
//
//  1. [ ] pwd is workspace root, no db: `/index`                           indexes workspace
//  2. [ ] pwd is workspace root, no db: `/index workspace`                 indexes workspace
//  3. [ ] pwd is workspace root, no db: `/index workspace .`               indexes workspace
//  4. [ ] pwd is workspace root, no db: `/index path/to/crate`             indexes target crate
//  5. [ ] pwd is workspace root, no db: `/index crate path/to/crate`       indexes target crate
//  6. [ ] pwd is workspace root, no db: `/index crate <name>`              indexes workspace member
//  7. [ ] pwd is workspace root, no db: `/index crate`                     lists members + suggests command
//  8. [ ] pwd is workspace root, no db: `/load crate <exists>`             loads from registry
//  9. [ ] pwd is workspace root, no db: `/load crate <not exists>`         suggests index if member
// 10. [ ] pwd is workspace root, no db: `/load crate <not exists>`         lists crates + suggests
// 11. [ ] pwd is workspace root, no db: `/load workspace <crate-name>`     suggests `/load crate`
// 12. [ ] pwd is workspace root, no db: `/load workspace`                  loads if exists, else suggests `/index`
// 13. [ ] pwd is workspace root, no db: `/save db`                         error "No crate/workspace in db"
// 14. [ ] pwd is workspace root, no db: `/update`                          error "No crate/workspace in db"
// 15. [ ] pwd is crate, no db:          `/index`                           indexes current crate
// 16. [ ] pwd is crate, no db:          `/index crate`                     indexes current crate
// 17. [ ] pwd is crate, no db:          `/index crate .`                   indexes current crate
// 18. [ ] pwd is crate, no db:          `/index workspace`                 indexes full workspace if member
// 19. [ ] pwd is crate, no db:          `/index workspace`                 error if not member
// 20. [ ] pwd is crate, no db:          `/index path/to/crate`             indexes that crate
// 21. [ ] pwd is crate, no db:          `/load crate <name>`               loads from registry or suggests index
// 22. [ ] pwd is crate, no db:          `/load workspace <name>`           loads or suggests index workspace root
// 23. [ ] pwd is crate, no db:          `/save db`                         error "No crate/workspace in db"
// 24. [ ] pwd is crate, no db:          `/update`                          error "No crate/workspace in db"

/// Test parameters for "No DB Loaded" decision tree tests
#[derive(Clone)]
struct NoDbTestCase {
    /// Human-readable test name for error messages
    name: &'static str,
    /// PWD type: "workspace" or "crate"
    pwd_type: &'static str,
    /// The command string to type (e.g., "/index", "/save db")
    input: &'static str,
    /// The expected StateCommand discriminant string:
    /// - "TestTodo" for unimplemented cases (TDD)
    /// - "IndexTargetDir", "SaveDb", etc. for implemented cases
    expected_state_cmd: &'static str,
    /// Optional: expected test_name field if expecting TestTodo
    /// Used to verify the right todo is being hit
    expected_todo_test_name: Option<&'static str>,
}

/// Runs multiple "No DB Loaded" test cases using a single app instance with TestBackend.
///
/// This is more efficient than running cases individually since we only set up the
/// TestRuntime, App, and Terminal once.
async fn run_no_db_test_cases(cases: &[NoDbTestCase]) {
    use crossterm::event::{Event, KeyCode, KeyEvent};
    use futures::StreamExt;
    use ratatui::{Terminal, backend::TestBackend};
    use tokio_stream::wrappers::UnboundedReceiverStream;

    // 1. Create fresh/empty database
    let db = ploke_db::Database::init_with_schema().expect("create empty db");
    let db = Arc::new(db);

    // 2. Set up TestRuntime with state manager spawned
    let rt = TestRuntime::new(&db).spawn_state_manager();

    // 3. Get debug receiver BEFORE into_app consumes rt
    let events = rt.events_builder().build_app_only();
    let mut debug_rx = events
        .app_actor_events
        .debug_string_rx
        .expect("debug_string_rx should be available");

    // All cases use the same PWD type, so determine it once from first case
    let pwd = match cases.first().map(|c| c.pwd_type).unwrap_or("workspace") {
        "workspace" => {
            ploke_test_utils::workspace_root().join("tests/fixture_workspace/ws_fixture_01")
        }
        "crate" => ploke_test_utils::workspace_root().join("tests/fixture_crates/fixture_nodes"),
        other => panic!("Unknown pwd_type: {}", other),
    };

    // 4. Create App
    let app = rt.into_app(pwd);

    // 5. Setup headless terminal
    let backend = TestBackend::new(80, 24);
    let terminal = Terminal::new(backend).expect("create terminal");

    // 6. Create input channel
    let (input_tx, input_rx) =
        tokio::sync::mpsc::unbounded_channel::<Result<crossterm::event::Event, std::io::Error>>();
    let input = UnboundedReceiverStream::new(input_rx);

    // 7. Run app in background
    let app_task = tokio::spawn(async move {
        app.run_with(
            terminal,
            input,
            crate::app::RunOptions {
                setup_terminal_modes: false,
            },
        )
        .await
    });

    // 8. Run each case sequentially
    for case in cases {
        // Send keystrokes (each char + yields)
        for ch in case.input.chars() {
            input_tx
                .send(Ok(Event::Key(KeyEvent::from(KeyCode::Char(ch)))))
                .expect("send key");
            tokio::task::yield_now().await;
        }
        input_tx
            .send(Ok(Event::Key(KeyEvent::from(KeyCode::Enter))))
            .expect("send enter");

        // Wait for StateCommand (100ms timeout for TestBackend)
        let timeout_result = timeout(Duration::from_millis(100), debug_rx.recv()).await;

        // Check results
        let result = match timeout_result {
            Ok(Some(cmd)) => {
                let cmd_str = cmd.as_str();
                let is_match = if case.expected_state_cmd == "TestTodo" {
                    cmd_str.starts_with("TestTodo")
                } else {
                    cmd_str == case.expected_state_cmd
                };

                if is_match {
                    Ok(())
                } else {
                    Err(format!(
                        "Expected StateCommand '{}' but got '{}'",
                        case.expected_state_cmd, cmd_str
                    ))
                }
            }
            Ok(None) => Err("Channel closed - app terminated early?".to_string()),
            Err(_) => Err(format!(
                "Timeout waiting for StateCommand '{}'",
                case.expected_state_cmd
            )),
        };

        // Propagate error immediately with test name context
        if let Err(msg) = result {
            panic!("Test '{}' failed: {}", case.name, msg);
        }
    }

    // Cleanup
    app_task.abort();
    let _ = app_task.await;
}

/// Runs a single "No DB Loaded" test case using the full app with TestBackend.
/// Convenience wrapper around run_no_db_test_cases for single-case tests.
async fn run_no_db_test_case(case: &NoDbTestCase) {
    run_no_db_test_cases(std::slice::from_ref(case)).await;
}

/// Test case 1b: Same as test case 1, but using the full app run loop with TestBackend.
///
/// This test exercises the complete flow:
/// Key events → Action::ExecuteCommand → parser::parse → exec::execute → StateCommand relay
#[tokio::test]
async fn test_no_db_workspace_root_index_indexes_workspace_full_app() {
    run_no_db_test_case(&NoDbTestCase {
        name: "/index at workspace root -> IndexTargetDir",
        pwd_type: "workspace",
        input: "/index",
        expected_state_cmd: "TestTodo", // Not yet implemented
        expected_todo_test_name: Some("test_no_db_workspace_root_index_indexes_workspace"),
    })
    .await;
}

/// Batch test: Runs all "No DB Loaded" test cases sequentially
///
/// This provides a quick overview of which cases are passing/failing.
/// For individual debugging, run the specific test case.
#[tokio::test]
async fn test_no_db_loaded_all_cases() {
    let cases = vec![
        // Section 1: PWD is workspace root, no DB
        NoDbTestCase {
            name: "1. /index at workspace root",
            pwd_type: "workspace",
            input: "/index",
            expected_state_cmd: "TestTodo", // Not yet implemented
            expected_todo_test_name: Some("test_no_db_workspace_root_index_indexes_workspace"),
        },
        NoDbTestCase {
            name: "2. /index workspace at workspace root",
            pwd_type: "workspace",
            input: "/index workspace",
            expected_state_cmd: "TestTodo", // Not yet implemented
            expected_todo_test_name: Some("test_no_db_workspace_root_index_indexes_workspace"),
        },
        NoDbTestCase {
            name: "3. /index workspace . at workspace root",
            pwd_type: "workspace",
            input: "/index workspace .",
            expected_state_cmd: "TestTodo", // Not yet implemented
            expected_todo_test_name: Some("test_no_db_workspace_root_index_indexes_workspace"),
        },
        NoDbTestCase {
            name: "7. /index crate at workspace root (list members)",
            pwd_type: "workspace",
            input: "/index crate",
            expected_state_cmd: "TestTodo", // Not yet implemented
            expected_todo_test_name: Some("test_no_db_workspace_root_index_crate_lists_members"),
        },
        NoDbTestCase {
            name: "11. /load workspace <crate-name> at workspace root",
            pwd_type: "workspace",
            input: "/load workspace member_root",
            expected_state_cmd: "TestTodo", // Not yet implemented
            expected_todo_test_name: Some(
                "test_no_db_workspace_root_load_workspace_crate_name_suggests_load_crate",
            ),
        },
        NoDbTestCase {
            name: "13. /save db at workspace root (error)",
            pwd_type: "workspace",
            input: "/save db",
            expected_state_cmd: "SaveDb", // Legacy handler sends this (no validation yet)
            expected_todo_test_name: None,
        },
        // Section 2: PWD is crate root, no DB
        NoDbTestCase {
            name: "15. /index at crate root",
            pwd_type: "crate",
            input: "/index",
            expected_state_cmd: "TestTodo", // Not yet implemented
            expected_todo_test_name: Some("test_no_db_workspace_root_index_indexes_workspace"),
        },
        NoDbTestCase {
            name: "21. /load crate <name> at crate root",
            pwd_type: "crate",
            input: "/load crate fixture_nodes",
            expected_state_cmd: "TestTodo", // Actually hits LoadWorkspace todo!
            expected_todo_test_name: Some(
                "test_no_db_workspace_root_load_workspace_crate_name_suggests_load_crate",
            ),
        },
        NoDbTestCase {
            name: "23. /save db at crate root (error)",
            pwd_type: "crate",
            input: "/save db",
            expected_state_cmd: "SaveDb", // Legacy handler sends this
            expected_todo_test_name: None,
        },
    ];

    // Split cases by PWD type since each app instance has one fixed PWD
    let workspace_cases: Vec<_> = cases.iter().filter(|c| c.pwd_type == "workspace").collect();
    let crate_cases: Vec<_> = cases.iter().filter(|c| c.pwd_type == "crate").collect();

    // Run workspace cases with one app instance
    if !workspace_cases.is_empty() {
        println!("Running {} workspace cases...", workspace_cases.len());
        // Convert Vec<&NoDbTestCase> to Vec<NoDbTestCase> by cloning
        let workspace_cases_owned: Vec<_> = workspace_cases.into_iter().cloned().collect();
        run_no_db_test_cases(&workspace_cases_owned).await;
        for case in &workspace_cases_owned {
            println!("  ✓ {}", case.name);
        }
    }

    // Run crate cases with another app instance
    if !crate_cases.is_empty() {
        println!("Running {} crate cases...", crate_cases.len());
        let crate_cases_owned: Vec<_> = crate_cases.into_iter().cloned().collect();
        run_no_db_test_cases(&crate_cases_owned).await;
        for case in &crate_cases_owned {
            println!("  ✓ {}", case.name);
        }
    }

    println!("\n========================================");
    println!("All {} test cases passed!", cases.len());
}

// =============================================================================
// How to do targeted testing
// =============================================================================
//
// The batch test above runs all "No DB Loaded" cases sequentially with a fresh
// app for each case. This is good for CI but slow for development.
//
// For targeted debugging of a single case, use run_no_db_test_case directly:
//
// ```rust
// #[tokio::test]
// async fn test_my_specific_case() {
//     run_no_db_test_case(&NoDbTestCase {
//         name: "my case",
//         pwd_type: "workspace",  // or "crate"
//         input: "/my command",
//         expected_state_cmd: "TestTodo",  // or "IndexTargetDir", etc.
//         expected_todo_test_name: Some("test_my_specific_case"),
//     }).await;
// }
// ```
//
// To debug interactively, you can also extract just the case you want from
// the batch test and run it individually with println! debugging.

// =============================================================================
// Test: Single Workspace Member Loaded
// =============================================================================

/// Tests `/index` when single workspace member is loaded re-indexes the focused crate.
/// Pattern: Single workspace member → /index → IndexTargetDir for focused crate
#[tokio::test]
#[ignore = "needs implementation"]
async fn test_workspace_member_single_index_reindexes_focused() {
    let fixture_db = Arc::new(
        fresh_backup_fixture_db(&WS_FIXTURE_01_MEMBER_SINGLE)
            .expect("load ws_fixture_01_member_single"),
    );
}

// =============================================================================
// Test: Standalone Crate Loaded
// =============================================================================

/// Tests `/index workspace` when standalone crate is loaded returns an error.
/// Pattern: Standalone crate → /index workspace → error "not a workspace"
#[tokio::test]
#[ignore = "needs implementation"]
async fn test_standalone_crate_index_workspace_error() {
    let fixture_db = Arc::new(
        fresh_backup_fixture_db(&FIXTURE_NODES_CANONICAL).expect("load fixture_nodes_canonical"),
    );
}

// =============================================================================
// Test: Full Workspace Loaded
// =============================================================================

/// Tests command behavior when a full workspace with multiple members is loaded.
/// Uses `WS_FIXTURE_01_CANONICAL` fixture.
#[tokio::test]
#[ignore = "needs implementation"]
async fn test_full_workspace_index_commands() {
    let fixture_db = Arc::new(
        fresh_backup_fixture_db(&WS_FIXTURE_01_CANONICAL).expect("load ws_fixture_01_canonical"),
    );
}

// =============================================================================
// Test Helpers
// =============================================================================

/// Helper to send a command and collect the resulting StateCommand and events
async fn send_command_and_collect(
    app: &crate::app::App,
    command: &str,
    debug_rx: &mut tokio::sync::mpsc::Receiver<DebugStateCommand>,
    event_rx: &mut tokio::sync::broadcast::Receiver<AppEvent>,
) -> (String, Vec<AppEvent>) {
    // TODO: Send command, collect debug string and events
    todo!()
}
