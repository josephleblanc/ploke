//! Command executor and registry for xtask commands.
//!
//! This module provides the core infrastructure for executing commands in the xtask
//! system, including:
//! - The `Command` trait for defining type-safe commands
//! - The `CommandExecutor` for managing command lifecycle and resource coordination
//! - The `CommandRegistry` for command registration and dispatch
//! - The `MaybeAsync` type for handling both sync and async commands uniformly

use std::any::Any;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde::Serialize;
use tracing::{info_span, instrument};

use crate::context::CommandContext;
use crate::error::XtaskError;
use crate::usage::UsageTracker;

/// The fundamental trait that all xtask commands implement.
///
/// This trait uses associated types `Output` and `Error` to allow both sync and async
/// commands to be handled uniformly through the executor.
///
/// # Example
/// ```ignore
/// impl Command for MyCommand {
///     type Output = MyOutput;
///     type Error = MyError;
///
///     fn name(&self) -> &'static str { "my command" }
///     fn category(&self) -> CommandCategory { CommandCategory::Utility }
///     fn requires_async(&self) -> bool { false }
///
///     fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
///         // Command implementation
///     }
/// }
/// ```
pub trait Command: Send + Sync + 'static {
    /// The type of output this command produces.
    /// Must be serializable for JSON output support.
    type Output: Serialize + Send + 'static;

    /// The type of error this command can return.
    /// Must be convertible to `XtaskError`.
    type Error: Into<XtaskError>;

    /// Unique identifier for this command (e.g., "db count-nodes").
    fn name(&self) -> &'static str;

    /// Category for grouping in help output.
    fn category(&self) -> CommandCategory;

    /// Execute the command with access to shared resources.
    ///
    /// The executor handles whether this runs synchronously or asynchronously
    /// based on `requires_async()`.
    fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error>;

    /// Returns true if this command requires an async runtime.
    fn requires_async(&self) -> bool;

    /// Estimated resource requirements for scheduling and validation.
    ///
    /// Default implementation indicates no special requirements.
    fn resource_requirements(&self) -> ResourceRequirements {
        ResourceRequirements::default()
    }
}

/// Categories for organizing commands in help output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandCategory {
    /// Parsing-related commands (syn_parser)
    Parse,
    /// Graph transformation commands
    Transform,
    /// Database operations
    Database,
    /// Embedding and indexing commands
    Ingest,
    /// Cross-crate pipeline commands
    Pipeline,
    /// Validation and integrity checks
    Validate,
    /// Environment setup commands
    Setup,
    /// External tool execution
    Tool,
    /// General utility commands
    Utility,
}

impl CommandCategory {
    /// Get a string representation of the category.
    pub fn as_str(&self) -> &'static str {
        match self {
            CommandCategory::Parse => "Parse",
            CommandCategory::Transform => "Transform",
            CommandCategory::Database => "Database",
            CommandCategory::Ingest => "Ingest",
            CommandCategory::Pipeline => "Pipeline",
            CommandCategory::Validate => "Validate",
            CommandCategory::Setup => "Setup",
            CommandCategory::Tool => "Tool",
            CommandCategory::Utility => "Utility",
        }
    }

    /// Get all categories in display order.
    pub fn all() -> &'static [CommandCategory] {
        &[
            CommandCategory::Parse,
            CommandCategory::Transform,
            CommandCategory::Database,
            CommandCategory::Ingest,
            CommandCategory::Pipeline,
            CommandCategory::Validate,
            CommandCategory::Setup,
            CommandCategory::Tool,
            CommandCategory::Utility,
        ]
    }
}

/// Resource requirements for command execution.
///
/// Used to validate that required resources are available before execution.
#[derive(Debug, Clone, Default)]
pub struct ResourceRequirements {
    /// Whether this command needs database access.
    pub needs_database: bool,
    /// Whether this command needs the embedding runtime.
    pub needs_embedding_runtime: bool,
    /// Whether this command needs IO manager access.
    pub needs_io_manager: bool,
    /// Whether this command needs the event bus.
    pub needs_event_bus: bool,
}

/// A wrapper that can hold either a sync result or an async future.
///
/// This type allows uniform handling of both sync and async operations,
/// with conversion to a blocking result when needed.
pub enum MaybeAsync<T> {
    /// A synchronous result that is immediately available.
    Ready(T),
    /// An asynchronous future that needs to be awaited.
    Future(Pin<Box<dyn Future<Output = T> + Send>>),
}

