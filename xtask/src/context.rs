//! Shared context for xtask commands.
//!
//! This module provides the [`CommandContext`] type, which offers lazy-initialized
//! access to expensive resources like database connections and embedding runtimes.
//!
//! The context uses [`once_cell::sync::OnceCell`] for thread-safe lazy initialization,
//! ensuring resources are only created when actually needed.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use once_cell::sync::OnceCell;
use ploke_db::{create_index_primary, Database};
use ploke_test_utils::fixture_dbs::{FixtureDb, FixtureImportMode};

use crate::error::XtaskError;

/// Shared context passed to all commands.
///
/// Provides lazy-initialized access to expensive resources like
/// database connections and embedding runtimes. The context is thread-safe
/// and can be shared across multiple commands.
///
/// # Example
/// ```ignore
/// use xtask::context::CommandContext;
///
/// let ctx = CommandContext::new().unwrap();
///
/// // Get database (lazy-initialized)
/// let db = ctx.get_database(None).unwrap();
///
/// // Get IO manager
/// let io = ctx.io_manager();
/// ```
pub struct CommandContext {
    /// Database pool - lazy initialized.
    database_pool: OnceCell<Arc<DatabasePool>>,

    /// Embedding runtime - lazy initialized.
    embedding_runtime: OnceCell<Arc<EmbeddingRuntimeManager>>,

    /// IO manager - shared across commands.
    io_manager: OnceCell<IoManagerHandle>,

    /// Workspace root detection cache.
    workspace_root: OnceCell<std::path::PathBuf>,

    /// Temporary directory for intermediate files.
    temp_dir: tempfile::TempDir,
}

impl CommandContext {
    /// Create a new command context.
    ///
    /// The context starts empty, with all resources being created on first access.
    ///
    /// # Errors
    /// Returns an error if the temporary directory cannot be created.
    pub fn new() -> Result<Self, XtaskError> {
        let temp_dir = tempfile::tempdir()
            .map_err(|e| XtaskError::new(format!("Failed to create temp dir: {e}")))?;

        Ok(Self {
            database_pool: OnceCell::new(),
            embedding_runtime: OnceCell::new(),
            io_manager: OnceCell::new(),
            workspace_root: OnceCell::new(),
            temp_dir,
        })
    }

    /// Get or create the database pool.
    ///
    /// The database pool is created on first access and cached for subsequent calls.
    ///
    /// # Errors
    /// Returns an error if the database pool cannot be created.
    pub fn database_pool(&self) -> Result<Arc<DatabasePool>, XtaskError> {
        if let Some(pool) = self.database_pool.get() {
            return Ok(Arc::clone(pool));
        }

        let pool = DatabasePool::new()?;
        self.database_pool
            .set(pool.clone())
            .map_err(|_| XtaskError::new("Database pool already initialized"))?;
        Ok(pool)
    }

    /// Get a database from the pool (or in-memory if no path specified).
    ///
    /// # Arguments
    /// * `path` - Optional path to a persistent database. If `None`, returns
    ///   the in-memory database.
    ///
    /// # Errors
    /// Returns an error if the database pool cannot be accessed.
    pub fn get_database(&self, path: Option<&Path>) -> Result<Arc<Database>, XtaskError> {
        let pool = self.database_pool()?;
        pool.get_or_create(path)
    }

    /// Load a registered backup fixture (see `ploke-test-utils` / `docs/testing/BACKUP_DB_FIXTURES.md`).
    pub fn get_database_from_fixture(
        &self,
        fixture: &'static FixtureDb,
    ) -> Result<Arc<Database>, XtaskError> {
        let pool = self.database_pool()?;
        pool.get_from_fixture(fixture)
    }

    /// Get or create the embedding runtime.
    ///
    /// The embedding runtime is created on first access and cached for subsequent calls.
    ///
    /// # Errors
    /// Returns an error if the embedding runtime cannot be created.
    pub fn embedding_runtime(&self) -> Result<Arc<EmbeddingRuntimeManager>, XtaskError> {
        if let Some(runtime) = self.embedding_runtime.get() {
            return Ok(Arc::clone(runtime));
        }

        let runtime = EmbeddingRuntimeManager::new()?;
        self.embedding_runtime
            .set(runtime.clone())
            .map_err(|_| XtaskError::new("Embedding runtime already initialized"))?;
        Ok(runtime)
    }

    /// Get the IO manager.
    ///
    /// The IO manager is created on first access and cached for subsequent calls.
    pub fn io_manager(&self) -> IoManagerHandle {
        self.io_manager
            .get_or_init(IoManagerHandle::new)
            .clone()
    }

