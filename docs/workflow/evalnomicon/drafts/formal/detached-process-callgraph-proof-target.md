# Detached Process And Crown Safety Proof Target

Status: draft target note  
Date: 2026-06-18  
Scope: Prototype 1 and future admitted Ploke autonomous-loop descendants

## Purpose

This note records the proof target that should drive future call-graph and code-graph work. It is not an implementation slice. It is the target shape we want to be able to verify before choosing concrete implementation steps.

The motivating long-horizon claim is that Ploke should be safe to run autonomously for many generations without a human supervising process cleanup, authority transfer, or admission boundaries. GraphRAG and symbol lookup are useful products of the call graph, but they are secondary to this proof target.

## Source anchors

Current conceptual anchors in code and docs:

- `crates/ploke-eval/src/cli/prototype1_state/mod.rs`
  - Runtime/Artifact/History/Crown terminology.
  - Immutable/mutated/ambient surface partition.
  - Digest-preserving ordinary descendant admission.
  - Parent/Successor handoff model.
- `crates/ploke-eval/src/cli/prototype1_process.rs`
  - Current child/successor process seam.
  - Current successor handoff records and History seal before successor launch.
- `crates/ploke-eval/src/cli/prototype1_state/walk/client.rs`
  - Current walk server spawn surface; under this target it must be tied to Parent lifetime or otherwise blocked from the long-horizon proof.
- `docs/workflow/evalnomicon/drafts/history/crown-authority-background.md`
  - Older but still useful Crown/History boundary background.

## Terms

### Runtime

An executing process hydrated from an `Artifact`. A Runtime may operate over an Artifact surface and may, under typed configuration, become a Parent.

### Parent

A Runtime with authority to advance a lineage under the current protocol. The authority state is not merely that a process exists; it is that the process has the permissioned Ruler state for a lineage.

### Crown

The lineage-scoped authority to mutate the active checkout and decide which successor may become the next Parent for that lineage. The Crown is not a process id, branch name, path, or machine location. It is the permissioned authority state.

### Detached process

For this proof target, a detached process is any operating-system process whose lifetime is not bounded by the runtime that created it.

This is a lifetime property, not a narrow `Command::spawn` classification. Git commands, cargo commands, child fanout processes, tool runners, editor launches, walk servers, helper daemons, and other operating-system processes are acceptable only if they are proved runtime-bounded.

### Runtime-bounded process

A process is runtime-bounded only if the creating runtime has a proof-relevant ownership path showing that the process and its relevant descendants cannot survive that runtime except through an explicitly admitted successor handoff.

Examples of candidate evidence include:

- the child handle is awaited and reaped;
- timeout paths kill and reap the child;
- cancellation/shutdown paths kill and reap the child;
- process-tree containment is enforced strongly enough that children or grandchildren cannot escape;
- a scoped guard/drop policy has explicit process-family cleanup semantics;
- the call path is blocking, such as a command equivalent to `.output()` or `.status()`, and any subprocess tree created by the invoked program is contained by policy.

Merely storing a process id, detaching stdio, using a process group, or writing a ready file is not enough by itself.

### Successor exception

The one allowed process-lifetime exception is the successor Parent handoff. Even there, the important claim is not that the successor must be created in the same operating-system process tree or by `exec` at the exact moment the old process exits.

Future designs may allow successor execution on another machine or VM, or eventually allow tightly constrained creation of a new lineage. Those are outside the current design scope, but the proof target should not preclude them. The invariant is instead:

```text
At most one permissioned Ruler-state Parent exists for a lineage at any time.
```

There may be overlapping operating-system processes during handoff, migration, cleanup, observation, or transport. There must not be overlapping permissioned mutable Parent authority for the same lineage.

## Primary theorem target

The first major formal claim should be shaped as:

```text
For every admitted descendant Runtime produced from the genesis Parent under the ordinary Ploke transition system:

1. the policy-bearing immutable surface digest is preserved;
2. no operating-system process created by that Runtime can outlive that Runtime, except through an admitted successor-handoff transition;
3. any admitted successor can become Parent<Ruling> only after validating the predecessor sealed History head, active Artifact identity, and policy-bearing surface commitment;
4. for each lineage, at most one permissioned Ruler-state Parent exists at any time.
```

This is an inductive claim over admitted descendants. It does not claim that no external process on the machine can run incompatible code. It claims that incompatible code is outside the admitted transition system and cannot enter the History/Crown mutation path.

## Surface-digest induction

The surface-digest rule is the base of the induction.

Current Prototype 1 ordinary succession partitions the relevant surface as:

```text
Immutable = crates/ploke-eval
Mutated   = all tool-description text files
Ambient   = empty declared surface
```

The policy-bearing immutable surface contains the code that defines parent creation, child and successor execution, History admission, Crown transitions, surface checks, handoff, and the rule that enforces the immutable surface itself.

The intended inductive shape is:

```text
Base:
  Genesis Parent is admitted with immutable surface digest D.

Step:
  A Parent admitted under digest D may execute or admit a child/successor only after proving the candidate Artifact also carries digest D for the policy-bearing surface.

Conclusion:
  Every admitted descendant produced by ordinary succession preserves the same authority and process-safety rules.
```

