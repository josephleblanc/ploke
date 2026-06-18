# Rustc Macro-Expansion Backend Plan

Status: design correction / extractor target  
Date: 2026-06-18  
Depends on:
- `detached-process-callgraph-proof-target.md`
- `callgraph-implementation-design-for-detached-process-proof.md`
- `macro-buildrs-callgraph-sequencing-survey.md`

## Thesis

Yes: for the long-term proof target, Ploke should use a compiler-grade expansion source rather than hand-implement full Rust macro expansion.

The best target is not "generic unresolved macro edges forever" and not "a hand-written Ploke macro expander for all Rust." The target should be:

```text
Ploke proof schema + BuildDomain/ExpansionBoundary
  backed by a pinned rustc extraction backend
  with cargo-expand/rust-analyzer/ploke-mbe used only as probes or partial fallbacks.
```

This avoids representation churn while still getting the full macro truth from Rust's compiler pipeline.

## Why the previous sequencing recommendation still stands

The earlier recommendation was not meant to say "avoid rustc expansion." It was meant to say: do not let raw expanded source become the canonical Ploke graph schema.

Even if we use `rustc` as the expansion engine, the proof artifact still needs first-class Ploke concepts:

- `BuildDomain`: exact Cargo target, features, target triple, profile, lockfile, rustc version, environment policy, immutable surface digest, and proof policy version.
- `ExpansionBoundary`: macro invocation, macro definition, proc-macro crate/function, build-script target, external dependency summary, and blocking/audit status.
- `ExpansionProvenance`: expansion stack, call-site span, definition span, expansion identifier, parent expansion, and hygiene/source mapping where available.
- `ResolutionState`: resolved, candidate set, ambiguous, unresolved, externally summarized, or blocked.

Those concepts remain necessary regardless of whether the expansion facts come from `rustc`, rust-analyzer, `cargo expand`, or a Ploke-local structural macro expander.

So the corrected sequence is:

```text
1. Define BuildDomain/ExpansionBoundary/proof-blocker schema.
2. Implement rustc-backed extractor into that schema.
3. Use extracted expanded/HIR facts to build call/effect/lifetime/authority graph.
```

The schema is first because it is the stable contract. The rustc backend is then the preferred producer of facts.

## Source-of-truth notes

- Rust compiler macro expansion is crate-level and produces an AST with macros expanded and modules inlined; the rustc-dev-guide names `MacroExpander::fully_expand_fragment` as the primary entry point:
  - https://rustc-dev-guide.rust-lang.org/macro-expansion.html
- `rustc_driver` / `rustc_interface` are the intended internal APIs for driving rustc as a library and running callbacks after compiler phases:
  - https://rustc-dev-guide.rust-lang.org/rustc-driver/intro.html
- Using rustc internals means `rustc_private`, which is unstable, viral in linkage, and requires rustc-dev/llvm-tools components on rustup toolchains:
  - https://doc.rust-lang.org/unstable-book/language-features/rustc-private.html
- `cargo-expand` is a wrapper around `rustc -Zunpretty=expanded` for expanded source display; useful for debugging/probes, not enough as a proof artifact:
  - https://docs.rs/crate/cargo-expand/latest/source/README.md
- rust-analyzer expands proc macros with proc-macro server processes and depends on build-script/proc-macro configuration; useful as a reference, but not a stable proof extractor API:
  - https://rust-analyzer.github.io/book/configuration.html

## Current VM facts

Live probe on this VM:

```text
rustc 1.96.0 (ac68faa20 2026-05-25)
cargo 1.96.0 (30a34c682 2026-05-25)
installed toolchain: stable-x86_64-unknown-linux-gnu
installed components: clippy, rustfmt
missing components: rustc-dev, rust-src in installed list
nightly-only `-Z` flags rejected by current rustc
cargo-expand not installed
rust-analyzer proxy exists but the rust-analyzer component/binary is not installed in the active toolchain
```

