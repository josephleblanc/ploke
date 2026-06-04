# Plan: scoring-mechanism comparison and Kanban indexing campaign

Created: 2026-06-04 03:58:41
Workspace: `/home/brasides/code/ploke`
Plan mode: no board or wiki mutation has been performed by this plan.

## Goal

Create a durable, even-ground comparison pipeline for Ploke-relevant scoring mechanisms. The pipeline should first use low-cost mini/spark-style workers for mechanical indexing, then use high-reasoning workers for synthesis questions about why equations appear effective, what benchmarks/models they are tied to, and what can transfer into Ploke's successor-selection mechanism.

The first pass should index:

- paper-internal equations and scoring mechanisms;
- benchmarks, tasks, metrics, and train/validation/test splits;
- models used, model roles, judge roles, prompts, appendices, and tool/sandbox access;
- open-source repo/code links and artifact availability;
- citation/reference overlap between papers;
- equation-to-benchmark-to-model links.

The second pass should answer higher-level questions, including:

- how comparable the mechanisms are once benchmark, model, and task-surface context is normalized;
- whether papers reference the same benchmarks, each other, or the same baselines;
- how much freedom agents had to modify code/prompts/tools/evaluators;
- how complex the worked-over codebases/repos were, especially for HyperAgents/DGM-H and similar systems;
- which scoring terms, gates, or exploration pressures are plausible Ploke design deltas.

## Current context and assumptions

### Existing relevant artifacts

Primary Ploke selection-scoring packet:

- `/home/brasides/wiki/queries/ploke/selection-scoring/index.md`
- `/home/brasides/wiki/queries/ploke/selection-scoring/current-mechanism.md`
- `/home/brasides/wiki/queries/ploke/selection-scoring/archive-parent-selection-mechanisms.md`

Existing research-campaign scoring corpus:

- `/home/brasides/wiki/queries/research/ploke-arxiv-cs-ai-2026-06-02/synthesis/formal-scoring-mechanisms.md`
- `/home/brasides/wiki/queries/research/ploke-arxiv-cs-ai-2026-06-02/synthesis/formal-scoring-equation-audit.md`
- `/home/brasides/wiki/queries/research/ploke-arxiv-cs-ai-2026-06-02/synthesis/scoring-mechanisms.md`
- `/home/brasides/wiki/queries/research/ploke-arxiv-cs-ai-2026-06-02/synthesis/symbolic-theses.md`
- `/home/brasides/wiki/queries/research/ploke-arxiv-cs-ai-2026-06-02/texts/*.txt`
- `/home/brasides/wiki/queries/research/ploke-arxiv-cs-ai-2026-06-02/papers/*.md`
- `/home/brasides/wiki/queries/research/ploke-arxiv-cs-ai-2026-06-02/deep-reads/*.md`
- `/home/brasides/wiki/queries/research/ploke-arxiv-cs-ai-2026-06-02/new-papers/2026-06-03/papers/*.md`

Local HyperAgents source and prior local notes:

- `/home/brasides/code/ploke/.agents/hyper-agents.txt`
- `/home/brasides/code/ploke/.agents/prototype1-hyperagents-handoff-2026-05-06.md`
- `/home/brasides/wiki/queries/source-ledger.md`

### Profile discovery

Read-only `hermes profile list` showed these relevant profiles:

- `researcher-mini` — `gpt-5.4-mini`; use for bounded extraction/indexing.
- `writer-mini` — `gpt-5.4-mini`; use for formatting and fan-in table drafting from reviewed shards.
- `reviewer-mini` — `gpt-5.4-mini`; use for checklist QA, schema coverage, link/source-anchor checks.
- `watcher-spark` — `gpt-5.3-codex-spark`; use only for cheap watchdog/status checks unless a source-extraction spark profile is created.
- `researcher-high` — `gpt-5.5`; use for second-pass reasoning and deep comparisons.
- `reviewer-high` — `gpt-5.5`; use for high-level review gates.
- `orchestrator-high` — `gpt-5.5`; use for board supervision if avoiding default/xhigh routes.

