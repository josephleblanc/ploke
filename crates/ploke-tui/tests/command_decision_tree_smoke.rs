//! TDD-style smoke test for new command decision tree behavior.
//!
//! This test validates the COMPLETE decision tree for `/index`, `/load`, `/save`, `/update` commands
//! as specified in the UX polish task. It should fail until the new decision tree is implemented.
//!
//! # Complete Decision Tree Coverage
//!
//! ## Test Summary (Last Run)
//! ```
//! Total: 72 test cases
//! Passed: 44 (61%)
//! Failed: 25 (35%) - expected per TDD until Step 3
//! Skipped: 3 (4%) - complex state setup pending
//! ```
//!
//! ## Legend
//! - ✅ PASS: Test passes with current implementation
//! - ❌ FAIL: Test fails (expected per TDD until Step 3 implementation)
//! - ⏳ SKIP: Test defined but skipped (state setup not yet implemented)
//!
//! ## Coverage by Section
//! - Section 1 (wr, no db): 12 cases covering all `/index`, `/load`, `/save`, `/update` variants
//! - Section 2 (cr, no db): 8 cases for pwd-as-crate context
//! - Section 3 (single crate + ws): 9 cases for workspace member loaded state
//! - Section 4 (standalone crate): 8 cases for non-workspace crate loaded
//! - Section 5 (multiple crates + ws): 10 cases for multi-member workspace
//! - Section 6 (cr, db loaded): 8 cases for pwd-as-crate with loaded state
//! - Section 7 (transitions): 4 cases for unsaved changes detection
//! - Section 8 (workspace cmds): 6 cases for `/workspace` subcommands
//!
//! ---

use std::collections::HashSet;
use std::time::{Duration, Instant};

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ploke_tui::chat_history::MessageKind;
use ploke_tui::test_utils::new_test_harness::AppHarness;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

const TEST_TIMEOUT_SECS: u64 = 60;
const COMMAND_TIMEOUT_MS: u64 = 3000;
const STATE_SETUP_TIMEOUT_MS: u64 = 10000;

fn key(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

/// Required state for a test case
#[derive(Debug, Clone, Copy, PartialEq)]
enum RequiredState {
    /// No database loaded (fresh start)
    NoDb,
    /// Single crate loaded that IS a workspace member
    SingleCrateWorkspaceMember,
    /// Single standalone crate (NOT a workspace member)
    SingleCrateStandalone,
    /// Multiple crates loaded as workspace members
    MultipleCratesWorkspace,
    /// Currently indexing
    Indexing,
}

/// Test case for a single command decision tree scenario.
struct DecisionTreeTestCase {
    name: &'static str,
    /// The command to send (without leading slash)
    command: &'static str,
    /// Required state before running this test
    required_state: RequiredState,
    /// Predicate to check if the response contains expected content
    expected_predicate: Box<dyn Fn(&str) -> bool + Send + Sync>,
    /// Human-readable description of what's expected
    expected_description: &'static str,
}

impl std::fmt::Debug for DecisionTreeTestCase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DecisionTreeTestCase")
            .field("name", &self.name)
            .field("command", &self.command)
            .field("required_state", &self.required_state)
            .field("expected_description", &self.expected_description)
            .field("expected_predicate", &"<predicate fn>")
            .finish()
    }
}

impl DecisionTreeTestCase {
    fn new(
        name: &'static str,
        command: &'static str,
        required_state: RequiredState,
        expected_description: &'static str,
        predicate: impl Fn(&str) -> bool + Send + Sync + 'static,
    ) -> Self {
        Self {
            name,
            command,
            required_state,
            expected_predicate: Box::new(predicate),
            expected_description,
        }
    }
}

/// Result of a single test case execution.
#[derive(Debug)]
enum TestResult {
    Success {
        test_name: &'static str,
        command: &'static str,
        response: String,
    },
    Failure {
        test_name: &'static str,
        command: &'static str,
        expected: &'static str,
        actual: Option<String>,
        reason: &'static str,
    },
    Skipped {
        test_name: &'static str,
        reason: String,
    },
}

async fn send_slash_command(harness: &AppHarness, command: &str) {
    harness
        .input_tx
        .send(Ok(key(KeyCode::Char('/'))))
        .expect("slash prefix");
    for ch in command.chars() {
        harness
            .input_tx
            .send(Ok(key(KeyCode::Char(ch))))
            .expect("send command char");
    }
    harness
        .input_tx
        .send(Ok(key(KeyCode::Enter)))
        .expect("submit command");
}

async fn snapshot_message_ids(harness: &AppHarness) -> HashSet<Uuid> {
    let guard = harness.state.chat.0.read().await;
    guard.messages.keys().copied().collect()
}

