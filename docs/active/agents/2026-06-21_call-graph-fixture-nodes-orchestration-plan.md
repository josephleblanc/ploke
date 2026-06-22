# Ploke fixture_nodes call-graph orchestration plan

Date: 2026-06-21
Title: fixture_nodes typed call-graph plan
Short description: Orchestrator-ready plan for a narrow, test-driven Ploke parser call-graph slice: structural call-site extraction first, exact `self.private_method()` resolution second, proof-fact projection only after typed parser facts are real.
Related planning files:
- `docs/active/agents/call-graph/README.md`
- `docs/active/agents/2026-06-22_call-graph-id-domain-correction.md`
- `docs/active/agents/readme.md`
- `crates/ingest/syn_parser/tests/uuid_phase2_partial_graphs/assoc_nodes/methods.rs`
- `crates/ingest/syn_parser/tests/uuid_phase3_resolution/type_relations_v2.rs`
- `crates/ingest/syn_parser/src/parser/relations.rs`
- `crates/ingest/syn_parser/src/parser/graph/code_graph.rs`
- `crates/ploke-records/src/proof_facts.rs`
- `crates/ploke-records/src/proof_effects.rs`

> For Hermes: use the `subagent-driven-development` skill to execute this plan task-by-task. Use a fresh implementer subagent per task, then a spec-compliance reviewer, then a code-quality reviewer. Do not advance to the next task until the current task's completion conditions and both reviews pass.

## 2026-06-22 correction

This plan has been updated to follow the call-site ID-domain correction. Call-site IDs are backed by `ploke_core::CallId`, not `NodeId`, and typed call-site wrappers live in `crates/ingest/syn_parser/src/parser/nodes/ids/internal/call_ids.rs`.

Do not implement future tasks by adding call-site IDs to `AnyNodeId`/`PrimaryNodeId`, by adding call-site variants to `ItemKind`, or by exposing broad production constructors such as `MethodCallSiteId::generate_synthetic(...)`. Parser extraction should use parser-internal constructors after observing the matching `syn` expression; tests should use explicit test/paranoid helpers.

For compaction/restart, read `docs/active/agents/call-graph/README.md` first.

## Goal

Make `syn_parser` emit typed, parser-owned call-site and call-resolution facts for a first real `fixture_nodes` target, without using stringly proof DTOs as substitutes for parser proof.

First target:

```rust
impl SimpleStruct {
    pub fn new(data: i32) -> Self { Self { data } }

    fn private_method(&self) -> i32 { self.data * 2 }

    pub fn public_method(&self) -> i32 { self.private_method() }
}
```

The first accepted green behavior is:

1. `fixture_nodes/src/impls.rs::SimpleStruct::public_method` owns exactly one structural call site for `self.private_method()`.
2. That call site has a deterministic typed call-site identifier, a typed owner, call kind, receiver classification, span/cardinality metadata, and one `BodyContainsCall` relation.
3. A second resolver slice resolves that call site to the existing typed `MethodNodeId` for `private_method`.
4. Unsupported, external, macro, dynamic, and ambiguous cases fail closed: record structural facts/statuses where appropriate, but do not fabricate resolved edges.

## Architecture

The parser call graph is a typed extension to the existing `syn_parser` graph, not a replacement for it and not a projection from `ploke-records` proof scaffolding.

Phase A records structural call sites from the original `syn` bodies while the parser is already visiting functions and methods. It must not reparse the stored `FunctionNode.body` / `MethodNode.body` token strings as the source of truth.

Phase B resolves only a narrow local receiver case: `self.method()` inside an inherent impl. The resolver may use existing module/impl/method relations and type graph facts, but broad trait dispatch, external crates, macro expansion, and dynamic function values stay unresolved/unsupported until separate tests drive them.

Phase C projects typed parser facts into `ploke-records` proof facts only after parser-side typed tests pass. `ploke-records::extract_proof_facts_from_source` is useful scaffolding, but it is not authoritative for parser call-graph correctness.

## Non-negotiable invariants

- No broad/stringly parser relations for resolved calls. Parser relations must encode allowed endpoint families in the type system.
- Call-site IDs must remain in the `CallId` universe, not the `NodeId` universe. Do not add call-site IDs to node endpoint families.
- Production call-site ID generation must stay parser-internal; deterministic test regeneration belongs in test/paranoid helpers.
- Do not weaken existing parser validation, fixture loading, relation uniqueness, import semantics, or backup fixture behavior.
- Do not add broad `allow(...)`, broad public visibility, or catch-all conversions just to make tests compile.
- Do not use `ploke-records` proof DTOs to satisfy parser tests.
- Do not modify `tests/fixture_crates/fixture_nodes` for the first green slice unless a later explicit task says so. The first structural and resolver tests should use existing fixture code.
- Keep Cargo target/cache behavior default on this VM. Do not set `CARGO_TARGET_DIR`. Put compact logs under `target/test-output/`, not build artifacts.
- A test command that exits 0 while running 0 tests is not verification. If `-- --exact` is used, use the full Rust test path or first list tests.