Assumption: do not assign this campaign to `researcher` or `reviewer` unless the user explicitly approves xhigh/default spend. Treat model tier as assignee routing, not as prose instructions to workers.

If the user specifically wants extraction workers on `gpt-5.3-mini`/spark rather than `gpt-5.4-mini`, create/preflight a dedicated `researcher-spark` profile before launching extraction cards. Existing `watcher-spark` is not obviously configured as a source-extraction researcher.

## Source-of-truth priority

Use this order when sources disagree or repeat information:

1. Paper text or local extracted text with line anchors, e.g. `texts/<arxiv>.txt` or `.agents/hyper-agents.txt`.
2. Existing paper/deep-read notes with explicit source anchors.
3. Formal scoring synthesis/audit notes for already-reviewed equation exactness labels.
4. Abstract-only/new-paper notes for routing and candidate inclusion only.
5. Web search/GitHub checks only for repo/code availability, citation metadata, and missing public links.

Workers must use `unknown`, `not found`, or `not specified` when a field is absent. Do not infer benchmark/model/prompt details from paper reputation or from neighboring works.

## Proposed documentation spine

Create a comparison packet under:

`/home/brasides/wiki/queries/ploke/selection-scoring/`

Recommended new files/directories:

```text
queries/ploke/selection-scoring/
  comparison-index.md
  WORKER_CONTRACT.md
  templates/
    mechanism-record-template.md
    benchmark-record-template.md
    model-prompt-record-template.md
    repo-record-template.md
    citation-edge-template.md
  shards/
    README.md
    <task-id>-<slug>.md
  ledgers/
    mechanism-ledger.md
    benchmark-ledger.md
    model-prompt-ledger.md
    repo-ledger.md
    citation-ledger.md
    equation-benchmark-map.md
  high-pass/
    codebase-action-surface-audit.md
    prompts-model-roles-audit.md
    equation-effectiveness-hypotheses.md
    ploke-selection-design-deltas.md
    final-synthesis.md
```

Root/hub integration:

- Add `comparison-index.md` to `/home/brasides/wiki/queries/ploke/selection-scoring/index.md` once the scaffold exists.
- Update root `/home/brasides/wiki/index.md` only when the comparison index or final synthesis becomes a durable entry point.
- Append `/home/brasides/wiki/log.md` for scaffold creation, fan-in integration, and final synthesis.

Concurrency rule: extraction workers write only to assigned shard files under `shards/`. They must not edit canonical ledgers, root index, or `log.md`. A fan-in card merges reviewed shard outputs into canonical ledgers sequentially.

## Record schemas

### Mechanism record

Fields:

- `mechanism_id`: stable slug, e.g. `2606.02488-raser-cost-aware-routing`.
- `paper_id`: arXiv ID or local source slug.
- `paper_title`.
- `mechanism_family`: archive/parent-selection, admission/verifier, cost-aware routing, credit assignment, trajectory localization, reward/objective, benchmark metric, evaluator/judge, skill/archive update, other.
- `equation_or_rule`: short plain description; include MathJax only when source-visible or already reviewed.
- `exactness_label`: exact theorem/definition, faithful formalization, interpretive compression, predicate formalization, not safely formalizable.
- `score_target`: what receives score/selection pressure: agent, branch, answer, route, skill, trace step, candidate artifact, evaluator, dataset item.
- `selection_effect`: admission, ranking, sampling probability, training loss, evaluation-only, reporting-only.
- `source_anchor`: path plus line range.
- `related_ploke_axis`: operational score, protocol delta, oracle axis, IMP@K, child-count exploration, archive admission, cost routing, verifier gate, prompt/tool policy.
- `confidence`: high/medium/low.

### Benchmark record

Fields:

- `benchmark_id`: normalized slug.
- `benchmark_name` and aliases.
- `domain`: coding, paper review, robotics, math grading, QA, enterprise reasoning, tool-use, agent trajectory, etc.
- `task_unit`: problem, repo issue, paper, robot episode, trace span, route query, etc.
- `metric_names`: accuracy, pass@k, success rate, MSE, reward, F1, cost, ELPD-LOO, etc.
- `split_usage`: train/validation/test/heldout/final-only.
- `staged_gate`: yes/no/unknown; source anchor.
- `ground_truth_or_judge`: human labels, programmatic verifier, AI judge, simulator, hidden tests, rubric.
- `papers_using_it`: paper ids.
- `source_anchor`.

### Model/prompt/tool record

Fields:

- `paper_id`.
- `model_role`: task agent, meta agent, judge, critic, evaluator, generator, baseline, router, reward designer.
- `model_name`: exact model string if available.
- `model_state`: frozen, finetuned, prompted, LoRA/adaptor, unknown.
- `tool_access`: shell, file edit, browser, simulator, code execution, API, none, unknown.
- `modification_scope`: prompt-only, source code, tool definitions, evaluator/rubric, training harness, model weights, memory/skills, unknown.
- `prompt_or_appendix_anchor`: path plus line/page/appendix reference.
- `safety_or_sandbox`: described constraints, if any.
- `cost_or_budget`: explicit cost/token/iteration budget if stated.

### Repo/code record

Fields:

- `paper_id`.
- `repo_url`.
- `code_status`: open, unavailable, paper-only, artifact-only, dead link, unknown.
- `license`: if visible.
- `commit_or_release`: if specified.
- `artifact_scope`: benchmark code, system implementation, prompts, data, notebooks, simulator, evaluation harness.
- `verification_method`: paper text only, GitHub visited, URL checked, package/release checked.
- `source_anchor`.

### Citation edge

Fields:

- `source_paper_id`.
- `target_paper_id_or_title`.
- `target_in_corpus`: yes/no/unknown.
- `relation_type`: cites, baseline, benchmark reuse, method extension, critique, same dataset, same repo, same model, related work only.
- `evidence_anchor`.
- `notes`.

### Equation-benchmark-model edge

Fields:

- `mechanism_id`.
- `benchmark_id`.
- `metric_name`.
- `model_roles`.
- `paper_claim`: improves, evaluates, trains, filters, selects, reports only.
- `evidence_anchor`.
- `transfer_question`: what this edge suggests for Ploke.

## Kanban board proposal

Board slug:

`scoring-mechanism-comparison-20260604`

Board description:

`Index paper scoring mechanisms against benchmarks, models, prompts, code links, citations, and Ploke successor-selection design hooks.`

Default workdir:

`/home/brasides/code/ploke`

Creation rule:

- Create board and root cards parked/blocked first.
- Wire parent dependencies in topological order.
- Verify graph state before unblocking the first roots.
- Use explicit `--board scoring-mechanism-comparison-20260604` on every Kanban CLI call.
- Prefer direct visible Kanban CLI calls. If bulk creation is scripted, show the script first and parse CLI JSON key `id`, not `task_id`.

## Kanban graph preview

### Phase 0 — scaffold and contract

| Card | Assignee | Parents | Output |
| --- | --- | --- | --- |
| `S0 scaffold comparison packet and worker contract` | `orchestrator-high` | none | Creates docs spine, templates, shard README, and exact worker contract. |
| `S1 mini profile preflight and routing check` | `watcher-spark` or `reviewer-mini` | none | Confirms profiles can spawn; flags whether a `researcher-spark` profile is needed. |
| `S2 scaffold review` | `reviewer-mini` | S0, S1 | Checks paths, templates, source list, no concurrent-write hazards. |

Acceptance for S0:

- `comparison-index.md` exists with frontmatter and links to current/archival/formal scoring docs.
- `WORKER_CONTRACT.md` contains schemas above and source-anchor rules.
- `templates/` and `shards/README.md` exist.
- Existing folder index is updated only after scaffold is ready.

### Phase 1 — low-cost mechanical indexing

Use `researcher-mini` for extraction cards unless a dedicated `researcher-spark` profile is created and preflighted.

| Card | Assignee | Parents | Scope | Output shard |
| --- | --- | --- | --- | --- |
| `M1 corpus inventory and mechanism seed list` | `researcher-mini` | S2 | Enumerate initial corpus from formal scoring docs, current Ploke docs, HyperAgents, top-8/deep-read/new-paper notes. | `shards/M1-corpus-inventory.md` |
| `M2 equation/mechanism extraction from formal scoring docs` | `researcher-mini` | S2 | Extract mechanism records from `formal-scoring-mechanisms.md` and equation audit. | `shards/M2-formal-mechanisms.md` |
| `M3 benchmark and metric ledger extraction` | `researcher-mini` | S2 | Extract benchmark names, metrics, splits, staged gates, ground truth/judges. | `shards/M3-benchmarks.md` |
| `M4 model, prompt, and tool-access extraction` | `researcher-mini` | S2 | Extract model names/roles, prompt appendices, tool access, sandbox/free-reign clues. | `shards/M4-model-prompts-tools.md` |
| `M5 repo and code availability extraction` | `researcher-mini` | S2 | Extract code URLs from papers/texts; verify obvious GitHub links if web access available. | `shards/M5-repos.md` |
| `M6 citation and shared-baseline edge extraction` | `researcher-mini` | S2 | Parse references/related-work notes for in-corpus citations, shared benchmarks, shared baselines. | `shards/M6-citations.md` |
| `M7 HyperAgents focused mechanical index` | `researcher-mini` | S2 | Extract DGM-H benchmark/model/prompt/tool/codebase/action-surface anchors from `.agents/hyper-agents.txt`. | `shards/M7-hyperagents.md` |
| `M8 June-03 promoted papers nearby mechanisms` | `researcher-mini` | S2 | Extract the same fields from EvoTrainer, SkillDAG, SAGE, DELTAMEM, Handoff Debt, StepFinder, etc. | `shards/M8-june03-nearby.md` |
| `Q1 shard schema/source-anchor QA` | `reviewer-mini` | M1-M8 | Checklist review of all shard files; no deep synthesis. | `shards/Q1-mini-qa.md` |
| `F1 merge reviewed shards into canonical ledgers` | `writer-mini` | Q1 | Merge records into `ledgers/*.md`; do not add new interpretation. | canonical ledgers |
| `Q2 ledger consistency and wikilink QA` | `reviewer-mini` | F1 | Link check, duplicate benchmark aliases, missing anchors, unknown fields. | QA note and repair list |

Initial corpus default for M1:

- all mechanisms already listed in `formal-scoring-mechanisms.md`;
- all candidates in `formal-scoring-equation-audit.md` that mention scoring/evaluation/mechanism formulas;
- Ploke `current-mechanism.md` and `archive-parent-selection-mechanisms.md`;
- HyperAgents local text;
- top-8 deep reads and 2026-06-03 promoted nearby mechanism notes.

Do not expand to all 173 papers until the pilot ledger proves the schema handles missing/implicit scoring mechanisms.

### Phase 2 — high-reasoning comparison pass

Use `researcher-high` and `reviewer-high`, not default/xhigh `researcher`/`reviewer`.

