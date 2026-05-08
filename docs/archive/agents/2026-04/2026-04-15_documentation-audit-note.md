# Documentation Audit Note

- date: 2026-04-15
- task title: documentation audit note
- task description: compact note capturing current README, folder-index, and doc-comment findings for later user review
- related planning files: `docs/active/agents/2026-04-15_orchestration-hygiene-and-artifact-monitor.md`, `docs/active/agents/2026-04-15_docs-hygiene-tracker.md`

## Highest-Signal Findings

1. `syn_parser` has at least one real behavior/doc mismatch.
   - [crates/ingest/syn_parser/README.md](/home/brasides/code/ploke/crates/ingest/syn_parser/README.md:40)
   - [crates/ingest/syn_parser/src/lib.rs](/home/brasides/code/ploke/crates/ingest/syn_parser/src/lib.rs:3)
   - [crates/ingest/syn_parser/src/discovery/mod.rs](/home/brasides/code/ploke/crates/ingest/syn_parser/src/discovery/mod.rs:92)
   - Current docs describe recursive discovery as `src/`-only, while implementation also scans `tests/`, `examples/`, and `benches` when enabled.
2. README coverage across crate roots is uneven.
   - Packages with README coverage include `ploke-db`, `ploke-eval`, `ploke-io`, `ploke-rag`, `ploke-tui`, and the ingest crates.
   - Missing/unclear crate-root README coverage is concentrated in helper/core/proc-macro packages such as `ploke-core`, `ploke-common`, `ploke-error`, `ploke-llm`, `ploke-protocol`, `ploke-ty-mcp`, and `ploke-test-utils`.
3. Folder-index quality is strongest where a root README already states purpose + entry points + authority rules.
   - [docs/workflow/README.md](/home/brasides/code/ploke/docs/workflow/README.md:1)
   - [docs/active/workflow/README.md](/home/brasides/code/ploke/docs/active/workflow/README.md:1)
   - These are the current best local model for future docs-folder TOC policy.

## Fixed During This Pass

- corrected the stale `open_questions.md` pointer in [docs/active/agents/readme.md](/home/brasides/code/ploke/docs/active/agents/readme.md:23)
- corrected the malformed known-limitations link in [README.md](/home/brasides/code/ploke/README.md:24)
- expanded [docs/workflow/README.md](/home/brasides/code/ploke/docs/workflow/README.md:11) and [docs/active/workflow/handoffs/README.md](/home/brasides/code/ploke/docs/active/workflow/handoffs/README.md:11) so they index more of their own folder contents

## Recommended Follow-Up

- treat `syn_parser` doc comments and README drift as a dedicated review item rather than casual cleanup
- track crate-root README coverage as an explicit inventory instead of assuming every crate should receive one immediately
- keep using one canonical README per durable docs folder, with short retrieval-oriented descriptions instead of prose-heavy summaries
