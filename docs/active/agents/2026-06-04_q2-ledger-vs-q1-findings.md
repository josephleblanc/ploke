---
title: Q2 ledger comparison against Q1 findings
created: 2026-06-04
updated: 2026-06-04
type: agent-report
tags: [ploke, scoring, ledger-qa, q1-q2]
sources:
  - /home/brasides/wiki/queries/ploke/selection-scoring/shards/Q1-mini-qa.md
  - /home/brasides/wiki/queries/ploke/selection-scoring/ledgers/mechanism-ledger.md
  - /home/brasides/wiki/queries/ploke/selection-scoring/ledgers/benchmark-ledger.md
  - /home/brasides/wiki/queries/ploke/selection-scoring/ledgers/model-prompt-ledger.md
  - /home/brasides/wiki/queries/ploke/selection-scoring/ledgers/repo-ledger.md
  - /home/brasides/wiki/queries/ploke/selection-scoring/ledgers/citation-ledger.md
  - /home/brasides/wiki/queries/ploke/selection-scoring/ledgers/equation-benchmark-map.md
confidence: medium
contested: false
---

# Q2 ledger comparison against Q1 findings

## Scope

Compared the Q1 accepted QA findings in `/home/brasides/wiki/queries/ploke/selection-scoring/shards/Q1-mini-qa.md` against the six canonical Q2 ledgers in `/home/brasides/wiki/queries/ploke/selection-scoring/ledgers/`:

- `mechanism-ledger.md`
- `benchmark-ledger.md`
- `model-prompt-ledger.md`
- `repo-ledger.md`
- `citation-ledger.md`
- `equation-benchmark-map.md`

Q1 says the repaired M1-M8 shards were acceptable for fan-in, with no remaining broken paths/schema blockers, and with three non-blocking fan-in caveats: keep M1 inventory separate from canonical mechanism promotion, consider more precise wording for the HyperAgents repo/code row, and preserve/add `confidence` / `record_status` on M8 equation-benchmark-model edges. Sources: `Q1-mini-qa.md:39-43`, `Q1-mini-qa.md:73-80`, `Q1-mini-qa.md:84-93`, `Q1-mini-qa.md:104`.

## Verdict

The current Q2 ledgers are **not consistent** with the accepted Q1 shard outputs. The original Q1 repair blockers appear repaired, and the M1 / M3 / M8 caveats are partly handled. However, the canonical ledgers are an incomplete fan-in: large portions of the reviewed M2, M3, M4, M7, and M8 records are absent, and the equation map now references mechanism and benchmark ids that are not defined in the corresponding ledgers.

Automated id comparison run from `/home/brasides/code/ploke` found:

| Category | Reviewed shard/source records | Q2 ledger records | Missing from Q2 | Notes |
| --- | ---: | ---: | ---: | --- |
| Mechanism rows | 38 | 6 | 32 | Missing most M2 formal mechanisms, all M7 mechanism rows, and six M8 mechanisms. |
| Benchmark-like rows | 32 | 5 | 27 | Six of the 27 are M3 metric-surface rows intentionally left shard-side; still 21 benchmark/benchmark-substrate rows are absent. |
| Model/prompt/tool rows | 25 | 9 | 16 | All M4 model/prompt/tool records are absent; Q2 has only M7 + M8 rows. |
| Repo/code rows | 13 | 5 | 8 | M5 + M7 are present; all M8 repo/code rows are absent. |
| Citation edges | 12 | 9 | 3 | M6 + M7 are present; M8 citation/baseline edges are absent. |
| Equation-benchmark-model edges | 5 | 3 | 2 | M8 edges are present with shared fields; M7 HyperAgents edges are absent. |

## Finding-by-finding comparison

### 1. Q1 blocking repairs are not the main remaining issue

