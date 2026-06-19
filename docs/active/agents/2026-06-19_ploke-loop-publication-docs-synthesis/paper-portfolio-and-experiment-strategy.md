# Ploke-loop paper portfolio and experiment strategy

Date: 2026-06-19
Task: `t_9c19fc7c`
Status: strategy document for a research-paper track; not a finished paper and not a results report
Audience priority: researchers first; investors second as proof-oriented/social-proof readers
Related artifacts:
- `docs/active/agents/2026-06-19_ploke-loop-publication-docs-synthesis/README.md`
- `docs/active/agents/2026-06-19_ploke-loop-publication-docs-synthesis/implementation-invariants-source-slice.md`
- `docs/active/agents/2026-06-19_ploke-loop-publication-docs-synthesis/graphrag-history-adapter-note.md`

## Executive recommendation

Lead with one arXiv-style systems/research paper rather than a product paper:

**Working title:** Authority-Carrying Runtime Succession for Self-Modifying Code Agents

**Thesis:** self-modifying code agents need a separation between code-understanding evidence and lineage authority. Ploke's current architecture is a useful case study because it already separates Rust code graph / GraphRAG evidence from Prototype 1 History/Crown authority, models parent/child/successor runtimes as distinct roles, and records several projection-vs-authority boundaries in code and docs. The publishable near-term contribution is not "Ploke proves safe self-improvement". It is a narrower architecture plus experiment plan: show how a self-improvement loop can make authority-bearing transitions explicit, identify where current code enforces the boundary, and run falsification experiments against projection leakage, child self-promotion, surface admission, and evidence provenance.

Recommended paper shape:

1. Submit as an arXiv preprint / workshop-style systems paper with explicit claim labels and negative evidence.
2. Treat investors as secondary readers: emphasize proof-oriented infrastructure and falsifiable milestones, not unearned autonomy claims.
3. Avoid claiming the full typestate proof, detached-process proof, or empirical self-improvement advantage until the missing home-machine typestate changes are pushed and the experiments below have actual results.

## Claim boundary table

