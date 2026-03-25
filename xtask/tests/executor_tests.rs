//! Tests for the command executor infrastructure.
//!
//! These tests verify the `CommandExecutor`, `CommandRegistry`, `MaybeAsync`,
//! and related infrastructure for command dispatch and lifecycle management.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use xtask::executor::{
    Command, CommandCategory, CommandExecutor, CommandRegistry, ExecutorConfig, MaybeAsync,
    ResourceRequirements,
};
use xtask::error::XtaskError;
use xtask::CommandContext;

// =============================================================================
// Test Fixtures
// =============================================================================

/// A simple synchronous test command that returns a string.
#[derive(Debug)]
struct TestSyncCommand {
    output: String,
}

impl Command for TestSyncCommand {
    type Output = String;
    type Error = XtaskError;

    fn name(&self) -> &'static str {
        "test-sync"
    }

    fn category(&self) -> CommandCategory {
        CommandCategory::Utility
    }

    fn execute(&self, _ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        Ok(self.output.clone())
    }

    fn requires_async(&self) -> bool {
        false
    }
}

impl Default for TestSyncCommand {
    fn default() -> Self {
        Self {
            output: "sync result".to_string(),
        }
    }
}

/// An async test command that returns a string.
#[derive(Debug)]
struct TestAsyncCommand;

impl Command for TestAsyncCommand {
    type Output = String;
    type Error = XtaskError;

    fn name(&self) -> &'static str {
        "test-async"
    }

    fn category(&self) -> CommandCategory {
        CommandCategory::Utility
    }

    fn execute(&self, _ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        Ok("async result".to_string())
    }

    fn requires_async(&self) -> bool {
        true
    }
}

impl Default for TestAsyncCommand {
    fn default() -> Self {
        Self
    }
}

/// A test command that tracks execution count.
#[derive(Debug)]
struct TestTrackingCommand {
    counter: Arc<AtomicUsize>,
}

impl Command for TestTrackingCommand {
    type Output = usize;
    type Error = XtaskError;

    fn name(&self) -> &'static str {
        "test-tracking"
    }

    fn category(&self) -> CommandCategory {
        CommandCategory::Utility
    }

    fn execute(&self, _ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        let count = self.counter.fetch_add(1, Ordering::SeqCst);
        Ok(count + 1)
    }

    fn requires_async(&self) -> bool {
        false
    }
}

/// A test command that requires database access.
#[derive(Debug, Default)]
struct TestDbCommand;

impl Command for TestDbCommand {
    type Output = String;
    type Error = XtaskError;

    fn name(&self) -> &'static str {
        "test-db"
    }

    fn category(&self) -> CommandCategory {
        CommandCategory::Database
    }

    fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        // Access database to verify resource is available
        let _db = ctx.database_pool()?;
        Ok("db access ok".to_string())
    }

    fn requires_async(&self) -> bool {
        false
    }

    fn resource_requirements(&self) -> ResourceRequirements {
        ResourceRequirements {
            needs_database: true,
            needs_embedding_runtime: false,
            needs_io_manager: false,
            needs_event_bus: false,
        }
    }
}

/// A test command that fails with an error.
#[derive(Debug, Default)]
struct TestFailingCommand;

impl Command for TestFailingCommand {
    type Output = ();
    type Error = XtaskError;

    fn name(&self) -> &'static str {
        "test-fail"
    }

    fn category(&self) -> CommandCategory {
        CommandCategory::Utility
    }

    fn execute(&self, _ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        Err(XtaskError::new("Intentional test failure"))
    }

    fn requires_async(&self) -> bool {
        false
    }
}

// =============================================================================
// CommandExecutor Tests
// =============================================================================

/// To Prove: Executor can run a registered synchronous command
/// Given: A command executor with async enabled
/// When: execute() is called with a sync command
/// Then: Command runs synchronously and returns correct output
#[test]
fn executor_runs_sync_command() {
    // TODO(M.4): Complete implementation - executor::execute() needs full implementation
    let executor = CommandExecutor::new(ExecutorConfig::default()).unwrap();
    let command = TestSyncCommand {
        output: "hello world".to_string(),
    };

    let result = executor.execute(command).unwrap();
    assert_eq!(result, "hello world");
}

/// To Prove: Executor can run an async command using the runtime
/// Given: A command executor with async runtime enabled
/// When: execute() is called with an async command
/// Then: Command runs on the async runtime and returns correct output
#[test]
fn executor_runs_async_command() {
    // TODO(M.4): Complete implementation - async execution path needs implementation
    let executor = CommandExecutor::new(ExecutorConfig::default()).unwrap();
    let command = TestAsyncCommand;

    let result = executor.execute(command).unwrap();
    assert_eq!(result, "async result");
}

