# Prototype 1 Startup/History Invariant Review - Reviewer B

Scope: uncommitted changes plus the requested anchors, reviewed for typestate and authority boundaries on 2026-05-01.

## Findings

### 1. High - Genesis admission is still only local store absence, not a coherent artifact/surface base case

Claim: A runtime may enter `Parent<Ready>` after `Startup<Validated>` checks "local genesis absence" or predecessor History plus current checkout evidence.

Evidence:
- The no-handoff path constructs `Startup::<Genesis>::from_history(identity, manifest_path)` and immediately calls `parent.ready(startup)` in `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3064`.
- `Startup<Genesis>::from_history` only reads `FsBlockStore::lineage_state`, checks the identity generation is `0`, and accepts `StoreHead::Absent` in `crates/ploke-eval/src/cli/prototype1_state/parent.rs:442` through `crates/ploke-eval/src/cli/prototype1_state/parent.rs:478`.
- `FsBlockStore::read_heads` treats a missing `heads.json` as an empty map when no stored block/index files are detected in `crates/ploke-eval/src/cli/prototype1_state/history.rs:678` through `crates/ploke-eval/src/cli/prototype1_state/history.rs:687`; `lineage_state` then returns `StoreHead::Absent` in `crates/ploke-eval/src/cli/prototype1_state/history.rs:849` through `crates/ploke-eval/src/cli/prototype1_state/history.rs:863`.
- The first live handoff creates the genesis block later if the store head is absent, in `crates/ploke-eval/src/cli/prototype1_process.rs:1145` through `crates/ploke-eval/src/cli/prototype1_process.rs:1165`.

Risk: Deleting, mispointing, or failing to initialize `prototype1/history` can make a generation-0 parent look like valid genesis. This can mask missing History state instead of proving the configured base case. The code does reject inconsistent projections when stored block/index files exist, but an empty or wrong store root is still accepted as authority absence. Genesis also does not bind the current clean tree key or surface commitment before `Parent<Ready>`, so the base case is weaker than the recursive successor case.

Suggested fix: Make genesis admission an explicit carrier with a setup/bootstrap witness, active checkout tree key, and surface commitment. Either seal/open the genesis History base before entering `Parent<Ready>`, or require a setup-created bootstrap record tied to the configured History root and current artifact. Reject an empty/missing store unless the bootstrap witness proves this is the intended first parent.

### 2. High - Successor startup gates the checkout, but not the sealed successor attempt identity

Claim: Invocation JSON may transport the successor to startup, but sealed History/current tree/current surface evidence is the authority for becoming the next parent.

Evidence:
- The successor path still gets the attempt role, node id, runtime id, journal path, and active parent root from invocation JSON loaded in `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3069` through `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3118`.
- `validate_prototype1_successor_continuation` only checks the mutable scheduler's latest continuation decision and the node branch id in `crates/ploke-eval/src/cli/prototype1_process.rs:384` through `crates/ploke-eval/src/cli/prototype1_process.rs:437`.
- `Startup::<Predecessor>::from_history` verifies the sealed head, current clean tree key, and current surface in `crates/ploke-eval/src/cli/prototype1_state/parent.rs:481` through `crates/ploke-eval/src/cli/prototype1_state/parent.rs:521`.
- The sealed block records `selected_successor` during handoff in `crates/ploke-eval/src/cli/prototype1_process.rs:955` through `crates/ploke-eval/src/cli/prototype1_process.rs:959`, but startup does not compare the loaded invocation runtime/node/journal to that sealed successor. The exposed sealed-head checks only verify artifact tree and surface in `crates/ploke-eval/src/cli/prototype1_state/history.rs:2805` through `crates/ploke-eval/src/cli/prototype1_state/history.rs:2850`.

Risk: The new carrier prevents a successor from entering `Parent<Ready>` unless the current checkout matches the sealed head's artifact and surface. That is a real improvement. But the attempt identity remains anchored in invocation JSON and mutable scheduler state, not in sealed History. A stale or rewritten invocation for the same current checkout can still define the runtime id and journal path used for ready/completion records, and the startup gate does not prove that this invocation is the exact successor attempt sealed by the predecessor.

Suggested fix: Bind successor startup to sealed successor evidence. For example, add a sealed-head verifier that compares `selected_successor` to a structured successor-start carrier, and pass that carrier into `Startup::<Predecessor>::from_history`. Treat scheduler continuation and invocation JSON as transport/evidence only; the admitted successor attempt should come from the sealed block or a sealed attempt-scoped handoff record.

### 3. Medium - `Startup<Validated>` is a real admission carrier, but its validation snapshot can go stale

Claim: `Startup<Validated>` is the local single-ruler startup gate.

