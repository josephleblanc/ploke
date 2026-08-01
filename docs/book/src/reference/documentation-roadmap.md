# Documentation Roadmap

This page is the working skeleton for building out the Ploke mdBook. It describes what each section should become, what code/docs must be inspected before expanding it, and what verification should happen after each write pass.

The book currently has a buildable scaffold plus imported generated-wiki content. The next phase is source-verified documentation: read the current code first, promote stable claims into the book, and keep temporary agent/run notes out of the canonical docs.

## Current baseline

- Book root: `docs/book`
- Navigation: `docs/book/src/SUMMARY.md`
- Generated HTML: `docs/book/book/` (gitignored)
- Current repo HEAD when this roadmap was written: `666477bf51bc9ce636c8cafc0230bf3d25df3656`
- Imported wiki source: `~/.hermes/wikis/ploke`
- Imported wiki snapshot: `97b1a101b9363b88b8e41f1447cb36d76a3eb8a2`, generated `2026-06-03T03:55:43Z`

The imported wiki is useful scaffolding, but it is not the final authority. Each detailed page should be refreshed against current source before being treated as canonical.

## Source-of-truth order

Use this priority when claims conflict:

1. Current source code and manifests.
2. Executable behavior and test/fixture contracts.
3. Current mdBook pages after source verification.
4. Root README and durable design docs under `docs/design`, `docs/testing`, and `docs/workflow` where applicable.
5. Generated wiki import, as a stale-but-useful scaffold.
6. Active agent notes and run reviews, only as leads for investigation, not canonical truth.

Explicit boundary: do not fold `docs/workflow/evalnomicon` into this project book. It is a separate evaluation-methodology book.

## Build-out workflow

For each section:

1. Read the existing mdBook page and its imported-wiki note.
2. Identify the source files, manifests, and tests that should anchor the page.
3. Replace or annotate imported claims with current-source claims.
4. Prefer stable architecture, responsibilities, public boundaries, and invariants over line-by-line implementation prose.
5. Add links to source paths or durable docs; avoid linking transient logs.
6. Run `mdbook build docs/book`.
7. Check `SUMMARY.md` links and markdown code-fence balance.

## Section build-out plan

### Introduction

**Current pages:**

- `docs/book/src/introduction.md`
- `docs/book/src/quick-start.md`

**Goal:** give a new reader the project purpose, release-facing entrypoint, major planes, and first useful run path.

**Explore before expanding:**

- `README.md`
- `Cargo.toml`
- `crates/ploke-tui/Cargo.toml`
- `crates/ploke-tui/src/main.rs`
- `crates/ploke-tui/src/lib.rs`
- `install.sh`

**Build-out tasks:**

- Verify the tagline and early-alpha caveat against the root README.
- Replace imported snapshot language with current-source language where needed.
- Add a concise entrypoint table: `ploke`, `ploke-eval`, `xtask`, and UI/demo binaries.
- Keep Quick Start user-facing; move contributor commands to Developer Guide.
- Add a short “what this book is / is not” note that distinguishes project docs, Evalnomicon, active agent notes, and `cargo doc`.

**Verification:**

- `cargo metadata --format-version 1 --no-deps` confirms binary target names and paths.
- `mdbook build docs/book` succeeds.

### User Guide

**Current pages:**

- `docs/book/src/user-guide/index.md`
- `docs/book/src/user-guide/first-run.md`
- `docs/book/src/user-guide/indexing.md`
- `docs/book/src/user-guide/commands-and-modes.md`

**Goal:** document the release-facing TUI workflows a user needs before reading architecture internals.

**Explore before expanding:**

- TUI command parser and help rendering under `crates/ploke-tui/src/`.
- Model and embedding picker code under `crates/ploke-tui/src/` and `crates/ploke-llm/src/`.
- Indexing command handlers and status updates.
- Root README command table.

**Build-out tasks:**