    /// Get the workspace root.
    ///
    /// This is detected by looking for the Cargo.toml file.
    ///
    /// # Errors
    /// Returns an error if the workspace root cannot be determined.
    pub fn workspace_root(&self) -> Result<&Path, XtaskError> {
        if let Some(root) = self.workspace_root.get() {
            return Ok(root.as_path());
        }

        // Use the lib.rs workspace_root function which returns a Result
        // Note: When compiling as part of the binary, this uses the lib version
        let root = find_workspace_root()?;
        self.workspace_root
            .set(root)
            .map_err(|_| XtaskError::new("Workspace root already initialized"))?;
        Ok(self.workspace_root.get().unwrap().as_path())
    }

    /// Get the temporary directory.
    pub fn temp_dir(&self) -> &tempfile::TempDir {
        &self.temp_dir
    }

    /// Validate that required resources are available.
    ///
    /// This is called by the executor before running a command to ensure
    /// all required resources can be initialized.
    ///
    /// # Arguments
    /// * `needs_database` - Whether to validate database access
    /// * `needs_embedding_runtime` - Whether to validate embedding runtime access
    pub fn validate_resources(
        &self,
        needs_database: bool,
        needs_embedding_runtime: bool,
    ) -> Result<(), XtaskError> {
        if needs_database {
            self.database_pool()?;
        }
        if needs_embedding_runtime {
            self.embedding_runtime()?;
        }
        Ok(())
    }
}

impl Default for CommandContext {
    fn default() -> Self {
        // This will panic if temp dir creation fails, but that's acceptable
        // for Default impl which should only be used in tests
        Self::new().expect("Failed to create CommandContext")
    }
}

/// Database pool: one shared in-memory database and cached file-backed imports.
pub struct DatabasePool {
    in_memory: std::sync::RwLock<Option<Arc<Database>>>,
    by_path: std::sync::RwLock<HashMap<PathBuf, Arc<Database>>>,
}

impl DatabasePool {
    /// Create a new database pool.
    pub fn new() -> Result<Arc<Self>, XtaskError> {
        Ok(Arc::new(Self {
            in_memory: std::sync::RwLock::new(None),
            by_path: std::sync::RwLock::new(HashMap::new()),
        }))
    }

    fn load_plain_backup(path: &Path) -> Result<Arc<Database>, XtaskError> {
        let db = Database::init_with_schema()?;
        let prior = db.relations_vec()?;
        db.import_from_backup(path, &prior)
            .map_err(|e| XtaskError::Database(e.to_string()))?;
        create_index_primary(&db)?;
        Ok(Arc::new(db))
    }

    fn load_backup_with_embeddings(path: &Path) -> Result<Arc<Database>, XtaskError> {
        let db = Database::init_with_schema()?;
        db.import_backup_with_embeddings(path)
            .map_err(|e| XtaskError::Database(e.to_string()))?;
        create_index_primary(&db)?;
        Ok(Arc::new(db))
    }

    /// Load a registered fixture with the correct import mode and primary index policy.
    pub fn get_from_fixture(&self, fixture: &'static FixtureDb) -> Result<Arc<Database>, XtaskError> {
        let key = fixture.path().canonicalize().map_err(|e| {
            XtaskError::validation(format!(
                "Could not resolve fixture database path {}: {}",
                fixture.path().display(),
                e
            ))
        })?;

        {
            let guard = self.by_path.read().unwrap();
            if let Some(db) = guard.get(&key) {
                return Ok(Arc::clone(db));
            }
        }

        let db_arc = match fixture.import_mode {
            FixtureImportMode::PlainBackup => Self::load_plain_backup(&key)?,
            FixtureImportMode::BackupWithEmbeddings => Self::load_backup_with_embeddings(&key)?,
        };

        let mut guard = self.by_path.write().unwrap();
        if let Some(db) = guard.get(&key) {
            return Ok(Arc::clone(db));
        }
        guard.insert(key, Arc::clone(&db_arc));
        Ok(db_arc)
    }