impl<T> MaybeAsync<T> {
    /// Block on the result if async, or return immediately if sync.
    ///
    /// # Panics
    /// Panics if called outside of an async runtime when the variant is `Future`.
    pub fn block(self) -> T {
        match self {
            MaybeAsync::Ready(val) => val,
            MaybeAsync::Future(fut) => {
                // Use tokio's block_in_place or a blocking recv
                // This is a simplified implementation
                todo!("Async execution requires tokio runtime integration")
            }
        }
    }

    /// Convert to a future that can be awaited.
    ///
    /// For `Ready` variants, creates a future that immediately resolves.
    pub async fn into_future(self) -> T {
        match self {
            MaybeAsync::Ready(val) => val,
            MaybeAsync::Future(fut) => fut.await,
        }
    }
}

impl<T> From<T> for MaybeAsync<T> {
    fn from(val: T) -> Self {
        MaybeAsync::Ready(val)
    }
}

/// Dynamic command trait for type erasure in the registry.
///
/// This trait allows storing heterogeneous command types in the registry
/// by erasing their specific output types.
pub trait DynCommand: Send {
    /// Get the command name.
    fn name(&self) -> &'static str;

    /// Get the command category.
    fn category(&self) -> CommandCategory;

    /// Execute the command and return the output as `Box<dyn Any>`.
    fn execute(&self, ctx: &CommandContext) -> Result<Box<dyn Any>, XtaskError>;

    /// Check if this command requires async execution.
    fn requires_async(&self) -> bool;

    /// Get resource requirements for this command.
    fn resource_requirements(&self) -> ResourceRequirements;
}

/// Type-erasing wrapper for concrete Command implementations.
struct DynCommandWrapper<C: Command> {
    command: C,
}

impl<C: Command> DynCommand for DynCommandWrapper<C> {
    fn name(&self) -> &'static str {
        self.command.name()
    }

    fn category(&self) -> CommandCategory {
        self.command.category()
    }

    fn execute(&self, ctx: &CommandContext) -> Result<Box<dyn Any>, XtaskError> {
        self.command
            .execute(ctx)
            .map(|output| Box::new(output) as Box<dyn Any>)
            .map_err(|e| e.into())
    }

    fn requires_async(&self) -> bool {
        self.command.requires_async()
    }

    fn resource_requirements(&self) -> ResourceRequirements {
        self.command.resource_requirements()
    }
}

/// Factory function type for creating commands from arguments.
pub type CommandFactory = Box<dyn Fn(&[&str]) -> Result<Box<dyn DynCommand>, XtaskError> + Send>;

/// Registry that holds all available commands.
///
/// The registry provides command lookup by name and supports
/// auto-discovery of commands. For now, auto-discovery is simplified
/// to manual registration.
#[derive(Default)]
pub struct CommandRegistry {
    /// Map from command name to factory function.
    commands: HashMap<&'static str, CommandFactory>,

    /// Categories and their associated command names.
    categories: HashMap<CommandCategory, Vec<&'static str>>,
}

impl CommandRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a command type with the registry.
    ///
    /// This is a simplified registration that stores the command metadata.
    /// Full argument parsing integration will be added later.
    ///
    /// # Type Parameters
    /// * `C` - The command type to register, must implement both `Command` and `StaticCommandMeta`
    pub fn register<C: Command + StaticCommandMeta>(&mut self) {
        let factory: CommandFactory = Box::new(|_args: &[&str]| {
            // For now, commands are constructed externally
            // Full argument parsing will be implemented in later phases
            todo!("Command construction from arguments not yet implemented")
        });

        let name = C::static_name();
        let category = C::static_category();

        self.commands.insert(name, factory);
        self.categories
            .entry(category)
            .or_default()
            .push(name);
    }

    /// Look up a command by name.
    pub fn get(&self, name: &str) -> Option<&CommandFactory> {
        self.commands.get(name)
    }

    /// Get all commands in a category.
    pub fn get_category(&self, category: CommandCategory) -> Option<&Vec<&'static str>> {
        self.categories.get(&category)
    }

    /// Check if a command is registered.
    pub fn contains(&self, name: &str) -> bool {
        self.commands.contains_key(name)
    }

    /// Get all registered command names.
    pub fn command_names(&self) -> impl Iterator<Item = &&'static str> {
        self.commands.keys()
    }

    /// Generate help text for all commands, organized by category.
    pub fn generate_help(&self) -> String {
        let mut output = String::from("xtask - Ploke workspace automation commands\n\n");

        for category in CommandCategory::all() {
            if let Some(commands) = self.categories.get(category) {
                output.push_str(&format!("{}:\n", category.as_str()));
                for cmd_name in commands {
                    output.push_str(&format!("  {}\n", cmd_name));
                }
                output.push('\n');
            }
        }

        output
    }

    /// Simplified auto-discovery for built-in commands.
    ///
    /// Currently this is a placeholder that will be expanded in later phases
    /// to support full auto-discovery.
    pub fn auto_discover(&mut self) {
        // Commands will be registered here as they are implemented
        // For now, this is a placeholder for future auto-discovery
        tracing::debug!("Auto-discovery called - full implementation coming in Phase 2");
    }
}