## Standard verification commands

Use focused commands first, then broaden.

```bash
# Existing typed graph baseline before and after parser graph changes.
cargo test -p syn_parser --features typed_type_graph type_relations_v2 -- --nocapture

# First structural target; exact module path may be adjusted after the test file lands.
cargo test -p syn_parser --features typed_type_graph fixture_nodes_public_method_records_self_private_method_call_site -- --nocapture

# Resolver target; only run after structural call-site extraction is green.
cargo test -p syn_parser --features typed_type_graph fixture_nodes_public_method_resolves_self_private_method_edge -- --nocapture

# Broad parser check after each green implementation slice.
cargo test -p syn_parser --features typed_type_graph -- --nocapture

# Formatting/checks before handoff.
cargo fmt --all --check
cargo check -p syn_parser --features typed_type_graph
```

If a command is long/noisy, save full output to `target/test-output/call-graph/<task-slug>.log` and summarize only the high-signal result in the orchestrator thread.

## Orchestrator protocol

For every task below:

1. Dispatch a fresh implementer subagent with the exact task text, relevant files, invariants, and expected verification command.
2. Require the implementer to report:
   - files changed,
   - exact commands run,
   - exact pass/fail outcome,
   - full log path for noisy commands,
   - whether any test ran 0 tests.
3. Dispatch a spec-compliance reviewer with the task completion conditions.
4. If spec review fails, dispatch a fix subagent and re-run spec review.
5. Only after spec review passes, dispatch a code-quality reviewer.
6. If quality review requests changes, dispatch a fix subagent and re-run both reviews as needed.
7. Mark the task complete only when all completion conditions pass and the worktree contains no unrelated changes.

Parallelization rule: tasks that touch parser call-site data structures, visitors, and resolver code are sequential. Do not dispatch multiple implementation agents against the same parser files at once. Documentation/review-only tasks may run in parallel only after the orchestrator confirms they will not edit the same files.

## Phase 0: Preflight and scope capture

### Task 0.1: Capture baseline state

Objective: Establish the live repo state and existing typed graph baseline before any implementation edits.

Files:
- Read: `AGENTS.md`
- Read: `docs/active/agents/readme.md`
- Read: `crates/ingest/syn_parser/tests/uuid_phase3_resolution/mod.rs`
- Read: `crates/ingest/syn_parser/tests/uuid_phase3_resolution/type_relations_v2.rs`
- Read: `crates/ingest/syn_parser/tests/uuid_phase2_partial_graphs/assoc_nodes/methods.rs`
- Write log: `target/test-output/call-graph/00-preflight-baseline.log`

Steps:
1. Run `git status --short` from repo root and save/quote the output.
2. Run `cargo test -p syn_parser --features typed_type_graph type_relations_v2 -- --nocapture 2>&1 | tee target/test-output/call-graph/00-preflight-baseline.log`.
3. Record whether the baseline is green, red, or blocked.
4. Record any pre-existing dirty files before implementation starts.

Completion conditions:
- `git status --short` is captured in the task report.
- The baseline command was actually run, or a blocker is reported with the exact error.
- If the baseline fails, the failure is classified as pre-existing and the orchestrator decides whether to pause or continue with a narrower RED test.
- No source files are changed by this task.

Stop/escalate if:
- Disk pressure, missing toolchain, or dependency failure prevents Cargo from running.
- The worktree has unrelated dirty implementation files that would make task attribution ambiguous.

### Task 0.2: Map existing extension points and blast radius

Objective: Identify the exact existing parser graph extension points before editing symbols.

Files:
- Read: `crates/ingest/syn_parser/src/parser/nodes/mod.rs`
- Read: `crates/ingest/syn_parser/src/parser/nodes/ids/mod.rs`
- Read: `crates/ingest/syn_parser/src/parser/relations.rs`
- Read: `crates/ingest/syn_parser/src/parser/graph/code_graph.rs`
- Read: `crates/ingest/syn_parser/src/parser/graph/mod.rs`
- Read: `crates/ingest/syn_parser/src/parser/graph/parsed_graph.rs`
- Read: `crates/ingest/syn_parser/src/parser/visitor/state.rs`
- Read: `crates/ingest/syn_parser/src/parser/visitor/code_visitor.rs`
- Read as needed: `crates/ingest/syn_parser/src/parser/visitor/code_visitor_syn1.rs`
- Write log: `target/test-output/call-graph/00-extension-point-map.md`

Steps:
1. Search for existing typed identifier wrapper patterns: `FunctionNodeId`, `MethodNodeId`, `AnyNodeId`, `PrimaryNodeId`, `TypeRelation`, and `GenericRelation`.
2. Search for all `CodeGraph { ... }` initializers and `append_all` paths.
3. Search for `visit_item_fn`, `visit_impl_item_fn`, and existing body/type processing helpers.
4. If GitNexus MCP tools are available in the executing session, run impact analysis before editing target symbols and record the blast radius. If they are not available, explicitly record that and use `search_files` / `read_file` fallback.
5. Produce a short map with the files/functions that each later task will touch.

