# History and Crown

History and Crown define the local authority boundary for Prototype 1. They do not make the whole execution environment trustworthy, and they do not claim distributed consensus.

## History

History is the durable authority surface for admitted lineage facts:

```text
History     = authenticated store over sealed lineage-local blocks
Lineage     = policy-governed projection over admitted Artifact continuity
Block       = one authority epoch for one lineage
Entry       = provenance-bearing fact admitted inside an epoch
Ingress     = append-only late/backchannel observation outside a sealed epoch
Projection  = disposable view or index derived from History or evidence
```

Scheduler snapshots, branch registries, CLI reports, preview aggregates, dashboards, and database side tables are projections or evidence sources. They do not become History authority by being read.

## Crown

The Crown is the one-at-a-time authority to mutate an active lineage. It is not:

- a process id
- a git branch
- a filesystem path
- a global singleton

For one lineage, at most one valid typestate carrier may hold `Crown<Ruling>`. During handoff there may be zero rulers: the predecessor has locked and retired, while the successor has not yet validated into the next ruling parent.

## Handoff boundary

The intended handoff shape is:

```text
Parent<Ruling>
  -> installs selected Artifact into the active checkout
  -> locks succession authority for the selected next runtime
  -> seals/appends History material
  -> launches successor runtime
  -> waits for successor acknowledgement
  -> exits as Parent

successor runtime
  -> validates active checkout and sealed predecessor evidence
  -> unlocks succession authority
  -> becomes Parent<Ruling>
```

This is a cross-runtime typed contract. The same in-memory Crown object does not cross the process boundary; sealed evidence plus the shared compiled protocol align the predecessor and successor.

## Current claim boundary

Current code comments describe the claim as local and narrow: tamper-evident, lineage-scoped, transition-checked History. Prototype 1 does not currently claim:

- distributed consensus
- global process uniqueness
- proof of LLM judgment correctness
- automatic authority for legacy JSON records
- automatic admission of policy-surface mutations

## Canonical sources

- `crates/ploke-eval/src/cli/prototype1_state/history.rs`
- `crates/ploke-eval/src/cli/prototype1_state/mod.rs`
- Draft source: `docs/workflow/evalnomicon/drafts/history/crown-authority-background.md` is background and should be checked against `history.rs` before promotion.