- Q1 source: the previous blockers were M1/M5 broken source paths plus M2/M4 schema gaps; the later Q1 note says these were repaired (`Q1-mini-qa.md:82-87`) and the final verdict says there is no remaining Q1 blocking repair list (`Q1-mini-qa.md:39-43`).
- Q2 evidence: current ledger scan found none of the old bad strings (`ploke-arxiv-cs-ai-2026-06-03`, `model-prompt-tool-ledger`, bad `2606.01435__don't...` path, or wrong June-03 `2606.01444` path). Q2 ledgers also have required frontmatter plus table columns for `confidence` / `record_status` where applicable (`mechanism-ledger.md:1-17,35-42`; `model-prompt-ledger.md:1-17,21-31`; `repo-ledger.md:21-27`).
- Status: **consistent / repaired**.
- Recommendation: no repair needed for the original Q1 broken-path/schema blockers; do not regress these normalized anchors and shared fields.

### 2. M1 inventory caveat is handled correctly

- Q1 source: M1 is inventory-shaped and should not be mechanically promoted as canonical mechanism rows (`Q1-mini-qa.md:73`, `Q1-mini-qa.md:91`).
- Q2 evidence: `mechanism-ledger.md` keeps M1-like material under `### Inventory-derived seeds` and explicitly says M1 rows remain source inventory, not canonical mechanism promotion (`mechanism-ledger.md:21-32`, `mechanism-ledger.md:44-48`).
- Status: **consistent**.
- Recommendation: keep this split. Do not treat the M1 source-inventory table as a fully canonical mechanism ledger without a separate interpretation pass.

### 3. Mechanism ledger is a partial fan-in and omits 32 reviewed mechanism rows

- Q1/source evidence: Q1 accepted M2 formal mechanisms, M7 HyperAgents, and M8 nearby mechanisms as reviewed (`Q1-mini-qa.md:74`, `Q1-mini-qa.md:79-80`). The reviewed sources contain:
  - M2 formal mechanism rows `M2-0001` through `M2-0025` (`M2-formal-mechanisms.md:22-46`).
  - M7 HyperAgents mechanism rows `dgm-h-parent-selection`, `dgm-h-archive-admission`, `dgm-h-staged-evaluation`, `dgm-h-cross-domain-average`, and `dgm-h-modifiable-parent-selection` (`M7-hyperagents.md:35-103`).
  - M8 nearby mechanism rows including SkillDAG, EvoTrainer, DeltaMem, Overlaying Governance, StepFinder, Handoff Debt, SAGE, and Reasoning Primitive Induction (`M8-june03-nearby.md:38-163`).
- Q2 evidence: `mechanism-ledger.md` currently has only six formal/nearby rows: `M2-0001`, `M2-0002`, `M2-0003`, `M2-0021`, `2606.03056-skilldag-typed-skill-graph`, and `2606.03108-evotrainer-diagnostic-branch-admission` (`mechanism-ledger.md:35-42`).
- Mismatch: missing M2 rows `M2-0004` through `M2-0020` and `M2-0022` through `M2-0025`; all five M7 `dgm-h-*` mechanism rows; and M8 rows for DeltaMem, Overlaying Governance, StepFinder, Handoff Debt, SAGE, and Reasoning Primitive Induction.
- Status: **blocking omission**.
- Recommendation: backfill the missing mechanism rows from the reviewed shards into `mechanism-ledger.md`, preserving source anchors, `confidence`, and `record_status`. If some reviewed rows are intentionally excluded, add an explicit exclusion/deferral note keyed by source id so the omission is visible and non-accidental.

### 4. Benchmark ledger omits reviewed benchmark records and leaves dangling equation-map benchmark ids

- Q1/source evidence: Q1 accepted M3 benchmark separation, M7 HyperAgents benchmark/reporting records, and M8 nearby benchmark rows (`Q1-mini-qa.md:75`, `Q1-mini-qa.md:79-80`). Relevant source ranges:
  - M3 benchmark substrates from RoleCDE through MCP-Persona (`M3-benchmarks.md:32-287`).
  - M7 benchmark rows Polyglot, Paper Review, Robotics Reward Design, IMO Grading, and `imp-at-k` (`M7-hyperagents.md:105-180`).
  - M8 benchmark rows Handoff Debt, Who&When, RuleArena NBA, MuSR team allocation, and NatPlan meeting planning (`M8-june03-nearby.md:165-250`).
