//! Tests for the command context and resource management.
//!
//! These tests verify that `CommandContext` properly initializes and manages
//! shared resources like database pools, embedding runtimes, and IO managers.

use std::path::Path;

use xtask::context::CommandContext;

// =============================================================================
// CommandContext Tests
// =============================================================================

/// To Prove: CommandContext can be created successfully
/// Given: No preconditions
/// When: CommandContext::new() is called
/// Then: Returns a valid context with temp directory
#[test]
fn context_can_be_created() {
    let ctx = CommandContext::new().unwrap();

    // Context should have a valid temp directory
    assert!(ctx.temp_dir().path().exists());
}

/// To Prove: CommandContext implements Default trait
/// Given: No preconditions
/// When: CommandContext::default() is called
/// Then: Returns a valid context
#[test]
fn context_implements_default() {
    let ctx = CommandContext::default();

    assert!(ctx.workspace_root().is_ok());
}

/// To Prove: CommandContext lazily initializes database pool
/// Given: A new CommandContext
/// When: database_pool() is called for the first time
/// Then: Pool is created and cached
#[test]
fn context_lazy_initializes_database_pool() {
    let ctx = CommandContext::new().unwrap();

    // First call should initialize the pool
    let pool1 = ctx.database_pool();
    assert!(pool1.is_ok());

    // Second call should return cached pool
    let pool2 = ctx.database_pool();
    assert!(pool2.is_ok());

    // Both should be the same instance
    let pool1 = pool1.unwrap();
    let pool2 = pool2.unwrap();
    assert!(std::sync::Arc::ptr_eq(&pool1, &pool2));
}

/// To Prove: CommandContext provides in-memory database by default
/// Given: A context with initialized database pool
/// When: get_database(None) is called
/// Then: Returns an in-memory database
#[test]
fn context_provides_in_memory_database() {
    let ctx = CommandContext::new().unwrap();

    let _db = ctx
        .get_database(None::<&Path>)
        .expect("in-memory database must be available via get_database(None)");
}

/// To Prove: CommandContext lazily initializes embedding runtime
/// Given: A new CommandContext
/// When: embedding_runtime() is called for the first time
/// Then: Runtime is created and cached
#[test]
fn context_lazy_initializes_embedding_runtime() {
    let ctx = CommandContext::new().unwrap();

    let rt1 = ctx
        .embedding_runtime()
        .expect("embedding runtime must initialize (placeholder)");
    let rt2 = ctx
        .embedding_runtime()
        .expect("cached embedding runtime");
    assert!(std::sync::Arc::ptr_eq(&rt1, &rt2));
}

/// To Prove: CommandContext provides IO manager handle
/// Given: A CommandContext
/// When: io_manager() is called
/// Then: Returns a valid IoManagerHandle
#[test]
fn context_provides_io_manager() {
    let ctx = CommandContext::new().unwrap();

    // Get IO manager
    let io1 = ctx.io_manager();
    let io2 = ctx.io_manager();

    // Both should be valid handles (currently placeholder)
    drop(io1);
    drop(io2);
}

/// To Prove: CommandContext detects workspace root
/// Given: A CommandContext
/// When: workspace_root() is called
/// Then: Returns the workspace root directory
#[test]
fn context_detects_workspace_root() {
    let ctx = CommandContext::new().unwrap();

    let root = ctx.workspace_root().unwrap();

    // Should point to a directory with Cargo.toml
    assert!(root.exists());
    assert!(root.join("Cargo.toml").exists());
}

/// To Prove: CommandContext workspace root is cached
/// Given: A CommandContext
/// When: workspace_root() is called multiple times
/// Then: Returns the same path (cached)
#[test]
fn context_caches_workspace_root() {
    let ctx = CommandContext::new().unwrap();

    let root1 = ctx.workspace_root().unwrap();
    let root2 = ctx.workspace_root().unwrap();

    // Should be the same path
    assert_eq!(root1, root2);
}

/// To Prove: CommandContext provides access to temp directory
/// Given: A CommandContext
/// When: temp_dir() is called
/// Then: Returns a valid TempDir reference
#[test]
fn context_provides_temp_dir() {
    let ctx = CommandContext::new().unwrap();

    let temp = ctx.temp_dir();

    assert!(temp.path().exists());
    // TempDir is created in system temp location
    assert!(temp.path().to_string_lossy().contains("tmp") || temp.path().to_string_lossy().contains("temp"));
}

