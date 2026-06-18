# Macro/build.rs And Call-Graph Sequencing Survey

Status: sequencing recommendation  
Date: 2026-06-18  
Depends on:
- `detached-process-callgraph-proof-target.md`
- `callgraph-implementation-design-for-detached-process-proof.md`

## Question

Should Ploke first implement the proof-grade call graph while leaving macros and build scripts as unresolved edges, then circle back, or should it spend time now on macro/build-script handling before the call graph?

## Recommendation

Do neither extreme.

The least-refactoring path toward the long-term proof target is:

1. **Handle macro/build-script provenance now as a first-class build-domain and expansion-boundary layer.**
2. **Do not attempt full macro expansion or full build-script semantic interpretation now.**
3. **Then implement the call/effect graph against that boundary-aware representation, with explicit unresolved/blocked/audited states.**

In short:

```text
BuildDomain + ExpansionBoundary first,
then CallGraph with unresolved/candidate/audited states,
then progressive expansion/resolution.
```

A call graph that merely adds generic unresolved edges for macros/build.rs without first-class build-domain and expansion-boundary nodes would almost certainly be refactored. But a full compiler-grade macro/build.rs subsystem before any call/effect graph would delay the process-safety work too much and is not necessary for the next useful proof slice.

## Survey facts from the current repository

### Workspace build scripts

`cargo metadata --format-version 1 --no-deps` reports **no workspace packages with `custom-build` targets**.

The only in-repository `build.rs` paths found by file search are:

- `tests/fixture_workspace/fixture_mock_serde/mock_serde/build.rs`
- `crates/ploke-tree/src/playback/fine/build.rs`
- `crates/ploke-tree/src/graph/build.rs`

The last two are ordinary Rust module files named `build.rs`, not Cargo build scripts.

Implication: for Ploke-owned workspace code, build-script handling can start as a Cargo-target inventory and policy layer rather than a complex semantic analyzer. The immediate workspace build-script risk is low.

### Dependency build scripts

Full `cargo metadata` over the current dependency graph reports many external build-script packages:

- all-dependency metadata: **148 packages with custom-build targets**;
- unified `ploke-eval` dependency closure: **117 packages with custom-build targets**;
- workspace custom-build packages: **0**.

These external build scripts should not be ignored for a proof artifact, but the first Ploke proof domain can treat them as locked dependency-summary boundaries:

- package id;
- version/source;
- Cargo.lock identity;
- target/config domain;
- whether the proof policy admits the dependency summary;
- whether the summary is audited, externally summarized, or blocked.

Trying to semantically interpret all external build scripts now would be a poor use of time.

### Procedural macros

`cargo metadata --no-deps` reports four workspace proc-macro crates:

- `derive_test_helpers`
- `ploke-db-derive`
- `ploke-test-macros`
- `syn_parser_macros`

Search of `proc_macros/` found no direct process-spawn patterns such as `Command::new`, `std::process`, `tokio::process`, `.spawn(`, `.output(`, `.status(`, `Stdio::`, `include!`, or `env!`.

Full dependency metadata includes many external proc-macro crates. The unified `ploke-eval` closure reported **59 proc-macro packages**, including the four workspace proc-macro crates.

Implication: proc macros need provenance in the graph now, but not full expansion semantics for every external proc macro. Workspace proc macros can plausibly get audited summaries early. External proc macros should start as locked summary boundaries.

### Declarative macros

The existing parser already has some macro representation:

- `MacroNode` records declarative and procedural macro definitions in `syn_parser`.
- `visit_item_macro` records `macro_rules!` definitions as `MacroKind::DeclarativeMacro` and stores the body tokens.
- `visit_item_fn` records proc-macro functions as `MacroKind::ProcedureMacro` when proc-macro attributes are present.
- `ploke-mbe` already parses `macro_rules!` definitions into a small IR and can parse expanded item tokens into coarse structural items.

But the existing graph does **not** yet have the proof-grade expansion facts we need:

- macro invocation nodes;
- expansion context identifiers;
- expansion stack/provenance edges;
- macro-introduced call sites/effects;
- invocation-to-definition relation;
- explicit unresolved/blocked expansion-boundary status.

A workspace-source count over `crates/`, `proc_macros/`, and `xtask/` found:

- `macro_rules!`: 80 occurrences in 36 files;
- `include!`: 5 occurrences in 3 files;
- `env!`: 51 occurrences in 39 files;
- `derive` attributes: 3527 occurrences in 479 files;
- `cfg`/`cfg_attr` attributes: 1561 occurrences in 397 files.

Implication: pretending macros are a corner case would be wrong. But the first critical step is not full expansion; it is representing macro boundaries and expansion provenance so later expansion can attach without changing the call/effect schema.

### Current call/effect surface

The current parser structural graph has typed item and type relations, but no call-site universe or resolved call/effect graph.

Existing plan `.hermes/plans/2026-06-15_225532-ploke-call-graph-typed-plan.md` already points in the right direction for typed call-site identifiers and conservative unresolved statuses.

A workspace-source scan over `crates/`, `proc_macros/`, and `xtask/` found process/async candidates that the future effect graph must classify:

- process command patterns: 94 occurrences in 30 files;
- `.spawn(`: 12 occurrences in 8 files;
- `.output(` / `.status(`: 111 occurrences in 39 files;
- `Stdio::`: 23 occurrences in 5 files;
- `tokio::spawn`: 138 occurrences in 41 files;
- `thread::spawn`: 13 occurrences in 10 files.

These include real Ploke process surfaces such as:

- `crates/ploke-eval/src/cli/prototype1_process.rs`
- `crates/ploke-eval/src/cli/prototype1_state/c3.rs`
- `crates/ploke-eval/src/cli/handlers/closure.rs`
- `crates/ploke-tui/src/tools/cargo.rs`
- `xtask/src/commands/parse_debug.rs`

Implication: the call/effect graph is urgently useful, but it should be built on representation that can distinguish ordinary source effects from macro-generated effects, external-summary effects, test-only effects, and build-domain effects.

## Option evaluation

### Option A: Full macro/build.rs handling before call graph

Benefits:

- maximally aligned with the final proof target;
- avoids leaving any expansion unknowns in the first proof attempt;
- could use compiler-expanded source as the starting point.

Costs:

- too much scope before getting process-effect inventory;
- external build scripts and proc macros in the dependency closure are numerous;
- risks turning the project into a compiler integration project before Ploke has its own proof intermediate representation;
- still would not remove the need for unresolved/candidate/external-summary states, because dynamic dispatch, foreign calls, generated code, and external dependencies remain.

Verdict: **not the best first implementation path**.

### Option B: Implement call graph now with generic unresolved edges for macros/build.rs, then circle back

Benefits:

- fastest path to visible graph output;
- useful for GraphRAG/navigation;
- allows early direct-call extraction and process-effect scans.

Costs:

- likely representation churn later;
- generic unresolved edges do not say whether the missing fact came from a macro invocation, a build script, a proc macro, an external dependency, a disabled `cfg`, or unsupported resolver logic;
- proof blockers would be too coarse to guide implementation;
- macro-introduced process effects would not have source/expansion provenance;
- build-domain identity would arrive after the call graph, forcing edge IDs and artifact identities to change.

Verdict: **too risky if done literally**.

### Option C: Build-domain and expansion-boundary foundation now, then call/effect graph

Benefits:

- makes the long-term proof boundary explicit before call-edge IDs and schemas harden;
- lets unresolved edges carry the reason and authority/proof consequence;
- allows early call/effect graph work without pretending macro/build facts are solved;
- aligns with existing `CompilationUnitKey`, `MacroNode`, `UnresolvedNode`, `ploke-mbe`, and Cozo relation extension patterns;
- supports GraphRAG and symbol lookup while preserving proof-grade monotonicity.

Costs:

- a small upfront delay before direct call-edge extraction;
- requires adding a few schema concepts that are not immediately used by navigation-only queries;
- initial proof artifact will still block on many external summaries.

Verdict: **recommended**.

## Concrete next steps

### Step 1: Add BuildDomain / ExpansionBoundary model, not full expansion

Add a durable representation for:

- Cargo metadata hash;
- Cargo.lock hash;
- package id;
- target kind;
- root file;
- target triple;
- profile;
- feature set;
- active `cfg` set or cfg-domain placeholder;
- rustc/toolchain identity;
- workspace/source digest;
- build-script targets in the closure;
- proc-macro crates in the closure;
- proof-policy version;
- immutable-surface digest.