Implication: a rustc-backed extractor implementation would first need either:

- install a pinned nightly toolchain with `rustc-dev`, `llvm-tools`, and likely `rust-src`; or
- use a rustc source-tree build / rustup component setup compatible with the extractor.

This should be isolated from normal Ploke stable builds.

## Backend options

### Option 1: `cargo expand` / `rustc -Zunpretty=expanded`

Use only as a smoke-test and fixture-generation tool.

Pros:

- quickest way to see expanded source;
- good human debugging aid;
- useful for golden files in tiny fixtures.

Cons:

- requires nightly/unstable compiler flags;
- produces pretty-printed source, not stable proof facts;
- loses or degrades source/hygiene/expansion provenance;
- does not directly provide typed call resolution, candidate sets, control-flow, or process-builder dataflow;
- executing expansion still runs build scripts/proc macros under Cargo's build policy.

Verdict: useful auxiliary tool, not the canonical path.

### Option 2: rust-analyzer integration

Use as a reference implementation or secondary check, not the primary proof authority.

Pros:

- already has declarative macro expansion, proc-macro support, incremental project model, and IDE-grade navigation;
- can inform Ploke's local `ploke-mbe` subset and fixture expectations.

Cons:

- rust-analyzer is optimized for IDE semantics, not content-addressed proof artifacts;
- its macro expansion is embedded in a salsa/incremental database and project model;
- LSP commands such as macro expansion are user-facing/diagnostic, not a stable extractor contract;
- proof needs deterministic artifact identity and precise blocker reasons, not only an IDE answer.

Verdict: reference and fallback only.

### Option 3: rustc-backed extraction backend

This is the preferred long-term implementation.

Build a separate extractor binary, for example:

```text
tools/ploke-rustc-extractor/
```

or a workspace-excluded crate, because it will require nightly/rustc-private and should not break normal stable Ploke builds.

The extractor should use `rustc_driver` / `rustc_interface` callbacks and emit Ploke JSON/JSONL facts. Ploke's stable parser/transform/db stack ingests those facts into the proof schema.

Pros:

- uses the same macro expansion engine as the compiler;
- can observe expanded AST/HIR and, later, type-checking facts;
- can preserve expansion provenance via rustc spans/expansion data;
- aligns the proof artifact with the admitted compiler/toolchain identity.

Cons:

- unstable rustc internals require pinned toolchain and maintenance;
- build scripts/proc macros execute during extraction unless sandboxed or summarized;
- extractor must be isolated so normal Ploke builds remain stable-compatible;
- exact rustc APIs will change, so the Ploke-facing fact schema must be stable and small.

Verdict: canonical path.

## Proposed architecture

```text
Cargo metadata + lockfile + proof policy
  -> BuildDomain enumerator
  -> Cargo invocation / rustc-wrapper plan
  -> ploke-rustc-extractor
       -> rustc_driver callbacks
       -> expanded AST/HIR/typeck facts
       -> expansion provenance facts
       -> call/effect seed facts
  -> Ploke normalized proof IR
  -> Cozo/GraphRAG projection
  -> proof kernel / CallGraphProofArtifact
```

The extractor should be an external fact producer. It should not replace `syn_parser`; it should supplement it.

`syn_parser` remains useful for:

- stable source identity;
- existing typed node identifiers;
- module tree and cfg work;
- source/documentation graph for GraphRAG;
- fallback/partial parse diagnostics;
- joining source spans and tracking hashes to compiler facts.

The rustc extractor supplies:

- expanded items;
- macro expansion provenance;
- HIR body structure;
- call-site owners and spans;
- call resolution after type checking;
- type and trait resolution needed for candidate sets;
- process-effect seed calls after macro expansion.

## Cargo integration strategy

The safest practical route is a two-mode extractor:

### Mode A: wrapper/pass-through mode

Run Cargo with a wrapper:

```bash
RUSTC_WRAPPER=/path/to/ploke-rustc-extractor cargo check --message-format=json ...
```

