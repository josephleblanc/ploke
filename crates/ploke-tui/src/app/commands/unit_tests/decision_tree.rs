//! Event-based smoke tests for command decision tree.
//!
//! This test file validates the command parser + executor/validator component
//! via its event interface. Tests are grouped by DB state for efficiency.
//!
//! ## Test Coverage
//!
//! ### No DB Loaded (Section 1 & 2)
//! - 100% coverage (24/24 cases defined)
//! - 24/24 cases tested
//! - 2/24 cases implemented (/save db error handling)
//! - 22/24 cases pending TestTodo
//!
//! ### Single Workspace Member Loaded (Section 3)
//! - 100% coverage (10/10 cases defined)
//! - 10/10 cases tested
//! - 0/10 cases implemented
//!
//! ### Standalone Crate Loaded (Section 4)
//! - 100% coverage (9/9 cases defined)
//! - 9/9 cases tested
//! - 0/9 cases implemented
//!
//! ### Full Workspace Loaded (Section 5)
//! - 100% coverage (12/12 cases defined)
//! - 12/12 cases tested
//! - 0/12 cases implemented
//!
//! ### DB Loaded, PWD is Crate (Section 6)
//! - 100% coverage (11/11 cases defined)
//! - 11/11 cases tested
//! - 0/11 cases implemented
//!
//! ### Transition Cases (Section 7)
//! - 100% coverage (4/4 cases defined)
//! - 4/4 cases tested
//! - 0/4 cases implemented
//!
//! Total: 70 test cases (0/70 implemented)

use std::sync::Arc;
use std::time::Duration;

use crate::app::commands::unit_tests::harness::{
    DebugStateCommand, TestRuntime, ValidationProbeEvent,
};
use crate::app::commands::{exec, parser};
use crate::app_state::core::WorkspaceFreshness;
use crate::test_support::config_home_lock;
use crate::user_config::{CommandStyle, WorkspaceRegistry, WorkspaceRegistryEntry};
use ploke_core::WorkspaceInfo;
use ploke_test_utils::{
    FIXTURE_NODES_CANONICAL, WS_FIXTURE_01_CANONICAL, WS_FIXTURE_01_MEMBER_SINGLE,
    fresh_backup_fixture_db,
};
use tempfile::{TempDir, tempdir};
use tokio::time::timeout;

// =============================================================================
// Test: No DB Loaded
// =============================================================================
//
// Section 1: PWD is workspace root, no DB
//  1. [x] pwd is workspace root, no db: `/index`                           indexes workspace
//  2. [x] pwd is workspace root, no db: `/index workspace`                 indexes workspace
//  3. [x] pwd is workspace root, no db: `/index workspace .`               indexes workspace
//  4. [x] pwd is workspace root, no db: `/index path/to/crate`             indexes target crate
//  5. [x] pwd is workspace root, no db: `/index crate path/to/crate`       indexes target crate
//  6. [x] pwd is workspace root, no db: `/index crate <name>`              indexes workspace member
//  7. [x] pwd is workspace root, no db: `/index crate`                     lists members + suggests command
//  8. [x] pwd is workspace root, no db: `/load crate <exists>`             loads from registry
//  9. [x] pwd is workspace root, no db: `/load crate <not exists>`         suggests index if member
// 10. [x] pwd is workspace root, no db: `/load crate <not exists>`         lists crates + suggests
// 11. [x] pwd is workspace root, no db: `/load workspace <crate-name>`     suggests `/load crate`
// 12. [x] pwd is workspace root, no db: `/load workspace`                  loads if exists, else suggests `/index`
// 13. [✓] pwd is workspace root, no db: `/save db`                         error (AddMessageImmediate)
// 14. [x] pwd is workspace root, no db: `/update`                          error (TestTodo)
//
// Section 2: PWD is crate root, no DB
// 15. [x] pwd is crate, no db:          `/index`                           indexes current crate
// 16. [x] pwd is crate, no db:          `/index crate`                     indexes current crate
// 17. [x] pwd is crate, no db:          `/index crate .`                   indexes current crate
// 18. [x] pwd is crate, no db:          `/index workspace`                 indexes full workspace if member
// 19. [x] pwd is crate, no db:          `/index workspace`                 error if not member
// 20. [x] pwd is crate, no db:          `/index path/to/crate`             indexes that crate
// 21. [x] pwd is crate, no db:          `/load crate <name>`               loads from registry or suggests index
// 22. [x] pwd is crate, no db:          `/load workspace <name>`           loads or suggests index workspace root
// 23. [✓] pwd is crate, no db:          `/save db`                         error (AddMessageImmediate)
// 24. [x] pwd is crate, no db:          `/update`                          error (TestTodo)
//
// Note: All cases are defined and tested. [x] indicates test exists (expects TestTodo until implemented)
//
// ## TDD Workflow for Implementing Tests
//
// 1. **Initial State**: All cases start with `expected_state_cmd: "TestTodo"` and run with `[PENDING]` status
//
// 2. **When Implementing a Command**:
//    - Replace `TestTodo` with the actual StateCommand discriminant (e.g., "IndexTargetDir", "AddMessageImmediate")
//    - If the command contains a message/error, set `expected_msg_contains` to verify content
//    - Remove `expected_todo_test_name` (only used for TestTodo)
//
// 3. **Test Output Will Show**:
//    - `[PENDING]` - Command not yet implemented (TestTodo received)
//    - `[IMPLEMENTED]` - Command matches expected_state_cmd (and expected_msg_contains if set)
//    - **PANIC** - Implementation returns wrong command or message
//
// Example transition for case 13 (/save db error):
// ```rust
// // BEFORE (TDD - pending):
// expected_state_cmd: "TestTodo",
// expected_todo_test_name: Some("test_no_db_workspace_root_save_db_error"),
// expected_msg_contains: None,
//
// // AFTER (implemented):
// expected_state_cmd: "AddMessageImmediate",
// expected_todo_test_name: None,
// expected_msg_contains: Some("No crate or workspace is loaded"),
// ```

/// Database setup variants for test cases.
/// Each variant maps to a fixture that will be loaded before running the test cases.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
enum DbSetup {
    /// Fresh empty database (no data loaded)
    #[default]
    None,
    /// Single workspace member loaded (WS_FIXTURE_01_MEMBER_SINGLE)
    SingleMember,
    /// Full workspace with multiple members (WS_FIXTURE_01_CANONICAL)
    FullWorkspace,
    /// Standalone crate not in any workspace (FIXTURE_NODES_CANONICAL)
    StandaloneCrate,
}

/// Working directory variants for test cases.
/// Each variant carries the path to use as the working directory.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum TestPwd {
    /// Working directory is a workspace root
    Workspace(&'static str),
    /// Working directory is a crate root
    Crate(&'static str),
}

impl Default for TestPwd {
    fn default() -> Self {
        TestPwd::Workspace("")
    }
}

impl TestPwd {
    /// Returns the path string for filesystem operations
    fn path(&self) -> &'static str {
        match self {
            TestPwd::Workspace(p) | TestPwd::Crate(p) => p,
        }
    }

    /// Returns true if this is a workspace PWD
    fn is_workspace(&self) -> bool {
        matches!(self, TestPwd::Workspace(_))
    }
}

/// Validation expectation for command execution
#[derive(Clone, Debug, PartialEq)]
pub enum ValidationExpectation {
    /// No validation required (command doesn't need preconditions)
    None,
    /// Validation should succeed
    Success,
    /// Validation should fail with optional reason
    Failure { reason: Option<String> },
}

/// Expected user-facing error information
#[derive(Clone, Debug)]
pub struct ExpectedUiError {
    /// Substring that should appear in the error message
    pub message_contains: Option<String>,
    /// Optional recovery command suggestion
    pub recovery_suggestion: Option<String>,
}

/// Stable `/load` contract expectation captured from parser and forwarded state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadExpectation {
    /// Parsed/forwarded load family.
    pub kind: parser::LoadKind,
    /// Optional target name or path.
    pub name: Option<&'static str>,
    /// Whether the command was forced.
    pub force: bool,
}

/// Test parameters for command decision tree tests.
/// Works for any DB state (None, SingleMember, FullWorkspace, StandaloneCrate).
#[derive(Clone)]
struct TestCase {
    /// Human-readable test name for error messages
    name: &'static str,
    /// Database setup: which fixture to load (or None for empty)
    db_setup: DbSetup,
    /// Working directory: workspace or crate path
    pwd: TestPwd,
    /// The command string to type (e.g., "/index", "/save db")
    input: &'static str,
    /// The expected StateCommand discriminant when implemented.
    /// Examples: "IndexTargetDir", "AddMessageImmediate", etc.
    /// If the actual command is TestTodo, the test will pass but mark as [PENDING].
    expected_state_cmd: &'static str,
    /// Optional: substring to check for in the command's debug representation
    /// Used to verify error messages or other content in the final implementation
    expected_msg_contains: Option<&'static str>,
    /// Optional: expected test_name field if the command emits TestTodo.
    /// Used to verify the right decision tree branch is being hit during TDD.
    expected_todo_test_name: Option<&'static str>,
    /// Optional: substring expected in the parsed `Command` debug output.
    /// Used to keep parse expectations centralized in the canonical table.
    expected_parsed_contains: Option<&'static str>,
    /// Optional: substring expected in the first forwarded `StateCommand`.
    /// Used to verify the executor forwards intent without resolving early.
    expected_forwarded_contains: Option<&'static str>,
    /// Optional: exact `/load` contract assertion for parser + forwarded state command.
    expected_load_contract: Option<LoadExpectation>,
    /// Optional: exact resolved `/load` target after state-side classification.
    expected_resolved_load_ref_contains: Option<&'static str>,
    /// Optional: substring expected in the resolved `/index` target directory.
    /// This keeps successful state-side resolution in the canonical contract.
    expected_resolved_index_target_contains: Option<&'static str>,
    /// Optional: substring expected in the focus hint emitted by `/index`.
    /// This keeps focus-switch behavior explicit without forcing mutation into resolve().
    expected_focus_root_contains: Option<&'static str>,
    /// Expected validation result (new field, defaults to None for backward compatibility)
    expected_validation: ValidationExpectation,
    /// Marks the currently loaded state stale before the command is sent.
    stale_loaded_state: bool,
    /// Expected user-facing error (if any)
    expected_error: ExpectedUiError,
}