/// Extension trait for Command to provide static metadata.
///
/// This trait is used by the registry to get metadata without
/// constructing a command instance.
pub trait StaticCommandMeta {
    /// Get the static command name.
    fn static_name() -> &'static str;

    /// Get the static command category.
    fn static_category() -> CommandCategory;
}

/// Blanket implementation for types implementing Command.
/// Note: This requires the command to be constructible for metadata access.
/// For full static metadata, commands should implement StaticCommandMeta.
impl<T: Command + Default> StaticCommandMeta for T {
    fn static_name() -> &'static str {
        T::default().name()
    }

    fn static_category() -> CommandCategory {
        T::default().category()
    }
}

/// Configuration for the command executor.
#[derive(Debug, Clone)]
pub struct ExecutorConfig {
    /// Whether to enable async runtime support.
    pub enable_async: bool,

    /// Path for usage tracking log file.
    pub usage_log_path: Option<std::path::PathBuf>,

    /// Directory for trace output.
    pub trace_output_dir: Option<std::path::PathBuf>,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            enable_async: true,
            usage_log_path: None,
            trace_output_dir: None,
        }
    }
}

/// Central executor that manages command lifecycle and resource coordination.
///
/// The executor provides:
/// - Shared context for all commands
/// - Async runtime management
/// - Usage tracking integration
/// - Tracing and instrumentation
pub struct CommandExecutor {
    /// Shared context for all commands.
    context: Arc<CommandContext>,

    /// Async runtime (shared across async commands).
    runtime: Option<Arc<tokio::runtime::Runtime>>,

    /// Usage statistics tracker.
    usage_tracker: Arc<UsageTracker>,
}

impl CommandExecutor {
    /// Create a new executor with the given configuration.
    ///
    /// # Errors
    /// Returns an error if:
    /// - The async runtime fails to create (when `enable_async` is true)
    /// - The usage tracker fails to initialize
    pub fn new(config: ExecutorConfig) -> Result<Self, XtaskError> {
        let runtime = if config.enable_async {
            Some(Arc::new(
                tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| XtaskError::new(format!("Failed to create runtime: {e}")))?,
            ))
        } else {
            None
        };

        let context = Arc::new(CommandContext::new()?);
        let usage_tracker = Arc::new(UsageTracker::new(config.usage_log_path)?);