- Add a source-verified command reference page.
- Split user workflows from contributor/debug workflows.
- Add configuration basics: provider API keys, config file path, environment variables, model selection, embedding selection.
- Add a troubleshooting page for user-facing failures: missing key, slow local embeddings, indexing stuck, terminal/UI oddities.
- Add screenshots or terminal examples later only if they can be maintained.

**Verification:**

- Commands listed in the book match the current parser/help surface.
- Any provider/network workflow is labeled with required credentials and not implied to work offline.

### Architecture

**Current pages:**

- `docs/book/src/architecture/index.md`
- `docs/book/src/architecture/workspace-map.md`
- `docs/book/src/architecture/runtime-flow.md`
- `docs/book/src/architecture/code-understanding-pipeline.md`
- `docs/book/src/architecture/eval-and-projection-plane.md`
- `docs/book/src/architecture/type-diagram.md`
- `docs/book/src/architecture/sequence-diagrams.md`

**Goal:** explain the system shape: runtime plane, code-understanding pipeline, retrieval/LLM/tool loop, and eval/projection plane.

**Explore before expanding:**

- `crates/ploke-tui/src/lib.rs` for bootstrap wiring.
- `crates/ploke-tui/src/app_state/commands.rs` for state mutation boundaries.
- `crates/ingest/syn_parser/src/lib.rs` and parser phase modules.
- `crates/ingest/ploke-transform/src/lib.rs` and `src/transform/`.
- `crates/ploke-db/src/lib.rs`, schema modules, and BM25/vector search modules.
- `crates/ingest/ploke-embed/src/lib.rs`, runtime, and indexer modules.
- `crates/ploke-rag/src/lib.rs`, `core`, `fusion`, and `context` modules.
- `crates/ploke-llm/src/lib.rs` and provider/session manager modules.
- `crates/ploke-io/src/lib.rs`, actor, handle, and path policy modules.
- `crates/ploke-protocol`, `crates/ploke-records`, and `crates/ploke-tree` projection boundaries.

**Build-out tasks:**

- Refresh the system diagram against current startup and service ownership.
- Add separate flow diagrams for startup, indexing, retrieval, tool execution, and passive-record projection.
- Add an invariant ledger section or page for boundaries that must not be weakened: state command boundary, I/O actor boundary, embedding-set identity, passive-record non-authority.
- Distinguish release-facing TUI architecture from internal Prototype 1/eval architecture.
- Add “where state lives” and “where effects happen” subsections.

**Verification:**

- Every diagram node maps to a real crate/module/type.
- Mermaid fences are balanced.
- Runtime claims are checked against source, not only the generated wiki.

### Crate Guide

**Current pages:**

- `docs/book/src/crate-guide/index.md`
- group pages: runtime, ingest/indexing, retrieval/LLM, protocol/projection
- imported crate pages for ten core crates

**Goal:** help contributors find the right crate and understand its responsibility boundaries without replacing `cargo doc`.

**Explore before expanding:**

- Root `Cargo.toml` workspace members and default members.
- Each crate `Cargo.toml`.
- Each crate `src/lib.rs` or `src/main.rs`.
- Public re-exports and feature flags.
- Nearby tests that reveal crate contracts.

**Build-out tasks:**

- Add missing pages for `ploke-selection-score`, `ploke-ty-mcp`, `ploke-records`, `ploke-eval`, `ploke-egui`, `ploke-tree-browser`, `ploke-tree-egui`, `ploke-core`, `ploke-error`, `common`, `test-utils`, `xtask`, and proc-macro crates.
- Convert imported crate pages into source-verified pages one at a time.
- For each crate page, use the same structure: purpose, public entrypoints, key files, used by, uses, invariants/gotchas, tests to run.
- Add a table mapping crate groups to workspace paths.
- Mark internal/experimental crates clearly, especially `ploke-eval` and projection UI crates.

**Verification:**

- `cargo metadata --format-version 1 --no-deps` agrees with the crate list.
- Each documented public entrypoint exists in source.
- Crate pages do not over-document private implementation details likely to drift.

### Developer Guide

**Current pages:**

