# Workspace Acceptance Review 2026-03-20

Backlinks:

- [2026-03-20_workspaces_implementation_plan.md](2026-03-20_workspaces_implementation_plan.md)
- [2026-03-20_workspaces_acceptance_criteria.md](2026-03-20_workspaces_acceptance_criteria.md)

This review responds to `(2026-03-20/4)` in
[docs/active/todo/2026-03-20_workspaces.md](../todo/2026-03-20_workspaces.md).

It also uses the following targeted code surveys:

- [indexing-state-coherence-survey.md](../agents/2026-03-workspaces/indexing-state-coherence-survey.md)
- [retrieval-scope-proof-survey.md](../agents/2026-03-workspaces/retrieval-scope-proof-survey.md)
- [save-load-update-correctness-survey.md](../agents/2026-03-workspaces/save-load-update-correctness-survey.md)

## Judgment

The phase order is logically coherent.

It already had the right monotone dependency shape:

1. readiness and fixtures
2. fixture-backed workspace snapshot coherence
3. explicit loaded-workspace state
4. manifest-driven indexing
5. per-crate status/update
6. registry-backed save/load
7. scoped retrieval
8. namespace-scoped subset DB operations
9. workspace-aware tooling

That order is valid because each later phase depends on a structure established
earlier and does not require a stronger primitive than the earlier phase is
intended to supply.

The original acceptance set was necessary but not sufficient.

It was necessary because each phase described a real prerequisite for the
stated product goals. It was not sufficient because several cross-phase
propositions were only implicit:

- successful workspace commands preserving one coherent state across TUI, DB,
  search, and IO policy
- authoritative workspace membership and explicit manifest-drift handling
- fixture-backed proof that `workspace_metadata.members` and restored
  `crate_context` rows agree
- retrieval scope being enforced before dense/BM25/hybrid candidate truncation

Without those propositions, the original document could prove components in
isolation while still allowing a globally inconsistent session state.

## Revisions made

The acceptance criteria and plan were updated to add the missing proof
obligations.

Main additions:

- global session-coherence and membership-authority propositions
- explicit Phase 1 proof of workspace snapshot coherence on committed fixtures
- Phase 3 `/index` atomicity and index-state alignment requirements
- Phase 4 manifest-drift visibility requirements
- Phase 5 registry-vs-snapshot authority and mismatch failure requirements
- Phase 6 pre-truncation scope enforcement requirements

## Necessity and sufficiency after revision

After these revisions, the acceptance structure is necessary for the defined
workspace goals and is conditionally sufficient, subject to the explicit limits
already documented in the acceptance file:

- exact global dense-ranking proofs remain out of scope because HNSW is
  approximate
- LLM disambiguation quality is not fully provable; only rejection/explicit
  targeting behavior is

Within those stated limits, the revised criteria now describe a logically valid
proof obligation for the intended workspace rollout rather than a collection of
component-local checks.

No product code was changed and no tests were run.
