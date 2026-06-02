# Architecture Proposal 3: Plugin-Based Command Executor with Async/Sync Hybrid

**Agent:** Architecture Agent 3  
**Date:** 2026-03-25  
**Milestone:** M.2.1 - Design Architecture + Documentation  
**Scope:** xtask commands feature architecture  

---

## Executive Summary

This proposal presents a **hybrid async/sync command architecture** using a plugin-based executor pattern. The design emphasizes:

1. **Type-safe command dispatch** via a unified `Command` trait
2. **Resource lifecycle management** through a context system
3. **Seamless async/sync interop** using `MaybeAsync` pattern
4. **Built-in observability** with usage tracking and tracing
5. **Test-first design** with a comprehensive test harness

---

## 1. Core Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           XTASK APPLICATION                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│  ┌─────────────┐    ┌─────────────────┐    ┌──────────────────────────┐    │
│  │   CLI Args  │───▶│  CommandRouter  │───▶│   CommandRegistry        │    │
│  └─────────────┘    └─────────────────┘    └──────────────────────────┘    │
│                                                      │                      │
│                           ┌──────────────────────────┼──────────────────┐   │
│                           │                          ▼                  │   │
│  ┌────────────────────────────────────────────────────────────────────┐ │   │
│  │                     COMMAND EXECUTOR                                │ │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌───────────┐  │ │   │
│  │  │   Parse     │  │  Transform  │  │    DB       │  │   TUI     │  │ │   │
│  │  │  Commands   │  │  Commands   │  │  Commands   │  │  Commands │  │ │   │
│  │  │  (sync)     │  │   (sync)    │  │  (mixed)    │  │  (async)  │  │ │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘  └───────────┘  │ │   │
│  │                                                                    │ │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌───────────┐  │ │   │
│  │  │   Ingest    │  │  Pipeline   │  │  Validate   │  │   Tool    │  │ │   │
│  │  │  Commands   │  │  Commands   │  │  Commands   │  │  Commands │  │ │   │
│  │  │  (async)    │  │  (mixed)    │  │  (mixed)    │  │  (async)  │  │ │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘  └───────────┘  │ │   │
│  └────────────────────────────────────────────────────────────────────┘ │   │
│                           │                                             │   │
│                           ▼                                             │   │
│  ┌────────────────────────────────────────────────────────────────────┐ │   │
│  │                     SHARED RESOURCES                                │ │   │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐  │ │   │
│  │  │  Database│ │ Embedding│ │   IO     │ │  Event   │ │  Tracing │  │ │   │
│  │  │   Pool   │ │  Runtime │ │ Manager  │ │   Bus    │ │ Subscriber│ │ │   │
│  │  └──────────┘ └──────────┘ └──────────┘ └──────────┘ └──────────┘  │ │   │
│  └────────────────────────────────────────────────────────────────────┘ │   │
│                           │                                             │   │
│                           ▼                                             │   │
│  ┌────────────────────────────────────────────────────────────────────┐ │   │
│  │                  OBSERVABILITY & FEEDBACK                           │ │   │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐                │ │   │
│  │  │  Usage   │ │  Suggest │ │   Log    │ │  Error   │                │ │   │
│  │  │  Stats   │ │  Engine  │ │  Capture │ │  Reports │                │ │   │
│  │  └──────────┘ └──────────┘ └──────────┘ └──────────┘                │ │   │
│  └────────────────────────────────────────────────────────────────────┘ │   │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Executor/Runner Pattern Design

### 2.1 Command Trait - The Core Abstraction

```rust
/// The fundamental trait that all xtask commands implement.
/// 
/// This trait uses an associated type `Output` to allow both sync and async
/// commands to be handled uniformly through the executor.
pub trait Command: Send + Sync + 'static {
    /// The type of output this command produces
    type Output: CommandOutput;
    
    /// The type of error this command can return
    type Error: Into<XtaskError>;
    
    /// Unique identifier for this command (e.g., "db count-nodes")
    fn name(&self) -> &'static str;
    
    /// Category for grouping in help output
    fn category(&self) -> CommandCategory;
    
    /// Execute the command with access to shared resources
    /// 
    /// The `ExecutionMode` determines whether this runs sync or async
    fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error>;
    
    /// Returns true if this command requires an async runtime
    fn requires_async(&self) -> bool;
    
    /// Estimated resource requirements for scheduling
    fn resource_requirements(&self) -> ResourceRequirements {
        ResourceRequirements::default()
    }
}

/// Marker trait for command outputs
pub trait CommandOutput: Serialize + Send + 'static {
    /// Render the output for display
    fn render(&self, format: OutputFormat) -> String;
    
    /// Get a brief summary for logging
    fn summary(&self) -> String;
}

/// Categories for organizing commands in help output
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandCategory {
    Parse,
    Transform,
    Database,
    Ingest,
    Pipeline,
    Validate,
    Setup,
    Tool,
    Utility,
}
```

### 2.2 Execution Modes - Handling Sync/Async Hybrid

```rust
/// Represents whether a command runs synchronously or asynchronously
pub enum ExecutionMode {
    /// Run synchronously on the current thread
    Sync,
    /// Run asynchronously using the tokio runtime
    Async { runtime: Arc<tokio::runtime::Runtime> },
}

/// A wrapper that can hold either a sync or async result
pub enum MaybeAsync<T> {
    Ready(T),
    Future(Pin<Box<dyn Future<Output = T> + Send>>),
}

impl<T> MaybeAsync<T> {
    /// Block on the result if async, or return immediately if sync
    pub fn block(self) -> T {
        match self {
            MaybeAsync::Ready(val) => val,
            MaybeAsync::Future(fut) => {
                // Use a thread-local runtime or blocking_recv
                block_on(fut)
            }
        }
    }
}

/// Macro for defining commands that can be either sync or async
#[macro_export]
macro_rules! define_command {
    (sync $name:ident => $impl:expr) => {
        impl Command for $name {
            fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
                $impl(self, ctx)
            }
            fn requires_async(&self) -> bool { false }
        }
    };
    (async $name:ident => $impl:expr) => {
        impl Command for $name {
            fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
                // The runtime will handle the async execution
                $impl(self, ctx)
            }
            fn requires_async(&self) -> bool { true }
        }
    };
}
```

### 2.3 The Command Executor