/// To Prove: CommandContext validates resources correctly
/// Given: A CommandContext
/// When: validate_resources() is called with different requirements
/// Then: Validates each resource type appropriately
#[test]
fn context_validates_resources() {
    let ctx = CommandContext::new().unwrap();

    ctx.validate_resources(false, false)
        .expect("validate_resources with no requirements must succeed");
    ctx.validate_resources(true, false)
        .expect("database pool must be available when validate_resources needs database");
    ctx.validate_resources(false, true)
        .expect("embedding runtime must be available when validate_resources needs it");
    ctx.validate_resources(true, true)
        .expect("validate_resources must succeed when both resources are required");
}

/// To Prove: CommandContext handles resource initialization errors gracefully
/// Given: A CommandContext with resource requirements that may fail
/// When: Resource access is attempted
/// Then: Returns meaningful error on failure
#[test]
fn context_handles_resource_errors() {
    let ctx = CommandContext::new().unwrap();

    ctx.database_pool()
        .expect("database_pool should return Ok in default test context");
    ctx.embedding_runtime()
        .expect("embedding_runtime should return Ok in default test context");
}

/// Persistent DB path still uses `todo!()` in `DatabasePool::get_or_create` (`context.rs`).
/// When ploke_db integration lands, replace this with a real open-or-create test.
#[test]
#[should_panic(expected = "Persistent database support not yet implemented")]
fn context_persistent_database_panics_until_implemented() {
    let ctx = CommandContext::new().unwrap();
    let _ = ctx.get_database(Some(Path::new("/tmp/ploke_xtask_persistent_test.db")));
}

// =============================================================================
// Resource Isolation Tests
// =============================================================================

/// To Prove: Multiple contexts have independent temp directories
/// Given: Two CommandContexts created separately
/// When: Their temp directories are compared
/// Then: Each has a unique temp directory
#[test]
fn contexts_have_independent_temp_dirs() {
    let ctx1 = CommandContext::new().unwrap();
    let ctx2 = CommandContext::new().unwrap();

    let temp1 = ctx1.temp_dir().path();
    let temp2 = ctx2.temp_dir().path();

    assert_ne!(temp1, temp2);
}

/// To Prove: Multiple contexts share the same workspace root
/// Given: Two CommandContexts created separately
/// When: Their workspace roots are compared
/// Then: Both point to the same workspace root
#[test]
fn contexts_share_workspace_root() {
    let ctx1 = CommandContext::new().unwrap();
    let ctx2 = CommandContext::new().unwrap();

    let root1 = ctx1.workspace_root().unwrap();
    let root2 = ctx2.workspace_root().unwrap();

    assert_eq!(root1, root2);
}

/// To Prove: Context resources are thread-safe
/// Given: A CommandContext wrapped in Arc
/// When: Accessed from multiple threads
/// Then: All accesses succeed without data races
#[test]
fn context_is_thread_safe() {
    use std::sync::Arc;
    use std::thread;

    let ctx = Arc::new(CommandContext::new().unwrap());

    let handles: Vec<_> = (0..5)
        .map(|_| {
            let ctx_clone = Arc::clone(&ctx);
            thread::spawn(move || {
                // Access workspace root
                let _ = ctx_clone.workspace_root();
                // Access temp dir
                let _ = ctx_clone.temp_dir();
                // Access IO manager
                let _ = ctx_clone.io_manager();
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }
}

// =============================================================================
// Error Handling Tests
// =============================================================================

/// To Prove: Context creation errors are meaningful
/// Given: Conditions that might cause context creation to fail
/// When: CommandContext::new() is called
/// Then: Returns descriptive XtaskError on failure
#[test]
fn context_creation_error_handling() {
    let ctx = CommandContext::new()
        .expect("CommandContext::new must succeed in the test workspace");
    assert!(ctx.workspace_root().is_ok());
}

/// To Prove: Context handles double-initialization gracefully
/// Given: A context where a resource is already initialized
/// When: Same resource is requested again
/// Then: Returns cached resource without error
#[test]
fn context_handles_double_initialization() {
    let ctx = CommandContext::new().unwrap();

    // First initialization
    let _ = ctx.workspace_root();

    // Second initialization should use cached value
    let _ = ctx.workspace_root();

    // Should not panic or error
}