impl Default for ExpectedUiError {
    fn default() -> Self {
        Self {
            message_contains: None,
            recovery_suggestion: None,
        }
    }
}

impl TestCase {
    /// Builder method to create a TestCase with common fields, using defaults for new fields.
    fn new(
        name: &'static str,
        db_setup: DbSetup,
        pwd: TestPwd,
        input: &'static str,
        expected_state_cmd: &'static str,
        expected_msg_contains: Option<&'static str>,
        expected_todo_test_name: Option<&'static str>,
    ) -> Self {
        let mut case = Self {
            name,
            db_setup,
            pwd,
            input,
            expected_state_cmd,
            expected_msg_contains,
            expected_todo_test_name,
            expected_parsed_contains: None,
            expected_forwarded_contains: None,
            expected_load_contract: None,
            expected_resolved_load_ref_contains: None,
            expected_resolved_index_target_contains: None,
            expected_focus_root_contains: None,
            expected_validation: ValidationExpectation::None,
            stale_loaded_state: false,
            expected_error: ExpectedUiError {
                message_contains: None,
                recovery_suggestion: None,
            },
        };

        if input.starts_with("/index") {
            case.with_index_contract()
        } else {
            case
        }
    }
}

impl Default for TestCase {
    fn default() -> Self {
        Self {
            name: "",
            db_setup: DbSetup::None,
            pwd: TestPwd::Workspace(""),
            input: "",
            expected_state_cmd: "",
            expected_msg_contains: None,
            expected_todo_test_name: None,
            expected_parsed_contains: None,
            expected_forwarded_contains: None,
            expected_load_contract: None,
            expected_resolved_load_ref_contains: None,
            expected_resolved_index_target_contains: None,
            expected_focus_root_contains: None,
            expected_validation: ValidationExpectation::None,
            stale_loaded_state: false,
            expected_error: ExpectedUiError::default(),
        }
    }
}

impl TestCase {
    fn with_index_contract(mut self) -> Self {
        self.expected_parsed_contains = Some("Index");
        self.expected_forwarded_contains = Some("Index(");
        self
    }

    fn with_resolved_index_target(mut self, expected: &'static str) -> Self {
        self.expected_resolved_index_target_contains = Some(expected);
        self
    }

    fn with_focus_root(mut self, expected: &'static str) -> Self {
        self.expected_focus_root_contains = Some(expected);
        self
    }

    fn with_load_contract(
        mut self,
        kind: parser::LoadKind,
        name: Option<&'static str>,
        force: bool,
    ) -> Self {
        self.expected_state_cmd = "Load";
        self.expected_parsed_contains = Some("Load {");
        self.expected_forwarded_contains = Some("Load(LoadCmd {");
        self.expected_load_contract = Some(LoadExpectation { kind, name, force });
        self
    }

    fn with_resolved_load_ref(mut self, expected: &'static str) -> Self {
        self.expected_resolved_load_ref_contains = Some(expected);
        self
    }

    fn with_resolve_ui_error(mut self, expected: ExpectedUiError) -> Self {
        self.expected_error = expected;
        self
    }

    fn with_stale_loaded_state(mut self) -> Self {
        self.stale_loaded_state = true;
        self
    }

    fn with_error(mut self, expected: ExpectedUiError) -> Self {
        self.with_resolve_ui_error(expected)
    }
}

impl Default for ValidationExpectation {
    fn default() -> Self {
        ValidationExpectation::None
    }
}

// Backwards compatibility alias - all NoDbTestCase usages should work
pub type NoDbTestCase = TestCase;

struct XdgConfigHomeGuard {
    old_xdg: Option<String>,
}

impl XdgConfigHomeGuard {
    fn set_to(path: &std::path::Path) -> Self {
        let old_xdg = std::env::var("XDG_CONFIG_HOME").ok();
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", path);
        }
        Self { old_xdg }
    }
}

impl Drop for XdgConfigHomeGuard {
    fn drop(&mut self) {
        if let Some(old_xdg) = self.old_xdg.take() {
            unsafe {
                std::env::set_var("XDG_CONFIG_HOME", old_xdg);
            }
        } else {
            unsafe {
                std::env::remove_var("XDG_CONFIG_HOME");
            }
        }
    }
}

struct LoadRegistrySandbox {
    _lock: tokio::sync::MutexGuard<'static, ()>,
    _tmp_dir: TempDir,
    _xdg_guard: XdgConfigHomeGuard,
}

async fn setup_load_registry() -> LoadRegistrySandbox {
    let lock = config_home_lock().lock().await;
    let tmp_dir = tempdir().expect("temp xdg config dir");
    let xdg_guard = XdgConfigHomeGuard::set_to(tmp_dir.path());

    let repo_root = ploke_test_utils::workspace_root();
    let fixture_crate_root = repo_root.join("tests/fixture_crates/fixture_nodes");

    let fixture_crate = WorkspaceInfo::from_root_path(fixture_crate_root.clone());
    let registry = WorkspaceRegistry {
        version: 1,
        entries: vec![WorkspaceRegistryEntry {
            workspace_id: fixture_crate.id.uuid().to_string(),
            workspace_name: fixture_crate.name.clone(),
            workspace_root: fixture_crate_root.clone(),
            snapshot_file: FIXTURE_NODES_CANONICAL.path(),
            focused_root: Some(fixture_crate_root.clone()),
            member_roots: vec![fixture_crate_root],
            active_embedding_set_rel: None,
        }],
    };
    registry
        .save_to_path(&WorkspaceRegistry::default_registry_path())
        .expect("save test workspace registry");

    LoadRegistrySandbox {
        _lock: lock,
        _tmp_dir: tmp_dir,
        _xdg_guard: xdg_guard,
    }
}

/// Helper to load a database fixture based on DbSetup variant.
fn load_db_fixture(setup: DbSetup) -> Arc<ploke_db::Database> {
    match setup {
        DbSetup::None => Arc::new(ploke_db::Database::init_with_schema().expect("create empty db")),
        DbSetup::SingleMember => Arc::new(
            fresh_backup_fixture_db(&WS_FIXTURE_01_MEMBER_SINGLE)
                .expect("load ws_fixture_01_member_single"),
        ),
        DbSetup::FullWorkspace => Arc::new(
            fresh_backup_fixture_db(&WS_FIXTURE_01_CANONICAL)
                .expect("load ws_fixture_01_canonical"),
        ),
        DbSetup::StandaloneCrate => Arc::new(
            fresh_backup_fixture_db(&FIXTURE_NODES_CANONICAL)
                .expect("load fixture_nodes_canonical"),
        ),
    }
}

/// Helper to resolve a TestPwd to an absolute PathBuf.
fn resolve_pwd(pwd: TestPwd) -> std::path::PathBuf {
    let base = pwd.path();
    if base.starts_with('/') {
        std::path::PathBuf::from(base)
    } else {
        ploke_test_utils::workspace_root().join(base)
    }
}

/// Runs multiple test cases grouped by (DbSetup, TestPwd) to minimize fixture loads.
///
/// Cases are automatically batched so each unique (fixture, pwd) combination only loads once.
async fn run_test_cases(cases: &[TestCase]) {
    use std::collections::HashMap;

    let _load_registry = setup_load_registry().await;

    let mut groups: HashMap<(DbSetup, TestPwd, bool), Vec<&TestCase>> = HashMap::new();
    for case in cases {
        groups
            .entry((case.db_setup, case.pwd, case.stale_loaded_state))
            .or_default()
            .push(case);
    }

    for ((db_setup, pwd, stale_loaded_state), group_cases) in groups {
        let db = load_db_fixture(db_setup);
        let rt = TestRuntime::new(&db).spawn_validation_probe();

        let events = rt.events_builder().build_app_only();
        let mut debug_rx = events
            .app_actor_events
            .debug_string_rx
            .expect("debug_string_rx should be available");
        let mut validation_rx = events
            .app_actor_events
            .validation_rx
            .expect("validation_rx should be available");

        let pwd_path = resolve_pwd(pwd);

        match db_setup {
            DbSetup::None => {}
            DbSetup::SingleMember => {
                rt.setup_loaded_workspace(
                    ploke_test_utils::workspace_root()
                        .join("tests/fixture_workspace/ws_fixture_01"),
                    vec![
                        ploke_test_utils::workspace_root()
                            .join("tests/fixture_workspace/ws_fixture_01/member_root"),
                    ],
                    Some(
                        ploke_test_utils::workspace_root()
                            .join("tests/fixture_workspace/ws_fixture_01/member_root"),
                    ),
                )
                .await;
            }
            DbSetup::FullWorkspace => {
                rt.setup_loaded_workspace(
                    ploke_test_utils::workspace_root()
                        .join("tests/fixture_workspace/ws_fixture_01"),
                    vec![
                        ploke_test_utils::workspace_root()
                            .join("tests/fixture_workspace/ws_fixture_01/member_root"),
                        ploke_test_utils::workspace_root()
                            .join("tests/fixture_workspace/ws_fixture_01/nested/member_nested"),
                    ],
                    Some(
                        ploke_test_utils::workspace_root()
                            .join("tests/fixture_workspace/ws_fixture_01/member_root"),
                    ),
                )
                .await;
            }
            DbSetup::StandaloneCrate => {
                rt.setup_loaded_standalone_crate(
                    ploke_test_utils::workspace_root().join("tests/fixture_crates/fixture_nodes"),
                )
                .await;
            }
        }

        if stale_loaded_state {
            rt.state_arc()
                .with_system_txn(|txn| {
                    for crate_id in txn.loaded_crate_ids() {
                        txn.set_workspace_freshness(crate_id, WorkspaceFreshness::Stale);
                    }
                })
                .await;
        }

        let mut app = rt.into_app_with_state_pwd(pwd_path).await;
        for case in group_cases {
            let trace =
                send_command_and_collect(&mut app, case.input, &mut debug_rx, &mut validation_rx)
                    .await;
            assert_case_trace(case, &trace);
        }
    }
}