```rust
/// Central executor that manages command lifecycle and resource coordination
pub struct CommandExecutor {
    /// Shared context for all commands
    context: Arc<CommandContext>,
    
    /// Async runtime (shared across async commands)
    runtime: Option<Arc<tokio::runtime::Runtime>>,
    
    /// Usage statistics tracker
    usage_tracker: Arc<UsageTracker>,
    
    /// Tracing subscriber for command instrumentation
    tracing_subscriber: Arc<TracingSubscriber>,
}

impl CommandExecutor {
    /// Create a new executor with the given configuration
    pub fn new(config: ExecutorConfig) -> Result<Self, XtaskError> {
        let runtime = if config.enable_async {
            Some(Arc::new(
                tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| XtaskError::new(format!("Failed to create runtime: {e}")))?
            ))
        } else {
            None
        };
        
        let context = Arc::new(CommandContext::new(config.resource_config)?);
        let usage_tracker = Arc::new(UsageTracker::new(config.usage_log_path)?);
        let tracing_subscriber = Arc::new(TracingSubscriber::new(config.trace_output_dir)?);
        
        Ok(Self {
            context,
            runtime,
            usage_tracker,
            tracing_subscriber,
        })
    }
    
    /// Execute a command with full lifecycle management
    #[instrument(skip(self, cmd), fields(command = %cmd.name()))]
    pub fn execute<C: Command>(&self, cmd: C) -> Result<C::Output, XtaskError> {
        // Pre-execution: record usage
        let usage_start = self.usage_tracker.record_start(cmd.name());
        
        // Setup tracing
        let _span = info_span!("execute_command", command = %cmd.name());
        let _enter = _span.enter();
        
        // Validate prerequisites
        self.validate_prerequisites(&cmd)?;
        
        // Execute based on mode
        let result = if cmd.requires_async() {
            self.execute_async(cmd)
        } else {
            self.execute_sync(cmd)
        };
        
        // Post-execution: record completion
        self.usage_tracker.record_completion(usage_start, result.is_ok());
        
        // Check for suggestion trigger
        if self.usage_tracker.should_show_suggestion() {
            self.show_suggestion();
        }
        
        result
    }
    
    fn execute_sync<C: Command>(&self, cmd: C) -> Result<C::Output, XtaskError> {
        cmd.execute(&self.context)
            .map_err(|e| e.into())
    }
    
    fn execute_async<C: Command>(&self, cmd: C) -> Result<C::Output, XtaskError> {
        let runtime = self.runtime.as_ref()
            .ok_or_else(|| XtaskError::new("Async runtime not available"))?;
        
        let context = Arc::clone(&self.context);
        runtime.block_on(async move {
            cmd.execute(&context).map_err(|e| e.into())
        })
    }
    
    fn validate_prerequisites<C: Command>(&self, cmd: &C) -> Result<(), XtaskError> {
        // Check resource availability
        let reqs = cmd.resource_requirements();
        self.context.validate_resources(&reqs)
    }
}
```

---

## 3. Command Trait for Extensibility

### 3.1 Declarative Command Derive Macro

```rust
/// Derive macro for simple command definitions
/// 
/// Example:
/// ```rust
/// #[derive(Command)]
/// #[command(name = "db count-nodes", category = Database)]
/// struct DbCountNodes {
///     #[arg(short = 'd', long = "database")]
///     db_path: Option<PathBuf>,
/// }
/// 
/// impl DbCountNodes {
///     fn run(&self, ctx: &CommandContext) -> Result<CountOutput, DbError> {
///         let db = ctx.get_database(self.db_path.as_deref())?;
///         let count = db.count_pending_embeddings()?;
///         Ok(CountOutput { count })
///     }
/// }
/// ```
#[proc_macro_derive(Command, attributes(command, arg))]
pub fn derive_command(input: TokenStream) -> TokenStream {
    // Implementation generates Command trait implementation
    // including argument parsing and help text generation
}
```

### 3.2 Command Registry with Auto-Discovery

```rust
/// Registry that holds all available commands
pub struct CommandRegistry {
    /// Map from command name to factory function
    commands: HashMap<&'static str, CommandFactory>,
    
    /// Categories and their commands
    categories: HashMap<CommandCategory, Vec<&'static str>>,
}

type CommandFactory = Box<dyn Fn(&[&str]) -> Result<Box<dyn DynCommand>, XtaskError>>;

/// Dynamic command trait for type erasure
pub trait DynCommand: Send {
    fn name(&self) -> &'static str;
    fn category(&self) -> CommandCategory;
    fn execute(&self, ctx: &CommandContext) -> Result<Box<dyn Any>, XtaskError>;
    fn requires_async(&self) -> bool;
}

impl CommandRegistry {
    /// Register a command type
    pub fn register<C: Command>(&mut self) {
        let factory = Box::new(|args: &[&str]| {
            // Parse arguments and construct command
            let cmd = C::from_args(args)?;
            Ok(Box::new(cmd) as Box<dyn DynCommand>)
        });
        
        self.commands.insert(C::static_name(), factory);
        self.categories
            .entry(C::static_category())
            .or_default()
            .push(C::static_name());
    }
    
    /// Auto-discover commands from modules
    pub fn auto_discover(&mut self) {
        // Register all built-in commands
        self.register::<ParseDiscoveryCommand>();
        self.register::<ParsePhasesResolveCommand>();
        self.register::<ParsePhasesMergeCommand>();
        self.register::<ParseWorkspaceCommand>();
        self.register::<TransformGraphCommand>();
        self.register::<TransformWorkspaceCommand>();
        self.register::<DbSaveCommand>();
        self.register::<DbLoadCommand>();
        self.register::<DbLoadFixtureCommand>();
        self.register::<DbCountNodesCommand>();
        self.register::<DbHnswBuildCommand>();
        self.register::<DbHnswRebuildCommand>();
        self.register::<DbBm25RebuildCommand>();
        self.register::<DbQueryCommand>();
        self.register::<IngestEmbedCommand>();
        self.register::<IngestIndexCommand>();
        self.register::<PipelineParseTransformCommand>();
        self.register::<PipelineFullIngestCommand>();
        self.register::<PipelineWorkspaceCommand>();
        self.register::<ValidateParseIntegrityCommand>();
        self.register::<ValidateDbHealthCommand>();
        self.register::<ValidateEndToEndCommand>();
        self.register::<SetupTestEnvCommand>();
        self.register::<SetupDevWorkspaceCommand>();
        self.register::<TuiHeadlessCommand>();
        self.register::<ToolNsReadCommand>();
        self.register::<ToolCodeLookupCommand>();
    }
    
