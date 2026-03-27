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
use ploke_db::{Database, create_index_primary};
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

    /// Workspace root detection cache.
    workspace_root: OnceCell<std::path::PathBuf>,
}

impl CommandContext {
    /// Create a new command context.
    ///
    /// The context starts empty, with all resources being created on first access.
    ///
    /// # Errors
    pub fn new() -> Result<Self, XtaskError> {
        Ok(Self {
            database_pool: OnceCell::new(),
            workspace_root: OnceCell::new(),
        })
    }

    /// Create a new command context with an optional workspace-root override.
    ///
    /// When `workspace_root` is provided, relative paths (e.g. in `parse` commands)
    /// are resolved against this root instead of the compiled-in ploke workspace root.
    pub fn new_with_workspace_root(workspace_root: Option<PathBuf>) -> Result<Self, XtaskError> {
        let ctx = Self::new()?;

        if let Some(root) = workspace_root {
            if !root.exists() {
                return Err(XtaskError::validation(format!(
                    "Workspace root `{}` does not exist",
                    root.display()
                ))
                .into());
            }
            if !root.is_dir() {
                return Err(XtaskError::validation(format!(
                    "Workspace root `{}` is not a directory",
                    root.display()
                ))
                .into());
            }
            let canon = root.canonicalize().map_err(|e| {
                XtaskError::validation(format!(
                    "Could not canonicalize workspace root `{}`: {e}",
                    root.display()
                ))
            })?;

            ctx.workspace_root
                .set(canon)
                .map_err(|_| XtaskError::new("Workspace root already initialized"))?;
        }

        Ok(ctx)
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
}

impl Default for CommandContext {
    fn default() -> Self {
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
        let prior = db.prior_rels_for_plain_backup_import()?;
        db.import_from_backup(path, &prior)
            .map_err(|e| XtaskError::Database(e.to_string()))?;
        db.ensure_compilation_unit_relations()
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
    pub fn get_from_fixture(
        &self,
        fixture: &'static FixtureDb,
    ) -> Result<Arc<Database>, XtaskError> {
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

    // Resource validation and IO/embedding managers are intentionally absent in the paused build.

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
}
