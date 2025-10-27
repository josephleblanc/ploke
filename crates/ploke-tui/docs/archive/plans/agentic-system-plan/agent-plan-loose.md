 - (ongoing): Update impl-logs with reasoning + steps taken ongoing.
     - request human input when blockers encountered and/or instructions too unclear to
     implement, create report explaining why blocker cannot be solved independently and requires
     human input, bring questions, attempt to resolve and continue, if not possible stop and
     request human input
     - request human input when tests needed behind cfg gating
     - otherwise continue working
 - save all the models called to a single file
 - use the `crates/ploke-tui/src/llm/openrouter/json_visitor.rs` functions to analyze the shape
 of the response across models
 - If shape is the same/similar to ModelEndpoint, then either use ModelEndpoint to test
 deserailization (if same) or create a similar struct (if different) and test same.
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
 - Evaluate current handling of endpoint cycle in `ploke-tui` crate so far, and identify how to
 streamline to make more ergonomic, simplify call sites where possible while improving
 performance.
 - Migrate system to use new approach
     - slash and burn for old approach where tests are repeated.
     - replace e2e tests with approach using gated TEST_APP in `test_harness.rs` behind
     `#[cfg(feature = "test_harness")]` for realistic end-to-end testing with multithreaded
     event system.
     - include snapshot testing, ensure UI/UX does not regress
 - TBD

 Human:
 - Integrate and/or build out trait-based Tool calling system, starting with
 `request_more_context` tool that uses vector similarity + bm25 search
     - test new trait system in unit tests
     - test e2e with TEST_APP and live API calls
     - if trait system valid, extend to other tools + refine approach
 - expand db methods for targeted code context search
     - get neighboring code items in module tree
     - get all code items in current file
 - expand tool calls
     - add tests + benches
 - invest more design time into agentic system (not yet created)
     - overall simple loops
     - prompt decomposition
     - planning
     - revisit tool design, re-evaluate current system
 - ensure current API system works as expected, and that we can make the expected calls
     - agent TODO list above finished
     - UI smoothed out for selecting model (currently buggy re: selecting model provider)
     - accurate + comprehensive model registry exists
     - API tested + validated, shapes of responses recorded, strong typing on all
     request/response schema for ergonimic use and mutation (filter, destructure, etc)
     - performant (efficient, low alloc, no dynamic dispatch, static dispatch)
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