    /// Generate help text for all commands
    pub fn generate_help(&self) -> String {
        // Generate structured help output
        let mut output = String::from("xtask - Ploke workspace automation commands\n\n");
        
        for category in CommandCategory::all() {
            if let Some(commands) = self.categories.get(&category) {
                output.push_str(&format!("{}:\n", category.as_str()));
                for cmd_name in commands {
                    output.push_str(&format!("  {}\n", cmd_name));
                }
                output.push('\n');
            }
        }
        
        output
    }
}
```

### 3.3 Modular Command Organization

```
xtask/src/
├── main.rs                    # Entry point, CLI parsing
├── lib.rs                     # Public exports
├── executor/
│   ├── mod.rs                 # Executor and Command trait
│   ├── context.rs             # CommandContext
│   ├── registry.rs            # CommandRegistry
│   └── error.rs               # Error types
├── commands/
│   ├── mod.rs                 # Common command infrastructure
│   ├── parse/                 # A.1: Parsing commands
│   │   ├── mod.rs
│   │   ├── discovery.rs
│   │   ├── phases.rs
│   │   └── workspace.rs
│   ├── transform/             # A.2: Transform commands
│   │   ├── mod.rs
│   │   ├── graph.rs
│   │   └── workspace.rs
│   ├── ingest/                # A.3: Ingestion commands
│   │   ├── mod.rs
│   │   ├── embed.rs
│   │   └── index.rs
│   ├── db/                    # A.4: Database commands
│   │   ├── mod.rs
│   │   ├── backup.rs
│   │   ├── fixtures.rs
│   │   ├── indexing.rs
│   │   └── query.rs
│   ├── pipeline/              # Cross-crate pipeline commands
│   │   ├── mod.rs
│   │   ├── parse_transform.rs
│   │   ├── full_ingest.rs
│   │   └── workspace.rs
│   ├── validate/              # Validation commands
│   │   ├── mod.rs
│   │   ├── parse_integrity.rs
│   │   ├── db_health.rs
│   │   └── end_to_end.rs
│   ├── setup/                 # Setup commands
│   │   ├── mod.rs
│   │   ├── test_env.rs
│   │   └── dev_workspace.rs
│   ├── tui/                   # A.5: Headless TUI commands
│   │   ├── mod.rs
│   │   ├── headless.rs
│   │   ├── input.rs
│   │   └── key.rs
│   └── tool/                  # A.6: Tool commands
│       ├── mod.rs
│       ├── ns_read.rs
│       ├── code_lookup.rs
│       └── cargo.rs
├── resources/                 # Resource management
│   ├── mod.rs
│   ├── database.rs
│   ├── embedder.rs
│   └── io_manager.rs
├── observability/             # Usage tracking, tracing
│   ├── mod.rs
│   ├── usage.rs
│   ├── tracing.rs
│   └── suggestions.rs
└── tests/                     # Test harness
    ├── mod.rs
    ├── harness.rs
    └── commands/
```

---

## 4. Resource Pool/Context Design

### 4.1 CommandContext - The Resource Container

```rust
/// Shared context passed to all commands.
/// 
/// Provides lazy-initialized access to expensive resources like
/// database connections and embedding runtimes.
pub struct CommandContext {
    /// Configuration for resource creation
    config: ResourceConfig,
    
    /// Database pool - lazy initialized
    database_pool: OnceLock<Arc<DatabasePool>>,
    
    /// Embedding runtime - lazy initialized
    embedding_runtime: OnceLock<Arc<EmbeddingRuntimeManager>>,
    
    /// IO manager - shared across commands
    io_manager: OnceLock<IoManagerHandle>,
    
    /// Event bus for async commands
    event_bus: OnceLock<Arc<EventBus>>,
    
    /// Tracing dispatcher
    tracing_dispatcher: tracing::Dispatch,
    
    /// Temporary directory for intermediate files
    temp_dir: TempDir,
    
    /// Workspace root detection cache
    workspace_root: OnceLock<PathBuf>,
}

impl CommandContext {
    pub fn new(config: ResourceConfig) -> Result<Self, XtaskError> {
        let temp_dir = tempfile::tempdir()
            .map_err(|e| XtaskError::new(format!("Failed to create temp dir: {e}")))?;
        
        Ok(Self {
            config,
            database_pool: OnceLock::new(),
            embedding_runtime: OnceLock::new(),
            io_manager: OnceLock::new(),
            event_bus: OnceLock::new(),
            tracing_dispatcher: create_tracing_dispatcher(),
            temp_dir,
            workspace_root: OnceLock::new(),
        })
    }
    
    /// Get or create the database pool
    pub fn database_pool(&self) -> Result<Arc<DatabasePool>, XtaskError> {
        self.database_pool
            .get_or_try_init(|| DatabasePool::new(self.config.database.clone()))
            .map(Arc::clone)
    }
    
    /// Get a database from the pool (or in-memory if no path specified)
    pub fn get_database(&self, path: Option<&Path>) -> Result<Arc<Database>, XtaskError> {
        let pool = self.database_pool()?;
        pool.get_or_create(path)
    }
    
    /// Get or create the embedding runtime
    pub fn embedding_runtime(&self) -> Result<Arc<EmbeddingRuntimeManager>, XtaskError> {
        self.embedding_runtime
            .get_or_try_init(|| EmbeddingRuntimeManager::new(self.config.embedding.clone()))
            .map(Arc::clone)
    }
    
    /// Get the IO manager
    pub fn io_manager(&self) -> IoManagerHandle {
        self.io_manager
            .get_or_init(IoManagerHandle::new)
            .clone()
    }
    
    /// Get or create the event bus
    pub fn event_bus(&self) -> Arc<EventBus> {
        self.event_bus
            .get_or_init(|| Arc::new(EventBus::new(EventBusCaps::default())))
            .clone()
    }
    
    /// Get the workspace root
    pub fn workspace_root(&self) -> Result<&Path, XtaskError> {
        self.workspace_root
            .get_or_try_init(|| find_workspace_root())
            .map(|p| p.as_path())
    }
    
    /// Validate that required resources are available
    pub fn validate_resources(&self, reqs: &ResourceRequirements) -> Result<(), XtaskError> {
        if reqs.needs_database {
            self.database_pool()?;
        }
        if reqs.needs_embedding_runtime {
            self.embedding_runtime()?;
        }
        Ok(())
    }
}
```

### 4.2 Database Pool Management

```rust
/// Manages database instances with lifecycle tracking
pub struct DatabasePool {
    /// Configuration for new databases
    config: DatabaseConfig,
    
    /// In-memory database (default)
    in_memory: RwLock<Option<Arc<Database>>>,
    
    /// Persistent databases by path
    persistent: RwLock<HashMap<PathBuf, Arc<Database>>>,
    
    /// Cleanup handlers for graceful shutdown
    cleanup_handlers: Mutex<Vec<Box<dyn FnOnce() + Send>>>,
}

impl DatabasePool {
    pub fn new(config: DatabaseConfig) -> Result<Arc<Self>, XtaskError> {
        Ok(Arc::new(Self {
            config,
            in_memory: RwLock::new(None),
            persistent: RwLock::new(HashMap::new()),
            cleanup_handlers: Mutex::new(Vec::new()),
        }))
    }
    
    /// Get or create a database
    pub fn get_or_create(&self, path: Option<&Path>) -> Result<Arc<Database>, XtaskError> {
        match path {
            None => {
                // In-memory database
                let mut guard = self.in_memory.write().unwrap();
                if let Some(ref db) = *guard {
                    Ok(Arc::clone(db))
                } else {
                    let db = Arc::new(Database::init_with_schema()
                        .map_err(|e| XtaskError::new(format!("Failed to init database: {e}")))?);
                    *guard = Some(Arc::clone(&db));
                    Ok(db)
                }
            }
            Some(path) => {
                // Persistent database
                let mut guard = self.persistent.write().unwrap();
                if let Some(db) = guard.get(path) {
                    Ok(Arc::clone(db))
                } else {
                    let db = self.create_persistent_database(path)?;
                    guard.insert(path.to_path_buf(), Arc::clone(&db));
                    Ok(db)
                }
            }
        }
    }
    