or, if necessary for workspace-only wrapping:

```bash
RUSTC_WORKSPACE_WRAPPER=/path/to/ploke-rustc-extractor cargo check --message-format=json ...
```

The wrapper records exact rustc invocations and Cargo JSON messages, then either:

- delegates to real rustc for normal compilation; and/or
- performs extraction for selected package targets using the same arguments.

This is how Ploke gets the real Cargo-resolved target, cfg, feature, dependency, build-script, proc-macro, and environment context instead of trying to reconstruct rustc arguments by hand.

### Mode B: direct-driver mode

For tests and focused fixtures, call:

```bash
ploke-rustc-extractor --rustc-args-json <captured-args.json> --emit-jsonl <facts.jsonl>
```

This allows deterministic fixture tests without rerunning the whole workspace.

## Rustc callback phases to use

The extractor should emit facts in layers. Exact API names will depend on the pinned compiler, but the conceptual phases are:

1. **Crate root parsed / pre-expansion**
   - source files and root module;
   - raw macro invocations where available;
   - pre-expansion parse failures/blockers.

2. **After expansion**
   - expanded item tree;
   - expansion identifiers;
   - call-site and definition spans;
   - expansion parent stack;
   - generated item provenance;
   - cfg-expanded active source surface.

3. **After HIR lowering / analysis**
   - HIR owners/bodies;
   - call expressions;
   - method calls;
   - closure and async block bodies;
   - `Drop`/desugaring-relevant edges where available.

4. **After type checking**
   - resolved callee `DefId` for direct/inherent/method calls when available;
   - trait method and generic candidate sets;
   - type information for process-builder values;
   - dynamic dispatch blockers or bounded candidates.

## Fact schema sketch

The extractor should emit stable Ploke facts, not raw rustc internals:

```text
BuildDomainFact
  build_domain_id
  cargo_metadata_hash
  cargo_lock_hash
  package_id
  target_kind
  target_name
  target_root
  target_triple
  host_triple
  profile
  features_hash
  active_cfg_hash
  rustc_version
  rustc_commit_hash
  extractor_version
  proof_policy_version

ExpansionBoundaryFact
  boundary_id
  build_domain_id
  boundary_kind        // macro_rules invocation, proc macro derive, attr macro, build script, include!, external summary
  source_span
  expansion_state      // resolved, expanded, blocked, externally summarized
  blocking_reason?
  macro_def_id?
  proc_macro_crate_id?
  build_script_package_id?

ExpansionEdgeFact
  parent_expansion_id?
  child_expansion_id
  invocation_boundary_id
  def_span?
  call_site_span
  output_hash

ExpandedItemFact
  item_id
  build_domain_id
  expansion_id?
  def_path?
  source_span
  generated_from_boundary_id?
  item_kind
  tracking_hash

CallSiteFact
  call_site_id
  owner_item_id
  build_domain_id
  expansion_id?
  source_span
  call_syntax_kind
  callee_text?

CallResolutionFact
  call_site_id
  resolution_state
  resolved_def_id?
  candidate_def_ids[]
  external_summary_id?
  blocking_reason?

EffectSeedFact
  effect_seed_id
  call_site_id
  effect_class       // process create/configure/wait/kill/reap, async spawn, thread spawn, authority, history, surface digest
  confidence
  blocker_if_unresolved
```

The normalized proof layer should be independent of rustc's current internal type names. If rustc changes, only the extractor adapter changes.

## Macro/proc-macro/build-script safety policy

A compiler-grade extractor will run code in two places:

- `build.rs` scripts;
- procedural macro crates.

For Ploke's autonomous proof target, that matters. Extraction must therefore record and eventually control:

- which build scripts executed;
- which proc macro dylibs were loaded;
- environment variables visible to them;
- filesystem/network policy;
- outputs written to `OUT_DIR`;
- generated files included by `include!`;
- source hashes of workspace proc macro crates;
- lockfile identities of external proc macro crates.