- Q2 evidence: `benchmark-ledger.md` has only five rows: Executable interactive games, ForeSci, VAIR, AGENTCL, and MCP-Persona (`benchmark-ledger.md:21-28`). Its fan-in note intentionally leaves M3 metric-surface rows on the shard side (`benchmark-ledger.md:29-32`).
- Mismatch: besides the intentionally shard-side M3 metric surfaces, Q2 still omits 11 M3 benchmark substrates, all five M7 benchmark/reporting rows, and all five M8 benchmark rows. This creates dangling references because `equation-benchmark-map.md` points at `handoff-debt-interrupted-coding-takeover`, `who-and-when`, and `rulearena-nba` (`equation-benchmark-map.md:23-25`), but those benchmark ids are not defined in `benchmark-ledger.md`.
- Status: **blocking omission / cross-ledger inconsistency**.
- Recommendation: backfill the missing non-metric-surface benchmark records into `benchmark-ledger.md`. Keep the M3 metric-surface rows shard-side only if final consumers are explicitly told to read `M3-benchmarks.md`; otherwise create a separate metric-surface ledger or add a clear cross-reference. At minimum, add the M8 benchmark ids referenced by `equation-benchmark-map.md`.

### 5. Model/prompt/tool ledger omits all M4 reviewed rows

- Q1/source evidence: Q1 says M4 is reviewed after repair and now has required frontmatter/sections plus per-record `confidence` / `record_status`; prompt anchors are either line-ranged or explicitly `not specified` (`Q1-mini-qa.md:76`). M4 contains 16 model/prompt/tool rows across task agents, critics/judges/evaluators, and routers/harnesses/safety gates (`M4-model-prompts-tools.md:24-238`).
- Q2 evidence: `model-prompt-ledger.md` has nine rows: HyperAgents plus the eight M8 nearby papers (`model-prompt-ledger.md:21-31`).
- Mismatch: the ledger omits every M4 row, including the rows for 2606.00103, 2606.00384, 2606.00476, 2606.00642, 2606.01435, 2606.02355, 2606.02470, 2606.02484, 2606.02109, 2606.02488, 2606.02373, 2606.01991, and 2606.01314. Several papers have multiple role-specific M4 rows, so a table keyed only by `paper_id` may be insufficient.
- Status: **blocking omission**.
- Recommendation: backfill M4 records into `model-prompt-ledger.md`. If the canonical table must stay one row per `paper_id`, add a role/subrecord key or split rows by `model_role` so duplicate paper ids such as 2606.00384, 2606.00476, and 2606.01435 are not collapsed.

### 6. Repo/code ledger preserves M5 + M7 but misses M8 repo rows; HyperAgents status wording caveat remains

- Q1/source evidence: Q1 says M5 repo rows are reviewed and conservative, and M7's HyperAgents repo row has a paper-stated URL while `code_status: paper-only`; Q1 suggests more precise canonical wording if the repo ledger distinguishes paper-stated URL from verified code (`Q1-mini-qa.md:77`, `Q1-mini-qa.md:79`, `Q1-mini-qa.md:92`). M8 has eight repo/code rows with `repo_url: not specified` / `code_status: unknown` and source anchors (`M8-june03-nearby.md:382-494`).
- Q2 evidence: `repo-ledger.md` includes the M5 rows plus HyperAgents (`repo-ledger.md:21-27`) and has a fan-in note to keep paper-stated availability separate from verified URLs (`repo-ledger.md:29-31`). The HyperAgents row still combines a concrete GitHub URL with `code_status: paper-only` (`repo-ledger.md:27`).
- Mismatch: all eight M8 repo/code rows are absent. The HyperAgents row still has the slightly awkward Q1 caveat state, although the notes clarify that the repo URL is paper-stated and not visited.
- Status: **partial omission + unresolved wording caveat**.
- Recommendation: either backfill the M8 repo/code rows or add an explicit policy note saying `unknown/not specified` repo rows are intentionally excluded from the canonical repo ledger. For HyperAgents, change `code_status` to a more precise value such as `paper-stated URL; not visited` or add a separate `verification_status` field while preserving the repo URL and source anchors.