Completion conditions:
- The extension-point map names the exact files and symbols to edit for node types, relations, graph storage, visitor state, body visiting, and resolver code.
- The map states whether `code_visitor_syn1.rs` must be kept compiling and whether it needs mirrored changes.
- No source files are changed by this task.

Stop/escalate if:
- The existing ID macro/type pattern is unclear enough that a new ID family would require a design decision rather than a mechanical extension.

## Phase 1: Structural RED test

### Task 1.1: Add compile-failing structural RED test for `self.private_method()`

Objective: Drive the parser call-site API from a real `fixture_nodes` row before production code exists.

Files:
- Create: `crates/ingest/syn_parser/tests/uuid_phase3_resolution/call_sites.rs`
- Modify: `crates/ingest/syn_parser/tests/uuid_phase3_resolution/mod.rs`

Test name:

```text
fixture_nodes_public_method_records_self_private_method_call_site
```

Required assertion shape:

```text
Given fixture_nodes/src/impls.rs SimpleStruct::public_method,
assert exactly one call site owned by that method.
The call site is a method call for self.private_method():
- owner == CallBodyOwnerId::Method(public_method_id)
- method_name == "private_method"
- receiver == self receiver classification
- arg_count == 0
- generic_arg_count == 0
- span is inside public_method's span
- cfgs are empty for this fixture row
- deterministic `MethodCallSiteId` regenerated by the test-only helper equals the parsed id
- exactly one BodyContainsCall relation links owner to call site
- private_method owns zero call sites in this fixture row
- no duplicate BodyContainsCall relation exists
```

Steps:
1. Follow the existing `AssocParanoidArgs` style from `uuid_phase2_partial_graphs/assoc_nodes/methods.rs` to regenerate `public_method` and `private_method` IDs from fixture, relative path, path, owner, name, span, and cfgs.
2. Reference the intended parser API names in the test even though they do not exist yet: `CallBodyOwnerId`, `AnyCallSiteId`, `MethodCallSiteId`, `CallNode`, `CallSiteRelation::BodyContainsCall`, and graph accessors for call sites/relations.
3. Gate the module with `#[cfg(feature = "typed_type_graph")]` in `uuid_phase3_resolution/mod.rs` unless the implementer proves the API is feature-independent and all default tests still compile.
4. Run the focused test command.

Expected RED:
- A compile failure because typed call-site API names/accessors do not exist yet, or an assertion failure showing zero call sites after the API scaffold lands.
- It must not fail because the test has syntax errors, imports the wrong fixture, runs zero tests, or uses a tautological assertion.

Completion conditions:
- The test file exists and is included by `uuid_phase3_resolution/mod.rs`.
- The test is strict: no `Ok(_) | Err(_)` acceptance, no `todo!()` in the test as the source of failure, no fallback branch accepting missing implementation.
- Running `cargo test -p syn_parser --features typed_type_graph fixture_nodes_public_method_records_self_private_method_call_site -- --nocapture` produces the expected RED failure.
- The task report includes the exact compiler/assertion failure.

Stop/escalate if:
- Existing fixture helper APIs cannot regenerate `public_method` / `private_method` IDs without duplicating large chunks of production ID logic.

### Task 1.2: Add local call-site paranoid helper only as much as the RED test needs

Objective: Keep the test readable and deterministic without introducing production call-site code prematurely.

Files:
- Modify: `crates/ingest/syn_parser/tests/uuid_phase3_resolution/call_sites.rs`
- Optional later extraction, only if repeated by more tests: `crates/ingest/syn_parser/tests/common/uuid_ids_utils.rs`

Steps:
1. Add a small test-only helper analogous to `AssocParanoidArgs`, named `CallParanoidArgs` only if the existing naming rule permits it.
2. The helper should carry only deterministic inputs needed for the first call-site ID: fixture name, relative file path, owner id, call kind, callee/method name, span, and cfg bytes.
3. Do not add a production constructor in this task.

Completion conditions:
- Helper duplication is limited to the test module unless at least two tests need it.
- The helper is deterministic and does not use random UUID generation.
- The original RED failure still points at missing production API or missing call sites, not helper defects.

Stop/escalate if:
- The helper would require copying private parser ID internals instead of using existing public/test helper patterns.

## Phase 2: Typed call-site surface and graph storage

### Task 2.1: Add minimal typed call-site IDs and node domain

Objective: Create the typed parser domain needed by the structural test, without implementing extraction yet.

Files:
- Create: `crates/ingest/syn_parser/src/parser/nodes/call.rs`
- Modify: `crates/ingest/syn_parser/src/parser/nodes/mod.rs`
- Create/modify: `crates/ingest/syn_parser/src/parser/nodes/ids/internal/call_ids.rs`
- Modify re-exports as needed: `crates/ingest/syn_parser/src/parser/nodes/ids/mod.rs`