First implementation can run in normal Cargo mode for fixtures and local development, but the proof artifact must mark external build/proc-macro execution as one of:

```text
Audited
ExternallySummarized
Sandboxed
Blocked
```

No proof should pass merely because the extractor managed to expand code.

## How this avoids churn

The churn risk is not "we used unresolved states." The churn risk is "we used unresolved states without enough shape."

If Ploke first defines `BuildDomain`, `ExpansionBoundary`, `ExpansionEdge`, `CallSite`, `CallResolution`, and `EffectSeed`, then a later rustc extractor can fill in missing facts without changing the proof model.

If Ploke instead starts from pretty-expanded source, it will likely need churn later because:

- pretty source is not a stable identity surface;
- expansion call-site/definition provenance is degraded;
- hygiene and generated spans are not first-class;
- proc-macro/build-script execution policy is outside the expanded text;
- call resolution and candidate sets still need HIR/typeck data.

Therefore: use rustc, but normalize rustc facts into Ploke's stable proof schema.

## Implementation sequence

### Phase 0: toolchain lane

- Add a workspace-excluded `tools/ploke-rustc-extractor/` crate.
- Add a local `rust-toolchain.toml` pinned to a nightly/toolchain with `rustc-dev`, `llvm-tools`, and `rust-src`.
- Keep this out of normal stable workspace builds.
- Add a script that verifies the toolchain and prints rustc commit hash.

### Phase 1: build-domain enumerator

- Use `cargo metadata` and Cargo JSON messages to enumerate package targets and build domains.
- Hash `Cargo.lock`, cargo metadata JSON, extractor binary, and proof policy.
- Emit `BuildDomainFact` JSONL.

### Phase 2: wrapper capture

- Implement pass-through `RUSTC_WRAPPER` behavior.
- Capture exact rustc arguments per crate target.
- Record build script/proc macro compilation and external package identities.
- Do not attempt semantic extraction yet.

### Phase 3: after-expansion extractor

- Run rustc driver callbacks for selected fixture crates.
- Emit `ExpansionBoundaryFact`, `ExpansionEdgeFact`, and `ExpandedItemFact`.
- Prove with fixtures that macro-generated `mod`, `use`, and `Command::new(...).status()` are visible after expansion with provenance.

### Phase 4: HIR call-site extractor

- Walk HIR bodies for call expressions, method calls, closures, async blocks, and macro-generated call sites.
- Emit `CallSiteFact` and preliminary `CallResolutionFact`.
- Mark unsupported/dynamic cases as blockers or candidate sets.

### Phase 5: process-effect seed classifier

- Classify calls to `std::process::Command`, `tokio::process::Command`, `CommandExt::exec`, `tokio::spawn`, and `thread::spawn`.
- Emit `EffectSeedFact`.
- Preserve builder-object identifiers for later dataflow.

### Phase 6: join into Ploke graph

- Add ingestion path from extractor JSONL into `ploke-transform` / `ploke-db`.
- Join compiler facts to existing `syn_parser` nodes by source span, file, target, def path, and tracking hash.
- Represent macro-generated items as generated-source nodes with expansion provenance, not as ordinary file-only nodes.

### Phase 7: proof-artifact gate

- Generate `CallGraphProofArtifact` for one narrow proof domain.
- It is acceptable if the artifact blocks on external summaries; it must block with precise reasons.

## Near-term recommendation

I would revise the previous next slice slightly:

```text
Do BuildDomain/ExpansionBoundary first,
but design it explicitly as the stable input contract for a rustc-backed extractor.
Then implement the rustc extractor as the preferred macro expansion backend,
starting with after-expansion facts before full HIR/typeck call resolution.
```

This means we are not punting on macros. We are choosing `rustc` as the long-term macro-expansion authority while preventing rustc's unstable internal shape or pretty-expanded source from becoming Ploke's proof schema.