| Claim | Publication status now | Source / code anchors | Evidence still required | Risk if overstated |
| --- | --- | --- | --- | --- |
| Ploke distinguishes Artifact from Runtime: changing files does not change the already-running parent binary, so descendants must be built and run before their behavior can be evaluated. | Publishable as architecture/design claim. | `docs/workflow/evalnomicon/src/prototype1/runtime-loop.md`; `docs/workflow/evalnomicon/src/prototype1/artifact-runtime-model.md`; `crates/ploke-eval/src/cli/prototype1_state/mod.rs`. | A current line-ref refresh before final paper prose; optional diagram backed by one recorded run. | Readers may interpret this as proof that all current run paths preserve the distinction. Keep it as a model plus partially implemented runtime shape. |
| Child self-evaluation is evidence, not promotion; successor authority requires parent-side selection and admission. | Publishable as a bounded architecture invariant / design rule. | `docs/workflow/evalnomicon/src/prototype1/selection-and-evaluation.md`; `docs/workflow/evalnomicon/src/prototype1/invariant-ledger.md`; `crates/ploke-eval/src/cli/prototype1_state/mod.rs`; `crates/ploke-eval/src/cli/prototype1_state/history/mod.rs`; `crates/ploke-eval/src/successor_selection/traversal.rs`. | Focused current-code audit of `cli_facing.rs`, successor handoff, and selection traversal; tests that malformed child reports cannot become selected successors. | If phrased as complete enforcement, it conflicts with transitional live-path notes and partial typestate wiring. |
| History is the durable authority surface; scheduler snapshots, branch registries, CLI reports, dashboards, side tables, and read-side records are projections/evidence, not authority. | Publishable as a central architecture claim, with status "partially implemented / local claim". | `docs/workflow/evalnomicon/src/prototype1/history-crown.md`; `docs/workflow/evalnomicon/src/prototype1/invariant-ledger.md`; `crates/ploke-eval/src/cli/prototype1_state/history/mod.rs`; `docs/book/src/architecture/eval-and-projection-plane.md`; `crates/ploke-records/src/history.rs`; `crates/ploke-tree/src/tests.rs`. | Executable projection-leakage tests; documented failure cases for attempts to advance authority from passive records. | Stronger wording could imply distributed consensus, global process uniqueness, or a proof that no external process can mutate files. Current docs explicitly deny those claims. |
| Crown is lineage-local authority, not a process id, branch, path, or global singleton. | Publishable as terminology and local model. | `docs/workflow/evalnomicon/src/prototype1/history-crown.md`; `crates/ploke-eval/src/cli/prototype1_state/history/mod.rs`; `crates/ploke-eval/src/cli/prototype1_state/parent.rs`; `crates/ploke-eval/src/cli/prototype1_state/invocation.rs`. | Current-code audit of startup, handoff, and successor admission; home-machine typestate deltas. | Do not claim a proven one-Crown theorem until typestate completion is present and checked. |
| The GraphRAG / code-understanding pipeline and Prototype 1 History can be connected by a narrow evidence-payload adapter without granting continuation authority. | Publishable as a design proposal and experiment hook, not as implemented product behavior. | `docs/book/src/architecture/code-understanding-pipeline.md`; `crates/ingest/syn_parser/src/lib.rs`; `crates/ploke-db/src/database.rs`; `crates/ingest/ploke-embed/src/runtime.rs`; `crates/ploke-rag/src/core/mod.rs`; `crates/ploke-rag/src/context/mod.rs`; `crates/ploke-tui/src/rag/context.rs`; `docs/active/agents/2026-06-19_ploke-loop-publication-docs-synthesis/graphrag-history-adapter-note.md`; `crates/ploke-eval/src/cli/prototype1_state/history/projection/mod.rs`. | Implement or prototype a read-side adapter that turns retrieval observations into History-compatible payload/evidence records; run with mock embeddings and fixed fixtures. | Overstating this as already integrated would collapse a proposed seam into a finished architecture. |
| Proof-grade call/effect facts are being separated from History authority: proof DTOs do not validate proofs, admit History, grant Crown, or execute build/proc-macro code. | Publishable as current implementation surface and proof-track direction. | `crates/ploke-records/src/proof_facts.rs`; `crates/ploke-records/src/proof_authority.rs`; `crates/ploke-db/src/proof_graph.rs`; `docs/workflow/evalnomicon/drafts/formal/detached-process-callgraph-proof-target.md`; `docs/workflow/evalnomicon/drafts/formal/callgraph-implementation-design-for-detached-process-proof.md`; `docs/workflow/evalnomicon/drafts/formal/rustc-macro-expansion-backend-plan.md`. | A proof-fact extraction demo over a small crate; blocker/provenance table; explicit macro/build-domain limitations. | The detached-process/Crown proof itself is speculative until compiler-grade extraction and proof checking exist. |
| Surface admission can enforce bounded mutation constraints before a child artifact is accepted for evaluation. | Publishable as implementation-backed mechanism if line refs are refreshed. | `crates/ploke-eval/src/cli/prototype1_state/backend/surface_admission.rs`; `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs`; `crates/ploke-eval/src/cli/prototype1_state/evidence.rs`; `crates/ploke-eval/src/cli/prototype1_state/evidence_inventory.rs`; `docs/workflow/evalnomicon/src/prototype1/edit-surface.md`. | Focused tests or a harness run covering hash mismatch, out-of-surface path, multi-file rejection, stale generator-surface version, and valid admitted patch. | Do not generalize the current concrete surface partition to a final edit model. |
| Ploke has empirical evidence that its self-improvement loop improves code quality or benchmark performance. | Not publishable yet as a claim. It is a target for experiments. | Future run records; `crates/ploke-eval`, `crates/ploke-tree`, `docs/active/agents/run-reviews/`, selection metrics surfaces. | A/B experiments with pre-registered metrics, baselines, seeds, and negative controls. | Investor-facing overclaim risk: architecture evidence is not outcome evidence. |
| Typestate transitions fully enforce Parent -> Child -> Completed child -> Selected successor -> incoming Parent in the live controller. | Blocked / speculative for this VM. | Current caveat in `crates/ploke-eval/src/cli/prototype1_state/mod.rs`; `c1.rs`-`c4.rs`; missing `crates/ploke-eval/src/cli/prototype1_state/typestate/**` noted by `implementation-invariants-source-slice.md`. | Pull or receive home-machine typestate changes; re-run source inventory and tests. | This is the most important thing not to overclaim. Current VM code says partial wiring. |

