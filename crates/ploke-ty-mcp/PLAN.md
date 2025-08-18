# Plan: Complete MCP API for Context7 and Git (ploke-ty-mcp)

Goals
- Provide a robust, async client-side API to manage MCP servers and call tools, built on rmcp.
- First-class, strongly-typed clients for:
  - Git MCP (critical internal operation).
  - Context7 MCP (docs discovery and retrieval).
- Support arbitrary, user-configured MCP servers via a config file.
- Clean cancellation, error handling, observability, and test strategy.
- Integrate smoothly with ploke-tui agentic flows without leaking rmcp details.

Non-goals (v0)
- Implement an in-process plugin framework (out of scope; see comparison doc).
- Full server-side MCP; we only implement the client side here.
- Advanced streaming/LLM event handling beyond basic tool invocation.

High-level Architecture
- Manager: McpManager orchestrates lifecycle (start/stop/cancel) for multiple MCP servers.
- Registry: maps server_id -> RunningService, with metadata, health/status.
- Typed Clients:
  - GitClient: thin wrapper over McpManager with typed methods (status, diff, add, commit, branch…).
  - Context7Client: typed methods (resolve_library_id, get_library_docs).
- Generic API: uniform call_tool(server_id, tool_name, arguments) for user-defined servers.
- Config-driven: Load servers from a config file; first-class servers can be autostarted and prioritized.
- Cancellation: Per-server cancel(), plus global cancel_all(); future: scoped cancellations per call.
- Error Model: McpError enum (Transport, Protocol, Tool, Spawn, Timeout, Canceled, NotFound).
- Observability: tracing spans for server lifecycle and tool calls; structured errors and durations.

Proposed Public API (Rust) – outline
- Module layout
  - src/
    - manager.rs        // McpManager, Lifecycle, Registry
    - types.rs          // McpError, ServerId, ServerSpec, ToolCall, ToolResult
    - config.rs         // Config structs and loader
    - clients/
      - git.rs          // GitClient (first-class)
      - context7.rs     // Context7Client (first-class)
    - util.rs           // helpers: cancellation, args/env merge, backoff, tracing
  - Re-exports from lib.rs

- Key types (conceptual; exact signatures to be implemented)
  - pub struct McpManager {
      // holds registry: HashMap<ServerId, RunningService<rmcp::RoleClient, ()>>
      // and server specs
    }
    - pub async fn from_config(cfg: McpConfig) -> Result<Self, McpError>
    - pub async fn ensure_started(&self, id: &ServerId) -> Result<(), McpError>
    - pub async fn cancel(&self, id: &ServerId) -> Result<(), McpError>
    - pub async fn cancel_all(&self) -> Result<(), McpError>
    - pub async fn list_tools(&self, id: &ServerId) -> Result<Vec<ToolDescriptor>, McpError>
    - pub async fn call_tool(&self, id: &ServerId, name: &str, args: serde_json::Value) -> Result<ToolResult, McpError>
    - pub fn client_git(&self) -> Option<clients::git::GitClient>
    - pub fn client_context7(&self) -> Option<clients::context7::Context7Client>

  - pub enum PrioritizedServer { Git, Context7 }
    - helper mapping to well-known server IDs: "git", "context7"

  - pub struct ServerSpec {
      pub id: ServerId,
      pub command: String,
      pub args: Vec<String>,
      pub env: std::collections::BTreeMap<String, String>,
      pub autostart: bool,
      pub restart_on_exit: bool,
      pub priority: u8, // lower = more important (git/context7 default to 0/1)
    }

  - pub struct McpConfig {
      pub servers: Vec<ServerSpec>,
    }

  - pub struct ToolDescriptor { pub name: String, pub description: Option<String> /* ... */ }
  - pub struct ToolResult { pub text: String /* aggregated text content from content blocks */ }

  - pub enum McpError {
      Transport(String),
      Protocol(String),
      Tool(String),
      Spawn(String),
      Timeout,
      Canceled,
      NotFound(String),
      Config(String),
      Io(std::io::Error),
      Anyhow(anyhow::Error),
    }

