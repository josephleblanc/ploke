# Agents Operating Guide

Purpose
- Define the interaction workflow between human and agents, and codify non-negotiable engineering principles for this codebase.

Workflow: Plan → Review → Implement
- When asked to produce a plan, create a standalone plan document under `docs/plans/agentic-system-plan/impl-plan/plan_YYYYMMDD-HHMMSSZ.md` containing:
  - Objectives and scope
  - Proposed steps with rationale and acceptance criteria
  - Deliverables and validation approach
- The human reviewer can either approve or propose changes.
  - If changes are proposed, you MUST update the plan document to incorporate them.
  - You MAY create a secondary document under `docs/plans/agentic-system-plan/impl-plan/` with:
    - Open questions / blockers requiring clarification
    - A brief critique of the requested changes if they degrade quality or introduce risk
- After the plan is approved or updated per instructions, proceed to implement according to the plan.
- Always link implementation logs (`impl_*.md`) to the corresponding plan file.

Engineering Principles
- Strong typing everywhere (no stringly typed plumbing):
  - All OpenRouter-touching code and tool schemas must use strongly typed structs/enums with `Serialize`/`Deserialize` derives (numeric fields as numeric types, e.g., `u32` for tokens, `f64` for costs).
  - Prefer enums and tagged unions to “detect” shapes; make invalid states unrepresentable.
  - Use `#[serde(untagged)]` only as a migration bridge with explicit deprecation notes.
  - Treat ad-hoc JSON maps and loosely typed values as errors at the boundaries; validate early and surface actionable messages.
- Safety-first editing:
  - Stage edits with verified file hashes; apply atomically via the IoManager; never write on hash mismatch.
  - Keep destructive operations behind explicit approvals.
- Evidence-based changes:
  - Run targeted and full test suites; prefer brief inline summaries (pass/fail/ignored counts and notable failures). Avoid writing run outputs to docs by default.
  - When deeper artifacts are required (e.g., live API diagnostics), keep them under `logs/` or `target/test-output/` and do not commit them unless explicitly requested.
  - Update design/reflection docs when making notable trade-offs.
  - Live gates discipline: When a live gate is ON (e.g., OpenRouter tests), do not report tests as green unless the live path was actually exercised and key properties were verified (tool_calls observed, proposal staged, approval → Applied, file delta). A “skip” must be treated as “not validated” and must not be counted as pass under live gates.
  - Evidence for readiness: For any claim of phase readiness, include verifiable proof in your summary (pass/fail counts, properties satisfied) and reference artifact paths generated under `target/test-output/...`. If evidence is missing, explicitly state that readiness is not established.

Operational Notes
- Plans, logs, and reports live in `docs/plans/agentic-system-plan/` and `docs/reports/`.
- Reference key docs from plan files so future agents easily discover prior work.

Rust Doc Comments (non-negotiable formatting)
- Use `//!` only for a single, file-top inner doc block documenting the crate or module.
- Use `///` for item-level docs (functions, structs, fields, tests) and inline explanations.
- Do not place `//!` after imports or mix multiple `//!` blocks in a file; prefer one contiguous block at the very top.
- Integration test files are crate roots; if documenting the whole test crate, put a single `//!` block at the very top. Otherwise, prefer `///` on specific items.

## Ongoing Plan 

Ongoing Plan Re:
Next we need to: 
TODO (starting with TODOs in this file to help identify where to start following points):
 - (ongoing): Update impl-logs with reasoning + steps taken ongoing.
     - request human input when blockers encountered and/or instructions too unclear to
     implement, create report explaining why blocker cannot be solved independently and requires
     human input, bring questions, attempt to resolve and continue, if not possible stop and
     request human input
     - request human input when tests needed behind cfg gating
     - otherwise continue working

  - Integrate and/or build out trait-based Tool calling system, starting with
   `request_more_context` tool that uses vector similarity + bm25 search
     - test new trait system in unit tests
     - test e2e with TEST_APP and live API calls
     - if trait system valid, extend to other tools + refine approach
  - expand tool calls
     - add tests + benches
  - expand db methods for targeted code context search
     - get neighboring code items in module tree
     - get all code items in current file
     - add tests + benches
 - expand the `crates/ploke-tui/src/llm/openrouter/json_visitor.rs` functions to analyze the shape
 of the response across model tool call responses
    - develop testing matrix framework against live endpoints
      - vary prompts
      - vary number of tools
      - vary providers
      - add benches for latency + serializable structs for tracking data of latency + other metrics
 - Create a persistent Model registry (we have a semi-working version now, but it is not
 grounded in the truth of the API expectations)
 - Transform response + filter providers/sort for desired fields
     - Use offical docs on API saved in `crates/ploke-tui/docs/openrouter/request_structure.md`
 - Develop a set of tests to make sure endpoint responses come back as expected.
     - happy paths
     - requests we expect to fail
     - gate behind cfg feature "live_api_tests"
 - Add documentation to all items. Create module-level documentation on API structure, expected
 values, use-cases, examples, etc.
 - Evaluate and streamline:
     - add benchmarks, both online/offline
     - record benches
     - profile performance for later comparison
     - smooth any super jagged edges
 - Migrate system to use new approach
     - slash and burn for old approach where tests are repeated.
     - replace e2e tests with approach using gated TEST_APP in `test_harness.rs` behind
     `#[cfg(feature = "test_harness")]` for realistic end-to-end testing with multithreaded
     event system.
     - include snapshot testing, ensure UI/UX does not regress
 - ensure current API system works as expected, and that we can make the expected calls
     - UI smoothed out for selecting model (currently buggy re: selecting model provider)
     - accurate + comprehensive model registry exists
     - API tested + validated, shapes of responses recorded, strong typing on all
     request/response schema for ergonimic use and mutation (filter, destructure, etc)
     - performant (efficient, low alloc, no dynamic dispatch, static dispatch)
 - TBD

 TODO:
 - invest more design time into agentic system (not yet created)
     - overall simple loops
     - prompt decomposition
     - planning
     - revisit tool design, re-evaluate current system
 - fill out tools + API calls into working, complete system
     - e2e tests exist and validate all testable properties offline
     - e2e + live tests exist and validate all testable properties online on a wide variety of
     endpoints
     - tests for happy + fail paths, observe expected defined errors where expected
     - snapshots and UI + UX are good, hotkeys exist, simple interactions in live TUI are good
 - revisit context management, arrive at clear design for a functioning memory system
     - implement memory system using db as primary storage
     - add observability tools (already written but need tests + integration)
 - integrate memory system with workflow, ensure modular + actor design maintains integrity or
     improves on integrity + organization (somewhat rats-nest of CommandState + AppEvent +
     EventBus)
 - revisit safety system + decide on sandboxing environment
     - integrate + test + TBD
 - begin using agents
     - refine + test + bench
         - prompts
         - observability
         - task complexity
     - experiment with agent organization systems
     - parallel agentic execution (branching + batched conversations)
 - begin deploying ploke-defined agents to improve ploke itself
     - start of self-evolutionary loop
     - start with refactors + clean up code base
     - extend features, e.g. 80% complete type resolution -> full implementation
 - revisit design of user profile creation + maintenance
     - integrate tools + memory
     - unify design
     - experiment