    fn create_persistent_database(&self, path: &Path) -> Result<Arc<Database>, XtaskError> {
        // Implementation for persistent DB creation
        // with schema initialization
        todo!()
    }
}

impl Drop for DatabasePool {
    fn drop(&mut self) {
        // Run cleanup handlers for graceful shutdown
        let handlers = std::mem::take(&mut *self.cleanup_handlers.lock().unwrap());
        for handler in handlers {
            handler();
        }
    }
}
```

### 4.3 Embedding Runtime Manager

```rust
/// Manages embedding runtime lifecycle and configuration
pub struct EmbeddingRuntimeManager {
    /// Configuration
    config: EmbeddingConfig,
    
    /// Active runtime (lazy initialized)
    runtime: RwLock<Option<Arc<EmbeddingRuntime>>>,
    
    /// Cancellation token for graceful shutdown
    cancel_token: CancellationToken,
}

impl EmbeddingRuntimeManager {
    pub fn new(config: EmbeddingConfig) -> Result<Arc<Self>, XtaskError> {
        let (cancel_token, _handle) = CancellationToken::new();
        
        Ok(Arc::new(Self {
            config,
            runtime: RwLock::new(None),
            cancel_token,
        }))
    }
    
    /// Get or create the embedding runtime
    pub fn get_runtime(&self) -> Result<Arc<EmbeddingRuntime>, XtaskError> {
        let guard = self.runtime.read().unwrap();
        if let Some(ref runtime) = *guard {
            return Ok(Arc::clone(runtime));
        }
        drop(guard);
        
        let mut guard = self.runtime.write().unwrap();
        if let Some(ref runtime) = *guard {
            return Ok(Arc::clone(runtime));
        }
        
        let runtime = self.create_runtime()?;
        *guard = Some(Arc::clone(&runtime));
        Ok(runtime)
    }
    
    fn create_runtime(&self) -> Result<Arc<EmbeddingRuntime>, XtaskError> {
        let processor = match self.config.backend {
            EmbeddingBackend::Mock => {
                EmbeddingProcessor::new_mock()
            }
            EmbeddingBackend::Local => {
                let embedder = LocalEmbedder::new(self.config.local_config.clone())
                    .map_err(|e| XtaskError::new(format!("Failed to create local embedder: {e}")))?;
                EmbeddingProcessor::new(EmbeddingSource::Local(embedder))
            }
            EmbeddingBackend::OpenRouter => {
                // Use TEST_OPENROUTER_API_KEY as specified
                let api_key = std::env::var("TEST_OPENROUTER_API_KEY")
                    .map_err(|_| XtaskError::new(
                        "TEST_OPENROUTER_API_KEY environment variable not set"
                    ))?;
                let config = OpenRouterConfig {
                    model: self.config.openrouter_model.clone(),
                    ..Default::default()
                };
                // Implementation details...
                todo!()
            }
        };
        
        let runtime = EmbeddingRuntime::with_default_set(processor);
        Ok(Arc::new(runtime))
    }
}
```

---

## 5. Test Harness for Commands

### 5.1 TestCommand Trait

```rust
/// Trait for commands that support testing
pub trait TestableCommand: Command {
    /// Create a test fixture for this command
    fn test_fixture(&self) -> Box<dyn CommandFixture>;
    
    /// Get test cases for this command
    fn test_cases(&self) -> Vec<TestCase>;
    
    /// Verify command output
    fn verify_output(&self, expected: &Self::Output, actual: &Self::Output) -> TestResult;
}

/// A fixture that sets up the environment for a command test
pub trait CommandFixture: Send {
    /// Setup the test environment
    fn setup(&mut self) -> Result<CommandContext, XtaskError>;
    
    /// Teardown the test environment
    fn teardown(&mut self) -> Result<(), XtaskError>;
    
    /// Get the test input
    fn input(&self) -> Vec<String>;
}

/// A test case for a command
pub struct TestCase {
    pub name: String,
    pub args: Vec<String>,
    pub expected_output: Option<serde_json::Value>,
    pub expected_error: Option<String>,
    pub timeout: Duration,
}

/// Result of a test verification
pub struct TestResult {
    pub passed: bool,
    pub message: String,
    pub diffs: Vec<StringDiff>,
}
```

### 5.2 Integration Test Harness

```rust
/// Test harness for running command tests
pub struct CommandTestHarness {
    executor: CommandExecutor,
    fixtures_dir: PathBuf,
}

impl CommandTestHarness {
    pub fn new() -> Result<Self, XtaskError> {
        let config = ExecutorConfig {
            enable_async: true,
            resource_config: ResourceConfig::default(),
            usage_log_path: None, // Don't track usage in tests
            trace_output_dir: Some(tempfile::tempdir()?.into_path()),
        };
        
        Ok(Self {
            executor: CommandExecutor::new(config)?,
            fixtures_dir: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests")
                .join("fixtures"),
        })
    }
    
    /// Run a command test
    pub fn run_test<C: Command>(
        &self,
        cmd: C,
        expected: ExpectedResult<C::Output>,
    ) -> TestReport {
        let start = Instant::now();
        
        let result = self.executor.execute(cmd);
        
        let duration = start.elapsed();
        let outcome = match (result, expected) {
            (Ok(actual), ExpectedResult::Success(expected)) => {
                if actual == expected {
                    TestOutcome::Passed
                } else {
                    TestOutcome::FailedOutputMismatch { actual, expected }
                }
            }
            (Ok(_), ExpectedResult::Failure(expected_err)) => {
                TestOutcome::FailedUnexpectedSuccess { expected_err }
            }
            (Err(actual_err), ExpectedResult::Success(_)) => {
                TestOutcome::FailedUnexpectedError { actual_err }
            }
            (Err(actual_err), ExpectedResult::Failure(expected_err_pattern)) => {
                if actual_err.to_string().contains(&expected_err_pattern) {
                    TestOutcome::Passed
                } else {
                    TestOutcome::FailedErrorMismatch { actual_err, expected_pattern: expected_err_pattern }
                }
            }
        };
        
        TestReport {
            duration,
            outcome,
        }
    }
    
    /// Run a command with a fixture database
    pub fn run_with_fixture<C, F>(
        &self,
        fixture_id: &str,
        cmd_factory: F,
    ) -> Result<C::Output, XtaskError>
    where
        C: Command,
        F: FnOnce(Arc<Database>) -> C,
    {
        let fixture = backup_db_fixture(fixture_id)
            .ok_or_else(|| XtaskError::new(format!("Unknown fixture: {fixture_id}")))?;
        
        let db = fresh_backup_fixture_db(fixture)?;
        let cmd = cmd_factory(Arc::new(db));
        
        self.executor.execute(cmd)
    }
}

