# ploke-selection-score

Small, pure scoring helpers for comparing Ploke selector mechanisms against source-backed paper mechanisms in the Obsidian formal scoring note.

Source authority:

`/home/brasides/wiki/queries/research/ploke-arxiv-cs-ai-2026-06-02/synthesis/formal-scoring-mechanisms.md`

## Layout

- `src/common/` — numeric helpers shared by mechanisms: ranking, normalization, rates, finite selection, vector math.
- `src/ploke/` — current Ploke selector pieces: evidence gates, frontier weighting, oracle/IMP helpers, and current-selector anchors.
- `src/papers/` — source-backed equation modules from the exact/scope-limited mechanism section, including HyperAgents/DGM-H archive parent-selection helpers.
- `src/protocols/` — formal decision logic and benchmark wrappers that are not full scalar objectives.
- `src/catalog/` — static metadata preserving paper ids, exactness labels, kinds, and line anchors.

Unresolved or candidate-only mechanisms belong in `catalog` as `MechanismKind::Unresolved` until the vault note has source-visible equations or predicates.

## Nearby candidate policy

The June-03 nearby mechanisms tracked from `queries/ploke/selection-scoring/ledgers/mechanism-ledger.md` are catalog metadata only for now. SkillDAG, DELTAMEM, StepFinder, Handoff Debt, and reasoning-primitive induction have useful admission/metric shapes, but the current E1 review leaves them blocked on primary-source line anchors, thresholds, objectives, clustering criteria, or span-score definitions. Do not add executable helpers for those families until a later source pass promotes a formula with an exactness label and tests.