This is why changes to `Cargo.toml`, build scripts, protocol code, History/Crown code, spawn wrappers, call-graph verifier code, or admission policy code cannot be ordinary mutable-surface edits. They require an explicit protocol-upgrade or fork transition with stronger admission rules.

## Successor handoff semantics

The successor proof should be about authority transfer, not merely same-host process replacement.

A too-narrow target would say:

```text
Parent execs successor, so parent and successor cannot overlap.
```

That may be useful for a local demo, but it is not the desired long-term model. A future successor may run on a different machine or VM. The predecessor may need to clean up local resources after the successor has been admitted. The protocol may eventually permit carefully constrained new-lineage creation. Therefore the durable claim must be:

```text
The predecessor loses permissioned Ruler authority before or at the same logical boundary where the successor can obtain permissioned Ruler authority, and the sealed History/Crown evidence makes any overlap in Ruler authority invalid.
```

A valid successor handoff must therefore provide proof-relevant evidence for:

- selected Artifact identity;
- selected Runtime or successor identity;
- lineage id;
- predecessor sealed block hash;
- current active checkout Artifact identity;
- immutable surface digest and partition;
- handoff transition identity and policy;
- predecessor authority retirement or Crown lock;
- successor admission check;
- exactly one admitted successor for the relevant lineage transition, unless a future explicit fork/new-lineage transition says otherwise.

A ready acknowledgement, pid, invocation file, or transport channel may be operational evidence, but none of those should be the authority source by itself.

## Walk command semantics

The walk command is currently in development and is intended to provide continuity of access across Parent generations. Under this proof target, the walk server must not become an unbounded detached process.

The design goal is:

```text
walk service lifetime is tied to the current Parent authority epoch, while startup/shutdown is coordinated so the successor Parent can resume or replace service without user-visible interruption.
```

That implies the call graph must be able to distinguish:

- a Parent-owned service process with explicit shutdown and transfer semantics;
- a successor-owned replacement service started after admission;
- a temporary overlap used only for graceful transition;
- an unowned detached server that can survive without any active Parent authority.

The last case is a proof blocker for the long-horizon autonomous claim.

## What the call graph must become

To prove the detached-process claim strongly, the call graph must become a typed effect, lifetime, and authority graph. A caller/callee graph is insufficient.

### 1. Build-accurate Rust call resolution

The graph must be computed for the actual admitted build configuration, including:

- Cargo workspace package graph;
- selected target triples and operating systems;
- features and `cfg` conditions;
- macro expansion;
- procedural macro expansion or audited summaries;
- build script execution policy;
- trait method resolution;
- generic monomorphization where needed for effect precision;
- dynamic dispatch candidate sets;
- async desugaring and task boundaries;
- panic/error paths that skip cleanup.

Every edge must carry a resolution state:

- resolved;
- candidate set;
- ambiguous;
- unresolved;
- externally summarized.

For process-lifetime proofs, unresolved dangerous edges are proof blockers, not missing data to ignore.

### 2. Complete process-effect inventory

The graph must identify every operating-system process effect, including direct and indirect forms:

- `std::process::Command` builders and terminal methods;
- `tokio::process::Command` and other async process wrappers;
- `CommandExt::exec`, `pre_exec`, `process_group`, session and signal manipulation;
- shell execution through `sh -c`, `bash -c`, scripting wrappers, or helper crates;
- crates that wrap process execution, such as command runner libraries;
- FFI calls that can create or replace processes, such as `fork`, `posix_spawn`, `exec*`, `system`, or daemonization helpers;
- build-time process effects from `build.rs`, proc macros, cargo build hooks, and code generators;
- dependency APIs whose audited summary includes process creation;
- test-only process effects when the proof domain includes tests or when test-only code can be compiled into admitted artifacts.

The inventory must preserve the source expansion chain so a spawned process hidden behind a macro or helper function can still be audited at the source that introduced it.

### 3. Command-builder dataflow

For each process effect, the graph must capture the builder dataflow:

- executable path or symbolic executable class;
- arguments;
- current working directory;
- environment variables added, removed, or inherited;
- stdin/stdout/stderr configuration;
- process group or session changes;
- privilege, uid/gid, namespace, sandbox, or container settings if any;
- timeout and cancellation policy;
- handle ownership path;
- journal/evidence records written before and after spawn;
- feature/config conditions controlling the path.

This matters because `git status` with inherited hooks, pagers, credential helpers, askpass helpers, or ssh transport is not the same proof object as a hermetic, noninteractive, contained git invocation.

### 4. Process-family containment model

A strong detached-process proof cannot stop at the immediate child handle. External programs are opaque and may themselves spawn children.

The graph must therefore connect process launches to a containment policy, such as:

- process group/session discipline plus verified kill/reap paths;
- cgroup or job-object ownership;
- namespace/container sandboxing;
- parent-death signal where appropriate;
- explicit ban on command forms that can escape containment;
- audited wrappers for allowed external executables.