Also add `ExpansionBoundary` or equivalent records for:

- declarative macro definition;
- declarative macro invocation;
- procedural macro crate/function;
- derive/attribute/function-like proc macro invocation;
- build script target;
- external dependency summary.

The important field is not expansion output yet. The important fields are:

```text
boundary_id
boundary_kind
build_domain_id
source_span_or_package_target
resolution_state
summary_id?
blocking_reason?
policy_status
```

### Step 2: Extend the existing unresolved model instead of using a single generic unresolved edge

Current `UnresolvedNode` reasons are structural/import-oriented. Add proof-oriented unresolved statuses for call/effect extraction, such as:

- `MacroExpansionNotAvailable`
- `ProcMacroSummaryMissing`
- `BuildScriptSummaryMissing`
- `ExternalDependencySummaryMissing`
- `CfgDomainNotMaterialized`
- `TypeResolutionMissing`
- `DynamicDispatchCandidateSetIncomplete`
- `UnsupportedExpressionShape`

For dangerous effects, these statuses must be proof blockers.

### Step 3: Add a tiny macro/build fixture matrix

Do this before the broad call graph so the schema is forced to express the cases that would otherwise cause refactoring.

Fixtures should include:

1. `macro_rules!` that expands to a function call;
2. `macro_rules!` that expands to `Command::new(...).status()`;
3. derive macro on a struct, represented as proc-macro boundary;
4. attribute macro on a function, represented as proc-macro boundary;
5. fixture crate with `build.rs` generating a Rust file used by `include!`;
6. `#[cfg]`-gated macro invocation that is disabled for one `CompilationUnitKey` and enabled for another.

The initial expected output should not require full expansion. It should require stable boundary records and proof-blocker reasons.

### Step 4: Implement the first call-site/effect extractor against that model

Then add typed call-site IDs and first-pass extraction for:

- direct path calls: `foo()`, `module::foo()`;
- method syntax as unresolved or candidate-set until type resolution is available;
- macro invocations as `MacroCallSite` / `ExpansionBoundary`, not ordinary unresolved calls;
- direct process-effect seeds: `std::process::Command::new`, `tokio::process::Command::new`, terminal `.status()`, `.output()`, `.spawn()`;
- async/task spawns: `tokio::spawn`, `thread::spawn`.

This gives immediate value for the detached-process survey while keeping the representation proof-compatible.

### Step 5: Add summary-first handling for workspace proc macros and build scripts

Because workspace has no real Cargo build scripts currently, start with tests/fixtures and external summary records.

For workspace proc macros, create audit summaries stating:

- source hash;
- whether the macro implementation itself spawns processes;
- whether generated tokens may introduce process effects;
- whether expansion must be inspected before a proof can pass.

The earlier survey found no direct process-spawn patterns in `proc_macros/`, so workspace proc macros are good early candidates for manually verified summaries.

### Step 6: Only then choose the compiler-integration path

Once the boundary-aware call/effect graph exists, decide whether expansion facts should come from:

- rustc/HIR/rustdoc-json-like facts;
- rust-analyzer integration;
- `cargo expand`/expanded source artifacts for selected proof domains;
- Ploke's own `ploke-mbe` expansion for a safe subset of `macro_rules!`;
- manual/audited summaries for external proc macros and build scripts.

At that point the expansion source can change without refactoring the proof schema.

## Decision rule for implementation slices

A slice is acceptable if it is **monotonic toward the proof target**:

- it may leave facts unresolved;
- it may block the proof conservatively;
- it may support only a small subset of call shapes;
- it may use external summaries for dependencies;
- but it must not encode calls/effects in a way that loses build-domain identity, expansion provenance, or proof-blocker reason.

A slice is not acceptable if it produces a useful navigation graph by flattening away the exact missing facts needed for the detached-process and Crown/Ruler proof.

## Bottom line

Spend the next work on the first step of the proof-grade plan, but keep it narrow:

```text
BuildDomain + ExpansionBoundary + proof-blocking unresolved statuses.
```

Then immediately move into the call/effect graph, starting with process-effect seeds. This path gives the call graph useful shape soon, but avoids the churn of retrofitting macro/build provenance after edge IDs, schema, and proof artifacts have already solidified.