Minimum types:

```text
CallId in ploke-core
CallBodyOwnerId = FunctionNodeId | MethodNodeId
PathCallSiteId(CallId)
MethodCallSiteId(CallId)
DynamicCallSiteId(CallId)
MacroCallSiteId(CallId)
AnyCallSiteId = PathCallSiteId | MethodCallSiteId | DynamicCallSiteId | MacroCallSiteId
CallSiteKind::{Path, Method, Dynamic, Macro}
CallNode::{Path, Method, Dynamic, Macro}
```

Minimum metadata for the first structural test:

```text
owner
span
cfgs
method_name for method calls
receiver classification sufficient to distinguish self receiver
arg_count
generic_arg_count
```

Steps:
1. Reuse the existing typed ID wrapper and endpoint-family style, but keep call-site wrappers over `CallId`, not `NodeId`.
2. Implement only conversions needed by the test and graph storage.
3. Add `Serialize`, `Deserialize`, `Clone`, `Copy` where consistent with adjacent ID types.
4. Keep production constructors parser-internal; expose deterministic regeneration only through test/paranoid helpers.
5. Keep naming within the repo rule: no variables/fields with more than three semantic parts. If a name wants four parts, use a nested typed carrier instead.

Completion conditions:
- `cargo check -p syn_parser --features typed_type_graph` compiles past the new type definitions or fails only because graph storage/accessors are not implemented yet in Task 2.2.
- The structural RED test failure moves from missing type names toward missing graph accessors/storage or zero parsed call sites.
- No broad `AnyNodeId`-only resolved call relation is introduced.

Stop/escalate if:
- Adding `AnyCallSiteId` would require weakening existing `AnyNodeId` / `PrimaryNodeId` invariants.

### Task 2.2: Add structural call-site relation and graph accessors

Objective: Store and merge parser call-site facts in `CodeGraph` / `ParsedCodeGraph`.

Files:
- Modify: `crates/ingest/syn_parser/src/parser/relations.rs`
- Modify: `crates/ingest/syn_parser/src/parser/graph/code_graph.rs`
- Modify: `crates/ingest/syn_parser/src/parser/graph/mod.rs`
- Modify: `crates/ingest/syn_parser/src/parser/graph/parsed_graph.rs`
- Modify: `crates/ingest/syn_parser/src/parser/visitor/state.rs`

Minimum relation:

```rust
CallSiteRelation::BodyContainsCall {
    source: CallBodyOwnerId,
    target: AnyCallSiteId,
}
```

Minimum graph additions:

```text
CodeGraph.call_sites: Vec<CallNode>
CodeGraph.call_site_relations: Vec<CallSiteRelation>
GraphAccess::call_sites()
GraphAccess::call_site_relations()
mutable accessors matching existing style
append_all merges both vectors
visitor state initializes both vectors
ParsedCodeGraph forwards accessors or exposes equivalent test-visible access
```

Steps:
1. Keep `CallSiteRelation` separate from `SyntacticRelation` unless a typed extension point already exists for non-primary relation families. The test should not force call sites into `PrimaryNodeId`.
2. Update all `CodeGraph { ... }` initializers discovered in Task 0.2.
3. Update merge/append and accessors.
4. Add relation uniqueness/cardinality helpers only if needed by tests; do not overbuild indexes yet.

Completion conditions:
- `cargo check -p syn_parser --features typed_type_graph` passes.
- The structural RED test runs and fails because there are zero call sites or zero `BodyContainsCall` relations, not because the API is missing.
- Existing baseline `cargo test -p syn_parser --features typed_type_graph type_relations_v2 -- --nocapture` still passes or any failure is explained as unrelated/pre-existing.

Stop/escalate if:
- Existing graph validation assumes every relation is a `SyntacticRelation` over primary nodes and there is no safe place for secondary call-site relations without a broader design decision.

## Phase 3: Structural extraction for the first method call

Status update 2026-06-22: Tasks 3.1 and 3.2 are implemented for the first narrow slice. `syn::ExprMethodCall` with literal `self` receiver now emits `CallNode::MethodCall` plus `BodyContainsCall`, and the focused fixture test passes. A follow-up structural slice also records `syn::ExprCall` with `syn::Expr::Path` callee as `CallNode::PathCall`; `fixture_nodes_use_imported_items_records_pathbuf_new_path_call_site` passes. Dynamic and macro forms remain future work.

### Task 3.1: Add a body call visitor for method calls

Objective: Extract `ExprMethodCall` facts from original `syn` bodies for the first narrow owner family.

Files:
- Create or modify: `crates/ingest/syn_parser/src/parser/visitor/call_extraction.rs` or the nearest existing visitor helper chosen in Task 0.2
- Modify: `crates/ingest/syn_parser/src/parser/visitor/mod.rs`
- Modify as needed: `crates/ingest/syn_parser/src/parser/visitor/code_visitor.rs`
- Modify as needed for compile parity: `crates/ingest/syn_parser/src/parser/visitor/code_visitor_syn1.rs`