/// Expected result of a test
pub enum ExpectedResult<T> {
    Success(T),
    Failure(String),
}
```

### 5.3 Test Organization

```rust
// tests/commands/parse_tests.rs
#[cfg(test)]
mod tests {
    use super::*;
    use xtask::test_harness::CommandTestHarness;
    
    #[test]
    fn test_parse_discovery_valid_crate() {
        let harness = CommandTestHarness::new().unwrap();
        
        let cmd = ParseDiscoveryCommand {
            target: PathBuf::from("tests/fixture_crates/fixture_nodes"),
        };
        
        let result = harness.run_test(cmd, ExpectedResult::Success(()));
        assert!(result.outcome.is_passed());
    }
    
    #[test]
    fn test_parse_discovery_missing_cargo_toml() {
        let harness = CommandTestHarness::new().unwrap();
        
        let cmd = ParseDiscoveryCommand {
            target: PathBuf::from("tests/fixture_crates/nonexistent"),
        };
        
        let result = harness.run_test(
            cmd,
            ExpectedResult::Failure("Cargo.toml not found".to_string())
        );
        assert!(result.outcome.is_passed());
    }
}

// tests/commands/db_tests.rs
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_db_count_nodes_with_fixture() {
        let harness = CommandTestHarness::new().unwrap();
        
        let result: Result<CountOutput, _> = harness.run_with_fixture(
            "fixture_nodes_canonical",
            |db| DbCountNodesCommand { db_path: None, db: Some(db) }
        );
        
        let output = result.expect("Command should succeed");
        assert!(output.count > 0, "Should have nodes in database");
    }
}
```

---

## 6. Usage Tracking Approach

### 6.1 Usage Tracker

```rust
/// Tracks command usage for analytics and suggestions
pub struct UsageTracker {
    /// Path to the usage log file
    log_path: PathBuf,
    
    /// In-memory buffer for recent usage
    buffer: Mutex<Vec<UsageRecord>>,
    
    /// Threshold for showing suggestions (every N runs)
    suggestion_threshold: usize,
    
    /// Last suggestion timestamp
    last_suggestion: Mutex<Option<Instant>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageRecord {
    pub timestamp: DateTime<Utc>,
    pub command_name: String,
    pub duration_ms: u64,
    pub success: bool,
    pub exit_code: Option<i32>,
}

impl UsageTracker {
    pub fn new(log_path: Option<PathBuf>) -> Result<Self, XtaskError> {
        let log_path = log_path.unwrap_or_else(|| {
            dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("ploke")
                .join("xtask_usage.jsonl")
        });
        
        // Ensure directory exists
        if let Some(parent) = log_path.parent() {
            fs::create_dir_all(parent)?;
        }
        
        Ok(Self {
            log_path,
            buffer: Mutex::new(Vec::new()),
            suggestion_threshold: 50, // As per requirements
            last_suggestion: Mutex::new(None),
        })
    }
    
    /// Record the start of a command execution
    pub fn record_start(&self, command_name: &str) -> UsageStart {
        UsageStart {
            command_name: command_name.to_string(),
            timestamp: Utc::now(),
        }
    }
    
    /// Record command completion
    pub fn record_completion(&self, start: UsageStart, success: bool) {
        let duration = Utc::now().signed_duration_since(start.timestamp);
        
        let record = UsageRecord {
            timestamp: start.timestamp,
            command_name: start.command_name,
            duration_ms: duration.num_milliseconds() as u64,
            success,
            exit_code: if success { Some(0) } else { Some(1) },
        };
        
        // Buffer the record
        {
            let mut buffer = self.buffer.lock().unwrap();
            buffer.push(record.clone());
            
            // Flush if buffer is large enough
            if buffer.len() >= 10 {
                self.flush_buffer(&buffer);
                buffer.clear();
            }
        }
    }
    
    fn flush_buffer(&self, buffer: &[UsageRecord]) {
        if buffer.is_empty() {
            return;
        }
        
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path);
        
        if let Ok(mut file) = file {
            for record in buffer {
                if let Ok(json) = serde_json::to_string(record) {
                    writeln!(file, "{}", json).ok();
                }
            }
        }
    }
    
    /// Get total command count
    pub fn total_command_count(&self) -> Result<usize, XtaskError> {
        let file = File::open(&self.log_path)?;
        let reader = BufReader::new(file);
        Ok(reader.lines().count())
    }
    
    /// Check if we should show a suggestion (every 50 runs)
    pub fn should_show_suggestion(&self) -> bool {
        let count = self.total_command_count().unwrap_or(0);
        
        if count > 0 && count % self.suggestion_threshold == 0 {
            let last = *self.last_suggestion.lock().unwrap();
            // Only show once per threshold
            last.map(|t| t.elapsed() > Duration::from_secs(60))
                .unwrap_or(true)
        } else {
            false
        }
    }
    
    /// Show the rolling suggestion
    pub fn show_suggestion(&self) {
        *self.last_suggestion.lock().unwrap() = Some(Instant::now());
        
        eprintln!("\n💡 Auto-generated suggestion:");
        eprintln!("   Have feedback or suggestions for xtask commands?");
        eprintln!("   Check {} and don't forget to be honest!\n",
            self.feedback_file_path().display()
        );
    }
    
    fn feedback_file_path(&self) -> PathBuf {
        self.log_path.with_file_name("xtask_feedback.md")
    }
}
```

### 6.2 Statistics and Reporting

```rust
/// Usage statistics report
pub struct UsageStats {
    pub total_commands: usize,
    pub total_success: usize,
    pub total_failure: usize,
    pub command_breakdown: HashMap<String, CommandStats>,
    pub average_duration_ms: f64,
}

pub struct CommandStats {
    pub count: usize,
    pub success_count: usize,
    pub failure_count: usize,
    pub average_duration_ms: f64,
    pub last_used: DateTime<Utc>,
}

