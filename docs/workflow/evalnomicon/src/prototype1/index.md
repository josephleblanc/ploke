# Prototype 1 Architecture

This section is the canonical Evalnomicon home for the Prototype 1 self-improvement loop.

Prototype 1 is a narrow, safety/budget-bounded loop for evaluating whether `ploke` can improve an agentic Rust-codebase harness by producing, evaluating, and selecting descendant artifacts. The architecture is not just `baseline -> patch -> rerun`; the modified artifact must hydrate a fresh child runtime, and that child runtime produces evidence from inside its own operational environment.

## Source priority

Use this priority when a draft and this section disagree:

1. Current code comments in `crates/ploke-eval/src/cli/prototype1_state/mod.rs` and `crates/ploke-eval/src/cli/prototype1_state/history.rs`.
2. Current book pages under `docs/workflow/evalnomicon/src/`.
3. Drafts under `docs/workflow/evalnomicon/drafts/`, treated as consolidation sources rather than canonical claims.
4. Research notes and papers under `research-symlink/`, treated as external support, warnings, or comparative framing.

## Pages

- [Runtime Loop](./runtime-loop.md)
- [Artifact and Runtime Model](./artifact-runtime-model.md)
- [Runtime Authority](./runtime-authority.md)
- [History and Crown](./history-crown.md)
- [Edit Surface](./edit-surface.md)
- [Selection and Evaluation](./selection-and-evaluation.md)
- [Persistence and Observability](./persistence-and-observability.md)
- [Invariant Ledger](./invariant-ledger.md)

## Status language

This section should explicitly label claims as one of:

- **Implemented:** enforced by current code.
- **Partially implemented:** current code enforces part of the claim but has named gaps.
- **Intended:** target architecture not yet enforced.
- **Not claimed:** a tempting stronger interpretation the project explicitly does not rely on.

The status labels matter because Prototype 1 mixes typed transition scaffolding, live controller paths, sealed History work, mutable scheduler projections, and older compatibility records.