Scope:
- Handle `syn::ExprMethodCall`.
- Classify `self.private_method()` receiver as self receiver.
- Record method name, arg count, generic arg count, span, cfgs, owner.
- Add `CallNode::Method` and `CallSiteRelation::BodyContainsCall`.
- Recurse into nested expressions enough to find ordinary method calls inside the body.

Steps:
1. Implement the visitor over `syn` AST nodes, not over stored body strings.
2. Accept owner context from the surrounding function/method visitor.
3. Use parser-internal deterministic `CallId` construction from owner + `CallSiteKind` + source coordinate + callee discriminant.
4. Avoid resolving the target method in this task.

Completion conditions:
- The first structural test either passes or fails only on precise metadata/cardinality differences that the task report identifies.
- No `CallRelation` / resolved edge is emitted in this task.
- No macro, dynamic, trait, or external resolution is attempted.
- Existing `type_relations_v2` baseline remains green.

Stop/escalate if:
- `proc_macro2::Span` byte offsets are unavailable in the existing parser configuration and deterministic call-site IDs cannot be based on stable source coordinates. The orchestrator must choose an alternate existing stable coordinate strategy before continuing.

### Task 3.2: Wire extraction for inherent impl methods

Objective: Ensure `ImplItemFn` bodies are visited with the correct `MethodNodeId` owner.

Files:
- Modify: `crates/ingest/syn_parser/src/parser/visitor/code_visitor.rs`
- Modify as needed: `crates/ingest/syn_parser/src/parser/visitor/code_visitor_syn1.rs`
- Read/check: `crates/ingest/syn_parser/src/parser/nodes/method.rs`
- Read/check: `crates/ingest/syn_parser/src/parser/nodes/impls.rs`

Steps:
1. Locate the point where `MethodNodeId` is generated for an impl method.
2. Invoke the body call visitor after the method ID is known and before/while the body is available as `syn` AST.
3. Pass the exact owner `CallBodyOwnerId::Method(public_method_id)`.
4. Ensure `private_method` with body `self.data * 2` produces zero call sites in the first fixture row.

Completion conditions:
- `fixture_nodes_public_method_records_self_private_method_call_site` passes.
- The test asserts `private_method` owns zero call sites and that assertion passes.
- The body visitor does not alter existing method node IDs, method spans, impl relations, or type relations.
- `cargo test -p syn_parser --features typed_type_graph type_relations_v2 -- --nocapture` passes after the change.

Stop/escalate if:
- Calling the body visitor changes existing method ID generation order or parent scope IDs.

### Task 3.3: Add function-owner smoke coverage only after method-owner green

Objective: Prove `CallBodyOwnerId::Function` is not dead API, without broadening resolution.

Files:
- Modify: `crates/ingest/syn_parser/tests/uuid_phase3_resolution/call_sites.rs`
- Modify: same visitor files as Task 3.1 if function bodies are not wired yet.

Steps:
1. Inspect existing `fixture_nodes` for a simple free function call inside a function body.
2. If an existing fixture row is suitable, add one structural-only test for a `FunctionNodeId` owner.
3. If no existing fixture row is suitable, do not modify fixtures in this task; record the gap for a later explicit fixture task.

Completion conditions:
- Either one strict function-owner structural test passes, or the task report records that no existing fixture row is suitable and no code was changed.
- No resolver behavior is added here.

Stop/escalate if:
- Adding function-owner coverage would require fixture modification before the method-owner slice has been reviewed.

## Phase 4: Structural non-fabrication coverage

### Task 4.1: Add macro-call structural/non-resolution tests

Objective: Ensure macro invocations are not misrepresented as ordinary path or method calls.

Files:
- Modify: `crates/ingest/syn_parser/tests/uuid_phase3_resolution/call_sites.rs`
- Modify extraction files from Phase 3 only if macro structural recording is implemented now.

Candidate fixture rows:
- `format!(...)` in `create_output`
- `println!(...)` in `print_value`

Required assertions:
- Macro call site is either recorded as `CallNode::Macro` or explicitly absent with an `Unsupported`/not-yet-supported status, depending on the accepted API from Phase 2.
- No `CallNode::Path` or `CallNode::Method` is fabricated for the macro invocation.
- No resolved `CallRelation` is emitted for macros in this phase.

Completion conditions:
- A strict test proves macro invocations do not create ordinary resolved call edges.
- If macro sites are structurally recorded, their call kind and relation cardinality are exact.
- If macro structural recording is deferred, the test still proves absence of ordinary fabricated calls.

Stop/escalate if:
- The parser cannot distinguish macro invocation syntax at this layer without invoking macro expansion; do not fake expansion.

### Task 4.2: Add external/builtin method non-fabrication test

Objective: Ensure an external/builtin-looking call such as `.len()` does not resolve to a fake local method.

Files:
- Modify: `crates/ingest/syn_parser/tests/uuid_phase3_resolution/call_sites.rs`
- Modify extraction files only as needed for structural recording.