impl UsageTracker {
    /// Generate a usage statistics report
    pub fn generate_stats(&self) -> Result<UsageStats, XtaskError> {
        let file = File::open(&self.log_path)?;
        let reader = BufReader::new(file);
        
        let mut total_commands = 0;
        let mut total_success = 0;
        let mut total_duration = 0u64;
        let mut command_stats: HashMap<String, CommandStats> = HashMap::new();
        
        for line in reader.lines() {
            let line = line?;
            if let Ok(record) = serde_json::from_str::<UsageRecord>(&line) {
                total_commands += 1;
                if record.success {
                    total_success += 1;
                }
                total_duration += record.duration_ms;
                
                let stats = command_stats
                    .entry(record.command_name.clone())
                    .or_insert_with(|| CommandStats {
                        count: 0,
                        success_count: 0,
                        failure_count: 0,
                        average_duration_ms: 0.0,
                        last_used: record.timestamp,
                    });
                
                stats.count += 1;
                if record.success {
                    stats.success_count += 1;
                } else {
                    stats.failure_count += 1;
                }
                stats.average_duration_ms = 
                    (stats.average_duration_ms * (stats.count - 1) as f64 
                     + record.duration_ms as f64) / stats.count as f64;
                if record.timestamp > stats.last_used {
                    stats.last_used = record.timestamp;
                }
            }
        }
        
        Ok(UsageStats {
            total_commands,
            total_success,
            total_failure: total_commands - total_success,
            command_breakdown: command_stats,
            average_duration_ms: if total_commands > 0 {
                total_duration as f64 / total_commands as f64
            } else {
                0.0
            },
        })
    }
}
```

---

## 7. Documentation Generation

### 7.1 Automatic Help Generation

```rust
/// Generates documentation for all registered commands
pub struct DocumentationGenerator {
    registry: CommandRegistry,
    output_dir: PathBuf,
}

impl DocumentationGenerator {
    /// Generate markdown documentation for all commands
    pub fn generate_markdown(&self) -> Result<String, XtaskError> {
        let mut doc = String::new();
        
        // Header
        doc.push_str("# xtask Commands Reference\n\n");
        doc.push_str("**Auto-generated:** ");
        doc.push_str(&Utc::now().to_rfc3339());
        doc.push_str("\n\n");
        
        // Table of contents
        doc.push_str("## Table of Contents\n\n");
        for category in CommandCategory::all() {
            let anchor = category.as_str().to_lowercase().replace(' ', "-");
            doc.push_str(&format!("- [{}](#{})\n", category.as_str(), anchor));
        }
        doc.push('\n');
        
        // Per-category documentation
        for category in CommandCategory::all() {
            if let Some(commands) = self.registry.get_category_commands(category) {
                doc.push_str(&format!("## {}\n\n", category.as_str()));
                
                for cmd_name in commands {
                    if let Some(cmd_info) = self.registry.get_command_info(cmd_name) {
                        self.document_command(&mut doc, cmd_info)?;
                    }
                }
            }
        }
        
        Ok(doc)
    }
    
    fn document_command(&self, doc: &mut String, info: CommandInfo) -> Result<(), XtaskError> {
        doc.push_str(&format!("### `{}`\n\n", info.name));
        doc.push_str(&format!("{}\n\n", info.description));
        
        if !info.examples.is_empty() {
            doc.push_str("**Examples:**\n\n");
            for example in &info.examples {
                doc.push_str(&format!("```bash\n{}\n```\n\n", example));
            }
        }
        
        doc.push_str("**Arguments:**\n\n");
        if info.args.is_empty() {
            doc.push_str("*None*\n\n");
        } else {
            doc.push_str("| Name | Type | Required | Description |\n");
            doc.push_str("|------|------|----------|-------------|\n");
            for arg in &info.args {
                doc.push_str(&format!(
                    "| `{}` | {} | {} | {} |\n",
                    arg.name,
                    arg.arg_type,
                    if arg.required { "Yes" } else { "No" },
                    arg.description
                ));
            }
            doc.push('\n');
        }
        
        doc.push('\n');
        Ok(())
    }
    
    /// Generate a "last updated" file for staleness checking
    pub fn write_last_updated(&self) -> Result<(), XtaskError> {
        let path = self.output_dir.join(".last_updated");
        let timestamp = Utc::now().to_rfc3339();
        fs::write(&path, timestamp)?;
        Ok(())
    }
    
