# ploke-selection-score

Small, pure scoring helpers for comparing Ploke selector mechanisms against source-backed paper mechanisms in the Obsidian formal scoring note.

Source authority:

`/home/brasides/wiki/queries/research/ploke-arxiv-cs-ai-2026-06-02/synthesis/formal-scoring-mechanisms.md`

## Layout

- `src/common/` — numeric helpers shared by mechanisms: ranking, normalization, rates, finite selection, vector math.
- `src/ploke/` — current Ploke selector pieces: evidence gates, frontier weighting, oracle/IMP helpers, and current-selector anchors.
- `src/papers/` — source-backed equation modules from the exact/scope-limited mechanism section.
- `src/protocols/` — formal decision logic and benchmark wrappers that are not full scalar objectives.
- `src/catalog/` — static metadata preserving paper ids, exactness labels, kinds, and line anchors.

Unresolved or candidate-only mechanisms belong in `catalog` as `MechanismKind::Unresolved` until the vault note has source-visible equations or predicates.