Evidence:
- `Parent<Checked>::ready` now consumes `Parent<Checked>` and requires `Startup<Validated>` in `crates/ploke-eval/src/cli/prototype1_state/parent.rs:427` through `crates/ploke-eval/src/cli/prototype1_state/parent.rs:439`.
- `Startup<S>` has private fields, and the only non-test production constructors for `Startup<Validated>` are inside `parent.rs` in `crates/ploke-eval/src/cli/prototype1_state/parent.rs:442` through `crates/ploke-eval/src/cli/prototype1_state/parent.rs:623`.
- `Startup<Validated>::validate_parent` checks the carried lineage, parent node id, generation, and kind/head consistency in `crates/ploke-eval/src/cli/prototype1_state/parent.rs:553` through `crates/ploke-eval/src/cli/prototype1_state/parent.rs:605`.

Risk: I did not find a direct crate-visible construction bypass for `Startup<Validated>` or `Parent<Ready>`. The gap is temporal: `from_history` validates a local snapshot and `ready` validates only the carried snapshot against identity. It does not re-read the store or hold a lease/lock before the parent turn starts. A concurrent append can make the admitted predecessor head stale without invalidating the already-created carrier. This aligns with the documented lack of process uniqueness, but it is not compiler-enforced single-ruler authority.

Suggested fix: Either narrow the claim to "snapshot-validated local startup" or make `Parent<Ready>` carry the checked `LineageState` plus a store lease/current-head recheck at the transition boundary. Add a test or fixture for head advancement between `Startup::<Predecessor>::from_history` and `Parent<Checked>::ready`.

### 4. Low - Structural naming drift remains around successor continuation/handoff helpers

Claim: Prototype 1 role/state structure should be preserved by carriers rather than long helper names or flattened procedure checks.

Evidence:
- The live path still uses helper names such as `validate_prototype1_successor_continuation` and `validate_prototype1_successor_node_continuation` in `crates/ploke-eval/src/cli/prototype1_process.rs:384` through `crates/ploke-eval/src/cli/prototype1_process.rs:437`.
- The handoff block assembly is a loose `HandoffBlock`/`handoff_block_fields` struct and function in `crates/ploke-eval/src/cli/prototype1_process.rs:1119` through `crates/ploke-eval/src/cli/prototype1_process.rs:1186`.
- `AGENTS.md` explicitly warns that subsystem/command/phase/action helper names usually indicate a missing module or carrier in `AGENTS.md:8` through `AGENTS.md:12`.

Risk: These names are not only style drift. They mark checks that are not yet represented as typed protocol objects, especially successor selection/continuation and startup handoff. That makes it easier for future code to add another call path that repeats part of the check while missing the History authority boundary.

Suggested fix: Move these checks behind a structural carrier such as `Successor<Selected>`/`Successor<Started>` or a successor-start component of `Startup<Predecessor>`. Keep the local method names short once the module/type boundary carries "Prototype 1 successor continuation" meaning.

## Enforcement Summary

Compiler-enforced:
- `Parent<Checked>::ready` cannot be called without a `Startup<Validated>` value.
- `Startup<Validated>` is not constructible by ordinary sibling modules because fields and the validation constructor are private to `parent.rs`.
- `Parent<Ready>` construction through struct literals is blocked outside `parent.rs` because `Parent` fields are private.
- `Crown<Ruling>` construction is private to `inner.rs`; the live seal path goes through `Parent<Selectable>::seal_block_with_artifact`.

Runtime-enforced:
- Parent checkout identity/node/scheduler agreement before `Parent<Checked>`.
- Genesis startup requires generation 0 and local absent History head.
- Predecessor startup requires present local head, verified sealed block hash, current clean tree key matching the sealed artifact claim, and current surface matching the sealed block surface.
- Store append checks stale head/root, lineage, height, and predecessor hash.

Documented or partial only:
- Genesis as an admitted base case for the current artifact/surface.
- Authenticated absence proof for missing History.
- Binding successor invocation/runtime id to the sealed `selected_successor`.
- OS process uniqueness or lease-based single-ruler execution.
- Fully structural successor selection/continuation carriers.

## Uncertainties And Tests To Add

- Add a successor startup test with a sealed block for runtime A and an invocation for runtime B over the same active checkout. It should fail once successor attempt identity is sealed.
- Add a genesis startup test where the History directory is missing after prior setup or where the manifest points at an empty wrong store. Decide whether this should reject or require an explicit bootstrap witness.
- Add a predecessor startup test where the store head advances after `Startup::<Predecessor>::from_history` but before `Parent<Checked>::ready`.
- Add negative tests for missing artifact claim and surface mismatch on `Startup::<Predecessor>::from_history`; the lower-level methods exist, but the startup constructor should have direct coverage.
- I ran `cargo test -p ploke-eval prototype1_state::parent::tests::genesis_startup -- --nocapture`; the two targeted genesis tests passed.

## Verdict

Commit with caveats.

The diff is moving in the right direction: `Startup<Validated>` is a real carrier for the live `Parent<Ready>` transition, and I did not find a simple construction bypass. Do not treat it as a complete History authority boundary yet. Before relying on it for longer runs, either narrow the claims around genesis and successor attempt identity or add the explicit carriers/checks described above.