## Proposed arXiv-style paper outline

### Abstract target

Self-modifying code agents blur three boundaries that conventional code-generation evaluations often treat as external: the artifact being edited, the runtime performing edits/evaluations, and the authority that decides which descendant becomes the next parent. This paper presents Ploke Prototype 1 as a case-study architecture for making those boundaries explicit. Ploke separates Rust code-graph / GraphRAG evidence from a lineage-local History/Crown authority model, records child self-evaluation as evidence rather than promotion, and treats successor handoff as a cross-runtime contract. We describe the current implementation surfaces, identify proof obligations and unsupported stronger claims, and propose executable falsification experiments for projection leakage, surface admission, evidence provenance, and successor selection.

Do not use the abstract to claim completed safety proof or benchmark wins until the experiments have results.

### 1. Introduction

Research question:

How can a self-modifying code agent evaluate descendant artifacts without letting diagnostics, child reports, or read-side projections accidentally become authority to advance the lineage?

Motivation sources:
- Runtime succession: `docs/workflow/evalnomicon/src/prototype1/runtime-loop.md`.
- Authority boundary: `docs/workflow/evalnomicon/src/prototype1/history-crown.md` and `runtime-authority.md`.
- Code-understanding pipeline: `docs/book/src/architecture/code-understanding-pipeline.md`.

Claim boundary:
- Say "architecture and falsification plan".
- Do not say "safe autonomous self-improvement is solved".

### 2. Background and related claim boundary

Position Ploke relative to:
- SWE-style verifier/reranker and trajectory-evaluation work: useful for selection evidence, not authority. Local source list: `docs/workflow/evalnomicon/src/prototype1/selection-and-evaluation.md`.
- GraphRAG/code-retrieval systems: Ploke uses code graph facts, sparse/dense retrieval, and token-budgeted context assembly, but retrieval does not authorize successor promotion. Anchors: `docs/book/src/architecture/code-understanding-pipeline.md`, `crates/ploke-rag/src/core/mod.rs`, `crates/ploke-rag/src/context/mod.rs`, `crates/ploke-tui/src/rag/context.rs`.
- Formal/proof-oriented self-modification: detached-process/Crown proof docs are target obligations, not current proof results. Anchors: `docs/workflow/evalnomicon/drafts/formal/*` and `crates/ploke-records/src/proof_facts.rs`.

Related-claim boundaries:
- Not distributed consensus.
- Not OS process uniqueness.
- Not proof of LLM judgment correctness.
- Not a guarantee that arbitrary external processes cannot mutate files.
- Not a claim that passive JSON/log records are authoritative.

### 3. System model: artifact, runtime, lineage, History, Crown

Core diagram to include:

```text
Artifact --hydrates--> Runtime(Role, State)
Runtime --writes evidence--> Journal / Ingress
Parent<Ruling> --admits facts--> History(Block<Entry>)
History + Policy --bounds--> Lineage projection
Parent --selects--> Successor artifact/runtime
```

Supporting anchors:
- `crates/ploke-eval/src/cli/prototype1_state/mod.rs` for Artifact, Runtime, Journal, History, and design constraints.
- `crates/ploke-eval/src/cli/prototype1_state/history/mod.rs` for History/Crown definitions and local authority limitations.
- `docs/workflow/evalnomicon/src/prototype1/artifact-runtime-model.md` for public terminology.