Candidate fixture row:
- `self.secret.len()` in `get_secret_len`, if present in the current fixture.

Required assertions:
- The `.len()` method call is structurally recorded as a method call if the body visitor sees it.
- It has no local `CallRelation::Method` target.
- It is marked unresolved/unsupported/external only after the resolution-status type exists; before that, absence of resolved edge is sufficient.

Completion conditions:
- The test proves no fake local `MethodNodeId` edge is emitted for `.len()`.
- The structural call-site count remains exact; no duplicate call site is introduced by nested receiver traversal.

Stop/escalate if:
- The existing fixture does not contain the candidate row. Record the gap; do not edit the fixture in this task unless the orchestrator explicitly opens a fixture-change task.

## Phase 5: Narrow resolver RED and GREEN

Status update 2026-06-22: Tasks 5.1 through 5.3 are implemented for the first narrow slice. The resolver storage/status surface exists, `resolve::call_resolution::resolve_call_relations_after_tree(...)` emits an exact `CallRelation::Method` for `self.private_method()`, and `fixture_nodes_public_method_resolves_self_private_method_edge` passes.

### Task 5.1: Add resolver RED test for `self.private_method()`

Objective: Drive the first real typed resolved call edge only after structural extraction is green.

Files:
- Modify: `crates/ingest/syn_parser/tests/uuid_phase3_resolution/call_sites.rs`

Test name:

```text
fixture_nodes_public_method_resolves_self_private_method_edge
```

Required assertion shape:

```rust
CallRelation::Method {
    source: private_method_call_site_id,
    target: private_method_method_node_id,
}
```

Also assert:
- exactly one resolution status for the call site,
- status is resolved/local/exact using the accepted enum names,
- no unresolved/ambiguous/external/unsupported status for the same call site,
- no duplicate resolved edge.

Steps:
1. Reuse the structural test helper to locate the same call site ID.
2. Regenerate `private_method_method_node_id` with the existing assoc method paranoid pattern.
3. Reference intended `CallRelation` / resolution-status API even if missing.

Expected RED:
- Compile failure for missing resolver relation/status types, or assertion failure because no edge exists.

Completion conditions:
- The RED failure is the expected missing relation/status/edge, not missing structural call-site facts.
- The structural test from Task 3.2 remains green.

Stop/escalate if:
- Structural extraction is not green. Do not start resolver work.

### Task 5.2: Add typed call-resolution relation/status storage

Objective: Add storage for resolved and unresolved call outcomes without implementing resolver logic yet.

Files:
- Modify: `crates/ingest/syn_parser/src/parser/relations.rs`
- Modify: `crates/ingest/syn_parser/src/parser/graph/code_graph.rs`
- Modify: `crates/ingest/syn_parser/src/parser/graph/mod.rs`
- Modify: `crates/ingest/syn_parser/src/parser/graph/parsed_graph.rs`
- Modify: `crates/ingest/syn_parser/src/parser/visitor/state.rs` if storage lives in `CodeGraph`

Minimum types:

```text
CallRelation::Function { source: PathCallSiteId, target: FunctionNodeId }      # may be storage-only initially
CallRelation::Method { source: MethodCallSiteId, target: MethodNodeId }
CallResolutionStatus::{Resolved, Unresolved, Ambiguous, External, Unsupported} # names may follow repo style
```

Steps:
1. Keep resolved function/method relations typed by endpoint family.
2. Store resolution status separately from structural `BodyContainsCall` relation.
3. Update graph initialization and merge paths.

Completion conditions:
- `cargo check -p syn_parser --features typed_type_graph` passes.
- Resolver RED test runs and fails because no resolved edge/status is emitted, not because storage is missing.
- Structural tests remain green.

Stop/escalate if:
- The status model needs authority/proof semantics from `ploke-records`; defer that to Phase 7 and keep parser statuses parser-local.

### Task 5.3: Implement exact inherent `self.method()` resolver

Objective: Resolve `self.private_method()` in `SimpleStruct::public_method` to the existing `private_method` `MethodNodeId`.

Files:
- Create or modify resolver module chosen in Task 0.2, likely under `crates/ingest/syn_parser/src/resolve/`
- Modify graph/visitor orchestration only as required to run the resolver after module/impl relations exist
- Read/check: `crates/ingest/syn_parser/src/parser/relations.rs`
- Read/check: `crates/ingest/syn_parser/src/resolve/relation_indexer.rs`
- Read/check: `crates/ingest/syn_parser/src/resolve/path_resolver.rs`

Resolver scope:
- Input: structural method call with self receiver.
- Owner: method inside an inherent impl.
- Candidate set: methods associated with the same impl as the owner method.
- Match: method name exactly equals call site's method name.
- If exactly one candidate: emit `CallRelation::Method` and resolved status.
- If zero candidates: unresolved/external/unsupported status, no edge.
- If more than one candidate: ambiguous status, no edge unless an explicit candidate-edge model is designed in a later task.