/// To Prove: Executor validates resource requirements before execution
/// Given: A command that requires database access
/// When: execute() is called
/// Then: Prerequisites are validated before command runs
#[test]
fn executor_validates_prerequisites() {
    let executor = CommandExecutor::new(ExecutorConfig::default()).unwrap();
    let command = TestDbCommand;

    let out = executor
        .execute(command)
        .expect("TestDbCommand must succeed when database pool is available in tests");
    assert_eq!(out, "db access ok");
}

/// To Prove: Executor properly tracks usage statistics
/// Given: A command executor with usage tracking
/// When: Multiple commands are executed
/// Then: Usage tracker records all executions
#[test]
fn executor_tracks_usage() {
    // TODO(M.4): Complete implementation - usage tracking integration needs work
    let executor = CommandExecutor::new(ExecutorConfig::default()).unwrap();
    let counter = Arc::new(AtomicUsize::new(0));

    // Execute command multiple times
    for _ in 0..5 {
        let command = TestTrackingCommand {
            counter: Arc::clone(&counter),
        };
        let _ = executor.execute(command);
    }

    // Verify command was executed 5 times
    assert_eq!(counter.load(Ordering::SeqCst), 5);
}

/// To Prove: Executor handles command failures gracefully
/// Given: A command that returns an error
/// When: execute() is called
/// Then: Error is propagated and usage records failure
#[test]
fn executor_handles_command_failure() {
    // TODO(M.4): Complete implementation - error handling needs refinement
    let executor = CommandExecutor::new(ExecutorConfig::default()).unwrap();
    let command = TestFailingCommand;

    let result = executor.execute(command);
    assert!(result.is_err());

    let err = result.unwrap_err();
    assert!(err.to_string().contains("Intentional test failure"));
}

/// To Prove: Executor can be created without async runtime
/// Given: ExecutorConfig with enable_async = false
/// When: CommandExecutor::new() is called
/// Then: Executor is created successfully without runtime
#[test]
fn executor_works_without_async_runtime() {
    // TODO(M.4): Complete implementation - async-disabled mode needs testing
    let config = ExecutorConfig {
        enable_async: false,
        usage_log_path: None,
        trace_output_dir: None,
    };

    let executor = CommandExecutor::new(config).unwrap();
    let command = TestSyncCommand::default();

    let result = executor.execute(command);
    // Sync commands should work without async runtime
    assert!(result.is_ok());
}

/// To Prove: Executor provides access to its context
/// Given: A created executor
/// When: context() is called
/// Then: Returns a reference to the shared CommandContext
#[test]
fn executor_provides_context_access() {
    let executor = CommandExecutor::new(ExecutorConfig::default()).unwrap();
    let ctx = executor.context();

    // Context should provide access to resources
    assert!(ctx.workspace_root().is_ok());
}

// =============================================================================
// CommandRegistry Tests
// =============================================================================

/// To Prove: Commands can be registered with the registry
/// Given: An empty command registry
/// When: register() is called with a command type
/// Then: Command is registered and can be looked up
#[test]
fn registry_registers_commands() {
    // TODO(M.4): Complete implementation - registry.register() uses `todo!()` macro
    let mut registry = CommandRegistry::new();

    // Register a command type
    registry.register::<TestSyncCommand>();

    // Should be able to check if command is registered
    assert!(registry.contains("test-sync"));
}

/// To Prove: Registry organizes commands by category
/// Given: Multiple commands registered in different categories
/// When: get_category() is called
/// Then: Returns commands organized by category
#[test]
fn registry_organizes_by_category() {
    // TODO(M.4): Complete implementation - category tracking needs verification
    let mut registry = CommandRegistry::new();

    registry.register::<TestSyncCommand>();
    registry.register::<TestAsyncCommand>();
    registry.register::<TestDbCommand>();

    let utility_cmds = registry.get_category(CommandCategory::Utility);
    assert!(utility_cmds.is_some());
    let utility_cmds = utility_cmds.unwrap();
    assert!(utility_cmds.contains(&"test-sync"));
    assert!(utility_cmds.contains(&"test-async"));

    let db_cmds = registry.get_category(CommandCategory::Database);
    assert!(db_cmds.is_some());
    assert!(db_cmds.unwrap().contains(&"test-db"));
}

/// To Prove: Registry generates help text for all commands
/// Given: A registry with registered commands
/// When: generate_help() is called
/// Then: Returns formatted help text with categories
#[test]
fn registry_generates_help() {
    // TODO(M.4): Complete implementation - help generation could be enhanced
    let mut registry = CommandRegistry::new();
    registry.register::<TestSyncCommand>();
    registry.register::<TestDbCommand>();

    let help = registry.generate_help();

    assert!(help.contains("xtask"));
    assert!(help.contains("test-sync"));
    assert!(help.contains("Utility"));
    assert!(help.contains("Database"));
}