- `docs/book/src/developer-guide/index.md`
- `docs/book/src/developer-guide/build-and-test.md`
- `docs/book/src/developer-guide/documentation-workflow.md`

**Goal:** document repeatable contributor workflows and verification commands.

**Explore before expanding:**

- `AGENTS.md`
- scoped crate `AGENTS.md` files, especially `crates/ploke-tui/AGENTS.md`
- `xtask/README.md`
- `docs/testing/README.md`
- `docs/testing/BACKUP_DB_FIXTURES.md`
- root README install/build sections

**Build-out tasks:**

- Add a fixture workflow page.
- Add a testing matrix page: fast checks, focused TUI tests, workspace tests, live-provider tests, ignored/gated tests.
- Add backup fixture schema-coupling rules.
- Add docs contribution rules: when to update mdBook vs active agent docs vs Evalnomicon.
- Add release/install docs if they grow beyond root README.

**Verification:**

- Commands are exact and run from the stated directory.
- Live/network/provider commands are labeled with required environment variables.
- Fixture guidance matches `docs/testing/BACKUP_DB_FIXTURES.md`.

### Reference

**Current pages:**

- `docs/book/src/reference/glossary.md`
- `docs/book/src/reference/documentation-roadmap.md`

**Goal:** provide stable lookup material that supports the narrative sections.

**Explore before expanding:**

- Type names and terminology in `ploke-tui`, `ploke-db`, `ploke-rag`, `ploke-llm`, `ploke-protocol`, `ploke-records`, and `ploke-tree`.
- Existing durable design docs under `docs/design` and `docs/testing`.
- Root README command/config descriptions.

**Build-out tasks:**

- Expand the glossary with source-anchored terms: `StateCommand`, `EmbeddingSet`, `RagService`, `IoManagerHandle`, `RunForest`, `Graph`, `Procedure`, `PassiveEvidence`, BM25, HNSW, Cozo.
- Add a configuration reference page.
- Add an environment-variable reference page.
- Add a command reference page or link to the User Guide command reference.
- Add an invariant ledger page once architecture claims are source-verified.

**Verification:**

- Terms match current source names and capitalization.
- Config/env variables are sourced from code or current README, not memory.

## Exploration order

Use this order to reduce drift and avoid polishing stale pages:

1. Navigation and entrypoints: `SUMMARY.md`, `Cargo.toml`, binary targets, README.
2. Runtime bootstrap: `ploke-tui` main/lib/state command boundaries.
3. Indexing pipeline: parser, transform, DB insert, embed/indexer, BM25.
4. Retrieval/chat/tool loop: RAG, LLM session, tool validation, I/O actor.
5. Eval/projection plane: protocol, records, tree, eval CLI, egui/browser projections.
6. Contributor workflows: xtask, fixtures, backup DB docs, live-provider tests.
7. Reference polish: glossary, config/env/command tables, invariant ledger.

## Page template for source-verified crate docs

Use this shape when promoting an imported crate page:

```markdown
# `crate-name`

## Purpose

One paragraph describing the crate boundary.

## Public entrypoints

- `ItemName` — what other crates should use it for.

## Key files

- `path/to/file.rs` — why it matters.

## Uses / used by

- Uses: crate/module list.
- Used by: crate/module list.

## Invariants and gotchas

- Boundary or correctness rule to preserve.

## Verification

- Exact command(s) or tests that exercise this crate.
```

## Promotion checklist

Before considering a page source-verified:

- [ ] Current source files have been read.
- [ ] Public item names and paths are current.
- [ ] Imported-wiki note is removed or replaced with a freshness note.
- [ ] Claims are scoped to implemented behavior, not intended behavior.
- [ ] Links resolve locally or to stable source URLs.
- [ ] `mdbook build docs/book` passes.
- [ ] SUMMARY links and markdown fences pass the local sanity check.

## Explicit non-goals

- Do not fold the Evalnomicon into this book.
- Do not import active run reviews or agent handoffs as canonical documentation.
- Do not make this a substitute for generated `cargo doc` API reference.
- Do not bulk-promote old drafts without a freshness check against source.