Steps:
1. Find the containing impl for the owner method using existing typed relations, not string path matching.
2. Enumerate associated method targets from that impl using existing relation helpers or a minimal local helper.
3. Match by method name only for this narrow slice.
4. Do not attempt trait dispatch, deref, autoderef, generics, external crate methods, inherent impls on other types, or macro-expanded calls.

Completion conditions:
- `fixture_nodes_public_method_resolves_self_private_method_edge` passes.
- Structural tests from Phase 3 and Phase 4 still pass.
- The resolver emits exactly one edge and one resolved status for the target call site.
- The resolver does not emit any edge for `.len()` or macro call tests.
- Existing `type_relations_v2` baseline passes after the change.

Stop/escalate if:
- Existing impl/method relations cannot identify the containing impl without adding a new relation. If so, write a new RED test for that missing relation before implementing it.

### Task 5.4: Add resolver no-fabrication regression tests

Objective: Lock the fail-closed behavior after the first resolver edge is green.

Files:
- Modify: `crates/ingest/syn_parser/tests/uuid_phase3_resolution/call_sites.rs`

Required tests:
- `.len()` external/builtin-like call has no local method edge.
- `format!` / `println!` macro sites have no function/method edge.
- Any unsupported/dynamic call-site family present in existing fixtures has no resolved edge unless explicitly supported.

Completion conditions:
- Each test fails if a broad resolver starts fabricating targets.
- All call relation counts are exact.
- Status assertions distinguish unresolved/unsupported/external from resolved when the status type is available.

Stop/escalate if:
- The status taxonomy is too vague to distinguish external from unsupported. Keep the test at the stronger invariant: no fabricated resolved edge.

## Phase 6: Parser API cleanup and documentation

### Task 6.1: Consolidate test helpers and public API boundaries

Objective: Make the new tests maintainable without widening production visibility unnecessarily.

Files:
- Modify only if repeated helpers exist: `crates/ingest/syn_parser/tests/common/uuid_ids_utils.rs`
- Modify only if needed: `crates/ingest/syn_parser/src/parser/nodes/call.rs`
- Modify only if needed: `crates/ingest/syn_parser/src/parser/graph/mod.rs`

Steps:
1. Move repeated call-site ID test helper code from `call_sites.rs` into `tests/common/uuid_ids_utils.rs` only after at least two tests use it.
2. Keep production constructors as narrow as possible.
3. Remove unused exports introduced during GREEN.
4. Run formatting.

Completion conditions:
- No new broad public exports exist solely for tests if a test-only helper can do the job.
- `cargo fmt --all --check` passes.
- Focused structural and resolver tests pass.

Stop/escalate if:
- Tests require access to private fields and the only easy fix is broad `pub`. Ask for a narrower test hook or constructor design.

### Task 6.2: Add active documentation for supported/unsupported call graph scope

Objective: Leave a durable scope note for future agents without claiming broad call-graph support.

Files:
- Create or modify a focused doc chosen by the orchestrator, preferably under `docs/active/agents/` while active; archive later when complete.
- Candidate: `docs/active/agents/2026-06-21_call-graph-fixture-nodes-status.md`

Required content:
- Supported now: structural method call sites; exact inherent `self.method()` same-impl resolution.
- Explicitly unsupported: trait dispatch, external crates, macro expansion, dynamic `Fn` values, autoref/autoderef, overloaded operators, async desugaring.
- Verification commands with latest observed output.
- Pointer to tests as source of truth.

Completion conditions:
- Documentation distinguishes test/proof scope from product scope.
- Documentation cites exact test names and commands.
- It does not claim broad live-loop proof readiness.

Stop/escalate if:
- The implementation has not reached resolver green; then document only structural support and open resolver work.

## Phase 7: Proof-fact projection after typed parser facts

Do not begin Phase 7 until Phases 3-5 are green and reviewed.

### Task 7.1: Add RED projection test from typed parser facts to `ploke-records` proof facts

Objective: Drive a projection from typed parser call graph facts into existing proof-fact DTOs without making DTOs the parser source of truth.

Files:
- Read: `crates/ploke-records/src/proof_facts.rs`
- Read: `crates/ploke-records/src/proof_effects.rs`
- Create/modify test in the appropriate crate after confirming dependency direction. Do not create a dependency cycle.

Required projection behavior:
- One `CallSiteFact` for the typed call site.
- One `CallEdgeFact` for the resolved edge.
- One `CallResolutionFact` showing resolved/local/exact status.
- Build domain / authority fields are populated from accepted context, not guessed strings.

Completion conditions:
- The RED test fails because the typed parser-to-records projection function does not exist, not because parser facts are missing.
- The test does not call `extract_proof_facts_from_source` as a substitute for parser output.
- Dependency direction is explicitly documented in the task report.

Stop/escalate if:
- `ploke-records` cannot depend on `syn_parser` without a cycle. In that case, define the projection in an existing higher-level crate or a test-only adapter and record the architecture decision before implementing.