Publishable claim:
- Ploke's model separates artifacts, executing runtimes, and lineage authority.

Speculative / future claim:
- The full model is enforced end-to-end by typestate in the live controller.

### 4. Implementation anchors

Subsections:

1. Rust code graph and retrieval evidence
   - Parse: `crates/ingest/syn_parser/src/lib.rs`.
   - Store/query: `crates/ploke-db/src/database.rs`.
   - Embeddings: `crates/ingest/ploke-embed/src/runtime.rs` and `crates/ingest/ploke-embed/src/indexer/mod.rs`.
   - RAG assembly: `crates/ploke-rag/src/core/mod.rs` and `crates/ploke-rag/src/context/mod.rs`.
   - TUI integration: `crates/ploke-tui/src/rag/context.rs`.

2. Authority and handoff
   - History/Crown: `crates/ploke-eval/src/cli/prototype1_state/history/mod.rs`.
   - Parent states: `crates/ploke-eval/src/cli/prototype1_state/parent.rs`.
   - Invocation roles: `crates/ploke-eval/src/cli/prototype1_state/invocation.rs`.
   - Successor records: `crates/ploke-eval/src/cli/prototype1_state/successor.rs`.

3. Evidence, projection, and passive records
   - Evidence grouping: `crates/ploke-eval/src/cli/prototype1_state/evidence.rs`.
   - Evidence inventory lanes: `crates/ploke-eval/src/cli/prototype1_state/evidence_inventory.rs`.
   - Passive History records: `crates/ploke-records/src/history.rs`.
   - Read-side projection: `crates/ploke-tree/src/browser.rs`, `crates/ploke-tree/src/graph/artifact_tree.rs`, and `crates/ploke-tree/src/tests.rs`.

4. Proof-track facts
   - `crates/ploke-records/src/proof_facts.rs`.
   - `crates/ploke-records/src/proof_authority.rs`.
   - `crates/ploke-db/src/proof_graph.rs`.

### 5. Main contribution: evidence-payload bridge rather than authority shortcut

Use the `graphrag-history-adapter-note.md` as the seed.

Proposed bridge:

```text
Code graph / RAG observation
  -> provenance-bearing EvidenceRef / SubjectRef / ProcedureRef payload
  -> History-compatible candidate or ingress record
  -> read-side History projection
  -> parent-side selection and continuation gate
```

Publishable claim:
- This bridge is the right architectural seam because it lets retrieval inform selection without giving retrieval the authority to advance lineage.

Implementation dependency:
- A small adapter prototype should use existing RAG outputs and History payload/projection types. It should be report-only at first.

Risk:
- If the adapter writes sealed History entries directly, it becomes an authority-surface change and needs separate review.

### 6. Experiment plan and falsification hooks

See the detailed portfolio below. The paper should pre-register hypotheses and negative controls before running experiments. The strongest near-term contribution is to show failed attempts to confuse evidence with authority are rejected or remain projection-only.

### 7. Limitations and proof obligations

Required limitation language:
- The VM checkout has partial typestate wiring; home-machine typestate changes are missing or not pushed.
- Current History is local and lineage-scoped, not distributed consensus.
- Proof-fact DTOs do not prove detached-process safety by themselves.
- Run outcomes and LLM judgments are evidence subject to policy, not authority.
- Existing docs include stale/draft material; final prose must cite refreshed current code anchors.

### 8. Roadmap

Near-term:
- Refresh current code anchors and line refs.
- Run projection-leakage, surface-admission, and GraphRAG-adapter dry-run experiments.
- Pull/review home-machine typestate changes.

Medium-term:
- Expand proof-fact extractor and proof graph tests.
- Add benchmark-style A/B loop experiments with held-out evals.
- Convert the paper into collaborator docs and a proof-spine appendix only after claims are classified.

## Executable experiment portfolio

### Experiment A: Projection-leakage negative tests