    /// Check if documentation is stale (> 48 hours old)
    pub fn is_documentation_stale(&self) -> Result<bool, XtaskError> {
        let path = self.output_dir.join(".last_updated");
        if !path.exists() {
            return Ok(true);
        }
        
        let content = fs::read_to_string(&path)?;
        let last_updated: DateTime<Utc> = content.parse()
            .map_err(|e| XtaskError::new(format!("Invalid timestamp: {e}")))?;
        
        let age = Utc::now().signed_duration_since(last_updated);
        Ok(age > Duration::hours(48))
    }
}
```

### 7.2 In-Line Documentation Standards

```rust
/// Macro for defining well-documented commands
/// 
/// This macro enforces documentation standards and generates
/// help text automatically.
#[macro_export]
macro_rules! documented_command {
    (
        $(#[$meta:meta])*
        pub struct $name:ident {
            $(
                $(#[$field_meta:meta])*
                $field_name:ident: $field_type:ty
            ),* $(,)?
        }
        
        impl Command for $name {
            name: $cmd_name:expr,
            category: $category:expr,
            description: $description:expr,
            
            $(
                example: $example:expr;
            )*
            
            execute: $execute:expr
        }
    ) => {
        $(#[$meta])*
        pub struct $name {
            $(
                $(#[$field_meta])*
                $field_name: $field_type,
            )*
        }
        
        impl Command for $name {
            fn name(&self) -> &'static str { $cmd_name }
            fn category(&self) -> CommandCategory { $category }
            
            fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
                $execute(self, ctx)
            }
        }
        
        impl $name {
            /// Get help text for this command
            pub fn help_text() -> String {
                let mut help = String::new();
                help.push_str(&format!("{}\n\n", $description));
                help.push_str(&format!("Usage: cargo xtask {} [OPTIONS]\n\n", $cmd_name));
                help.push_str("Options:\n");
                $(
                    help.push_str(&format!("  {}\n", stringify!($field_name)));
                )*
                help.push_str("\nExamples:\n");
                $(
                    help.push_str(&format!("  {}\n", $example));
                )*
                help
            }
        }
    };
}
```

---

## 8. Example Implementations

### 8.1 Sync Command Example: `parse discovery`

```rust
// src/commands/parse/discovery.rs
use syn_parser::discovery::run_discovery_phase;

/// Parse the discovery phase for a crate or workspace
#[derive(Debug, Clone)]
pub struct ParseDiscoveryCommand {
    /// Path to the crate or workspace
    pub target: PathBuf,
    
    /// Whether to show warnings
    pub show_warnings: bool,
}

impl Command for ParseDiscoveryCommand {
    type Output = DiscoveryOutput;
    type Error = SynParserError;
    
    fn name(&self) -> &'static str { "parse discovery" }
    fn category(&self) -> CommandCategory { CommandCategory::Parse }
    fn requires_async(&self) -> bool { false }
    
    #[instrument(skip(self, ctx), fields(target = %self.target.display()))]
    fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        // Resolve the target path
        let target = if self.target.is_absolute() {
            self.target.clone()
        } else {
            ctx.workspace_root()?.join(&self.target)
        };
        
        // Validate the target exists
        if !target.exists() {
            return Err(SynParserError::Discovery(
                DiscoveryError::CratePathNotFound { path: target }
            ));
        }
        
        // Run discovery phase
        let output = run_discovery_phase(None, &[target])?;
        
        // Output warnings if requested
        if self.show_warnings && output.has_warnings() {
            for warning in output.warnings() {
                eprintln!("Warning: {}", warning);
            }
        }
        
        // Print summary
        println!("Discovered {} crate(s)", output.crate_contexts().len());
        for (path, context) in output.iter_crate_contexts() {
            println!("  - {} at {}", context.name, path.display());
            println!("    Files: {}", context.files.len());
            println!("    Dependencies: {}", context.dependencies.0.len());
        }
        
        Ok(output)
    }
}

impl CommandOutput for DiscoveryOutput {
    fn render(&self, format: OutputFormat) -> String {
        match format {
            OutputFormat::Json => serde_json::to_string_pretty(self).unwrap_or_default(),
            OutputFormat::Table => {
                // Generate table output
                let mut output = String::new();
                output.push_str(&format!("Crate Contexts: {}\n", self.crate_contexts().len()));
                // ... table formatting
                output
            }
            OutputFormat::Human => self.summary(),
        }
    }
    
    fn summary(&self) -> String {
        format!("Discovered {} crate(s)", self.crate_contexts().len())
    }
}
```

### 8.2 Async Command Example: `ingest embed`

```rust
// src/commands/ingest/embed.rs

/// Run the embedding generation pipeline
#[derive(Debug, Clone)]
pub struct IngestEmbedCommand {
    /// Path to the database (or in-memory if None)
    pub db_path: Option<PathBuf>,
    
    /// Embedding backend to use
    pub backend: EmbeddingBackend,
    
    /// Batch size override
    pub batch_size: Option<usize>,
}

impl Command for IngestEmbedCommand {
    type Output = IngestEmbedOutput;
    type Error = EmbedError;
    
    fn name(&self) -> &'static str { "ingest embed" }
    fn category(&self) -> CommandCategory { CommandCategory::Ingest }
    fn requires_async(&self) -> bool { true }
    
    fn resource_requirements(&self) -> ResourceRequirements {
        ResourceRequirements {
            needs_database: true,
            needs_embedding_runtime: true,
            ..Default::default()
        }
    }
    
    #[instrument(skip(self, ctx), fields(backend = ?self.backend))]
    fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        // Note: This runs synchronously but the actual work happens in block_on
        // The executor will handle running this in an async context
        
        let db = ctx.get_database(self.db_path.as_deref())
            .map_err(|e| EmbedError::Database(e.into()))?;
        
        let io_manager = ctx.io_manager();
        
        // Setup embedding runtime based on backend
        let processor = self.create_processor()?;
        let embedding_runtime = Arc::new(EmbeddingRuntime::with_default_set(processor));
        
        // Create cancellation token
        let (cancel_token, cancel_handle) = CancellationToken::new();
        
        // Setup channels for progress
        let (progress_tx, mut progress_rx) = broadcast::channel::<IndexingStatus>(32);
        let (_control_tx, control_rx) = mpsc::channel::<IndexerCommand>(4);
        
        // Create indexer task
        let indexer = IndexerTask::new(
            Arc::clone(&db),
            io_manager,
            Arc::clone(&embedding_runtime),
            cancel_token,
            cancel_handle,
            self.batch_size,
        );
        
        // Track progress in a separate task
        let progress_tracker = tokio::spawn(async move {
            let mut total_processed = 0;
            while let Ok(status) = progress_rx.recv().await {
                total_processed += status.recent_processed;
                if status.status == IndexStatus::Completed {
                    break;
                }
                if status.status == IndexStatus::Failed(ref msg) {
                    eprintln!("Indexing failed: {}", msg);
                    break;
                }
            }
            total_processed
        });
        
        // Run the indexer
        let indexer_handle = tokio::spawn(async move {
            indexer.run(Arc::new(progress_tx), control_rx).await
        });
        
        // Wait for completion
        let result = tokio::runtime::Handle::current().block_on(async {
            let (indexer_result, processed) = tokio::join!(indexer_handle, progress_tracker);
            
            indexer_result.map_err(|e| EmbedError::JoinFailed(e.to_string()))??;
            
            Ok::<_, EmbedError>(processed.unwrap_or(0))
        })?;
        
        // Build HNSW index
        create_index_primary(&db)
            .map_err(|e| EmbedError::Database(e.into()))?;
        
        Ok(IngestEmbedOutput {
            nodes_processed: result,
        })
    }
}

impl IngestEmbedCommand {
    fn create_processor(&self) -> Result<EmbeddingProcessor, EmbedError> {
        match self.backend {
            EmbeddingBackend::Mock => Ok(EmbeddingProcessor::new_mock()),
            EmbeddingBackend::Local => {
                let config = EmbeddingConfig::default();
                let embedder = LocalEmbedder::new(config)
                    .map_err(|e| EmbedError::LocalModel(e.to_string()))?;
                Ok(EmbeddingProcessor::new(EmbeddingSource::Local(embedder)))
            }
            EmbeddingBackend::OpenRouter => {
                // Use TEST_OPENROUTER_API_KEY - no overrides accepted by design
                let api_key = std::env::var("TEST_OPENROUTER_API_KEY")
                    .map_err(|_| EmbedError::Config(
                        "TEST_OPENROUTER_API_KEY environment variable not set. \
                         This is required for OpenRouter embedding commands.".to_string()
                    ))?;
                
                let config = OpenRouterConfig::default();
                let env = OpenRouterEmbedEnv::from_parts(api_key, None);
                let backend = OpenRouterBackend::new_with_env(&config, env)
                    .map_err(|e| EmbedError::Config(e.to_string()))?;
                
                Ok(EmbeddingProcessor::new(EmbeddingSource::OpenRouter(backend)))
            }
        }
    }
}
```

### 8.3 Pipeline Command Example: `pipeline parse-transform`

```rust
// src/commands/pipeline/parse_transform.rs

/// Parse and transform in a single pipeline
#[derive(Debug, Clone)]
pub struct PipelineParseTransformCommand {
    pub crate_path: PathBuf,
    pub db_path: Option<PathBuf>,
}

impl Command for PipelineParseTransformCommand {
    type Output = PipelineOutput;
    type Error = XtaskError;
    
    fn name(&self) -> &'static str { "pipeline parse-transform" }
    fn category(&self) -> CommandCategory { CommandCategory::Pipeline }
    fn requires_async(&self) -> bool { false }
    
    #[instrument(skip(self, ctx))]
    fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        let start = Instant::now();
        
        // Stage 1: Initialize database
        info!("Stage 1/3: Initializing database...");
        let db = ctx.get_database(self.db_path.as_deref())?;
        
        // Stage 2: Parse
        info!("Stage 2/3: Parsing crate at {:?}...", self.crate_path);
        let mut output = try_run_phases_and_merge(&self.crate_path)
            .map_err(|e| XtaskError::new(format!("Parse failed: {e}")))?;
        
        let merged = output.extract_merged_graph()
            .ok_or_else(|| XtaskError::new("Missing merged graph"))?;
        let tree = output.extract_module_tree()
            .ok_or_else(|| XtaskError::new("Missing module tree"))?;
        
        let parse_stats = ParseStats {
            functions: merged.functions().len(),
            types: merged.defined_types().len(),
            modules: merged.modules().len(),
            relations: merged.relations().len(),
        };
        
        // Stage 3: Transform
        info!("Stage 3/3: Transforming to database...");
        
        // Create schema
        ploke_transform::schema::create_schema_all(&db)
            .map_err(|e| XtaskError::new(format!("Schema creation failed: {e}")))?;
        
        // Transform
        transform_parsed_graph(&db, merged, &tree)
            .map_err(|e| XtaskError::new(format!("Transform failed: {e}")))?;
        
        let duration = start.elapsed();
        
        // Output summary
        println!("✓ Pipeline completed in {}ms", duration.as_millis());
        println!("  Parsed: {} functions, {} types, {} modules",
            parse_stats.functions,
            parse_stats.types,
            parse_stats.modules
        );
        
        Ok(PipelineOutput {
            parse_stats,
            duration,
        })
    }
}
```

---

## 9. Error Handling Design

```rust
/// Unified error type for xtask commands
#[derive(Debug, thiserror::Error)]
pub enum XtaskError {
    #[error("{0}")]
    Generic(String),
    