struct CaseTrace {
    parsed: String,
    commands: Vec<DebugStateCommand>,
    validations: Vec<ValidationProbeEvent>,
}

fn assert_case_trace(case: &TestCase, trace: &CaseTrace) {
    assert_parsed_contract(case, trace);
    assert_forwarded_contract(case, trace);
    assert_effect_contract(case, trace);
}

fn assert_parsed_contract(case: &TestCase, trace: &CaseTrace) {
    if let Some(expected) = case.expected_parsed_contains {
        assert!(
            trace.parsed.contains(expected),
            "Test '{}' failed: parsed command '{}' did not contain '{}'",
            case.name,
            trace.parsed,
            expected
        );
    }
}

fn assert_forwarded_contract(case: &TestCase, trace: &CaseTrace) {
    if let Some(expected) = case.expected_forwarded_contains {
        let cmd = trace.commands.first().unwrap_or_else(|| {
            panic!("Test '{}' failed: no forwarded command captured", case.name)
        });
        assert!(
            cmd.as_str().contains(expected),
            "Test '{}' failed: forwarded command '{}' did not contain '{}'",
            case.name,
            cmd.as_str(),
            expected
        );
    }
}

fn assert_load_contract(case: &TestCase, trace: &CaseTrace) {
    let Some(expected) = &case.expected_load_contract else {
        return;
    };

    let expected_kind = format!("kind: {:?}", expected.kind);
    let expected_name = match expected.name {
        Some(name) => format!("name: Some(\"{}\")", name),
        None => "name: None".to_string(),
    };
    let expected_force = format!("force: {}", expected.force);

    assert!(
        trace.parsed.contains("Load {")
            && trace.parsed.contains(&expected_kind)
            && trace.parsed.contains(&expected_name)
            && trace.parsed.contains(&expected_force),
        "Test '{}' failed: parsed load command '{}' did not match {:?}",
        case.name,
        trace.parsed,
        expected
    );

    let cmd = trace
        .commands
        .first()
        .unwrap_or_else(|| panic!("Test '{}' failed: no forwarded command captured", case.name));
    let cmd_str = cmd.as_str();
    assert!(
        cmd_str.contains("Load(LoadCmd {")
            && cmd_str.contains(&expected_kind)
            && cmd_str.contains(&expected_name)
            && cmd_str.contains(&expected_force),
        "Test '{}' failed: forwarded load command '{}' did not match {:?}",
        case.name,
        cmd_str,
        expected
    );
}

fn assert_effect_contract(case: &TestCase, trace: &CaseTrace) {
    let commands = &trace.commands;
    let validations = &trace.validations;
    let is_index_case = case.input.starts_with("/index");

    assert_load_contract(case, trace);

    if let Some(expected_load_ref) = case.expected_resolved_load_ref_contains {
        let resolved_load_ref = validations.iter().find_map(|validation| {
            validation
                .resolved_load()
                .map(|resolution| resolution.workspace_ref.clone())
        });
        let resolved_load_ref = resolved_load_ref.unwrap_or_else(|| {
            panic!(
                "Test '{}' failed: expected resolved load ref containing '{}' but none was captured",
                case.name, expected_load_ref
            )
        });
        assert!(
            resolved_load_ref.contains(expected_load_ref),
            "Test '{}' failed: resolved load ref '{}' did not contain '{}'",
            case.name,
            resolved_load_ref,
            expected_load_ref
        );
    }

    if let Some(expected_target) = case.expected_resolved_index_target_contains {
        let resolved_target = validations.iter().find_map(|validation| {
            validation
                .resolved_index_target()
                .map(|resolution| resolution.target_dir.to_display_string())
        });
        let resolved_target = resolved_target.unwrap_or_else(|| {
            panic!(
                "Test '{}' failed: expected resolved index target containing '{}' but none was captured",
                case.name, expected_target
            )
        });
        assert!(
            resolved_target.contains(expected_target),
            "Test '{}' failed: resolved index target '{}' did not contain '{}'",
            case.name,
            resolved_target,
            expected_target
        );
    }

    if let Some(expected_focus_root) = case.expected_focus_root_contains {
        let actual_focus_root = validations.iter().find_map(|validation| {
            validation
                .focus_root()
                .map(|path| path.to_string_lossy().into_owned())
        });
        let actual_focus_root = actual_focus_root.unwrap_or_else(|| {
            panic!(
                "Test '{}' failed: expected focus root containing '{}' but none was captured",
                case.name, expected_focus_root
            )
        });
        assert!(
            actual_focus_root.contains(expected_focus_root),
            "Test '{}' failed: focus root '{}' did not contain '{}'",
            case.name,
            actual_focus_root,
            expected_focus_root
        );
    }

    match commands.first() {
        Some(cmd) => {
            let cmd_str = cmd.as_str();
            let validation = validations.first();

            if cmd_str.starts_with("TestTodo") {
                if case.expected_state_cmd == "TestTodo" {
                    let todo_name = case.expected_todo_test_name.unwrap_or("unknown");
                    println!(
                        "  [PENDING] {} - awaiting implementation (TestTodo: {})",
                        case.name, todo_name
                    );
                    assert!(
                        validation.is_none(),
                        "TestTodo should not require validation, got {:?}",
                        validation
                    );
                } else {
                    panic!(
                        "Test '{}' failed: expected '{}' but got TestTodo ({:?})",
                        case.name, case.expected_state_cmd, case.expected_todo_test_name
                    );
                }
            } else if cmd_str.starts_with("Index") && case.expected_state_cmd == "TestTodo" {
                let todo_name = case.expected_todo_test_name.unwrap_or("unknown");
                println!(
                    "  [FORWARDED] {} - intent forwarded ({}) while downstream behavior remains pending ({})",
                    case.name, cmd_str, todo_name
                );
            } else if cmd_str.starts_with("Workspace") && case.expected_state_cmd == "TestTodo" {
                let todo_name = case.expected_todo_test_name.unwrap_or("unknown");
                println!(
                    "  [FORWARDED] {} - intent forwarded ({}) while downstream behavior remains pending ({})",
                    case.name, cmd_str, todo_name
                );
            } else if cmd_str.starts_with("Load") && case.expected_state_cmd == "TestTodo" {
                let todo_name = case.expected_todo_test_name.unwrap_or("unknown");
                println!(
                    "  [FORWARDED] {} - intent forwarded ({}) while downstream behavior remains pending ({})",
                    case.name, cmd_str, todo_name
                );
            } else {
                let matched_command = commands.iter().find(|cmd| {
                    let cmd_str = cmd.as_str();
                    let discriminant_match = cmd_str.starts_with(case.expected_state_cmd);
                    let msg_match = if case.expected_state_cmd == "Workspace" {
                        true
                    } else {
                        case.expected_msg_contains
                            .map_or(true, |expected_msg| cmd_str.contains(expected_msg))
                    };
                    discriminant_match && msg_match
                });

                let matched_validation_error = if case.expected_state_cmd == "AddMessageImmediate" {
                    let expected_msg = case
                        .expected_msg_contains
                        .or(case.expected_error.message_contains.as_deref());
                    expected_msg.and_then(|expected| {
                        validation
                            .and_then(|v| v.error_message().filter(|msg| msg.contains(expected)))
                    })
                } else {
                    None
                };

                if matched_command.is_some() || matched_validation_error.is_some() {
                    if !is_index_case {
                        match &case.expected_validation {
                            ValidationExpectation::None => {
                                if matched_validation_error.is_none() {
                                    if let Some(validation) = validation {
                                        if let Some(result) = validation.validation() {
                                            assert!(
                                                result.is_ok(),
                                                "Implicitly validated command should not fail, got {:?}",
                                                validation
                                            );
                                        }
                                    }
                                }
                            }
                            ValidationExpectation::Success => {
                                let validation = validation.expect("validation event missing");
                                assert_eq!(
                                    validation.validation(),
                                    Some(&Ok(())),
                                    "Command should validate successfully"
                                );
                            }
                            ValidationExpectation::Failure { reason } => {
                                let validation = validation.expect("validation event missing");
                                match validation.validation() {
                                    Some(Err(err_msg)) => {
                                        if let Some(expected_reason) = reason {
                                            assert!(
                                                err_msg.contains(expected_reason),
                                                "Validation error should contain '{}', got '{}'",
                                                expected_reason,
                                                err_msg
                                            );
                                        }
                                    }
                                    other => {
                                        panic!("Expected validation failure, got {:?}", other)
                                    }
                                }
                            }
                        }
                    }

                    if case.expected_error.message_contains.is_some()
                        || case.expected_error.recovery_suggestion.is_some()
                    {
                        if let Some(expected_msg) = &case.expected_error.message_contains {
                            let actual_msg = validation
                                .and_then(|v| {
                                    v.resolve_error().or_else(|| v.error_message()).or_else(|| {
                                        match v.validation() {
                                            Some(Err(err_msg)) => Some(err_msg.as_str()),
                                            _ => None,
                                        }
                                    })
                                })
                                .or_else(|| {
                                    commands.iter().skip(1).find_map(|cmd| {
                                        if cmd.as_str().contains(expected_msg) {
                                            Some(cmd.as_str())
                                        } else {
                                            None
                                        }
                                    })
                                });
                            if let Some(actual_msg) = actual_msg {
                                assert!(
                                    actual_msg.contains(expected_msg),
                                    "Error message should contain '{}', got '{}'",
                                    expected_msg,
                                    actual_msg
                                );
                            } else {
                                panic!(
                                    "Expected error message containing '{}', but no error was emitted",
                                    expected_msg
                                );
                            }
                        }

                        if let Some(expected_recovery) = &case.expected_error.recovery_suggestion {
                            if let Some(actual_recovery) =
                                validation.and_then(|v| v.recovery_suggestion())
                            {
                                assert!(
                                    actual_recovery.contains(expected_recovery),
                                    "Recovery suggestion should contain '{}', got '{}'",
                                    expected_recovery,
                                    actual_recovery
                                );
                            } else {
                                panic!(
                                    "Expected recovery suggestion containing '{}', but none was provided",
                                    expected_recovery
                                );
                            }
                        }
                    }

                    println!(
                        "  [IMPLEMENTED] {} - {}",
                        case.name, case.expected_state_cmd
                    );
                } else {
                    let expected_desc = if let Some(msg) = case.expected_msg_contains {
                        format!("{} containing '{}'", case.expected_state_cmd, msg)
                    } else {
                        case.expected_state_cmd.to_string()
                    };
                    panic!(
                        "Test '{}' failed: Expected '{}' but got trace {:?} / validation {:?}",
                        case.name, expected_desc, commands, validations
                    );
                }
            }
        }
        None => panic!(
            "Test '{}' failed: Channel closed or no command captured",
            case.name
        ),
    }
}