If an opaque external command can spawn a grandchild that survives the parent runtime, the process-lifetime proof fails unless an operating-system-level containment rule prevents that escape.

### 5. Runtime ownership and cleanup dominance

For each non-successor process effect, the graph must prove cleanup dominance:

```text
spawn/create process
  -> handle is stored in a runtime-owned scope
  -> all normal exits wait/reap or prove completion
  -> all timeout paths kill/reap
  -> all cancellation/shutdown paths kill/reap
  -> all error paths before ownership transfer clean up
  -> parent runtime termination cannot leave the process family alive
```

This requires control-flow information in addition to call edges. The graph must reason about early returns, `?`, panic boundaries where relevant, async cancellation, dropped futures, task aborts, and destructor/drop guarantees.

### 6. Distinguish operating-system processes from async tasks

Async tasks are not detached operating-system processes, but they may own or drive process effects. The graph must model them as lifetime scopes:

- which runtime owns the task;
- whether the task is joined or abortable;
- whether it can spawn operating-system processes;
- whether cancellation runs cleanup;
- whether it can continue after Parent authority ends.

A `tokio::spawn` is not automatically a detached process, but an unjoined task that can spawn or supervise processes is still proof-relevant.

### 7. Authority-token graph

The call graph must connect process effects to authority tokens and typestate transitions.

At minimum it must model constructors, private fields, move-only transitions, and module visibility for:

- `Parent<...>` states;
- `Crown<Ruling>`;
- `Crown<Locked>`;
- `Parent<Retired>`;
- `Startup<Validated>`;
- successor invocation/admission carriers;
- channel/box types whose messages are preconditions for transitions;
- History block open/seal/append operations.

The graph must prove that no code path can mint or regain permissioned Ruler authority outside the intended transition system.

### 8. Durable evidence graph

The proof must bind source-code effects to durable run evidence:

- sealed History blocks;
- History head projections;
- transition journals;
- successor invocation records;
- successor ready/completion records;
- surface commitment records;
- child evaluation records;
- walk service lifecycle records if the walk service participates in Parent continuity.

Runtime evidence is not the proof by itself, but it is necessary to show that the specific admitted run followed the statically verified transition system.

### 9. Negative proof and proof blockers

The graph must produce an auditable negative result:

```text
All operating-system process effects in the admitted codebase are accounted for.
Every non-successor process effect is runtime-bounded under a verified containment and cleanup policy.
The only outliving process authority path is successor handoff, and Crown semantics prevent overlapping permissioned Ruler authority.
```

The verifier must fail closed. Blockers include:

- unresolved dangerous calls;
- unaudited external command wrappers;
- opaque dependencies with possible process effects;
- macro expansions that cannot be inspected;
- build scripts or proc macros outside the immutable/audited surface;
- process spawns without tracked handles;
- process groups or daemonization without runtime-owned cleanup;
- async tasks that can outlive Parent authority and spawn processes;
- feature/cfg paths not included in the proof domain;
- mutable-surface changes that can affect process policy, Cargo metadata, build scripts, or dependency resolution.

### 10. Proof artifact requirements

The call graph should eventually emit a proof artifact that is itself content-addressed and admitted as evidence. The artifact should include:

- source revision and Artifact identity;
- Cargo metadata and lockfile identity;
- feature/cfg/build profile;
- immutable surface digest;
- dependency summaries and versions;
- all process-effect sites and classifications;
- all unresolved/candidate dangerous edges;
- typestate/authority-token constructor graph;
- cleanup dominance evidence for each runtime-bounded process;
- successor exception proof obligations and discharge status;
- verifier version and policy version.

A successor should be able to reject admission if the proof artifact is absent, stale, computed for a different surface digest, or contains unresolved proof blockers.

## Limitations the target must explicitly address

Known hard areas are not reasons to weaken the target, but they must be represented explicitly:

- Macro expansion can hide process effects unless the graph uses compiler-grade expansion or trusted expansion artifacts.
- `build.rs` and procedural macros are executable code at build time; if they are not immutable or audited, they can invalidate the claim.
- External executables are opaque; strong proof requires containment or tightly audited command policy.
- Dependencies may hide process creation; the graph needs dependency summaries or whole-dependency analysis for the admitted build.
- Dynamic dispatch and trait objects require candidate-set reasoning, not best-effort single-edge resolution.
- Platform-specific process semantics differ; the proof target must name the operating systems it covers.
- Panic, cancellation, and dropped async futures can bypass ordinary cleanup unless modeled.
- Future remote successor/migration designs require authority-transfer proof independent of local process-tree parentage.

## Non-goals for this draft

This draft does not choose the implementation slice, verifier backend, database schema, or exact proof assistant boundary. It intentionally states the target first so those choices can be derived from the required guarantees.

This draft also does not claim the current implementation already satisfies the theorem. It defines what the call graph and surrounding proof machinery must be capable of proving before the long-horizon autonomous safety claim should be made.

See also `callgraph-implementation-design-for-detached-process-proof.md` for the implementation-target design derived from this theorem target.