### 7. Citation ledger has M6 + M7, but omits three M8 edges

- Q1/source evidence: Q1 says M6 citation/baseline edges are conservative and anchored (`Q1-mini-qa.md:78`), and M8 contains citation/baseline edges for Reasoning Primitive Induction -> SkillDAG, StepFinder -> Who&When, and Handoff Debt -> repository-only takeover (`M8-june03-nearby.md:496-528`).
- Q2 evidence: `citation-ledger.md` includes the eight M6 edges plus the M7 HyperAgents -> Darwin Gödel Machine baseline edge (`citation-ledger.md:21-31`).
- Mismatch: M8's three citation/baseline edges are absent.
- Status: **omission**.
- Recommendation: add the three M8 edges to `citation-ledger.md`, preserving relation types and evidence anchors, or explicitly mark them as deferred if the canonical citation ledger is meant to exclude local-baseline/benchmark-reuse edges.

### 8. Equation-benchmark-model map satisfies the M8 shared-field caveat but omits M7 edges and references undefined ids

- Q1/source evidence: Q1 caveat says M8 equation-benchmark-model edges should add or preserve `confidence` / `record_status` if the canonical edge ledger requires them (`Q1-mini-qa.md:80`, `Q1-mini-qa.md:93`). M7 contains two equation-benchmark-model edges for cross-domain average and `imp-at-k` (`M7-hyperagents.md:232-252`); M8 contains three edges (`M8-june03-nearby.md:531-560`).
- Q2 evidence: `equation-benchmark-map.md` has the three M8 edges with `confidence` and `record_status` columns (`equation-benchmark-map.md:21-25`).
- Mismatch: the two M7 edges are absent. Also, the three present M8 edges reference mechanism ids and benchmark ids that are missing from `mechanism-ledger.md` and `benchmark-ledger.md` (see Findings 3 and 4).
- Status: **partly resolved, partly blocking**.
- Recommendation: add the two M7 edges, keeping `imp-at-k` explicitly `reports only`, and backfill the corresponding mechanism and benchmark definitions so the equation map does not point to undefined ledger ids.

## Consolidated repair list

1. Re-run fan-in over reviewed shards M2, M3, M4, M7, and M8 into the canonical ledgers, using Q1's accepted repaired shards as source of truth.
2. Backfill missing `mechanism-ledger.md` rows: M2-0004..M2-0020, M2-0022..M2-0025, all five M7 `dgm-h-*` mechanisms, and the six omitted M8 mechanisms.
3. Backfill `benchmark-ledger.md` with missing non-metric-surface M3 benchmark substrates, all M7 benchmark/reporting rows, and all M8 benchmark rows. Keep M3 metric surfaces shard-side only with an explicit cross-reference or separate metric-surface ledger.
4. Backfill `model-prompt-ledger.md` with all M4 rows; avoid collapsing multiple role-specific records under the same `paper_id`.
5. Decide repo-ledger policy for `not specified`/`unknown` M8 repo rows; either add them or document their intentional exclusion. Normalize HyperAgents `code_status` wording.
6. Add the three M8 citation/baseline edges to `citation-ledger.md`.
7. Add the two M7 equation-benchmark-model edges to `equation-benchmark-map.md` and ensure every edge's mechanism and benchmark ids are defined in their corresponding ledgers.
8. After repair, rerun id-presence, path-anchor, wikilink, and MathJax checks across all six ledgers.