/// Runs a single test case through the full app loop with `TestBackend`.
/// This is intentionally kept as a smoke test for the keystroke path only.
async fn run_test_case(case: &TestCase) {
    use crossterm::event::{Event, KeyCode, KeyEvent};
    use futures::StreamExt;
    use ratatui::{Terminal, backend::TestBackend};
    use tokio_stream::wrappers::UnboundedReceiverStream;

    let _load_registry = setup_load_registry().await;

    let db = load_db_fixture(case.db_setup);
    let rt = TestRuntime::new(&db).spawn_validation_probe();

    let events = rt.events_builder().build_app_only();
    let mut debug_rx = events
        .app_actor_events
        .debug_string_rx
        .expect("debug_string_rx should be available");
    let mut validation_rx = events
        .app_actor_events
        .validation_rx
        .expect("validation_rx should be available");

    let pwd_path = resolve_pwd(case.pwd);
    match case.db_setup {
        DbSetup::None => {}
        DbSetup::SingleMember => {
            rt.setup_loaded_workspace(
                ploke_test_utils::workspace_root().join("tests/fixture_workspace/ws_fixture_01"),
                vec![
                    ploke_test_utils::workspace_root()
                        .join("tests/fixture_workspace/ws_fixture_01/member_root"),
                ],
                Some(
                    ploke_test_utils::workspace_root()
                        .join("tests/fixture_workspace/ws_fixture_01/member_root"),
                ),
            )
            .await;
        }
        DbSetup::FullWorkspace => {
            rt.setup_loaded_workspace(
                ploke_test_utils::workspace_root().join("tests/fixture_workspace/ws_fixture_01"),
                vec![
                    ploke_test_utils::workspace_root()
                        .join("tests/fixture_workspace/ws_fixture_01/member_root"),
                    ploke_test_utils::workspace_root()
                        .join("tests/fixture_workspace/ws_fixture_01/nested/member_nested"),
                ],
                Some(
                    ploke_test_utils::workspace_root()
                        .join("tests/fixture_workspace/ws_fixture_01/member_root"),
                ),
            )
            .await;
        }
        DbSetup::StandaloneCrate => {
            rt.setup_loaded_standalone_crate(
                ploke_test_utils::workspace_root().join("tests/fixture_crates/fixture_nodes"),
            )
            .await;
        }
    }

    if case.stale_loaded_state {
        rt.state_arc()
            .with_system_txn(|txn| {
                for crate_id in txn.loaded_crate_ids() {
                    txn.set_workspace_freshness(crate_id, WorkspaceFreshness::Stale);
                }
            })
            .await;
    }

    let app = rt.into_app_with_state_pwd(pwd_path).await;
    let parsed_debug = format!("{:?}", parser::parse(&app, case.input, CommandStyle::Slash));
    let backend = TestBackend::new(80, 24);
    let terminal = Terminal::new(backend).expect("create terminal");
    let (input_tx, input_rx) =
        tokio::sync::mpsc::unbounded_channel::<Result<crossterm::event::Event, std::io::Error>>();
    let input = UnboundedReceiverStream::new(input_rx);

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

    for ch in case.input.chars() {
        input_tx
            .send(Ok(Event::Key(KeyEvent::from(KeyCode::Char(ch)))))
            .expect("send key");
        tokio::task::yield_now().await;
    }
    input_tx
        .send(Ok(Event::Key(KeyEvent::from(KeyCode::Enter))))
        .expect("send enter");

    let mut trace = collect_case_trace(&mut debug_rx, &mut validation_rx).await;
    trace.parsed = parsed_debug;
    app_task.abort();
    let _ = app_task.await;
    assert_case_trace(case, &trace);
}

// Backwards compatibility wrapper
async fn run_no_db_test_cases(cases: &[NoDbTestCase]) {
    // Convert NoDbTestCase (old struct) to TestCase (new struct with db_setup and pwd)
    // This is a shim for backwards compatibility until all callers are updated
    run_test_cases(cases).await;
}

async fn run_no_db_test_case(case: &NoDbTestCase) {
    run_test_case(case).await;
}

async fn collect_case_trace(
    debug_rx: &mut tokio::sync::mpsc::Receiver<DebugStateCommand>,
    validation_rx: &mut tokio::sync::mpsc::Receiver<ValidationProbeEvent>,
) -> CaseTrace {
    let mut commands = Vec::new();
    let mut validations = Vec::new();

    match timeout(Duration::from_millis(100), debug_rx.recv()).await {
        Ok(Some(cmd)) => commands.push(cmd),
        Ok(None) => {
            return CaseTrace {
                parsed: String::new(),
                commands,
                validations,
            };
        }
        Err(_) => {
            return CaseTrace {
                parsed: String::new(),
                commands,
                validations,
            };
        }
    }

    if let Ok(Some(validation)) = timeout(Duration::from_millis(100), validation_rx.recv()).await {
        validations.push(validation);
    }

    let mut idle_rounds = 0;
    while idle_rounds < 2 {
        let mut got_any = false;

        match timeout(Duration::from_millis(10), debug_rx.recv()).await {
            Ok(Some(cmd)) => {
                commands.push(cmd);
                got_any = true;
            }
            Ok(None) => break,
            Err(_) => {}
        }

        match timeout(Duration::from_millis(10), validation_rx.recv()).await {
            Ok(Some(validation)) => {
                validations.push(validation);
                got_any = true;
            }
            Ok(None) => break,
            Err(_) => {}
        }

        if got_any {
            idle_rounds = 0;
        } else {
            idle_rounds += 1;
            tokio::task::yield_now().await;
        }
    }

    CaseTrace {
        parsed: String::new(),
        commands,
        validations,
    }
}

/// Test case 1b: Same as test case 1, but using the full app run loop with TestBackend.
///
/// This test exercises the input-to-command path:
/// Key events → Action::ExecuteCommand → parser::parse → exec::execute → probe relay
#[tokio::test]
async fn test_no_db_workspace_root_index_indexes_workspace_full_app() {
    run_test_case(
        &TestCase::new(
            "/index at workspace root -> IndexTargetDir",
            DbSetup::None,
            TestPwd::Workspace("tests/fixture_workspace/ws_fixture_01"),
            "/index",
            "Index",
            None,
            Some("test_no_db_workspace_root_index_indexes_workspace"),
        )
        .with_resolved_index_target("tests/fixture_workspace/ws_fixture_01"),
    )
    .await;
}