**Question:** Can passive records, read-side projections, CLI summaries, or scheduler snapshots advance History/Crown authority?

**Hypothesis:** Projection artifacts can be loaded, queried, and rendered, but cannot mutate History, admit successor authority, or apply patches without going through `ploke-eval` authority paths.

**Executable with current architecture:** yes, as unit/integration tests and/or a report-only harness.

**Current anchors:**
- `docs/book/src/architecture/eval-and-projection-plane.md`.
- `crates/ploke-records/src/history.rs`.
- `crates/ploke-tree/src/browser.rs`.
- `crates/ploke-tree/src/graph/artifact_tree.rs`.
- `crates/ploke-tree/src/tests.rs`.
- `crates/ploke-eval/src/cli/prototype1_state/history/mod.rs`.

**Implementation sketch:**
1. Construct passive record fixtures with selection/handoff-looking data.
2. Load them through `ploke-tree` projections.
3. Assert the projection exposes read-side graph nodes/edges but no API path returns `Parent<Ruling>`, `Crown<Ruling>`, sealed History append authority, or patch-apply permission.
4. Add an adversarial fixture with conflicting parent/child metadata and assert it remains diagnostic/projection data.

**Required evidence for paper:**
- Test names, fixture paths, commands, pass/fail logs.
- A short table of attempted authority escalations and observed rejection/projection-only behavior.

**Risks:**
- This proves API boundary behavior, not whole-machine safety.
- If live code has convenience shortcuts in `cli_facing.rs`, the paper must report them as gaps.

### Experiment B: Surface-admission adversarial suite

**Question:** Does the current edit-surface admission path reject stale, out-of-policy, or over-broad candidate mutations before child evaluation or successor consideration?

**Hypothesis:** Candidate edits with stale hashes, out-of-surface paths, span violations, or mismatched generator-surface versions fail admission; valid bounded edits produce checked evidence only.

**Executable with current architecture:** yes, with focused Rust tests around the admission path.