| Card | Assignee | Parents | Output |
| --- | --- | --- | --- |
| `H1 benchmark/model overlap synthesis` | `researcher-high` | Q2 | `high-pass/benchmark-model-overlap.md`: shared benchmarks, shared model families, common baselines, citation adjacency. |
| `H2 codebase and action-surface audit` | `researcher-high` | Q2 | `high-pass/codebase-action-surface-audit.md`: codebase complexity, task repo scale, modification scope, free-reign/tool/sandbox matrix. |
| `H3 prompt and appendix comparative audit` | `researcher-high` | Q2 | `high-pass/prompts-model-roles-audit.md`: prompt appendices, model role separation, judge prompts, meta-agent instructions. |
| `H4 equation-effectiveness hypotheses` | `researcher-high` | H1, H2, H3 | `high-pass/equation-effectiveness-hypotheses.md`: why equation families may work under their benchmark/model conditions. |
| `H5 Ploke selection design deltas` | `researcher-high` | H4 | `high-pass/ploke-selection-design-deltas.md`: candidate Ploke score/gate changes, with no-code proposal boundaries. |
| `R1 high-pass review gate` | `reviewer-high` | H1-H5 | Reviews source grounding, no overclaiming, no xhigh/default routing drift. |
| `F2 final comparison synthesis and feedback packet` | `writer-mini` | R1 | `high-pass/final-synthesis.md` plus optional feedback-inbox card if user wants quick KEEP/ADAPT/DROP routing. |
| `R2 final review and wiki assimilation check` | `reviewer-high` | F2 | Final signoff, root index/log update verification, unresolved-link report. |

High-pass question set:

1. Which mechanisms are actually comparable once score target, benchmark denominator, model role, and selection effect are normalized?
2. Which equations are benchmark/reporting metrics only, versus mechanisms that actively drive training, routing, archive admission, or parent selection?
3. Which papers share benchmark substrates, baselines, model families, or code artifacts?
4. Which papers cite each other directly, and which only share background families such as DGM, DGM-H, ADAS, SkillDAG-like skill graphs, or verifier/routing papers?
5. In HyperAgents and nearby systems, what was the mutable object: prompt, source code, task agent, meta agent, evaluator, selection rule, tool harness, memory, or model weights?
6. How constrained was the model: tools, sandbox, budget, fixed prompts, fixed parent selector, fixed evaluator, validation-only selection, human gates?
7. What scoring components could Ploke adopt as observability-only metrics first, before making them selector-driving?
8. Which components would weaken Ploke invariants if made score-bearing without explicit review?

## Worker-contract highlights

Every extraction card should include these instructions inline or by reference to `WORKER_CONTRACT.md`:

- Load/use `llm-wiki`, `obsidian`, and `research-pipeline` skills if visible to the profile; otherwise follow the inline contract.
- Read assigned source files only; do not broadly search unless the card explicitly asks for repo/link verification.
- Write only the assigned shard file under `shards/`.
- Every non-empty row needs a source anchor: file path plus line range or paper note section.
- Use `unknown`, `not specified`, or `not found` rather than guessing.
- Distinguish project triage score from paper-internal scoring mechanism.
- Distinguish benchmark metric from score-driving selector/reward/training objective.
- Do not convert prose-only mechanisms into equations.
- If a source seems important but ambiguous, add it to `needs_high_review` with the exact anchor and question.
- Mini workers should not change root `index.md`, `log.md`, canonical ledgers, or other workers' shards.

## Validation and health checks

After scaffold:

- `comparison-index.md`, `WORKER_CONTRACT.md`, templates, and shard README exist.
- Selection-scoring folder index links resolve.
- No raw source files edited.

After Phase 1:

- Each shard file exists and has frontmatter or a standard header.
- No shard has rows without source anchors unless the row explicitly says `not found`.
- Canonical ledgers contain no duplicate benchmark ids without alias resolution.
- `equation-benchmark-map.md` has only anchored edges.
- Model/prompt ledger distinguishes task agents, meta agents, judges, critics, routers, and baselines.
- Repo ledger separates paper-stated code links from actually verified URLs.
- Citation ledger marks `target_in_corpus` and relation type.

After Phase 2:

- High-pass synthesis cites canonical ledgers and line-anchored sources, not only worker summaries.
- Design deltas are proposal-only and separate observability metrics from selector-driving changes.
- Any proposed weakening of Ploke invariants is labeled as a risk and routed to user decision, not silently recommended.
- Root wiki index/log are updated only after final reviewed artifacts exist.
- Link check: no unresolved `[[wikilinks]]` in new hub/ledger/high-pass docs.
- MathJax balance check for any document with equations.

## Risks and mitigations

| Risk | Mitigation |
| --- | --- |
| Mini workers hallucinate missing details. | Require source anchors for every row; reviewer-mini rejects unanchored rows; high workers only synthesize reviewed ledgers. |
| Multiple workers edit same ledger concurrently. | Workers write shards only; single fan-in card merges canonical ledgers. |
| Citation/reference parsing creates weak relation claims. | Edge type must distinguish `related work only` from baseline/benchmark/method extension. |
| Benchmark names are duplicated under aliases. | Benchmark ledger has normalized id plus aliases; Q2 resolves duplicates before high pass. |
| Repo links are stale or paper-only. | Repo ledger has `verification_method`; high pass does not trust paper-stated availability as verified implementation. |
| Equations are compared without benchmark/model context. | Equation-benchmark-model edge table is the central fan-in artifact; high synthesis must cite it. |
| HyperAgents gets overinterpreted from one local text. | Dedicated M7 extracts anchors; H2/H3 must separate paper text, repo link, and unverified code behavior. |
| Board roots race before graph wiring. | Create roots parked/blocked, wire dependencies, verify graph, then unblock intended roots. |
| User wanted gpt-5.3-mini spark but only watcher-spark exists. | Either use `researcher-mini` gpt-5.4-mini for extraction or create/preflight `researcher-spark` before board launch. |
| High pass drifts into xhigh/default profiles. | Assign only `researcher-high`, `reviewer-high`, `orchestrator-high`; audit non-done assignees before dispatch. |

## Open questions with defaults

1. Initial corpus size?
   - Default: pilot on existing formal-scoring mechanisms + HyperAgents + top-8/Junes-03 nearby mechanism notes, then decide whether to expand to all 173 papers.

2. Should repo URLs be verified by web/GitHub in Phase 1?
   - Default: yes for URLs explicitly present in paper text/notes, but no broad repo hunting unless a paper claims code availability without a link.

3. Should spark profile be created for extraction?
   - Default: use existing `researcher-mini` unless the user explicitly wants a new `researcher-spark` profile.

4. Should final outputs be a single consolidated synthesis or a hub-plus-ledgers packet?
   - Default: hub-plus-ledgers packet; final synthesis links to ledgers instead of duplicating every row.

5. Should a watchdog be created?
   - Default: only after the board exists and the user approves unattended progress. Use `watcher-spark` for cheap status ticks if appropriate.

## Suggested kickoff sequence after plan approval

1. Confirm whether to use `researcher-mini` or create a `researcher-spark` extraction profile.
2. Create the wiki scaffold and `WORKER_CONTRACT.md`.
3. Create board `scoring-mechanism-comparison-20260604` with explicit default workdir `/home/brasides/code/ploke`.
4. Create S0/S1/S2 and Phase 1 extraction cards in topological order; keep roots parked until graph is verified.
5. Run/preflight the first pilot extraction batch.
6. Review pilot shards before expanding to all candidate papers.
7. Merge reviewed ledgers.
8. Launch high-pass cards with `researcher-high` and `reviewer-high` only.
9. Produce final synthesis and optional feedback-inbox card.
10. Update root wiki index/log and run link/MathJax checks.

## Non-goals

- Do not edit Ploke source code in this campaign.
- Do not alter the current selection scoring implementation.
- Do not make any new mechanism score-driving in Ploke without a separate design/review gate.
- Do not ingest or rewrite raw source bodies under `/home/brasides/wiki/raw/`.
- Do not re-summarize every paper abstract; index only fields needed for comparison and later synthesis.