    /// In-memory schema (empty) or import from a Cozo backup file on disk.
    pub fn get_or_create(&self, path: Option<&Path>) -> Result<Arc<Database>, XtaskError> {
        match path {
            None => {
                let mut guard = self.in_memory.write().unwrap();
                if let Some(ref db) = *guard {
                    return Ok(Arc::clone(db));
                }
                let db = Arc::new(Database::init_with_schema()?);
                *guard = Some(Arc::clone(&db));
                Ok(db)
            }
            Some(p) => {
                let key = p.canonicalize().map_err(|e| {
                    XtaskError::validation(format!(
                        "Database path `{}` could not be resolved: {}",
                        p.display(),
                        e
                    ))
                    .with_recovery(
                        "Ensure the backup file exists (copy a registered fixture from tests/backup_dbs/ if needed). Use an absolute path or a path relative to the current working directory.",
                    )
                })?;

                {
                    let guard = self.by_path.read().unwrap();
                    if let Some(db) = guard.get(&key) {
                        return Ok(Arc::clone(db));
                    }
                }

                if !key.is_file() {
                    return Err(
                        XtaskError::validation(format!(
                            "Database backup file `{}` does not exist or is not a file",
                            key.display()
                        ))
                        .with_recovery(
                            "Check the path or use `db load-fixture <id>` with a registered fixture id (see `cargo xtask help-topic db`).",
                        ),
                    );
                }

                let db_arc = Self::load_plain_backup(&key)?;
                let mut guard = self.by_path.write().unwrap();
                if let Some(db) = guard.get(&key) {
                    return Ok(Arc::clone(db));
                }
                guard.insert(key, Arc::clone(&db_arc));
                Ok(db_arc)
            }
        }
    }
}

/// Placeholder embedding runtime manager.
///
/// This is a simplified implementation. The full implementation will integrate
/// with ploke_embed for actual embedding management.
pub struct EmbeddingRuntimeManager;

impl EmbeddingRuntimeManager {
    /// Create a new embedding runtime manager.
    ///
    /// # Errors
    /// Returns an error if the manager cannot be initialized.
    pub fn new() -> Result<Arc<Self>, XtaskError> {
        Ok(Arc::new(Self))
    }
}

/// Placeholder IO manager handle.
///
/// This is a simplified implementation. The full implementation will integrate
/// with ploke_io for actual IO management.
#[derive(Clone)]
pub struct IoManagerHandle;

impl IoManagerHandle {
    /// Create a new IO manager handle.
    pub fn new() -> Self {
        Self
    }
}

impl Default for IoManagerHandle {
    fn default() -> Self {
        Self::new()
    }
}

/// Find the workspace root directory.
///
/// This is duplicated from lib.rs to avoid conflicts with main.rs's workspace_root().
fn find_workspace_root() -> Result<std::path::PathBuf, XtaskError> {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    // xtask is at <workspace>/xtask, so workspace root is the parent
    manifest_dir
        .parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| XtaskError::new("Could not determine workspace root"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_new() {
        let ctx = CommandContext::new().unwrap();
        // Just verify creation succeeds
        assert!(ctx.workspace_root().is_ok());
    }

    #[test]
    fn test_context_default() {
        let ctx = CommandContext::default();
        assert!(ctx.workspace_root().is_ok());
    }

    #[test]
    fn test_io_manager() {
        let ctx = CommandContext::new().unwrap();
        let io1 = ctx.io_manager();
        let io2 = ctx.io_manager();
        // Both should be the same instance (cloned)
        // We can't really test this without internal state, but it compiles
        drop(io1);
        drop(io2);
    }

    #[test]
    fn test_temp_dir() {
        let ctx = CommandContext::new().unwrap();
        let temp = ctx.temp_dir();
        assert!(temp.path().exists());
    }

    #[test]
    fn test_validate_resources() {
        let ctx = CommandContext::new().unwrap();
        assert!(ctx.validate_resources(false, false).is_ok());
        assert!(ctx.validate_resources(true, false).is_ok());
    }

    #[test]
    fn test_database_pool_new() {
        let pool = DatabasePool::new().unwrap();
        // Get in-memory database
        let db = pool.get_or_create(None).unwrap();
        // Getting again should return the same instance
        let db2 = pool.get_or_create(None).unwrap();
        // They're Arcs, so we can compare pointers
        assert!(Arc::ptr_eq(&db, &db2));
    }

    #[test]
    fn test_io_manager_handle() {
        let handle = IoManagerHandle::new();
        let cloned = handle.clone();
        // Just verify they can be created and cloned
        drop(handle);
        drop(cloned);
    }

    #[test]
    fn test_context_loads_canonical_fixture() {
        use ploke_test_utils::FIXTURE_NODES_CANONICAL;

        let ctx = CommandContext::new().unwrap();
        let db = ctx
            .get_database_from_fixture(&FIXTURE_NODES_CANONICAL)
            .expect("canonical fixture should load");
        let qr = db
            .raw_query("?[count(id)] := *function { id }")
            .expect("fixture should contain function rows");
        assert!(!qr.rows.is_empty());
    }

    #[test]
    fn test_embedding_runtime_manager() {
        let mgr = EmbeddingRuntimeManager::new().unwrap();
        // Just verify creation succeeds
        drop(mgr);
    }
}