**Current anchors:**
- `crates/ploke-eval/src/cli/prototype1_state/backend/surface_admission.rs`.
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs`.
- `crates/ploke-eval/src/cli/prototype1_state/evidence.rs`.
- `crates/ploke-eval/src/cli/prototype1_state/evidence_inventory.rs`.
- `docs/workflow/evalnomicon/src/prototype1/edit-surface.md`.

**Implementation sketch:**
1. Build fixture candidate surfaces with one valid edit and adversarial variants.
2. Exercise `GitWorktreeBackend::validate_edit_surface_candidate` or the narrowest public/internal admission seam.
3. Assert exact error classes for invalid path, hash mismatch, out-of-bounds span, multi-file drift, and generator-surface mismatch.
4. Assert valid admission creates checked evidence, not authority to apply outside the admitted surface.

**Required evidence for paper:**
- Test matrix with rejection causes.
- Example admitted evidence payload with sensitive local paths redacted or normalized.

**Risks:**
- Current concrete surface partition is a Prototype 1 policy. Do not imply final generality.
- If tests require widening Rust visibility, prefer test-only module placement or report blocked rather than weakening encapsulation.

### Experiment C: GraphRAG-to-History adapter dry run

**Question:** Can code graph/RAG observations be converted into provenance-bearing History-compatible evidence payloads without granting continuation authority?

**Hypothesis:** Existing parser/DB/RAG outputs contain enough node identity, spans, hashes, namespace, scores, and snippets to form candidate/evidence payloads. Those payloads can be consumed by History projection or selection analysis as evidence, while continuation remains gated elsewhere.

**Executable with current architecture:** yes as a report-only adapter prototype using mock embeddings or a pre-indexed fixture; write-side sealed History admission should be deferred.

**Current anchors:**
- `docs/book/src/architecture/code-understanding-pipeline.md`.
- `crates/ingest/syn_parser/src/lib.rs`.
- `crates/ploke-db/src/database.rs`.
- `crates/ingest/ploke-embed/src/runtime.rs`.
- `crates/ploke-rag/src/core/mod.rs`.
- `crates/ploke-rag/src/context/mod.rs`.
- `crates/ploke-tui/src/rag/context.rs`.
- `crates/ploke-eval/src/cli/prototype1_state/history/projection/mod.rs`.
- `docs/active/agents/2026-06-19_ploke-loop-publication-docs-synthesis/graphrag-history-adapter-note.md`.

**Implementation sketch:**
1. Use a small Rust fixture workspace and deterministic/mock embeddings.
2. Query RAG for a bounded task-relevant context.
3. Normalize each returned context part into a payload carrying node id, canonical path, byte span, content hash, retrieval procedure id, retrieval scores, and query id.
4. Feed the normalized payload into a report-only selection-analysis function or serialized artifact.
5. Assert no `Continuation<Allowed>` or successor admission object is produced by the adapter.

**Required evidence for paper:**
- Fixture, query, returned node table, payload schema, and an example JSON/redacted artifact.
- Demonstration that selection/continuation authority is not available from the adapter alone.

**Risks:**
- Current RAG context assembly may have placeholder/range-normalization limits; report degraded type-context behavior honestly.
- Provider credentials must not appear in artifacts; use mock/local embeddings for paper fixtures when possible.

### Experiment D: Successor-selection evidence audit

**Question:** Are selected successors distinguishable from child self-reports, child metrics, and parent policy decisions in persisted evidence?

**Hypothesis:** A run record can be audited to show separate identities for child artifact/runtime, evaluator/policy, parent selection, and successor handoff.

**Executable with current architecture:** partly yes, depending on available run artifacts; current code and projection crates support the audit but fresh controlled runs may be needed.

**Current anchors:**
- `docs/workflow/evalnomicon/src/prototype1/selection-and-evaluation.md`.
- `crates/ploke-eval/src/cli/prototype1_state/evidence.rs`.
- `crates/ploke-eval/src/cli/prototype1_state/evidence_inventory.rs`.
- `crates/ploke-eval/src/successor_selection/traversal.rs`.
- `crates/ploke-tree/src/browser.rs`.
- `crates/ploke-tree/src/tests.rs`.

**Implementation sketch:**
1. Choose one fresh controlled Prototype 1 run or a documented run-review artifact.
2. Extract child evidence, parent selection decision, handoff record, and projection graph.
3. Produce a custody table: subject, producer, evaluator, policy, recorder, authority treatment, and projection consumer.
4. Attempt to remove one field at a time and record whether comparability or authority admission fails.

**Required evidence for paper:**
- Custody table and run artifact identifiers.
- Negative cases for missing evaluator/policy identity.

**Risks:**
- Historical run reviews may be stale, machine-local, or produced before current schema changes.
- If fresh runs depend on home-machine typestate changes, mark the experiment blocked.

### Experiment E: Proof-fact extraction and blocker ledger

**Question:** Can Ploke represent proof-relevant call/effect facts and blockers without conflating them with proof validation or runtime authority?

**Hypothesis:** Current proof DTOs and proof graph storage can hold build domains, expansion boundaries, call sites, effect seeds, authority facts, and blockers; they can support a proof-obligation ledger even before compiler-grade proof checking exists.

**Executable with current architecture:** yes for DTO/schema/storage and small extraction demos; no for the full detached-process proof.

**Current anchors:**
- `crates/ploke-records/src/proof_facts.rs`.
- `crates/ploke-records/src/proof_authority.rs`.
- `crates/ploke-db/src/proof_graph.rs`.
- `docs/workflow/evalnomicon/drafts/formal/detached-process-callgraph-proof-target.md`.
- `docs/workflow/evalnomicon/drafts/formal/callgraph-implementation-design-for-detached-process-proof.md`.
- `docs/workflow/evalnomicon/drafts/formal/rustc-macro-expansion-backend-plan.md`.

**Implementation sketch:**
1. Pick a small fixture crate with at least one explicit authority-shaped call and one macro/build boundary.
2. Emit proof facts or hand-authored DTO fixtures through the current schema.
3. Store/query them in the proof graph.
4. Record blockers for macro expansion, build script effects, external summaries, or unresolved call edges rather than erasing them.

**Required evidence for paper:**
- DTO examples, schema version, proof graph query outputs, blocker table.

**Risks:**
- This is proof-infrastructure evidence, not a proof of detached-process safety.
- Macro/build-domain completeness is explicitly future work.

### Experiment F: A/B self-improvement loop scorecard

**Question:** Does adding authority/evidence discipline improve loop reliability, candidate quality, or reviewer trust compared with an unstructured baseline?

**Hypothesis:** A disciplined loop will produce fewer authority/provenance violations, clearer failure classification, and more reviewable patches; code-quality improvement is possible but not assumed.

**Executable with current architecture:** possible after run fixture selection and typestate caveats are resolved; likely a later paper revision rather than the first preprint.

**Current anchors:**
- `crates/ploke-eval` Prototype 1 loop code.
- `docs/active/agents/run-reviews/` for run-review pattern.
- `docs/workflow/evalnomicon/src/prototype1/selection-and-evaluation.md`.
- `docs/active/agents/2026-06-02_prototype1-state-loop-walkthrough/README.md`.

**Implementation sketch:**
1. Define a fixed task suite with seeded prompts and held-out evaluator checks.
2. Compare baseline loop behavior against one authority/evidence-discipline treatment.
3. Metrics: build/test pass, patch coherence, authority/provenance violation count, selection explainability, reviewer acceptance, cost/time, and regression count.
4. Include negative controls where no acceptable child exists.

**Required evidence for paper:**
- Pre-registered suite, commands, seeds, model/provider route, logs, and scorecard.

**Risks:**
- Model/provider drift can dominate results.
- Current architecture docs support the experiment design, but results are not yet available.

## Implementation dependencies

| Dependency | Needed for | Current status / anchor | Paper treatment |
| --- | --- | --- | --- |
| Home-machine typestate changes | Strong claims about complete move-only Parent/Child/Successor transition enforcement. | Missing from this VM; `implementation-invariants-source-slice.md` notes absent `prototype1_state/typestate/**`; `mod.rs` says partial wiring. | Block strong typestate proof claims until pushed and audited. |
| Current line-ref refresh | Final paper citations and diagrams. | Parent inventories cite source files but line refs may have drifted. | Required before final prose; acceptable to omit exact line refs in strategy. |
| Controlled fixtures for RAG/History adapter | Experiment C. | Current parser/DB/RAG and History projection surfaces exist. | Use mock embeddings and small fixture to avoid credentials/network. |
| Projection and admission tests | Experiments A/B. | Existing tests and admission code exist, but dedicated paper tests need writing. | Make these first because they falsify overclaim risks. |
| Run artifact corpus | Experiment D/F. | Run-review directories exist, but may be stale or local. | Prefer fresh controlled runs; label historical evidence as anecdotal. |
| Proof extractor maturity | Experiment E and proof appendix. | DTOs/proof graph exist; full compiler-grade extraction is a target. | Publish as proof-obligation infrastructure, not proof result. |

## Recommended paper portfolio

### Main paper: Authority-Carrying Runtime Succession for Self-Modifying Code Agents

Type: arXiv / workshop systems paper.

Contribution:
- Architecture model and source-grounded implementation map.
- Claim boundary discipline for evidence vs authority.
- Experiment portfolio with at least projection-leakage, surface-admission, and adapter dry-run results.

Minimum evidence before posting:
- Source map and claim table from this document.
- Passing focused tests for at least Experiments A and B, or explicit failed tests reported as gaps.
- One report-only GraphRAG-to-History adapter artifact or a clearly scoped design-only section if not implemented.

### Short paper idea 1: Evidence Is Not Authority: Projection Leakage Tests for Agent Run Graphs

Type: focused workshop note or blog-to-paper.

Publishable core:
- Passive records and read-side graph projections should not advance control-plane authority.
- Ploke's `ploke-records` / `ploke-tree` / `ploke-eval` split provides a concrete testbed.

Needed experiments:
- Experiment A.

Why useful:
- Smaller and less blocked by typestate completion.
- Strong social-proof artifact because it shows rigorous negative testing.

### Short paper idea 2: Bridging GraphRAG Evidence into Lineage-Aware Agent Selection

Type: systems note / retrieval-for-agents workshop.

Publishable core:
- Retrieval observations can be normalized as provenance-bearing evidence for selection without becoming successor authority.

Needed experiments:
- Experiment C.
- Optional custody table from Experiment D.

Why useful:
- Connects Ploke's existing Rust code graph / RAG story to Prototype 1 without requiring full proof-track completion.

### Short paper idea 3: Proof-Fact DTOs for Detached-Process Safety Claims

Type: proof-infrastructure / formal-methods position note.

Publishable core:
- Separate proof-useful call/effect facts from proof validation and runtime authority.
- Preserve blockers explicitly rather than hiding macro/build-script uncertainty.

Needed experiments:
- Experiment E.

Why useful:
- Aligns with the evalnomicon proof-spine work while keeping speculative proof claims honest.

### Short paper idea 4: Surface Admission as a Boundary for Self-Editing Agents

Type: software-engineering reliability note.

Publishable core:
- Bounded edit-surface admission catches common self-editing failure modes before they enter evaluation or selection.

Needed experiments:
- Experiment B.
- Optional controlled loop scorecard from Experiment F.

Why useful:
- Most directly implementation-grounded and practical for reviewers.

## Suggested execution order

1. Refresh anchors and mark all claims as publishable/speculative/blocked.
2. Run Experiment A projection-leakage tests.
3. Run Experiment B surface-admission adversarial tests.
4. Build Experiment C as a report-only adapter dry run.
5. Pull/review home-machine typestate changes before strengthening any transition-proof language.
6. Add Experiment D custody audit from fresh controlled run artifacts.
7. Add Experiment E proof-fact/blocker demo.
8. Decide whether Experiment F belongs in the first preprint or a follow-up.

## Red lines for paper prose

Do not write:
- "Ploke proves safe self-improvement."
- "The Crown guarantees only one process can mutate the repository."
- "GraphRAG retrieval determines successor authority."
- "Child self-evaluation proves the child is better."
- "The typestate transition is fully enforced in the current VM checkout."
- "Proof facts prove detached-process safety."

Safer wording:
- "Ploke models lineage authority separately from code-understanding evidence."
- "Current code and docs support a local, lineage-scoped History/Crown claim with named gaps."
- "Projection artifacts are evidence or views; they are not intended to grant authority."
- "The proposed adapter converts retrieval observations into evidence payloads and leaves continuation authority behind the existing selection/admission gate."
- "The detached-process proof spine identifies proof obligations and implementation blockers."

## Open blockers and caveats

- Home-machine typestate changes are missing from this VM. This blocks any final claim that the typed transition path is complete.
- Current live controller code still includes transitional surfaces such as `cli_facing.rs`; it must be audited before final line-cited paper claims.
- Some older docs are stale-but-useful. Prefer `docs/workflow/evalnomicon/src/prototype1/*`, current code comments, and the 2026-06-19 inventories before citing older drafts.
- Git/RAG/model/provider experiments must record commands, seeds, provider routes, and fixture state. Do not invent results.
- Credential-bearing paths and environment values must not appear in paper artifacts.

## Bottom line

The strongest near-term paper is a rigorous architecture-and-experiment strategy paper: Ploke as a source-grounded case study in keeping evidence, projections, and lineage authority separate in a self-modifying code-agent loop. The credible novelty is the explicit authority boundary and falsification plan, not an already-complete proof or benchmark win. The first executable milestones should be projection-leakage tests, surface-admission adversarial tests, and a report-only GraphRAG-to-History evidence adapter.