    #[error("Parse error: {0}")]
    Parse(#[from] SynParserError),
    
    #[error("Transform error: {0}")]
    Transform(#[from] TransformError),
    
    #[error("Database error: {0}")]
    Database(#[from] ploke_error::Error),
    
    #[error("Embedding error: {0}")]
    Embedding(#[from] EmbedError),
    
    #[error("Resource error: {0}")]
    Resource(String),
    
    #[error("Validation error: {context}")]
    Validation {
        context: String,
        recovery: Option<String>,
    },
    
    #[error("Command '{command}' failed: {reason}")]
    CommandFailed {
        command: String,
        reason: String,
        underlying: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}

impl XtaskError {
    /// Create a new generic error
    pub fn new(message: impl Into<String>) -> Self {
        Self::Generic(message.into())
    }
    
    /// Add context to an error
    pub fn with_context(self, context: impl Into<String>) -> Self {
        Self::CommandFailed {
            command: context.into(),
            reason: self.to_string(),
            underlying: Some(Box::new(self)),
        }
    }
    
    /// Get recovery suggestion if available
    pub fn recovery_suggestion(&self) -> Option<&str> {
        match self {
            Self::Validation { recovery, .. } => recovery.as_deref(),
            Self::CommandFailed { .. } => Some("Check the command arguments and try again."),
            _ => None,
        }
    }
    
    /// Print a user-friendly error report
    pub fn print_report(&self) {
        eprintln!("\n❌ Error: {}", self);
        
        if let Some(recovery) = self.recovery_suggestion() {
            eprintln!("\n💡 Recovery suggestion: {}", recovery);
        }
        
        // Check if there's a tracing log
        if let Ok(log_dir) = std::env::var("PLOKE_TRACE_DIR") {
            eprintln!("\n📝 Trace log available in: {}", log_dir);
            eprintln!("   Search for relevant spans using: rg '{}' {}",
                self.to_string().split_whitespace().next().unwrap_or(""),
                log_dir
            );
        }
    }
}
```

---

## 10. Pros and Cons of This Approach

### 10.1 Advantages

| Aspect | Benefit |
|--------|---------|
| **Type Safety** | The `Command` trait ensures all commands implement required methods; compile-time checking prevents missing implementations |
| **Unified Interface** | Both sync and async commands use the same `execute` method; executor handles the difference transparently |
| **Lazy Resource Management** | `OnceLock` in `CommandContext` ensures expensive resources (database, embedder) are only created when needed |
| **Testability** | `TestableCommand` trait and `CommandTestHarness` enable consistent testing patterns across all commands |
| **Observability** | Built-in tracing, usage tracking, and statistics collection work uniformly for all commands |
| **Extensibility** | New commands only need to implement the `Command` trait; auto-discovery reduces boilerplate |
| **Documentation** | Derive macros and doc generation ensure help text stays synchronized with code |

### 10.2 Challenges and Mitigations

| Challenge | Mitigation |
|-----------|------------|
| **Complex Type Signatures** | The `Command` trait uses associated types which can be verbose; provide derive macros to reduce boilerplate |
| **Async Runtime Overhead** | Commands that don't need async still pay some overhead; use feature flags to disable async support when not needed |
| **Resource Lifecycle Complexity** | `Arc<RwLock<...>>` patterns add complexity; provide clear `Drop` implementations and cleanup handlers |
| **Error Type Conversion** | Many error types need conversion to `XtaskError`; implement `From` impls and provide `?` operator support |
| **Learning Curve** | The abstraction layers require understanding; provide extensive examples and templates |

### 10.3 Comparison with Alternatives

| Approach | Pros | Cons | Best For |
|----------|------|------|----------|
| **This Proposal (Plugin/Executor)** | Type-safe, extensible, testable, observability built-in | More complex to implement initially | Large command suites with shared resources |
| **Simple Match Statement** | Simple, no abstractions needed | Hard to test, no resource sharing, becomes unwieldy | Small, simple command sets |
| **Clap Subcommands** | Good CLI parsing, derives available | Doesn't solve resource management or async/sync split | CLI-focused tools |
| **Actor Model** | Natural for async, good concurrency | Overkill for mostly-sync commands, more complex | Heavily async workloads |

---

## 11. Migration Path from Current xtask

```
Current xtask:
  main.rs - match statement dispatching to functions

Migration steps:
  1. Create executor/ module with Command trait
  2. Port existing commands one at a time:
     - verify-fixtures → commands/utility/verify_fixtures.rs
     - profile-ingest → commands/ingest/profile.rs
     - etc.
  3. Keep old functions as wrappers during transition
  4. Once all commands migrated, remove match statement
  5. Use CommandRegistry for dispatch

Phase 1 (M.3): Skeleton + 3-5 core commands
Phase 2 (M.4): Full command suite
Phase 3 (M.5): Advanced features (pipelines, cross-crate)
```

---

## 12. Summary

This architecture proposal provides:

1. **A unified command trait** that handles both sync and async commands through a common interface
2. **Lazy resource management** via `CommandContext` with `OnceLock` initialization
3. **Comprehensive testability** through the `TestableCommand` trait and `CommandTestHarness`
4. **Built-in observability** with usage tracking, tracing, and automatic suggestions
5. **Automatic documentation** generation that stays synchronized with code
6. **Clear error handling** with recovery suggestions and context

The design prioritizes **type safety**, **testability**, and **maintainability** while providing escape hatches for complex use cases.