First-class typed clients
- Context7Client
  - pub async fn resolve_library_id(&self, name: &str) -> Result<String, McpError>
  - pub async fn get_library_docs(&self, id: &str, tokens: usize, topic: &str) -> Result<String, McpError>
  - Internally calls manager.call_tool("context7", "...", json!({...})) and normalizes results.

- GitClient
  - pub async fn status(&self, repo_path: &str) -> Result<String, McpError>
  - pub async fn diff(&self, repo_path: &str, args: Option<Vec<String>>) -> Result<String, McpError>
  - pub async fn add(&self, repo_path: &str, paths: Vec<String>) -> Result<String, McpError>
  - pub async fn commit(&self, repo_path: &str, message: &str, all: bool) -> Result<String, McpError>
  - pub async fn checkout(&self, repo_path: &str, branch: &str, create: bool) -> Result<String, McpError>
  - pub async fn branch(&self, repo_path: &str) -> Result<String, McpError>
  - etc. (map to mcp-server-git tools)
  - v0 returns normalized text; future: structured outputs.

Config Design
- File format: TOML (human-friendly).
- Default path: $XDG_CONFIG_HOME/ploke/mcp.toml or ~/.config/ploke/mcp.toml
- Example:
  ```toml
  [servers.git]
  id = "git"
  command = "uvx"
  args = ["mcp-server-git"]
  autostart = true
  restart_on_exit = false
  priority = 0

  [servers.context7]
  id = "context7"
  command = "npx"
  args = ["-y", "@upstash/context7-mcp"]
  autostart = true
  restart_on_exit = false
  priority = 1

  [servers.foo]
  id = "foo"
  command = "/opt/custom/foo-mcp"
  args = []
  autostart = false
  restart_on_exit = true
  priority = 10
  ```

- Environment
  - Allow per-server env overrides in config.
  - Security: optional allowlist for commands; warn on non-standard binaries.

Cancellation and Timeouts
- Each RunningService supports cancel(). Expose:
  - McpManager::cancel(id), cancel_all().
- Tool calls accept an optional timeout in a future API; v0 uses tokio::time::timeout with a default.

Observability
- Use tracing: spans for server_start, list_tools, call_tool, cancel.
- Include server_id, tool_name; record durations and errors.

Integration with ploke-tui
- Provide a small facade that ploke-tui can hold in AppState:
  - let manager = McpManager::from_config(load_default_config()?);
  - manager.ensure_started("git").await?;
  - let git = manager.client_git().unwrap();
  - let status = git.status(".").await?;
- For agentic flows, offer generic entry point: manager.call_tool(server_id, tool, args).

Testing Strategy
- Unit tests
  - Manager registry behaviors, error mapping, config parsing.
  - ToolResult text aggregation from rmcp::content.
- Integration tests (behind env flags)
  - Spawn context7 via npx and git via uvx; verify basic tool calls.
  - Gated by env var PLOKE_E2E_MCP=1; otherwise skipped.
- Golden tests for normalizing outputs where stable.
- CI: mark e2e as optional due to external deps.

Milestones
- M0: Scaffolding (types.rs, manager.rs, config.rs) and minimal manager that starts/cancels.
- M1: Typed Context7Client and GitClient using existing helper functions; generic call_tool.
- M2: Config file loader and autostart prioritized servers; basic tracing; timeouts.
- M3: Error normalization, improved outputs; docs; skip/enable e2e tests.
- M4: Extended git API coverage; streaming support (if needed).
- M5: Stability: backoff/restart policies, health checks.

Risks and Mitigations
- External tool presence (npx/uvx): detect and error clearly; document install steps.
- Server churn/crashes: restart policy; backoff; telemetry to user.
- Output instability: normalize/parse where feasible; rely on substring assertions initially.

Deliverables (v0–v1)
- McpManager, typed clients for git/context7, config loader, basic docs.
- E2E examples and tests gated behind feature/env.