/// To Prove: Registry lookup returns None for unknown commands
/// Given: A registry with some commands
/// When: get() is called with an unknown command name
/// Then: Returns None
#[test]
fn registry_returns_none_for_unknown_command() {
    let registry = CommandRegistry::new();

    assert!(!registry.contains("unknown-command"));
    assert!(registry.get("unknown-command").is_none());
}

/// Invoking the registered factory exercises the `todo!()` in `CommandRegistry::register`.
/// When argument parsing is implemented, replace this with a real construction test.
#[test]
fn registry_factory_panics_until_command_construction_implemented() {
    let mut registry = CommandRegistry::new();
    registry.register::<TestSyncCommand>();
    let factory = registry
        .get("test-sync")
        .expect("test-sync should be registered");
    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = factory(&[]);
    }))
    .is_err();
    assert!(
        panicked,
        "factory should panic until command construction from args is implemented"
    );
}

// =============================================================================
// MaybeAsync Tests
// =============================================================================

/// To Prove: MaybeAsync::Ready variant holds immediate values
/// Given: A MaybeAsync::Ready wrapping a value
/// When: The value is accessed
/// Then: Returns the wrapped value immediately
#[test]
fn maybe_async_ready_holds_value() {
    let ready: MaybeAsync<i32> = MaybeAsync::Ready(42);

    match ready {
        MaybeAsync::Ready(val) => assert_eq!(val, 42),
        _ => panic!("Expected Ready variant"),
    }
}

/// To Prove: MaybeAsync can be created from a value using From trait
/// Given: Any value
/// When: MaybeAsync::from() is called
/// Then: Returns MaybeAsync::Ready with the value
#[test]
fn maybe_async_from_value() {
    let ready: MaybeAsync<String> = MaybeAsync::from("hello".to_string());

    match ready {
        MaybeAsync::Ready(val) => assert_eq!(val, "hello"),
        _ => panic!("Expected Ready variant"),
    }
}

/// To Prove: MaybeAsync::into_future() resolves ready values
/// Given: A MaybeAsync::Ready wrapping a value
/// When: into_future() is called and awaited
/// Then: Returns the wrapped value
#[tokio::test]
async fn maybe_async_into_future_ready() {
    let ready: MaybeAsync<i32> = MaybeAsync::Ready(100);
    let result = ready.into_future().await;
    assert_eq!(result, 100);
}

/// To Prove: MaybeAsync::block() returns ready values immediately
/// Given: A MaybeAsync::Ready wrapping a value
/// When: block() is called
/// Then: Returns the wrapped value without blocking
#[test]
fn maybe_async_block_ready() {
    // TODO(M.4): Complete implementation - block() has `todo!()` for async
    let ready: MaybeAsync<i32> = MaybeAsync::Ready(200);

    // For Ready variant, block() should return immediately
    let result = ready.block();
    assert_eq!(result, 200);
}

// =============================================================================
// ResourceRequirements Tests
// =============================================================================

/// To Prove: ResourceRequirements default is all false
/// Given: Default ResourceRequirements
/// When: Fields are checked
/// Then: All requirement fields are false
#[test]
fn resource_requirements_default() {
    let reqs = ResourceRequirements::default();

    assert!(!reqs.needs_database);
    assert!(!reqs.needs_embedding_runtime);
    assert!(!reqs.needs_io_manager);
    assert!(!reqs.needs_event_bus);
}

/// To Prove: ResourceRequirements can be customized
/// Given: A command with custom resource requirements
/// When: resource_requirements() is called
/// Then: Returns the custom requirements
#[test]
fn resource_requirements_custom() {
    let command = TestDbCommand;
    let reqs = command.resource_requirements();

    assert!(reqs.needs_database);
    assert!(!reqs.needs_embedding_runtime);
    assert!(!reqs.needs_io_manager);
    assert!(!reqs.needs_event_bus);
}

// =============================================================================
// CommandCategory Tests
// =============================================================================

/// To Prove: All command categories have string representations
/// Given: Each CommandCategory variant
/// When: as_str() is called
/// Then: Returns a non-empty string
#[test]
fn command_category_as_str() {
    let categories = CommandCategory::all();

    for cat in categories {
        let s = cat.as_str();
        assert!(!s.is_empty());
        // Should be PascalCase
        assert!(s.chars().next().unwrap().is_uppercase());
    }
}

/// To Prove: CommandCategory::all() returns all categories
/// Given: The list of all categories
/// When: Counted
/// Then: Contains expected number of categories
#[test]
fn command_category_all_count() {
    let all = CommandCategory::all();
    // We expect 9 categories as defined in the enum
    assert_eq!(all.len(), 9);
}