        Ok(Self {
            context,
            runtime,
            usage_tracker,
        })
    }

    /// Execute a command with full lifecycle management.
    ///
    /// This method handles:
    /// - Usage tracking (start and completion)
    /// - Resource validation
    /// - Sync/async execution dispatch
    /// - Suggestion triggers (every 50 runs)
    #[instrument(skip(self, cmd), fields(command = %cmd.name()))]
    pub fn execute<C: Command>(&self, cmd: C) -> Result<C::Output, XtaskError> {
        // Pre-execution: record usage start
        let usage_start = self.usage_tracker.record_start(cmd.name());

        // Setup tracing span
        let _span = info_span!("execute_command", command = %cmd.name());
        let _enter = _span.enter();

        tracing::info!("Executing command: {}", cmd.name());

        // Validate prerequisites
        self.validate_prerequisites(&cmd)?;

        // Execute based on mode
        let result = if cmd.requires_async() {
            self.execute_async(cmd)
        } else {
            self.execute_sync(cmd)
        };

        // Post-execution: record completion
        let success = result.is_ok();
        self.usage_tracker.record_completion(usage_start, success);

        // Check for suggestion trigger (every 50 runs)
        if self.usage_tracker.should_show_suggestion() {
            self.usage_tracker.show_suggestion();
        }

        result
    }

    /// Execute a synchronous command.
    fn execute_sync<C: Command>(&self, cmd: C) -> Result<C::Output, XtaskError> {
        cmd.execute(&self.context).map_err(|e| e.into())
    }

    /// Execute an asynchronous command.
    fn execute_async<C: Command>(&self, cmd: C) -> Result<C::Output, XtaskError> {
        let runtime = self
            .runtime
            .as_ref()
            .ok_or_else(|| XtaskError::new("Async runtime not available"))?;

        let context = Arc::clone(&self.context);
        runtime.block_on(async move { cmd.execute(&context).map_err(|e| e.into()) })
    }

    /// Validate that required resources are available for the command.
    fn validate_prerequisites<C: Command>(&self, cmd: &C) -> Result<(), XtaskError> {
        let reqs = cmd.resource_requirements();

        if reqs.needs_database {
            self.context.database_pool()?;
        }

        if reqs.needs_embedding_runtime {
            self.context.embedding_runtime()?;
        }

        Ok(())
    }

    /// Get a reference to the usage tracker.
    pub fn usage_tracker(&self) -> &UsageTracker {
        &self.usage_tracker
    }

    /// Get a reference to the command context.
    pub fn context(&self) -> &CommandContext {
        &self.context
    }
}

/// Macro for defining simple synchronous commands.
///
/// # Example
/// ```ignore
/// define_command!(sync MyCommand => |cmd, ctx| {
///     // Sync implementation
///     Ok(MyOutput {})
/// });
/// ```
#[macro_export]
macro_rules! define_sync_command {
    ($name:ident => $impl:expr) => {
        impl $crate::executor::Command for $name {
            fn execute(
                &self,
                ctx: &$crate::context::CommandContext,
            ) -> Result<Self::Output, Self::Error> {
                $impl(self, ctx)
            }
            fn requires_async(&self) -> bool {
                false
            }
        }
    };
}

/// Macro for defining asynchronous commands.
///
/// # Example
/// ```ignore
/// define_command!(async MyCommand => |cmd, ctx| {
///     // Async implementation
///     Ok(MyOutput {})
/// });
/// ```
#[macro_export]
macro_rules! define_async_command {
    ($name:ident => $impl:expr) => {
        impl $crate::executor::Command for $name {
            fn execute(
                &self,
                ctx: &$crate::context::CommandContext,
            ) -> Result<Self::Output, Self::Error> {
                $impl(self, ctx)
            }
            fn requires_async(&self) -> bool {
                true
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_category_as_str() {
        assert_eq!(CommandCategory::Parse.as_str(), "Parse");
        assert_eq!(CommandCategory::Database.as_str(), "Database");
        assert_eq!(CommandCategory::Utility.as_str(), "Utility");
    }

    #[test]
    fn test_command_category_all() {
        let all = CommandCategory::all();
        assert_eq!(all.len(), 9);
        assert!(all.contains(&CommandCategory::Parse));
        assert!(all.contains(&CommandCategory::Database));
    }

    #[test]
    fn test_maybe_async_ready() {
        let ready: MaybeAsync<i32> = MaybeAsync::Ready(42);
        assert!(matches!(ready, MaybeAsync::Ready(42)));
    }

    #[test]
    fn test_registry_new() {
        let registry = CommandRegistry::new();
        assert!(!registry.contains("test"));
    }

    #[test]
    fn test_registry_generate_help() {
        let registry = CommandRegistry::new();
        let help = registry.generate_help();
        assert!(help.contains("xtask"));
    }

    #[test]
    fn test_resource_requirements_default() {
        let reqs = ResourceRequirements::default();
        assert!(!reqs.needs_database);
        assert!(!reqs.needs_embedding_runtime);
        assert!(!reqs.needs_io_manager);
        assert!(!reqs.needs_event_bus);
    }

    #[test]
    fn test_executor_config_default() {
        let config = ExecutorConfig::default();
        assert!(config.enable_async);
        assert!(config.usage_log_path.is_none());
        assert!(config.trace_output_dir.is_none());
    }
}