/// Batch test: Runs all "No DB Loaded" test cases sequentially
///
/// This provides a quick overview of which cases are passing/failing.
/// For individual debugging, run the specific test case.
#[tokio::test]
async fn test_no_db_loaded_all_cases() {
    let cases = vec![
        // Section 1: PWD is workspace root, no DB (14 cases)
        TestCase::new(
            "1. /index at workspace root",
            DbSetup::None,
            TestPwd::Workspace("tests/fixture_workspace/ws_fixture_01"),
            "/index",
            "Index",
            None,
            None,
        )
        .with_resolved_index_target("tests/fixture_workspace/ws_fixture_01"),
        TestCase::new(
            "2. /index workspace at workspace root",
            DbSetup::None,
            TestPwd::Workspace("tests/fixture_workspace/ws_fixture_01"),
            "/index workspace",
            "Index",
            None,
            None,
        )
        .with_resolved_index_target("tests/fixture_workspace/ws_fixture_01"),
        TestCase::new(
            "3. /index workspace . at workspace root",
            DbSetup::None,
            TestPwd::Workspace("tests/fixture_workspace/ws_fixture_01"),
            "/index workspace .",
            "Index",
            None,
            None,
        )
        .with_resolved_index_target("tests/fixture_workspace/ws_fixture_01"),
        TestCase::new(
            "4. /index path/to/crate at workspace root",
            DbSetup::None,
            TestPwd::Workspace("tests/fixture_workspace/ws_fixture_01"),
            "/index member_root",
            "Index",
            None,
            None,
        )
        .with_resolved_index_target("tests/fixture_workspace/ws_fixture_01/member_root"),
        TestCase::new(
            "5. /index crate path/to/crate at workspace root",
            DbSetup::None,
            TestPwd::Workspace("tests/fixture_workspace/ws_fixture_01"),
            "/index crate member_root",
            "Index",
            None,
            None,
        )
        .with_resolved_index_target("tests/fixture_workspace/ws_fixture_01/member_root"),
        TestCase::new(
            "6. /index crate <member-name> at workspace root",
            DbSetup::None,
            TestPwd::Workspace("tests/fixture_workspace/ws_fixture_01"),
            "/index crate member_root",
            "Index",
            None,
            Some("test_no_db_workspace_root_index_crate_member_name"),
        )
        .with_resolved_index_target("tests/fixture_workspace/ws_fixture_01/member_root"),
        TestCase::new(
            "7. /index crate at workspace root (list members)",
            DbSetup::None,
            TestPwd::Workspace("tests/fixture_workspace/ws_fixture_01"),
            "/index crate",
            "TestTodo",
            None,
            Some("test_no_db_workspace_root_index_crate_lists_members"),
        ),
        TestCase::new(
            "8. /load crate <exists> at workspace root",
            DbSetup::None,
            TestPwd::Workspace("tests/fixture_workspace/ws_fixture_01"),
            "/load crate fixture_nodes",
            "Load",
            None,
            None,
        )
        .with_load_contract(parser::LoadKind::Crate, Some("fixture_nodes"), false)
        .with_resolved_load_ref("fixture_nodes"),
        TestCase::new(
            "9. /load crate <member> forwards to Workspace(LoadDb) for validation",
            DbSetup::None,
            TestPwd::Workspace("tests/fixture_workspace/ws_fixture_01"),
            "/load crate member_root",
            "Load",
            None,
            None,
        )
        .with_load_contract(parser::LoadKind::Crate, Some("member_root"), false)
        .with_resolve_ui_error(ExpectedUiError {
            message_contains: Some("No saved crate named 'member_root' was found.".to_string()),
            recovery_suggestion: Some(
                "`/index crate member_root` to index it before loading.".to_string(),
            ),
        }),
        TestCase::new(
            "10. /load crate <nonexistent> forwards to Workspace(LoadDb) for validation",
            DbSetup::None,
            TestPwd::Workspace("tests/fixture_workspace/ws_fixture_01"),
            "/load crate nonexistent_crate",
            "Load",
            None,
            None,
        )
        .with_load_contract(parser::LoadKind::Crate, Some("nonexistent_crate"), false)
        .with_resolve_ui_error(ExpectedUiError {
            message_contains: Some(
                "No saved crate named 'nonexistent_crate' was found.".to_string(),
            ),
            recovery_suggestion: Some(
                "`/index crate nonexistent_crate` to index it before loading.".to_string(),
            ),
        }),
        TestCase::new(
            "11. /load workspace <name> forwards to Workspace(LoadDb) for validation",
            DbSetup::None,
            TestPwd::Workspace("tests/fixture_workspace/ws_fixture_01"),
            "/load workspace member_root",
            "Load",
            None,
            None,
        )
        .with_load_contract(parser::LoadKind::Workspace, Some("member_root"), false)
        .with_resolve_ui_error(ExpectedUiError {
            message_contains: Some("Workspace 'member_root' was not found.".to_string()),
            recovery_suggestion: Some(
                "`/load crate member_root` if you meant the crate.".to_string(),
            ),
        }),
        TestCase::new(
            "12. /load workspace (no arg) loads or suggests /index",
            DbSetup::None,
            TestPwd::Workspace("tests/fixture_workspace/ws_fixture_01"),
            "/load workspace",
            "Load",
            None,
            None,
        )
        .with_load_contract(parser::LoadKind::Workspace, None, false)
        .with_resolve_ui_error(ExpectedUiError {
            message_contains: Some(
                "No saved workspace named 'ws_fixture_01' was found.".to_string(),
            ),
            recovery_suggestion: Some(
                "`/index workspace` from the workspace root before loading it.".to_string(),
            ),
        }),
        TestCase {
            expected_validation: ValidationExpectation::Failure {
                reason: Some("No crate or workspace is loaded".to_string()),
            },
            expected_error: ExpectedUiError {
                message_contains: Some("No crate or workspace is loaded".to_string()),
                recovery_suggestion: None,
            },
            ..TestCase::new(
                "13. /save db at workspace root (error - no db loaded)",
                DbSetup::None,
                TestPwd::Workspace("tests/fixture_workspace/ws_fixture_01"),
                "/save db",
                "Workspace",
                None,
                None,
            )
        },
        TestCase {
            expected_validation: ValidationExpectation::Failure {
                reason: Some("No crate or workspace is loaded".to_string()),
            },
            ..TestCase::new(
                "14. /update at workspace root (error - no db loaded)",
                DbSetup::None,
                TestPwd::Workspace("tests/fixture_workspace/ws_fixture_01"),
                "/update",
                "TestTodo",
                None,
                Some("test_no_db_workspace_root_update_error"),
            )
        },
        // Section 2: PWD is crate root, no DB (10 cases)
        TestCase::new(
            "15. /index at crate root",
            DbSetup::None,
            TestPwd::Crate("tests/fixture_crates/fixture_nodes"),
            "/index",
            "Index",
            None,
            None,
        )
        .with_resolved_index_target("tests/fixture_crates/fixture_nodes"),
        TestCase::new(
            "16. /index crate at crate root",
            DbSetup::None,
            TestPwd::Crate("tests/fixture_crates/fixture_nodes"),
            "/index crate",
            "Index",
            None,
            None,
        )
        .with_resolved_index_target("tests/fixture_crates/fixture_nodes"),
        TestCase::new(
            "17. /index crate . at crate root",
            DbSetup::None,
            TestPwd::Crate("tests/fixture_crates/fixture_nodes"),
            "/index crate .",
            "Index",
            None,
            None,
        )
        .with_resolved_index_target("tests/fixture_crates/fixture_nodes"),
        TestCase::new(
            "18. /index workspace at crate root (if member)",
            DbSetup::None,
            TestPwd::Crate("tests/fixture_crates/fixture_nodes"),
            "/index workspace",
            "TestTodo",
            None,
            Some("test_no_db_crate_root_index_workspace_if_member"),
        ),
        TestCase::new(
            "19. /index workspace at crate root (error if not member)",
            DbSetup::None,
            TestPwd::Crate("tests/fixture_crates/fixture_nodes"),
            "/index workspace",
            "TestTodo",
            None,
            Some("test_no_db_crate_root_index_workspace_not_member"),
        ),
        TestCase::new(
            "20. /index path/to/crate at crate root",
            DbSetup::None,
            TestPwd::Crate("tests/fixture_crates/fixture_nodes"),
            "/index /some/other/crate",
            "Index",
            None,
            Some("test_no_db_crate_root_index_path"),
        )
        .with_error(ExpectedUiError {
            message_contains: Some("Failed to normalize target path".to_string()),
            recovery_suggestion: None,
        }),
        TestCase::new(
            "21. /load crate <name> at crate root",
            DbSetup::None,
            TestPwd::Crate("tests/fixture_crates/fixture_nodes"),
            "/load crate fixture_nodes",
            "Load",
            None,
            None,
        )
        .with_load_contract(parser::LoadKind::Crate, Some("fixture_nodes"), false)
        .with_resolved_load_ref("fixture_nodes"),
        TestCase::new(
            "22. /load workspace <name> at crate root",
            DbSetup::None,
            TestPwd::Crate("tests/fixture_crates/fixture_nodes"),
            "/load workspace some_workspace",
            "Load",
            None,
            None,
        )
        .with_load_contract(parser::LoadKind::Workspace, Some("some_workspace"), false)
        .with_resolve_ui_error(ExpectedUiError {
            message_contains: Some(
                "No saved workspace named 'some_workspace' was found.".to_string(),
            ),
            recovery_suggestion: Some(
                "`/index workspace` from the workspace root before loading it.".to_string(),
            ),
        }),
        TestCase {
            expected_validation: ValidationExpectation::Failure {
                reason: Some("No crate or workspace is loaded".to_string()),
            },
            expected_error: ExpectedUiError {
                message_contains: Some("No crate or workspace is loaded".to_string()),
                recovery_suggestion: None,
            },
            ..TestCase::new(
                "23. /save db at crate root (error - no db loaded)",
                DbSetup::None,
                TestPwd::Crate("tests/fixture_crates/fixture_nodes"),
                "/save db",
                "Workspace",
                None,
                None,
            )
        },
        TestCase {
            expected_validation: ValidationExpectation::Failure {
                reason: Some("No crate or workspace is loaded".to_string()),
            },
            ..TestCase::new(
                "24. /update at crate root (error - no db loaded)",
                DbSetup::None,
                TestPwd::Crate("tests/fixture_crates/fixture_nodes"),
                "/update",
                "TestTodo",
                None,
                Some("test_no_db_crate_root_update_error"),
            )
        },
    ];

    // Run all cases - the runner automatically groups by (db_setup, pwd)
    run_test_cases(&cases).await;

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
//         pwd: TestPwd::Workspace("tests/fixture_workspace/ws_fixture_01"),  // or TestPwd::Crate(...)
//         input: "/my command",
//         expected_state_cmd: "TestTodo",  // or "IndexTargetDir", etc.
//         expected_msg_contains: None,
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
//
// - [ ] single crate + workspace member: `/index` re-indexes focused crate
// - [ ] single crate + workspace member: `/index workspace` re-indexes entire workspace
// - [ ] single crate + workspace member: `/index crate <focused>` re-indexes focused
// - [ ] single crate + workspace member: `/index crate <other member>` switches focus + indexes
// - [ ] single crate + workspace member: `/index crate <not member>` error + guidance
// - [ ] single crate + workspace member: `/load crate <member>` suggests `/index crate`
// - [ ] single crate + workspace member: `/load crate <not member>` error + guidance
// - [ ] single crate + workspace member: `/load crate <not in registry>` suggests index
// - [ ] single crate + workspace member: `/save db` saves workspace snapshot
// - [ ] single crate + workspace member: `/update` scans focused crate, re-indexes if stale

/// Batch test: Runs all "Single Workspace Member Loaded" test cases sequentially.
///
/// Uses `WS_FIXTURE_01_MEMBER_SINGLE` fixture which has one workspace member loaded.
/// This provides a quick overview of which cases are passing/failing.
#[tokio::test]
async fn test_single_member_all_cases() {
    let cases = vec![
        // Section 3: Single workspace member loaded (10 cases)
        TestCase::new(
            "3.1 /index re-indexes focused crate",
            DbSetup::SingleMember,
            TestPwd::Workspace("tests/fixture_workspace/ws_fixture_01"),
            "/index",
            "Index",
            None,
            None,
        )
        .with_resolved_index_target("tests/fixture_workspace/ws_fixture_01/member_root"),
        TestCase::new(
            "3.2 /index workspace re-indexes entire workspace",
            DbSetup::SingleMember,
            TestPwd::Workspace("tests/fixture_workspace/ws_fixture_01"),
            "/index workspace",
            "Index",
            None,
            None,
        )
        .with_resolved_index_target("tests/fixture_workspace/ws_fixture_01"),
        TestCase::new(
            "3.3 /index crate <focused> re-indexes focused",
            DbSetup::SingleMember,
            TestPwd::Workspace("tests/fixture_workspace/ws_fixture_01"),
            "/index crate member_root",
            "Index",
            None,
            None,
        )
        .with_resolved_index_target("tests/fixture_workspace/ws_fixture_01/member_root"),
        TestCase::new(
            "3.4 /index crate <other member> switches focus + indexes",
            DbSetup::SingleMember,
            TestPwd::Workspace("tests/fixture_workspace/ws_fixture_01"),
            "/index crate member_nested",
            "Index",
            None,
            None,
        )
        .with_resolved_index_target("tests/fixture_workspace/ws_fixture_01/nested/member_nested")
        .with_focus_root("tests/fixture_workspace/ws_fixture_01/nested/member_nested"),
        TestCase::new(
            "3.5 /index crate <not member> error + guidance",
            DbSetup::SingleMember,
            TestPwd::Workspace("tests/fixture_workspace/ws_fixture_01"),
            "/index crate not_a_member",
            "Index",
            None,
            None,
        )
        .with_error(ExpectedUiError {
            message_contains: Some(
                "crate 'not_a_member' is not loaded in the current workspace".to_string(),
            ),
            recovery_suggestion: None,
        }),
        TestCase::new(
            "3.6 /load crate <member> forwards to Workspace(LoadDb) for validation",
            DbSetup::SingleMember,
            TestPwd::Workspace("tests/fixture_workspace/ws_fixture_01"),
            "/load crate member_root",
            "Load",
            None,
            None,
        )
        .with_load_contract(parser::LoadKind::Crate, Some("member_root"), false)
        .with_resolve_ui_error(ExpectedUiError {
            message_contains: Some("Crate 'member_root' is already loaded.".to_string()),
            recovery_suggestion: Some("`/index` to re-index 'member_root'.".to_string()),
        }),
        TestCase::new(
            "3.7 /load crate <not member> forwards to Workspace(LoadDb) for validation",
            DbSetup::SingleMember,
            TestPwd::Workspace("tests/fixture_workspace/ws_fixture_01"),
            "/load crate not_a_member",
            "Load",
            None,
            None,
        )
        .with_load_contract(parser::LoadKind::Crate, Some("not_a_member"), false)
        .with_resolve_ui_error(ExpectedUiError {
            message_contains: Some("No saved crate named 'not_a_member' was found.".to_string()),
            recovery_suggestion: Some(
                "`/index crate not_a_member` to index it before loading.".to_string(),
            ),
        }),
        TestCase::new(
            "3.8 /load crate <not in registry> forwards to Workspace(LoadDb) for validation",
            DbSetup::SingleMember,
            TestPwd::Workspace("tests/fixture_workspace/ws_fixture_01"),
            "/load crate new_crate",
            "Load",
            None,
            None,
        )
        .with_load_contract(parser::LoadKind::Crate, Some("new_crate"), false)
        .with_resolve_ui_error(ExpectedUiError {
            message_contains: Some("No saved crate named 'new_crate' was found.".to_string()),
            recovery_suggestion: Some(
                "`/index crate new_crate` to index it before loading.".to_string(),
            ),
        }),
        TestCase::new(
            "3.9 /save db saves workspace snapshot",
            DbSetup::SingleMember,
            TestPwd::Workspace("tests/fixture_workspace/ws_fixture_01"),
            "/save db",
            "Workspace", // TODO: Assert resolved SaveDb effect once the probe forwards intents
            None,
            None,
        ),
        TestCase::new(
            "3.10 /update scans focused crate, re-indexes if stale",
            DbSetup::SingleMember,
            TestPwd::Workspace("tests/fixture_workspace/ws_fixture_01"),
            "/update",
            "TestTodo",
            None,
            None,
        ),
    ];

    run_test_cases(&cases).await;

    println!("\n========================================");
    println!("All {} single-member test cases completed!", cases.len());
}

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
//
// - [ ] standalone crate: `/index` re-indexes loaded crate
// - [ ] standalone crate: `/index crate <loaded>` re-indexes
// - [ ] standalone crate: `/index crate <different>` error "use `/load crate`"
// - [ ] standalone crate: `/index workspace` error "not a workspace"
// - [ ] standalone crate: `/load crate <name>` check unsaved, then load (or force)
// - [ ] standalone crate: `/load workspace <name>` check unsaved, then load workspace
// - [ ] standalone crate: `/save db` saves standalone snapshot
// - [ ] standalone crate: `/update` scans and re-indexes if stale
// - [ ] standalone crate: `/index path/to/other` error "use `/load crate`"

/// Batch test: Runs all "Standalone Crate Loaded" test cases sequentially.
///
/// Uses `FIXTURE_NODES_CANONICAL` fixture which has a standalone crate loaded.
#[tokio::test]
async fn test_standalone_crate_all_cases() {
    let cases = vec![
        // Section 4: Standalone crate loaded (9 cases)
        TestCase::new(
            "4.1 /index re-indexes loaded crate",
            DbSetup::StandaloneCrate,
            TestPwd::Crate("tests/fixture_crates/fixture_nodes"),
            "/index",
            "Index",
            None,
            None,
        )
        .with_resolved_index_target("tests/fixture_crates/fixture_nodes"),
        TestCase::new(
            "4.2 /index crate <loaded> re-indexes",
            DbSetup::StandaloneCrate,
            TestPwd::Crate("tests/fixture_crates/fixture_nodes"),
            "/index crate fixture_nodes",
            "Index",
            None,
            None,
        )
        .with_resolved_index_target("tests/fixture_crates/fixture_nodes"),
        TestCase::new(
            "4.3 /index crate <different> error 'use /load crate'",
            DbSetup::StandaloneCrate,
            TestPwd::Crate("tests/fixture_crates/fixture_nodes"),
            "/index crate other_crate",
            "Index",
            None,
            None,
        )
        .with_error(ExpectedUiError {
            message_contains: Some(
                "crate 'other_crate' is not loaded in the current workspace".to_string(),
            ),
            recovery_suggestion: None,
        }),
        TestCase::new(
            "4.4 /index workspace error 'not a workspace'",
            DbSetup::StandaloneCrate,
            TestPwd::Crate("tests/fixture_crates/fixture_nodes"),
            "/index workspace",
            "Index",
            None,
            None,
        )
        .with_error(ExpectedUiError {
            message_contains: Some("Current directory is not a workspace member".to_string()),
            recovery_suggestion: Some(
                "Open or load a workspace member first, then run `/index workspace` again."
                    .to_string(),
            ),
        }),
        TestCase::new(
            "4.5 /load crate <name> forwards to Workspace(LoadDb) for validation",
            DbSetup::StandaloneCrate,
            TestPwd::Crate("tests/fixture_crates/fixture_nodes"),
            "/load crate other_crate",
            "Load",
            None,
            None,
        )
        .with_load_contract(parser::LoadKind::Crate, Some("other_crate"), false)
        .with_resolve_ui_error(ExpectedUiError {
            message_contains: Some("No saved crate named 'other_crate' was found.".to_string()),
            recovery_suggestion: Some(
                "`/index crate other_crate` to index it before loading.".to_string(),
            ),
        }),
        TestCase::new(
            "4.6 /load workspace <name> (now forwards to LoadDb)",
            DbSetup::StandaloneCrate,
            TestPwd::Crate("tests/fixture_crates/fixture_nodes"),
            "/load workspace some_workspace",
            "Load",
            None,
            None,
        )
        .with_load_contract(parser::LoadKind::Workspace, Some("some_workspace"), false)
        .with_resolve_ui_error(ExpectedUiError {
            message_contains: Some(
                "No saved workspace named 'some_workspace' was found.".to_string(),
            ),
            recovery_suggestion: Some(
                "`/index workspace` from the workspace root before loading it.".to_string(),
            ),
        }),
        TestCase::new(
            "4.7 /save db saves standalone snapshot",
            DbSetup::StandaloneCrate,
            TestPwd::Crate("tests/fixture_crates/fixture_nodes"),
            "/save db",
            "Workspace", // TODO: Assert resolved SaveDb effect once the probe forwards intents
            None,
            None,
        ),
        TestCase::new(
            "4.8 /update scans and re-indexes if stale",
            DbSetup::StandaloneCrate,
            TestPwd::Crate("tests/fixture_crates/fixture_nodes"),
            "/update",
            "TestTodo",
            None,
            None,
        ),
        TestCase::new(
            "4.9 /index path/to/other runs update (not validating target)",
            DbSetup::StandaloneCrate,
            TestPwd::Crate("tests/fixture_crates/fixture_nodes"),
            "/index /some/other/crate",
            "Index",
            None,
            Some("test_standalone_index_path"),
        )
        .with_error(ExpectedUiError {
            message_contains: Some("Failed to normalize target path".to_string()),
            recovery_suggestion: None,
        }),
    ];

    run_test_cases(&cases).await;

    println!("\n========================================");
    println!("All {} standalone crate test cases completed!", cases.len());
}

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
// - [ ] multi crate + workspace: `/index` re-indexes all members
// - [ ] multi crate + workspace: `/index crate <member>` indexes that member
// - [ ] multi crate + workspace: `/index crate <not member>` error + guidance
// - [ ] multi crate + workspace: `/index workspace` re-indexes all (same as `/index`)
// - [ ] multi crate + workspace: `/index path/to/crate` indexes if within workspace
// - [ ] multi crate + workspace: `/index path/to/crate` error if outside workspace
// - [ ] multi crate + workspace: `/load crate <member>` suggests `/index crate`
// - [ ] multi crate + workspace: `/load crate <not member>` check unsaved, unload, load single
// - [ ] multi crate + workspace: `/load crate <not in registry>` suggests index
// - [ ] multi crate + workspace: `/load workspace <different>` check unsaved, load new
// - [ ] multi crate + workspace: `/save db` saves workspace snapshot
// - [ ] multi crate + workspace: `/update` scans all members, re-indexes stale

/// Batch test: Runs all "Full Workspace Loaded" test cases sequentially.
///
/// Uses `WS_FIXTURE_01_CANONICAL` fixture which has multiple workspace members loaded.
#[tokio::test]
async fn test_full_workspace_all_cases() {
    let cases = vec![
        // Section 5: Full workspace loaded (11 cases)
        TestCase::new(
            "5.1 /index re-indexes all members",
            DbSetup::FullWorkspace,
            TestPwd::Workspace("tests/fixture_workspace/ws_fixture_01"),
            "/index",
            "Index",
            None,
            None,
        )
        .with_resolved_index_target("tests/fixture_workspace/ws_fixture_01"),
        TestCase::new(
            "5.2 /index crate <member> indexes that member",
            DbSetup::FullWorkspace,
            TestPwd::Workspace("tests/fixture_workspace/ws_fixture_01"),
            "/index crate member_root",
            "Index",
            None,
            None,
        )
        .with_resolved_index_target("tests/fixture_workspace/ws_fixture_01/member_root"),
        TestCase::new(
            "5.3 /index crate <not member> error + guidance",
            DbSetup::FullWorkspace,
            TestPwd::Workspace("tests/fixture_workspace/ws_fixture_01"),
            "/index crate not_a_member",
            "Index",
            None,
            None,
        )
        .with_error(ExpectedUiError {
            message_contains: Some(
                "crate 'not_a_member' is not loaded in the current workspace".to_string(),
            ),
            recovery_suggestion: None,
        }),
        TestCase::new(
            "5.4 /index workspace re-indexes all (same as /index)",
            DbSetup::FullWorkspace,
            TestPwd::Workspace("tests/fixture_workspace/ws_fixture_01"),
            "/index workspace",
            "Index",
            None,
            None,
        )
        .with_resolved_index_target("tests/fixture_workspace/ws_fixture_01"),
        TestCase::new(
            "5.5 /index path/to/crate indexes if within workspace",
            DbSetup::FullWorkspace,
            TestPwd::Workspace("tests/fixture_workspace/ws_fixture_01"),
            "/index member_root",
            "Index",
            None,
            Some("test_full_workspace_index_path_within"),
        )
        .with_resolved_index_target("tests/fixture_workspace/ws_fixture_01/member_root"),
        TestCase::new(
            "5.6 /index path/to/crate error if outside workspace",
            DbSetup::FullWorkspace,
            TestPwd::Workspace("tests/fixture_workspace/ws_fixture_01"),
            "/index /outside/workspace/crate",
            "Index",
            None,
            Some("test_full_workspace_index_path_outside"),
        )
        .with_error(ExpectedUiError {
            message_contains: Some("Failed to normalize target path".to_string()),
            recovery_suggestion: None,
        }),
        TestCase::new(
            "5.7 /load crate <member> (now forwards to LoadDb)",
            DbSetup::FullWorkspace,
            TestPwd::Workspace("tests/fixture_workspace/ws_fixture_01"),
            "/load crate member_root",
            "Load",
            None,
            None,
        )
        .with_load_contract(parser::LoadKind::Crate, Some("member_root"), false)
        .with_resolve_ui_error(ExpectedUiError {
            message_contains: Some("Crate 'member_root' is already loaded.".to_string()),
            recovery_suggestion: Some("`/index` to re-index 'member_root'.".to_string()),
        }),
        TestCase::new(
            "5.8 /load crate <not member> with --force (now forwards to LoadDb)",
            DbSetup::FullWorkspace,
            TestPwd::Workspace("tests/fixture_workspace/ws_fixture_01"),
            "/load crate not_a_member --force",
            "Load",
            None,
            None,
        )
        .with_load_contract(parser::LoadKind::Crate, Some("not_a_member"), true)
        .with_resolve_ui_error(ExpectedUiError {
            message_contains: Some("No saved crate named 'not_a_member' was found.".to_string()),
            recovery_suggestion: Some(
                "`/index crate not_a_member` to index it before loading.".to_string(),
            ),
        }),
        TestCase::new(
            "5.9 /load crate <not in registry> (now forwards to LoadDb)",
            DbSetup::FullWorkspace,
            TestPwd::Workspace("tests/fixture_workspace/ws_fixture_01"),
            "/load crate new_crate",
            "Load",
            None,
            None,
        )
        .with_load_contract(parser::LoadKind::Crate, Some("new_crate"), false)
        .with_resolve_ui_error(ExpectedUiError {
            message_contains: Some("No saved crate named 'new_crate' was found.".to_string()),
            recovery_suggestion: Some(
                "`/index crate new_crate` to index it before loading.".to_string(),
            ),
        }),
        TestCase::new(
            "5.10 /load workspace <different> with --force (now forwards to LoadDb)",
            DbSetup::FullWorkspace,
            TestPwd::Workspace("tests/fixture_workspace/ws_fixture_01"),
            "/load workspace other_workspace --force",
            "Load",
            None,
            None,
        )
        .with_load_contract(parser::LoadKind::Workspace, Some("other_workspace"), true)
        .with_resolve_ui_error(ExpectedUiError {
            message_contains: Some(
                "No saved workspace named 'other_workspace' was found.".to_string(),
            ),
            recovery_suggestion: Some(
                "`/index workspace` from the workspace root before loading it.".to_string(),
            ),
        }),
        TestCase::new(
            "5.11 /save db saves workspace snapshot",
            DbSetup::FullWorkspace,
            TestPwd::Workspace("tests/fixture_workspace/ws_fixture_01"),
            "/save db",
            "Workspace", // TODO: Assert resolved SaveDb effect once the probe forwards intents
            None,
            None,
        ),
        TestCase::new(
            "5.12 /update scans all members, re-indexes stale",
            DbSetup::FullWorkspace,
            TestPwd::Workspace("tests/fixture_workspace/ws_fixture_01"),
            "/update",
            "TestTodo",
            None,
            None,
        ),
    ];

    run_test_cases(&cases).await;

    println!("\n========================================");
    println!("All {} full workspace test cases completed!", cases.len());
}

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
// Test: DB Loaded, PWD is Crate (Section 6)
// =============================================================================
//
// - [ ] pwd=crate, db loaded: `/index` re-indexes crate at pwd if loaded
// - [ ] pwd=crate, db loaded: `/index` error if pwd crate not loaded
// - [ ] pwd=crate, db loaded: `/index workspace` re-indexes workspace if pwd is member
// - [ ] pwd=crate, db loaded: `/index workspace` error if pwd not member
// - [ ] pwd=crate, db loaded: `/index crate <pwd match>` re-indexes
// - [ ] pwd=crate, db loaded: `/index crate <different loaded>` switches focus + indexes
// - [ ] pwd=crate, db loaded: `/index crate <not loaded>` follows workspace rules
// - [ ] pwd=crate, db loaded: `/load crate <pwd match>` error "already loaded"
// - [ ] pwd=crate, db loaded: `/load crate <different>` check unsaved, then load
// - [ ] pwd=crate, db loaded: `/load workspace <name>` check unsaved, then load
// - [ ] pwd=crate, db loaded: `/save db`, `/update` same as workspace root rules

/// Batch test: Runs all "DB Loaded, PWD is Crate" test cases sequentially.
///
/// Uses `WS_FIXTURE_01_CANONICAL` fixture with PWD set to a member crate.
#[tokio::test]
async fn test_pwd_crate_loaded_all_cases() {
    let cases = vec![
        // Section 6: DB loaded, PWD is crate (11 cases)
        TestCase::new(
            "6.1 /index re-indexes crate at PWD if loaded",
            DbSetup::FullWorkspace,
            TestPwd::Crate("tests/fixture_workspace/ws_fixture_01/member_root"),
            "/index",
            "Index",
            None,
            None,
        )
        .with_resolved_index_target("tests/fixture_workspace/ws_fixture_01/member_root"),
        TestCase::new(
            "6.2 /index error if PWD crate not loaded",
            DbSetup::FullWorkspace,
            TestPwd::Crate("tests/fixture_workspace/ws_fixture_01/not_a_member"),
            "/index",
            "Index",
            None,
            None,
        )
        .with_error(ExpectedUiError {
            message_contains: Some(
                "Current directory is not a loaded crate. Use `/index crate <path>` to index a specific crate."
                    .to_string(),
            ),
            recovery_suggestion: Some(
                "Open or load a crate first, then run `/index` again.".to_string(),
            ),
        }),
        TestCase::new(
            "6.3 /index workspace re-indexes workspace if PWD is member",
            DbSetup::FullWorkspace,
            TestPwd::Crate("tests/fixture_workspace/ws_fixture_01/member_root"),
            "/index workspace",
            "Index",
            None,
            None,
        )
        .with_resolved_index_target("tests/fixture_workspace/ws_fixture_01"),
        TestCase::new(
            "6.4 /index workspace error if PWD not member",
            DbSetup::FullWorkspace,
            TestPwd::Crate("/some/random/crate"),
            "/index workspace",
            "Index",
            None,
            None,
        )
        .with_error(ExpectedUiError {
            message_contains: Some("Current directory is not a workspace member".to_string()),
            recovery_suggestion: Some(
                "Open or load a workspace member first, then run `/index workspace` again."
                    .to_string(),
            ),
        }),
        TestCase::new(
            "6.5 /index crate <PWD match> re-indexes",
            DbSetup::FullWorkspace,
            TestPwd::Crate("tests/fixture_workspace/ws_fixture_01/member_root"),
            "/index crate member_root",
            "Index",
            None,
            None,
        )
        .with_resolved_index_target("tests/fixture_workspace/ws_fixture_01/member_root"),
        TestCase::new(
            "6.6 /index crate <different loaded> switches focus + indexes",
            DbSetup::FullWorkspace,
            TestPwd::Crate("tests/fixture_workspace/ws_fixture_01/member_root"),
            "/index crate member_nested",
            "Index",
            None,
            None,
        )
        .with_resolved_index_target("tests/fixture_workspace/ws_fixture_01/nested/member_nested"),
        TestCase::new(
            "6.7 /index crate <not loaded> follows workspace rules",
            DbSetup::FullWorkspace,
            TestPwd::Crate("tests/fixture_workspace/ws_fixture_01/member_root"),
            "/index crate not_loaded_crate",
            "Index",
            None,
            None,
        )
        .with_error(ExpectedUiError {
            message_contains: Some(
                "crate 'not_loaded_crate' is not loaded in the current workspace".to_string(),
            ),
            recovery_suggestion: None,
        }),
        TestCase::new(
            "6.8 /load crate <PWD match> (now forwards to LoadDb)",
            DbSetup::FullWorkspace,
            TestPwd::Crate("tests/fixture_workspace/ws_fixture_01/member_root"),
            "/load crate member_root",
            "Load",
            None,
            None,
        )
        .with_load_contract(parser::LoadKind::Crate, Some("member_root"), false)
        .with_resolve_ui_error(ExpectedUiError {
            message_contains: Some("Crate 'member_root' is already loaded.".to_string()),
            recovery_suggestion: Some("`/index` to re-index 'member_root'.".to_string()),
        }),
        TestCase::new(
            "6.9 /load crate <different> (now forwards to LoadDb)",
            DbSetup::FullWorkspace,
            TestPwd::Crate("tests/fixture_workspace/ws_fixture_01/member_root"),
            "/load crate different_crate",
            "Load",
            None,
            None,
        )
        .with_load_contract(parser::LoadKind::Crate, Some("different_crate"), false)
        .with_resolve_ui_error(ExpectedUiError {
            message_contains: Some("No saved crate named 'different_crate' was found.".to_string()),
            recovery_suggestion: Some(
                "`/index crate different_crate` to index it before loading.".to_string(),
            ),
        }),
        TestCase::new(
            "6.10 /load workspace <name> (now forwards to LoadDb)",
            DbSetup::FullWorkspace,
            TestPwd::Crate("tests/fixture_workspace/ws_fixture_01/member_root"),
            "/load workspace other_workspace",
            "Load",
            None,
            None,
        )
        .with_load_contract(parser::LoadKind::Workspace, Some("other_workspace"), false)
        .with_resolve_ui_error(ExpectedUiError {
            message_contains: Some("No saved workspace named 'other_workspace' was found.".to_string()),
            recovery_suggestion: Some(
                "`/index workspace` from the workspace root before loading it.".to_string(),
            ),
        }),
        TestCase::new(
            "6.11 /save db, /update same as workspace root rules",
            DbSetup::FullWorkspace,
            TestPwd::Crate("tests/fixture_workspace/ws_fixture_01/member_root"),
            "/save db",
            "Workspace", // TODO: Assert resolved SaveDb effect once the probe forwards intents
            None,
            None,
        ),
    ];

    run_test_cases(&cases).await;

    println!("\n========================================");
    println!("All {} PWD-crate test cases completed!", cases.len());
}

// =============================================================================
// Test: Transition Cases (Section 7)
// =============================================================================
//
// - [ ] `/load workspace <name>` when standalone loaded: check unsaved, prompt or force
// - [ ] `/load crate <name>` when workspace loaded: check unsaved, prompt or force
// - [ ] `/load crate <name>` standalone→standalone: check unsaved, prompt or force
// - [ ] `/index` when db loaded: destructive re-parse (no force needed)

/// Batch test: Runs all "Transition Cases" test cases sequentially.
///
/// Tests behavior when switching between different loaded states.
#[tokio::test]
async fn test_transition_all_cases() {
    let cases = vec![
        // Section 7: Transition cases
        TestCase::new(
            "7.1 /load workspace <name> when standalone (now forwards to LoadDb)",
            DbSetup::StandaloneCrate,
            TestPwd::Crate("tests/fixture_crates/fixture_nodes"),
            "/load workspace some_workspace",
            "Load",
            None,
            None,
        )
        .with_load_contract(parser::LoadKind::Workspace, Some("some_workspace"), false)
        .with_resolve_ui_error(ExpectedUiError {
            message_contains: Some(
                "No saved workspace named 'some_workspace' was found.".to_string(),
            ),
            recovery_suggestion: Some(
                "`/index workspace` from the workspace root before loading it.".to_string(),
            ),
        }),
        TestCase::new(
            "7.2 /load crate <name> when workspace loaded: check unsaved, prompt or force",
            DbSetup::FullWorkspace,
            TestPwd::Workspace("tests/fixture_workspace/ws_fixture_01"),
            "/load crate some_crate",
            "TestTodo",
            None,
            Some("test_transition_load_crate_from_workspace"),
        ),
        TestCase::new(
            "7.3 /load crate <name> standalone→standalone: check unsaved, prompt or force",
            DbSetup::StandaloneCrate,
            TestPwd::Crate("tests/fixture_crates/fixture_nodes"),
            "/load crate other_crate",
            "TestTodo",
            None,
            Some("test_transition_load_crate_standalone_to_standalone"),
        ),
        TestCase::new(
            "7.4 /index when db loaded: destructive re-parse (no force needed)",
            DbSetup::FullWorkspace,
            TestPwd::Workspace("tests/fixture_workspace/ws_fixture_01"),
            "/index",
            "TestTodo",
            None,
            Some("test_transition_index_when_loaded"),
        ),
    ];

    run_test_cases(&cases).await;

    println!("\n========================================");
    println!("All {} transition test cases completed!", cases.len());
}

#[tokio::test]
async fn test_load_validate_blocks_stale_state_without_force() {
    let db = load_db_fixture(DbSetup::StandaloneCrate);
    let rt = TestRuntime::new(&db).spawn_validation_probe();
    let crate_root = ploke_test_utils::workspace_root().join("tests/fixture_crates/fixture_nodes");
    rt.setup_loaded_standalone_crate(crate_root).await;

    rt.state_arc()
        .with_system_txn(|txn| {
            for crate_id in txn.loaded_crate_ids() {
                txn.set_workspace_freshness(crate_id, WorkspaceFreshness::Stale);
            }
        })
        .await;

    let cmd = crate::app_state::commands::LoadCmd {
        kind: parser::LoadKind::Workspace,
        name: Some("ws_fixture_01".to_string()),
        force: false,
    };
    let resolution = crate::app_state::commands::LoadResolution {
        workspace_ref: "ws_fixture_01".to_string(),
        replaces_loaded_state: true,
    };

    let err = cmd
        .validate(&rt.state_arc(), &resolution)
        .await
        .expect_err("stale loaded state should block /load without --force");
    assert_eq!(
        err.to_string(),
        "Current loaded crate or workspace has stale state"
    );
}

#[tokio::test]
async fn test_load_validate_allows_stale_state_with_force() {
    let db = load_db_fixture(DbSetup::StandaloneCrate);
    let rt = TestRuntime::new(&db).spawn_validation_probe();
    let crate_root = ploke_test_utils::workspace_root().join("tests/fixture_crates/fixture_nodes");
    rt.setup_loaded_standalone_crate(crate_root).await;

    rt.state_arc()
        .with_system_txn(|txn| {
            for crate_id in txn.loaded_crate_ids() {
                txn.set_workspace_freshness(crate_id, WorkspaceFreshness::Stale);
            }
        })
        .await;

    let cmd = crate::app_state::commands::LoadCmd {
        kind: parser::LoadKind::Workspace,
        name: Some("ws_fixture_01".to_string()),
        force: true,
    };
    let resolution = crate::app_state::commands::LoadResolution {
        workspace_ref: "ws_fixture_01".to_string(),
        replaces_loaded_state: true,
    };

    cmd.validate(&rt.state_arc(), &resolution)
        .await
        .expect("force should bypass stale-state load validation");
}

// =============================================================================
// Test Helpers
// =============================================================================

/// Helper to send a command and collect the resulting StateCommand and events
async fn send_command_and_collect(
    app: &mut crate::app::App,
    command: &str,
    debug_rx: &mut tokio::sync::mpsc::Receiver<DebugStateCommand>,
    validation_rx: &mut tokio::sync::mpsc::Receiver<ValidationProbeEvent>,
) -> CaseTrace {
    let parsed = parser::parse(app, command, CommandStyle::Slash);
    let parsed_debug = format!("{:?}", parsed);
    exec::execute(app, parsed);
    tokio::task::yield_now().await;
    let mut trace = collect_case_trace(debug_rx, validation_rx).await;
    trace.parsed = parsed_debug;
    trace
}