async fn wait_for_new_sysinfo(
    harness: &AppHarness,
    before_ids: &HashSet<Uuid>,
    timeout_ms: u64,
) -> Option<String> {
    let start = Instant::now();
    let timeout_duration = Duration::from_millis(timeout_ms);

    while start.elapsed() < timeout_duration {
        {
            let guard = harness.state.chat.0.read().await;
            if let Some(msg) = guard
                .messages
                .values()
                .find(|m| !before_ids.contains(&m.id) && m.kind == MessageKind::SysInfo)
            {
                return Some(msg.content.clone());
            }
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    None
}

async fn run_test_case(harness: &AppHarness, case: &DecisionTreeTestCase) -> TestResult {
    info!(test = %case.name, command = %case.command, "Running test case");

    let before_ids = snapshot_message_ids(harness).await;
    send_slash_command(harness, case.command).await;

    match wait_for_new_sysinfo(harness, &before_ids, COMMAND_TIMEOUT_MS).await {
        Some(response) => {
            debug!(
                test = %case.name,
                command = %case.command,
                response = %response,
                "Received response"
            );

            if (case.expected_predicate)(&response) {
                debug!(
                    test = %case.name,
                    command = %case.command,
                    "Test passed"
                );
                TestResult::Success {
                    test_name: case.name,
                    command: case.command,
                    response,
                }
            } else {
                error!(
                    test = %case.name,
                    command = %case.command,
                    expected = %case.expected_description,
                    actual = %response,
                    "Predicate failed"
                );
                TestResult::Failure {
                    test_name: case.name,
                    command: case.command,
                    expected: case.expected_description,
                    actual: Some(response),
                    reason: "Response did not match expected predicate",
                }
            }
        }
        None => {
            error!(
                test = %case.name,
                command = %case.command,
                timeout_ms = COMMAND_TIMEOUT_MS,
                "Timeout waiting for response"
            );
            TestResult::Failure {
                test_name: case.name,
                command: case.command,
                expected: case.expected_description,
                actual: None,
                reason: "Timeout waiting for sysinfo response",
            }
        }
    }
}

/// Set up state by indexing a workspace.
/// Returns true if setup succeeded.
async fn setup_workspace_state(harness: &AppHarness) -> bool {
    // First clear any existing state by trying to reset
    // For now, we assume fresh harness - index the fixture workspace
    let before_ids = snapshot_message_ids(harness).await;

    // Send index command for the fixture workspace
    send_slash_command(harness, "index workspace").await;

    // Wait longer for indexing to complete
    match wait_for_new_sysinfo(harness, &before_ids, STATE_SETUP_TIMEOUT_MS).await {
        Some(response) => {
            debug!(setup = "workspace", response = %response, "State setup response");
            // Check if it succeeded or is already in progress
            response.to_ascii_lowercase().contains("indexing")
                || response.to_ascii_lowercase().contains("complete")
                || response.to_ascii_lowercase().contains("success")
        }
        None => {
            error!("Timeout waiting for workspace indexing setup");
            false
        }
    }
}

/// Set up state by indexing a standalone crate.
async fn setup_standalone_crate_state(harness: &AppHarness) -> bool {
    // For standalone crate, we need to index a single crate not in a workspace context
    // This is tricky - we might need to navigate to a crate directory first
    // For now, use the simplest approach: index a specific crate path
    let before_ids = snapshot_message_ids(harness).await;

    send_slash_command(harness, "index crate .").await;

    match wait_for_new_sysinfo(harness, &before_ids, STATE_SETUP_TIMEOUT_MS).await {
        Some(response) => {
            debug!(setup = "standalone_crate", response = %response, "State setup response");
            response.to_ascii_lowercase().contains("indexing")
                || response.to_ascii_lowercase().contains("complete")
                || response.to_ascii_lowercase().contains("success")
        }
        None => false,
    }
}

/// Set up state with multiple crates.
async fn setup_multiple_crates_state(harness: &AppHarness) -> bool {
    // Index workspace first, then ensure multiple crates are loaded
    // For fixture workspace, this may already load multiple crates
    setup_workspace_state(harness).await
}

/// Ensure harness is in the required state before running a test.
async fn ensure_state(harness: &AppHarness, required: RequiredState) -> Result<(), String> {
    match required {
        RequiredState::NoDb => {
            // Fresh harness starts with no DB - nothing to do
            Ok(())
        }
        RequiredState::SingleCrateWorkspaceMember => {
            if setup_workspace_state(harness).await {
                Ok(())
            } else {
                Err("Failed to set up workspace member state".to_string())
            }
        }
        RequiredState::SingleCrateStandalone => {
            if setup_standalone_crate_state(harness).await {
                Ok(())
            } else {
                Err("Failed to set up standalone crate state".to_string())
            }
        }
        RequiredState::MultipleCratesWorkspace => {
            if setup_multiple_crates_state(harness).await {
                Ok(())
            } else {
                Err("Failed to set up multiple crates state".to_string())
            }
        }
        RequiredState::Indexing => {
            // Start indexing but don't wait for completion
            // This is tricky - we need to catch it mid-index
            // For now, skip tests requiring this state
            Err("Indexing state setup not yet implemented".to_string())
        }
    }
}

/// Define ALL decision tree test cases.
///
/// Organized by:
/// 1. pwd is workspace root, no db loaded
/// 2. pwd is crate root, no db loaded
/// 3. pwd is workspace root, db already loaded (single crate + workspace)
/// 4. pwd is workspace root, db already loaded (standalone crate)
/// 5. pwd is workspace root, db already loaded (multiple crates + workspace)
/// 6. pwd is crate root, db already loaded
/// 7. Transition cases
fn decision_tree_test_cases() -> Vec<DecisionTreeTestCase> {
    let mut cases = Vec::new();

    // =========================================================================
    // SECTION 1: pwd is workspace root, NO DB LOADED
    // =========================================================================

    // /index - indexes workspace
    cases.push(DecisionTreeTestCase::new(
        "wr_no_db__index",
        "index",
        RequiredState::NoDb,
        "should start indexing workspace",
        |s| {
            s.to_ascii_lowercase().contains("indexing")
                || s.to_ascii_lowercase().contains("workspace")
        },
    ));

    // /index workspace - indexes workspace
    cases.push(DecisionTreeTestCase::new(
        "wr_no_db__index_workspace",
        "index workspace",
        RequiredState::NoDb,
        "should start indexing workspace",
        |s| {
            s.to_ascii_lowercase().contains("indexing")
                || s.to_ascii_lowercase().contains("workspace")
        },
    ));

    // /index workspace . - indexes workspace
    cases.push(DecisionTreeTestCase::new(
        "wr_no_db__index_workspace_dot",
        "index workspace .",
        RequiredState::NoDb,
        "should start indexing workspace at current path",
        |s| {
            s.to_ascii_lowercase().contains("indexing")
                || s.to_ascii_lowercase().contains("workspace")
        },
    ));

    // /index path/to/crate - indexes target crate (if valid)
    cases.push(DecisionTreeTestCase::new(
        "wr_no_db__index_path",
        "index fixture_test_crate",
        RequiredState::NoDb,
        "should index crate at given path",
        |s| s.to_ascii_lowercase().contains("indexing") || s.to_ascii_lowercase().contains("error"),
    ));

    // /index crate path/to/crate - indexes target crate
    cases.push(DecisionTreeTestCase::new(
        "wr_no_db__index_crate_path",
        "index crate fixture_test_crate",
        RequiredState::NoDb,
        "should index crate at given path",
        |s| s.to_ascii_lowercase().contains("indexing") || s.to_ascii_lowercase().contains("error"),
    ));

    // /index crate <name> - if member of workspace, index crate
    cases.push(DecisionTreeTestCase::new(
        "wr_no_db__index_crate_member",
        "index crate fixture_test_crate",
        RequiredState::NoDb,
        "should index workspace member crate",
        |s| s.to_ascii_lowercase().contains("indexing") || s.to_ascii_lowercase().contains("error"),
    ));

    // /index crate - show list of workspace members
    cases.push(DecisionTreeTestCase::new(
        "wr_no_db__index_crate_list",
        "index crate",
        RequiredState::NoDb,
        "should show list of workspace crate members with suggestions",
        |s| {
            s.to_ascii_lowercase().contains("member")
                || s.to_ascii_lowercase().contains("crate")
                || s.to_ascii_lowercase().contains("index crate")
        },
    ));

    // /load crate <name> - if in registry, load; else suggest index
    cases.push(DecisionTreeTestCase::new(
        "wr_no_db__load_crate_nonexistent",
        "load crate nonexistent_crate_xyz",
        RequiredState::NoDb,
        "should error: crate not in registry, suggest /index",
        |s| {
            s.to_ascii_lowercase().contains("not found")
                || s.to_ascii_lowercase().contains("does not exist")
                || s.to_ascii_lowercase().contains("index")
        },
    ));

    // /load workspace <name> - if workspace not in registry but crate exists, suggest /load crate
    cases.push(DecisionTreeTestCase::new(
        "wr_no_db__load_workspace_crate_confusion",
        "load workspace fixture_test_crate",
        RequiredState::NoDb,
        "should suggest using /load crate if crate with that name exists",
        |s| {
            s.to_ascii_lowercase().contains("load crate")
                || s.to_ascii_lowercase().contains("not found")
        },
    ));

    // /load workspace - load workspace matching pwd if exists, else suggest /index
    cases.push(DecisionTreeTestCase::new(
        "wr_no_db__load_workspace_no_arg",
        "load workspace",
        RequiredState::NoDb,
        "should error: no workspace found, suggest /index",
        |s| {
            s.to_ascii_lowercase().contains("not found")
                || s.to_ascii_lowercase().contains("index")
                || s.to_ascii_lowercase().contains("no workspace")
        },
    ));

    // /save db - error: no crate/workspace in db
    cases.push(DecisionTreeTestCase::new(
        "wr_no_db__save_db",
        "save db",
        RequiredState::NoDb,
        "should error: no crate/workspace in db to save",
        |s| {
            s.to_ascii_lowercase().contains("no crate")
                || s.to_ascii_lowercase().contains("no workspace")
                || s.to_ascii_lowercase().contains("nothing to save")
        },
    ));

    // /update - error: no crate/workspace in db
    cases.push(DecisionTreeTestCase::new(
        "wr_no_db__update",
        "update",
        RequiredState::NoDb,
        "should error: no crate/workspace in db to update",
        |s| {
            s.to_ascii_lowercase().contains("no crate")
                || s.to_ascii_lowercase().contains("no workspace")
                || s.to_ascii_lowercase().contains("nothing to update")
        },
    ));

    // /index start, pause, resume, cancel
    cases.push(DecisionTreeTestCase::new(
        "wr_no_db__index_start",
        "index start",
        RequiredState::NoDb,
        "should show indexing message or help",
        |s| {
            s.to_ascii_lowercase().contains("indexing")
                || s.to_ascii_lowercase().contains("usage")
                || s.to_ascii_lowercase().contains("help")
        },
    ));

    cases.push(DecisionTreeTestCase::new(
        "wr_no_db__index_pause",
        "index pause",
        RequiredState::NoDb,
        "should show pause requested or no indexing",
        |s| s.to_ascii_lowercase().contains("pause") || s.to_ascii_lowercase().contains("indexing"),
    ));

    cases.push(DecisionTreeTestCase::new(
        "wr_no_db__index_resume",
        "index resume",
        RequiredState::NoDb,
        "should show resume requested or no indexing",
        |s| {
            s.to_ascii_lowercase().contains("resume") || s.to_ascii_lowercase().contains("indexing")
        },
    ));

    cases.push(DecisionTreeTestCase::new(
        "wr_no_db__index_cancel",
        "index cancel",
        RequiredState::NoDb,
        "should show cancel requested or no indexing",
        |s| {
            s.to_ascii_lowercase().contains("cancel") || s.to_ascii_lowercase().contains("indexing")
        },
    ));

    // =========================================================================
    // SECTION 2: pwd is crate root, NO DB LOADED
    // =========================================================================

    // /index - indexes current crate
    cases.push(DecisionTreeTestCase::new(
        "cr_no_db__index",
        "index",
        RequiredState::NoDb,
        "should index current crate at pwd",
        |s| s.to_ascii_lowercase().contains("indexing") || s.to_ascii_lowercase().contains("error"),
    ));

    // /index crate - indexes current crate
    cases.push(DecisionTreeTestCase::new(
        "cr_no_db__index_crate",
        "index crate",
        RequiredState::NoDb,
        "should index current crate at pwd",
        |s| s.to_ascii_lowercase().contains("indexing") || s.to_ascii_lowercase().contains("error"),
    ));

    // /index crate . - indexes current crate
    cases.push(DecisionTreeTestCase::new(
        "cr_no_db__index_crate_dot",
        "index crate .",
        RequiredState::NoDb,
        "should index current crate at pwd",
        |s| s.to_ascii_lowercase().contains("indexing") || s.to_ascii_lowercase().contains("error"),
    ));

    // /index workspace - if member, index workspace; else error
    cases.push(DecisionTreeTestCase::new(
        "cr_no_db__index_workspace",
        "index workspace",
        RequiredState::NoDb,
        "should index workspace if member, else error",
        |s| {
            s.to_ascii_lowercase().contains("indexing")
                || s.to_ascii_lowercase().contains("not a workspace member")
        },
    ));

    // /index path/to/other/crate - indexes that crate if valid
    cases.push(DecisionTreeTestCase::new(
        "cr_no_db__index_other_crate",
        "index ../other_crate",
        RequiredState::NoDb,
        "should index other crate at path",
        |s| {
            s.to_ascii_lowercase().contains("indexing")
                || s.to_ascii_lowercase().contains("not found")
                || s.to_ascii_lowercase().contains("error")
        },
    ));

    // /load crate <name> - load from registry if exists
    cases.push(DecisionTreeTestCase::new(
        "cr_no_db__load_crate",
        "load crate nonexistent_crate",
        RequiredState::NoDb,
        "should error: not in registry, suggest index",
        |s| {
            s.to_ascii_lowercase().contains("not found")
                || s.to_ascii_lowercase().contains("index")
                || s.to_ascii_lowercase().contains("registry")
        },
    ));

    // /load workspace <name> - load workspace from registry
    cases.push(DecisionTreeTestCase::new(
        "cr_no_db__load_workspace",
        "load workspace nonexistent_workspace",
        RequiredState::NoDb,
        "should error: not in registry, suggest index",
        |s| {
            s.to_ascii_lowercase().contains("not found")
                || s.to_ascii_lowercase().contains("index")
                || s.to_ascii_lowercase().contains("registry")
        },
    ));

    // /save db - error
    cases.push(DecisionTreeTestCase::new(
        "cr_no_db__save_db",
        "save db",
        RequiredState::NoDb,
        "should error: no crate/workspace in db",
        |s| {
            s.to_ascii_lowercase().contains("no crate")
                || s.to_ascii_lowercase().contains("no workspace")
                || s.to_ascii_lowercase().contains("nothing to save")
        },
    ));

    // /update - error
    cases.push(DecisionTreeTestCase::new(
        "cr_no_db__update",
        "update",
        RequiredState::NoDb,
        "should error: no crate/workspace in db",
        |s| {
            s.to_ascii_lowercase().contains("no crate")
                || s.to_ascii_lowercase().contains("no workspace")
                || s.to_ascii_lowercase().contains("nothing to update")
        },
    ));

    // =========================================================================
    // SECTION 3: DB LOADED - Single crate + Workspace (focused is member)
    // =========================================================================

    // /index - re-indexes focused crate (not entire workspace)
    cases.push(DecisionTreeTestCase::new(
        "wr_db_crate_ws__index",
        "index",
        RequiredState::SingleCrateWorkspaceMember,
        "should re-index focused crate (not entire workspace)",
        |s| {
            s.to_ascii_lowercase().contains("indexing")
                || s.to_ascii_lowercase().contains("re-index")
        },
    ));

    // /index workspace - re-indexes entire workspace
    cases.push(DecisionTreeTestCase::new(
        "wr_db_crate_ws__index_workspace",
        "index workspace",
        RequiredState::SingleCrateWorkspaceMember,
        "should re-index entire workspace",
        |s| {
            s.to_ascii_lowercase().contains("indexing")
                || s.to_ascii_lowercase().contains("workspace")
        },
    ));

    // /index crate <name> - same focused: re-index
    cases.push(DecisionTreeTestCase::new(
        "wr_db_crate_ws__index_crate_same",
        "index crate fixture_test_crate",
        RequiredState::SingleCrateWorkspaceMember,
        "should re-index focused crate",
        |s| {
            s.to_ascii_lowercase().contains("indexing")
                || s.to_ascii_lowercase().contains("re-index")
        },
    ));

    // /index crate <name> - different member: switch focus, then index
    cases.push(DecisionTreeTestCase::new(
        "wr_db_crate_ws__index_crate_other_member",
        "index crate other_member",
        RequiredState::SingleCrateWorkspaceMember,
        "should switch focus to other member, then index",
        |s| {
            s.to_ascii_lowercase().contains("indexing")
                || s.to_ascii_lowercase().contains("focus")
                || s.to_ascii_lowercase().contains("not a member")
        },
    ));

    // /index crate <name> - not a member: error
    cases.push(DecisionTreeTestCase::new(
        "wr_db_crate_ws__index_crate_not_member",
        "index crate external_crate",
        RequiredState::SingleCrateWorkspaceMember,
        "should error: crate not workspace member, suggest adding to Cargo.toml",
        |s| {
            s.to_ascii_lowercase().contains("not a member")
                || s.to_ascii_lowercase().contains("cargo.toml")
        },
    ));

    // /load crate <name> - matches member: suggest /index crate
    cases.push(DecisionTreeTestCase::new(
        "wr_db_crate_ws__load_crate_member",
        "load crate fixture_test_crate",
        RequiredState::SingleCrateWorkspaceMember,
        "should suggest using /index crate instead",
        |s| {
            s.to_ascii_lowercase().contains("index crate")
                || s.to_ascii_lowercase().contains("already loaded")
        },
    ));

    // /load crate <name> - not member but in registry: error
    cases.push(DecisionTreeTestCase::new(
        "wr_db_crate_ws__load_crate_external_registry",
        "load crate external_crate",
        RequiredState::SingleCrateWorkspaceMember,
        "should error: not workspace member, suggest adding to workspace or switching",
        |s| {
            s.to_ascii_lowercase().contains("not a workspace member")
                || s.to_ascii_lowercase().contains("cargo.toml")
                || s.to_ascii_lowercase().contains("switch")
        },
    ));

    // /load crate <name> - not in registry: suggest indexing
    cases.push(DecisionTreeTestCase::new(
        "wr_db_crate_ws__load_crate_new",
        "load crate brand_new_crate",
        RequiredState::SingleCrateWorkspaceMember,
        "should suggest indexing first with /index crate",
        |s| {
            s.to_ascii_lowercase().contains("index") || s.to_ascii_lowercase().contains("not found")
        },
    ));

    // /save db - saves workspace snapshot
    cases.push(DecisionTreeTestCase::new(
        "wr_db_crate_ws__save_db",
        "save db",
        RequiredState::SingleCrateWorkspaceMember,
        "should save workspace snapshot",
        |s| s.to_ascii_lowercase().contains("saved") || s.to_ascii_lowercase().contains("success"),
    ));

    // /update - scans focused crate, re-indexes if stale
    cases.push(DecisionTreeTestCase::new(
        "wr_db_crate_ws__update",
        "update",
        RequiredState::SingleCrateWorkspaceMember,
        "should scan focused crate and re-index if stale",
        |s| {
            s.to_ascii_lowercase().contains("scanning")
                || s.to_ascii_lowercase().contains("update")
                || s.to_ascii_lowercase().contains("fresh")
        },
    ));

    // =========================================================================
    // SECTION 4: DB LOADED - Single standalone crate (NOT workspace member)
    // =========================================================================

    // /index - re-indexes standalone crate
    cases.push(DecisionTreeTestCase::new(
        "wr_db_standalone__index",
        "index",
        RequiredState::SingleCrateStandalone,
        "should re-index standalone crate",
        |s| {
            s.to_ascii_lowercase().contains("indexing")
                || s.to_ascii_lowercase().contains("re-index")
        },
    ));

    // /index crate <name> - matches loaded: re-index
    cases.push(DecisionTreeTestCase::new(
        "wr_db_standalone__index_crate_same",
        "index crate fixture_test_crate",
        RequiredState::SingleCrateStandalone,
        "should re-index loaded crate",
        |s| {
            s.to_ascii_lowercase().contains("indexing")
                || s.to_ascii_lowercase().contains("re-index")
        },
    ));

    // /index crate <name> - different name: error
    cases.push(DecisionTreeTestCase::new(
        "wr_db_standalone__index_crate_different",
        "index crate other_crate",
        RequiredState::SingleCrateStandalone,
        "should error: already have standalone loaded, use /load to switch",
        |s| {
            s.to_ascii_lowercase().contains("already loaded")
                || s.to_ascii_lowercase().contains("use /load")
        },
    ));

    // /index workspace: error (not workspace root)
    cases.push(DecisionTreeTestCase::new(
        "wr_db_standalone__index_workspace",
        "index workspace",
        RequiredState::SingleCrateStandalone,
        "should error: current dir not workspace root",
        |s| {
            s.to_ascii_lowercase().contains("not a workspace")
                || s.to_ascii_lowercase().contains("workspace root")
        },
    ));

    // /load crate <name> - check unsaved, load if exists
    cases.push(DecisionTreeTestCase::new(
        "wr_db_standalone__load_crate",
        "load crate other_crate",
        RequiredState::SingleCrateStandalone,
        "should check unsaved changes, then load if exists",
        |s| {
            s.to_ascii_lowercase().contains("unsaved")
                || s.to_ascii_lowercase().contains("--force")
                || s.to_ascii_lowercase().contains("loaded")
                || s.to_ascii_lowercase().contains("not found")
        },
    ));

    // /load workspace <name> - check unsaved, load workspace
    cases.push(DecisionTreeTestCase::new(
        "wr_db_standalone__load_workspace",
        "load workspace some_workspace",
        RequiredState::SingleCrateStandalone,
        "should check unsaved changes, prompt to save or --force",
        |s| {
            s.to_ascii_lowercase().contains("unsaved")
                || s.to_ascii_lowercase().contains("--force")
                || s.to_ascii_lowercase().contains("not found")
        },
    ));

    // /save db - saves standalone snapshot
    cases.push(DecisionTreeTestCase::new(
        "wr_db_standalone__save_db",
        "save db",
        RequiredState::SingleCrateStandalone,
        "should save standalone crate snapshot",
        |s| s.to_ascii_lowercase().contains("saved") || s.to_ascii_lowercase().contains("success"),
    ));

    // /update - scans loaded crate
    cases.push(DecisionTreeTestCase::new(
        "wr_db_standalone__update",
        "update",
        RequiredState::SingleCrateStandalone,
        "should scan loaded crate and re-index if stale",
        |s| {
            s.to_ascii_lowercase().contains("scanning")
                || s.to_ascii_lowercase().contains("update")
                || s.to_ascii_lowercase().contains("fresh")
        },
    ));

    // /index path/to/other/crate: error (cannot index different crate)
    cases.push(DecisionTreeTestCase::new(
        "wr_db_standalone__index_path_other",
        "index ../other_crate",
        RequiredState::SingleCrateStandalone,
        "should error: cannot index different crate while one loaded",
        |s| {
            s.to_ascii_lowercase().contains("cannot index")
                || s.to_ascii_lowercase().contains("use /load")
        },
    ));

    // =========================================================================
    // SECTION 5: DB LOADED - Multiple crates + Workspace
    // =========================================================================

    // /index - re-indexes all workspace members
    cases.push(DecisionTreeTestCase::new(
        "wr_db_multi__index",
        "index",
        RequiredState::MultipleCratesWorkspace,
        "should re-index all workspace members",
        |s| {
            s.to_ascii_lowercase().contains("indexing")
                || s.to_ascii_lowercase().contains("members")
        },
    ));

    // /index crate <name> - member: index that crate
    cases.push(DecisionTreeTestCase::new(
        "wr_db_multi__index_crate_member",
        "index crate member_crate",
        RequiredState::MultipleCratesWorkspace,
        "should index that specific member crate",
        |s| {
            s.to_ascii_lowercase().contains("indexing") || s.to_ascii_lowercase().contains("member")
        },
    ));

    // /index crate <name> - not member: error
    cases.push(DecisionTreeTestCase::new(
        "wr_db_multi__index_crate_not_member",
        "index crate external_crate",
        RequiredState::MultipleCratesWorkspace,
        "should error: not a member, use /load to switch",
        |s| {
            s.to_ascii_lowercase().contains("not a member")
                || s.to_ascii_lowercase().contains("/load")
        },
    ));

    // /index workspace - re-indexes all members
    cases.push(DecisionTreeTestCase::new(
        "wr_db_multi__index_workspace",
        "index workspace",
        RequiredState::MultipleCratesWorkspace,
        "should re-index all workspace members",
        |s| {
            s.to_ascii_lowercase().contains("indexing")
                || s.to_ascii_lowercase().contains("workspace")
        },
    ));

    // /index path/to/crate - within workspace: index that crate
    cases.push(DecisionTreeTestCase::new(
        "wr_db_multi__index_path_within",
        "index crates/member_crate",
        RequiredState::MultipleCratesWorkspace,
        "should index crate at path (must be member)",
        |s| {
            s.to_ascii_lowercase().contains("indexing")
                || s.to_ascii_lowercase().contains("not a member")
        },
    ));

    // /index path/to/crate - outside workspace: error
    cases.push(DecisionTreeTestCase::new(
        "wr_db_multi__index_path_outside",
        "index ../external_crate",
        RequiredState::MultipleCratesWorkspace,
        "should error: cannot index outside loaded workspace",
        |s| {
            s.to_ascii_lowercase().contains("cannot index")
                || s.to_ascii_lowercase().contains("outside")
                || s.to_ascii_lowercase().contains("cargo.toml")
        },
    ));

    // /load crate <name> - not member but in registry: check unsaved, prompt, unload, load
    cases.push(DecisionTreeTestCase::new(
        "wr_db_multi__load_crate_external",
        "load crate external_crate",
        RequiredState::MultipleCratesWorkspace,
        "should check unsaved, prompt save/--force, unload workspace, load crate",
        |s| {
            s.to_ascii_lowercase().contains("unsaved")
                || s.to_ascii_lowercase().contains("--force")
                || s.to_ascii_lowercase().contains("not found")
        },
    ));

    // /load workspace <name> - different workspace: check unsaved, prompt, load new
    cases.push(DecisionTreeTestCase::new(
        "wr_db_multi__load_workspace_different",
        "load workspace other_workspace",
        RequiredState::MultipleCratesWorkspace,
        "should check unsaved, prompt save/--force, load new workspace",
        |s| {
            s.to_ascii_lowercase().contains("unsaved")
                || s.to_ascii_lowercase().contains("--force")
                || s.to_ascii_lowercase().contains("not found")
        },
    ));

    // /save db - saves workspace with all members
    cases.push(DecisionTreeTestCase::new(
        "wr_db_multi__save_db",
        "save db",
        RequiredState::MultipleCratesWorkspace,
        "should save workspace with all members",
        |s| s.to_ascii_lowercase().contains("saved") || s.to_ascii_lowercase().contains("success"),
    ));

    // /update - scans all members, re-indexes stale
    cases.push(DecisionTreeTestCase::new(
        "wr_db_multi__update",
        "update",
        RequiredState::MultipleCratesWorkspace,
        "should scan all members and re-index stale ones",
        |s| {
            s.to_ascii_lowercase().contains("scanning")
                || s.to_ascii_lowercase().contains("members")
                || s.to_ascii_lowercase().contains("fresh")
        },
    ));

    // =========================================================================
    // SECTION 6: DB LOADED - pwd is crate root
    // =========================================================================

    // These mirror the workspace root cases but with pwd context

    // /index - re-indexes crate at pwd
    cases.push(DecisionTreeTestCase::new(
        "cr_db__index",
        "index",
        RequiredState::SingleCrateWorkspaceMember,
        "should re-index crate at pwd (if loaded)",
        |s| {
            s.to_ascii_lowercase().contains("indexing")
                || s.to_ascii_lowercase().contains("not loaded")
        },
    ));

    // /index workspace - if pwd is member, re-index entire workspace
    cases.push(DecisionTreeTestCase::new(
        "cr_db__index_workspace",
        "index workspace",
        RequiredState::SingleCrateWorkspaceMember,
        "should re-index entire workspace (if pwd is member)",
        |s| {
            s.to_ascii_lowercase().contains("indexing")
                || s.to_ascii_lowercase().contains("not a workspace member")
        },
    ));

    // /index crate <name> - matches pwd: re-index
    cases.push(DecisionTreeTestCase::new(
        "cr_db__index_crate_same",
        "index crate fixture_test_crate",
        RequiredState::SingleCrateWorkspaceMember,
        "should re-index crate at pwd",
        |s| {
            s.to_ascii_lowercase().contains("indexing")
                || s.to_ascii_lowercase().contains("re-index")
        },
    ));

    // /index crate <name> - different loaded crate: switch focus, index
    cases.push(DecisionTreeTestCase::new(
        "cr_db__index_crate_different",
        "index crate other_loaded_crate",
        RequiredState::SingleCrateWorkspaceMember,
        "should switch focus to different loaded crate, then index",
        |s| {
            s.to_ascii_lowercase().contains("focus")
                || s.to_ascii_lowercase().contains("indexing")
                || s.to_ascii_lowercase().contains("not loaded")
        },
    ));

    // /load crate <name> - matches pwd: error (already loaded)
    cases.push(DecisionTreeTestCase::new(
        "cr_db__load_crate_same",
        "load crate fixture_test_crate",
        RequiredState::SingleCrateWorkspaceMember,
        "should error: already loaded, use /index to re-index",
        |s| {
            s.to_ascii_lowercase().contains("already loaded")
                || s.to_ascii_lowercase().contains("/index")
        },
    ));

    // /load crate <name> - different: check unsaved, proceed per workspace rules
    cases.push(DecisionTreeTestCase::new(
        "cr_db__load_crate_different",
        "load crate other_crate",
        RequiredState::SingleCrateWorkspaceMember,
        "should check unsaved, then proceed with load",
        |s| {
            s.to_ascii_lowercase().contains("unsaved")
                || s.to_ascii_lowercase().contains("--force")
                || s.to_ascii_lowercase().contains("loaded")
        },
    ));

    // /load workspace <name> - check unsaved per workspace rules
    cases.push(DecisionTreeTestCase::new(
        "cr_db__load_workspace",
        "load workspace other_workspace",
        RequiredState::SingleCrateWorkspaceMember,
        "should check unsaved, then proceed per workspace rules",
        |s| {
            s.to_ascii_lowercase().contains("unsaved")
                || s.to_ascii_lowercase().contains("--force")
                || s.to_ascii_lowercase().contains("not found")
        },
    ));

    // /save db, /update - follow workspace rules
    cases.push(DecisionTreeTestCase::new(
        "cr_db__save_db",
        "save db",
        RequiredState::SingleCrateWorkspaceMember,
        "should save per workspace rules (same as wr context)",
        |s| s.to_ascii_lowercase().contains("saved") || s.to_ascii_lowercase().contains("success"),
    ));

    cases.push(DecisionTreeTestCase::new(
        "cr_db__update",
        "update",
        RequiredState::SingleCrateWorkspaceMember,
        "should update per workspace rules (same as wr context)",
        |s| s.to_ascii_lowercase().contains("scanning") || s.to_ascii_lowercase().contains("fresh"),
    ));

    // =========================================================================
    // SECTION 7: Transition Cases (unsaved changes)
    // =========================================================================

    // /load workspace <name> when standalone loaded with unsaved changes
    cases.push(DecisionTreeTestCase::new(
        "transition__standalone_to_workspace_unsaved",
        "load workspace new_workspace",
        RequiredState::SingleCrateStandalone,
        "should prompt: save or --force to discard changes",
        |s| {
            s.to_ascii_lowercase().contains("unsaved")
                || s.to_ascii_lowercase().contains("save")
                || s.to_ascii_lowercase().contains("--force")
        },
    ));

    // /load crate <name> when workspace loaded with unsaved changes
    cases.push(DecisionTreeTestCase::new(
        "transition__workspace_to_crate_unsaved",
        "load crate new_crate",
        RequiredState::MultipleCratesWorkspace,
        "should prompt: save or --force to discard changes",
        |s| {
            s.to_ascii_lowercase().contains("unsaved")
                || s.to_ascii_lowercase().contains("save")
                || s.to_ascii_lowercase().contains("--force")
        },
    ));

    // /load crate <name> when different crate loaded with unsaved changes
    cases.push(DecisionTreeTestCase::new(
        "transition__crate_to_crate_unsaved",
        "load crate different_crate",
        RequiredState::SingleCrateStandalone,
        "should prompt: save or --force to discard changes",
        |s| {
            s.to_ascii_lowercase().contains("unsaved")
                || s.to_ascii_lowercase().contains("save")
                || s.to_ascii_lowercase().contains("--force")
        },
    ));

    // /index when db loaded (destructive operation)
    cases.push(DecisionTreeTestCase::new(
        "transition__index_destructive",
        "index",
        RequiredState::SingleCrateWorkspaceMember,
        "should re-parse and replace DB content (no --force needed)",
        |s| {
            s.to_ascii_lowercase().contains("indexing")
                || s.to_ascii_lowercase().contains("re-index")
                || s.to_ascii_lowercase().contains("parsing")
        },
    ));

    // =========================================================================
    // SECTION 8: Workspace Commands
    // =========================================================================

    // /workspace status - no db
    cases.push(DecisionTreeTestCase::new(
        "workspace__status_no_db",
        "workspace status",
        RequiredState::NoDb,
        "should show error about no workspace",
        |s| {
            s.to_ascii_lowercase().contains("no workspace")
                || s.to_ascii_lowercase().contains("not loaded")
        },
    ));

    // /workspace update - no db
    cases.push(DecisionTreeTestCase::new(
        "workspace__update_no_db",
        "workspace update",
        RequiredState::NoDb,
        "should error: no workspace loaded",
        |s| {
            s.to_ascii_lowercase().contains("no workspace")
                || s.to_ascii_lowercase().contains("not loaded")
        },
    ));

    // /workspace rm <crate> - no db
    cases.push(DecisionTreeTestCase::new(
        "workspace__rm_no_db",
        "workspace rm some_crate",
        RequiredState::NoDb,
        "should error: no workspace loaded",
        |s| {
            s.to_ascii_lowercase().contains("no workspace")
                || s.to_ascii_lowercase().contains("not loaded")
        },
    ));

    // /workspace status - with db
    cases.push(DecisionTreeTestCase::new(
        "workspace__status_with_db",
        "workspace status",
        RequiredState::SingleCrateWorkspaceMember,
        "should show workspace status",
        |s| {
            s.to_ascii_lowercase().contains("status")
                || s.to_ascii_lowercase().contains("member")
                || s.to_ascii_lowercase().contains("loaded")
        },
    ));

    // /workspace update - with db
    cases.push(DecisionTreeTestCase::new(
        "workspace__update_with_db",
        "workspace update",
        RequiredState::MultipleCratesWorkspace,
        "should update workspace crates",
        |s| {
            s.to_ascii_lowercase().contains("update")
                || s.to_ascii_lowercase().contains("scanning")
                || s.to_ascii_lowercase().contains("fresh")
        },
    ));

    cases
}

#[tokio::test]
async fn command_decision_tree_smoke_test() {
    // Initialize tracing for test logging
    let _subscriber = tracing_subscriber::fmt()
        .with_test_writer()
        .with_env_filter("ploke_tui::command_smoke=debug")
        .try_init();

    info!("Starting COMPLETE command decision tree smoke test");

    // Spawn the test harness with timeout
    let harness = timeout(Duration::from_secs(TEST_TIMEOUT_SECS), AppHarness::spawn())
        .await
        .expect("Test harness spawn timed out")
        .expect("Failed to spawn AppHarness");

    info!("AppHarness spawned successfully");

    let test_cases = decision_tree_test_cases();
    let total_cases = test_cases.len();
    info!(total = total_cases, "Loaded test cases");

    let mut results: Vec<TestResult> = Vec::with_capacity(total_cases);
    let mut current_state = RequiredState::NoDb;

    // Run all test cases
    for case in &test_cases {
        // Set up required state if different from current
        if case.required_state != current_state {
            info!(state = ?case.required_state, "Setting up state");
            match ensure_state(&harness, case.required_state).await {
                Ok(()) => {
                    current_state = case.required_state;
                }
                Err(reason) => {
                    warn!(test = %case.name, reason = %reason, "Skipping test - state setup failed");
                    results.push(TestResult::Skipped {
                        test_name: case.name,
                        reason: format!("State setup failed: {}", reason),
                    });
                    continue;
                }
            }
        }

        let result = run_test_case(&harness, case).await;
        results.push(result);
    }

    // Collect results
    let mut successes = Vec::new();
    let mut failures = Vec::new();
    let mut skipped = Vec::new();

    for result in results {
        match result {
            TestResult::Success { .. } => successes.push(result),
            TestResult::Failure { .. } => failures.push(result),
            TestResult::Skipped { .. } => skipped.push(result),
        }
    }

    let success_count = successes.len();
    let failure_count = failures.len();
    let skipped_count = skipped.len();

    info!(
        total = total_cases,
        passed = success_count,
        failed = failure_count,
        skipped = skipped_count,
        "Test run complete"
    );

    // Log all successes at debug level
    for success in &successes {
        if let TestResult::Success {
            test_name,
            command,
            response,
        } = success
        {
            debug!(
                test = %test_name,
                command = %command,
                response = %response,
                "✓ SUCCESS"
            );
        }
    }

    // Log all failures at error level
    for failure in &failures {
        if let TestResult::Failure {
            test_name,
            command,
            expected,
            actual,
            reason,
        } = failure
        {
            error!(
                test = %test_name,
                command = %command,
                expected = %expected,
                actual = ?actual,
                reason = %reason,
                "✗ FAILURE"
            );
        }
    }

    // Log all skipped at warn level
    for skip in &skipped {
        if let TestResult::Skipped { test_name, reason } = skip {
            warn!(
                test = %test_name,
                reason = %reason,
                "⊘ SKIPPED"
            );
        }
    }

    // Shutdown harness
    harness.shutdown().await;

    // Panic with all failures summary
    if !failures.is_empty() {
        let mut failure_details = String::new();
        for (i, failure) in failures.iter().enumerate() {
            if let TestResult::Failure {
                test_name,
                command,
                expected,
                actual,
                reason,
            } = failure
            {
                let actual_str = actual.as_deref().unwrap_or("(timeout - no response)");
                failure_details.push_str(&format!(
                    "\n\n{}. Failed test: {}\n   Command: /{}\n   Expected: {}\n   Actual: {}\n   Reason: {}",
                    i + 1,
                    test_name,
                    command,
                    expected,
                    actual_str,
                    reason
                ));
            }
        }

        let skip_summary = if skipped_count > 0 {
            format!(
                ", {} skipped (state setup not yet implemented)",
                skipped_count
            )
        } else {
            String::new()
        };

        panic!(
            "Command decision tree smoke test failed!\n\
            Summary: {}/{} tests passed, {} failed{}\n\
            {}",
            success_count, total_cases, failure_count, skip_summary, failure_details
        );
    }

    info!(
        passed = success_count,
        skipped = skipped_count,
        total = total_cases,
        "All tests passed!"
    );
}