### Task 7.2: Implement the minimal projection adapter

Objective: Convert the first typed parser call graph slice into proof facts while preserving parser authority.

Files:
- Exact file depends on Task 7.1 dependency-direction result.
- Likely involved: `crates/ploke-records/src/proof_facts.rs`, `crates/ploke-records/src/proof_effects.rs`, or a higher-level projection crate.

Steps:
1. Map typed call-site ID to `CallSiteId` deterministically.
2. Map typed target `MethodNodeId` to `DefinitionId` / accepted proof target representation.
3. Emit resolution fact only for resolved typed edges.
4. Preserve unresolved/unsupported statuses as proof-blocking when policy requires it; do not silently drop blockers.

Completion conditions:
- Projection RED test passes.
- Existing `ploke-records` proof tests pass.
- Parser tests still pass; projection does not feed back into parser behavior.

Stop/escalate if:
- Authority/proof policy needs detached-process or Crown invariants not represented in parser facts. Pause and write a separate proof-policy RED test.

## Phase 8: Final verification and handoff

### Task 8.1: Run final focused and broad verification

Objective: Prove the completed slice is working and did not regress nearby parser behavior.

Files:
- Write logs under `target/test-output/call-graph/final-*.log`

Commands:

```bash
cargo test -p syn_parser --features typed_type_graph fixture_nodes_public_method_records_self_private_method_call_site -- --nocapture 2>&1 | tee target/test-output/call-graph/final-structural.log
cargo test -p syn_parser --features typed_type_graph fixture_nodes_public_method_resolves_self_private_method_edge -- --nocapture 2>&1 | tee target/test-output/call-graph/final-resolver.log
cargo test -p syn_parser --features typed_type_graph type_relations_v2 -- --nocapture 2>&1 | tee target/test-output/call-graph/final-type-relations-v2.log
cargo check -p syn_parser --features typed_type_graph 2>&1 | tee target/test-output/call-graph/final-check.log
cargo fmt --all --check 2>&1 | tee target/test-output/call-graph/final-fmt.log
```

Completion conditions:
- All commands complete successfully, or any failing command is classified with exact output and open blocker.
- No command reports running 0 intended tests.
- Logs exist at the stated paths.
- `git diff --stat` and `git status --short` are reported.

Stop/escalate if:
- Full workspace tests reveal unrelated failures that would be expensive to triage. Report them as separate blockers rather than hiding them.

### Task 8.2: Final spec and quality review

Objective: Confirm the implemented slice matches this plan and does not overclaim.

Files:
- Read all changed files.
- Read final logs from Task 8.1.

Spec review checklist:
- Structural call site for `self.private_method()` exists exactly once.
- `BodyContainsCall` relation exists exactly once.
- Resolver edge exists exactly once and targets the typed `private_method` ID.
- Negative macro/external tests prevent fabricated edges.
- `ploke-records` projection, if implemented, consumes typed parser facts and does not replace them.
- Unsupported cases are documented as unsupported.

Quality review checklist:
- Reuses existing parser extension points.
- No broad visibility widening.
- No broad `allow`/ignore.
- No fixture mutation unless explicitly opened by a task.
- No custom Cargo target/cache settings.
- Names obey repo naming guidance.
- Tests fail for real behavior and are not tautological.

Completion conditions:
- Spec reviewer returns PASS.
- Quality reviewer returns APPROVED or only minor non-blocking notes accepted by the orchestrator.
- Final user-facing summary cites exact commands/log paths and separates landed behavior from still-open scope.

Stop/escalate if:
- The implementation passed tests by weakening invariants or by accepting missing behavior. Revert or fix before claiming completion.

## Out of scope for this plan

- Trait method dispatch.
- External crate resolution.
- Macro expansion-derived call edges.
- Dynamic `Fn` / function-pointer call resolution.
- Autoref/autoderef, deref coercions, blanket impls, specialization, async lowering, operator overload calls.
- Database schema migrations for call graphs.
- Full proof/Crown/detached-process invariants beyond minimal typed parser-to-proof projection.

## Initial task order for an orchestrator

1. Task 0.1 baseline.
2. Task 0.2 extension-point map.
3. Task 1.1 structural RED test.
4. Task 1.2 helper cleanup if needed.
5. Task 2.1 typed call-site IDs/nodes.
6. Task 2.2 graph storage/accessors.
7. Task 3.1 body visitor.
8. Task 3.2 impl-method wiring.
9. Task 4.1 macro non-fabrication.
10. Task 4.2 external/builtin non-fabrication.
11. Task 5.1 resolver RED.
12. Task 5.2 resolver storage.
13. Task 5.3 exact inherent self-method resolver.
14. Task 5.4 resolver no-fabrication regressions.
15. Task 6.1 helper/API cleanup.
16. Task 6.2 supported-scope doc.
17. Phase 7 projection tasks only if parser facts are green and the orchestrator explicitly opens proof projection.
18. Phase 8 final verification and review.
