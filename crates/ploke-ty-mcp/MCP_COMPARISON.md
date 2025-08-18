# Comparative Analysis: MCP-based Integration vs. Project-specific Implementations

Context
We need reliable, scriptable access to:
- Git operations (critical internal workflows).
- Context7 documentation discovery and retrieval (external capability).
- Future arbitrary tools defined by users.

Options Compared
1) MCP-based clients (current direction)
- Description: Use existing MCP servers, communicate via rmcp child-process transport.
- Pros:
  - Pluggability: uniform protocol for many tools; easy to add user-defined servers.
  - Isolation: servers run as separate processes; failure isolation and security.
  - Velocity: reuse mature servers (git, context7) with minimal code.
  - Consistent Model: tools, prompts, resources share a common RPC shape.
- Cons:
  - Performance: process boundaries and JSON serialization overhead.
  - Operational deps: requires node/uvx or server binaries installed.
  - Stability variance: tool output formats may evolve; server availability matters.
  - Observability across process boundary requires extra work.

2) Native Rust integrations (non-MCP)
- Description: Link directly to Rust crates (e.g., gix or git2 for Git; custom HTTP clients for services).
- Pros:
  - Performance: in-process; lower latency and overhead.
  - Reliability: fewer moving parts; no external processes.
  - Type safety: strong Rust types for inputs/outputs.
  - Offline: easier to vend a fully offline experience for certain operations.
- Cons:
  - Scope: must implement each integration ourselves.
  - Maintainability: ongoing API changes in upstream services.
  - Flexibility: harder to let users add arbitrary external tools without a plugin story.

3) Custom plugin system (non-MCP, ploke-specific)
- Description: Define our own plugin protocol (trait objects, dynamic libs, WASM, or custom RPC).
- Pros:
  - Tailored to our needs; precise types and semantics.
  - Performance and observability fully under our control.
  - Could support sandboxed execution via WASM.
- Cons:
  - High complexity: design, security, tooling, and ecosystem buy-in.
  - Fragmentation: fewer ready-to-use integrations initially.
  - Time-to-value: significant engineering investment before payoffs.

Decision Matrix (summarized)
- Speed-to-deliver:
  - MCP: High
  - Native: Medium (Git easy; Context7 equivalent would be custom)
  - Custom plugin: Low
- Extensibility for user-defined tools:
  - MCP: High
  - Native: Low (unless we also build a plugin system)
  - Custom plugin: High (after heavy upfront cost)
- Performance and reliability for critical paths:
  - MCP: Medium
  - Native: High
  - Custom plugin: High (if well designed)
- Maintenance surface:
  - MCP: Medium (server churn)
  - Native: Medium–High (we own the integrations)
  - Custom plugin: High initially

Recommendation
- Adopt a hybrid approach with an abstraction boundary:
  - Define a ploke-ty-mcp API that hides rmcp specifics behind McpManager and typed clients.
  - Use MCP for external/general-purpose or user-defined tools (Context7 and future arbitrary servers).
  - For critical internal operations (Git), start with MCP for velocity but keep a trait-based facade so we can swap to a native Rust backend later if needed.
    - Example: define a trait GitOps { status, diff, add, commit, … }, and implement it with Mcp GitClient now.
    - Later, add a NativeGitClient (using gix or git2) behind a feature flag and selectable at runtime.
- Rationale:
  - We get immediate functionality and user extensibility.
  - We avoid lock-in by designing an interface that supports multiple backends.
  - We maintain the option to optimize critical paths without redesigning the agentic flows.

Migration/Fallback Strategy
- Start with MCP-backed GitClient and Context7Client.
- Introduce interfaces:
  - pub trait GitOps { … } implemented by McpGitClient now; later add NativeGitClient.
  - pub trait DocsLookup { resolve_id, get_docs } implemented by Context7Client; later add other providers if desired.
- Selection:
  - Config flag selects backend per capability (e.g., git.backend = "mcp" | "native").
  - Safe to run both; McpManager can still support user-defined servers.
- Testing:
  - Keep e2e tests for MCP, plus unit/integration tests for native backend.
  - Use the same high-level tests in ploke-tui by mocking the trait.

Risks
- Divergent behaviors across backends.
  - Mitigation: normalize outputs and define clear result contracts; add conformance tests.
- External dependency churn (Node/uvx/servers).
  - Mitigation: pre-flight checks and clear diagnostics; optional vendored/native paths for critical ops.
- Increased surface area (two backends).
  - Mitigation: adopt stable traits and shared test suites.

Conclusion
- Short-term: MCP yields quick wins and extensibility with minimal code.
- Long-term: Keep the door open for native implementations where performance/robustness matters most.
- The proposed plan (see PLAN.md) implements this hybrid approach from the start.
