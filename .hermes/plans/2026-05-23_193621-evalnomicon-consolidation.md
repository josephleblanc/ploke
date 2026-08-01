# Evalnomicon Documentation Consolidation Plan

> **For Hermes:** Use this as the planning spine before editing Evalnomicon docs. Do not bulk-move drafts. Promote content through small, reviewable changes.

**Goal:** Turn the fractured Evalnomicon drafts, code-comment architecture notes, and symlinked research corpus into a cohesive mdBook-style documentation structure for Prototype 1 and the broader evaluation framework.

**Primary source priority:**
1. Current code comments in `crates/ploke-eval/src/cli/prototype1_state/mod.rs` and `crates/ploke-eval/src/cli/prototype1_state/history.rs` for current Prototype 1 claims.
2. Current Evalnomicon book pages under `docs/workflow/evalnomicon/src/`.
3. Drafts under `docs/workflow/evalnomicon/drafts/` as material to distill, not copy wholesale.
4. Chat history / older notes as background unless still reflected in current code/docs.
5. `research-symlink/` as external support, warnings, and comparative framing.

**Key principle:** Separate authority claims, evidence/projection maps, research source cards, and historical drafts. Do not let stale drafts become canonical by relocation alone.

---

## Current Findings

### Corpus

- Book source: `docs/workflow/evalnomicon/src/`
  - 13 files: 12 markdown, 1 txt.
- Draft corpus: `docs/workflow/evalnomicon/drafts/`
  - 50 files: 49 markdown, 1 txt.
- Research corpus: `research-symlink/`
  - symlink to `/home/brasides/code/research/papers`.
  - 32 files: 28 PDFs, 3 markdown, 1 txt.

### Immediate book-spine issues

- `docs/workflow/evalnomicon/src/SUMMARY.md` links to missing:
  - `./meta-experiments/protocol-operationalization-memory.md`
- Actual file is:
  - `./meta-experiments/01-protocol-operationalization-memory.md`
- `docs/workflow/evalnomicon/src/meta-experiments/index.md` has the same broken link.
- `docs/workflow/evalnomicon/src/protocols/noms.md` exists but is not in `SUMMARY.md`.
- `docs/workflow/evalnomicon/src/meta-experiments/02-prototing-protocol.md` exists but is not in `SUMMARY.md`.
- `src/protocols/noms.md` lacks an H1.

---

## Proposed Documentation Spine

### 1. Core / Foundations

Likely files:

- `docs/workflow/evalnomicon/src/core/conceptual-framework.md`
- `docs/workflow/evalnomicon/src/core/procedure-model.md`
- `docs/workflow/evalnomicon/src/core/formal-notation.md`
- `docs/workflow/evalnomicon/src/core/runtime-artifact-model.md`
- `docs/workflow/evalnomicon/src/core/evidence-and-authority.md`

Purpose:

- OM / NOM / CM / unknown dimensions.
- Method / executor / evidence contract.
- Procedure state transitions.
- Artifact/runtime distinction.
- Evidence vs projection vs authority.

Strong sources:

- `docs/workflow/evalnomicon/src/core/conceptual-framework.md`
- `docs/workflow/evalnomicon/src/core/sources.md`
- `docs/workflow/evalnomicon/drafts/formal/procedure-notation.md`
- `docs/workflow/evalnomicon/drafts/formal/trait-first-reification.md`
- `docs/workflow/evalnomicon/drafts/formal/module-tree-and-trait-algebra.md`

### 2. Prototype 1 Architecture

New section recommended:

- `docs/workflow/evalnomicon/src/prototype1/index.md`
- `docs/workflow/evalnomicon/src/prototype1/runtime-loop.md`
- `docs/workflow/evalnomicon/src/prototype1/artifact-runtime-model.md`
- `docs/workflow/evalnomicon/src/prototype1/runtime-authority.md`
- `docs/workflow/evalnomicon/src/prototype1/history-crown.md`
- `docs/workflow/evalnomicon/src/prototype1/edit-surface.md`
- `docs/workflow/evalnomicon/src/prototype1/selection-and-evaluation.md`
- `docs/workflow/evalnomicon/src/prototype1/persistence-and-observability.md`
- `docs/workflow/evalnomicon/src/prototype1/invariant-ledger.md`

Purpose:

- Explain the actual self-improvement loop.
- Distinguish Artifact / Runtime / Tree / History / Lineage.
- Explain Parent / Child / Successor roles.
- Explain Crown authority and handoff.
- Explain child selection and why child self-report is evidence, not promotion.
- Mark implemented vs intended vs explicitly-not-claimed boundaries.

Strong sources:

- `crates/ploke-eval/src/cli/prototype1_state/mod.rs`
- `crates/ploke-eval/src/cli/prototype1_state/history.rs`
- `docs/workflow/evalnomicon/drafts/runtime/loop.md`
- `docs/workflow/evalnomicon/drafts/runtime/authority.md`
- `docs/workflow/evalnomicon/drafts/runtime/child.md`
- `docs/workflow/evalnomicon/drafts/runtime/parent-child-channel.md`
- `docs/workflow/evalnomicon/drafts/history/crown-authority-background.md`
- `docs/workflow/evalnomicon/drafts/edit-surface/model.md`
- `docs/workflow/evalnomicon/drafts/selection/README.md`

### 3. Protocols

Likely files:

- `docs/workflow/evalnomicon/src/protocols/cli-first-introspection.md`
- `docs/workflow/evalnomicon/src/protocols/noms.md`
- `docs/workflow/evalnomicon/src/protocols/eval-triage.md`
- `docs/workflow/evalnomicon/src/protocols/history-crown-audit.md`
- `docs/workflow/evalnomicon/src/protocols/runtime-playback-survey.md`
- `docs/workflow/evalnomicon/src/protocols/source-promotion.md`

Purpose:

- Repeatable human/LLM review procedures.
- NOM definition template.
- Eval triage.
- History/Crown audit.
- Runtime playback inventory procedure.
- Rules for promoting draft material into canonical docs.

Strong sources:

- `docs/workflow/evalnomicon/src/protocols/noms.md`
- `docs/workflow/evalnomicon/drafts/eval/triage-rubric.md`
- `docs/workflow/evalnomicon/drafts/observability/runtime-playback/inventory/playback-coverage-pass.md`
- Weekly audit requirements in `prototype1_state/mod.rs` and `history.rs`.

### 4. Reference / Operator Appendices

Likely files:

- `docs/workflow/evalnomicon/src/reference/persistence-map.md`
- `docs/workflow/evalnomicon/src/reference/record-surface-map.md`
- `docs/workflow/evalnomicon/src/reference/runtime-playback.md`
- `docs/workflow/evalnomicon/src/reference/operator-views.md`
- `docs/workflow/evalnomicon/src/reference/prototype1-file-inventory.md`

Purpose:

- Durable file maps.
- Join keys.
- Runtime playback surfaces.
- Projection-vs-authority warnings.
- Operator-facing evidence locations.

Strong sources:

- `docs/workflow/evalnomicon/drafts/persistence/map-2026-05-03/synthesis.md`
- `docs/workflow/evalnomicon/drafts/observability/runtime-playback/README.md`
- `docs/workflow/evalnomicon/drafts/observability/runtime-playback/agent-turn.md`
- `docs/workflow/evalnomicon/drafts/observability/runtime-playback/inventory/record-surface-map.md`
- `docs/workflow/evalnomicon/drafts/observability/run-tree-browser-design.md`

### 5. Research / Source Cards

Likely files:

- `docs/workflow/evalnomicon/src/research/index.md`
- `docs/workflow/evalnomicon/src/research/source-cards.md`
- `docs/workflow/evalnomicon/src/research/child-selection.md`
- `docs/workflow/evalnomicon/src/research/self-improvement-and-circularity.md`
- `docs/workflow/evalnomicon/src/research/typestate-and-protocols.md`
- `docs/workflow/evalnomicon/src/research/authenticated-history.md`
- `docs/workflow/evalnomicon/src/research/benchmark-adapters.md`

Purpose:

- Annotated bibliography.
- Map papers to framework questions.
- Distinguish direct design support from warnings/background.
- Track read status.

Strong sources:

- `research-symlink/ploke-child-selection-2026-05-06/README.md`
- `research-symlink/ploke-child-selection-2026-05-06/hyper-agents-ploke-notes.md`
- `research-symlink/ploke-child-selection-2026-05-06/hyper-agents.txt`
- `research-symlink/consensus/strand-rust-coder-paper.md`
- local PDF corpus under `research-symlink/`.

### 6. Experiments / Comparative Evaluation

Likely files:

- `docs/workflow/evalnomicon/src/experiments/hyperagents-gap-review.md`
- `docs/workflow/evalnomicon/src/experiments/benchmark-generalization.md`
- `docs/workflow/evalnomicon/src/experiments/selection-metrics.md`
- `docs/workflow/evalnomicon/src/experiments/eval-design.md`

Purpose:

- Benchmark-family expansion.
- HyperAgents comparison.
- Held-out/staged evaluation.
- Selection-policy experiments.
- Threats to validity.

Strong sources:

- `docs/workflow/evalnomicon/drafts/eval/hyperagents-gap-review-2026-05-17.md`
- `docs/workflow/evalnomicon/drafts/eval/ha-review-plan.md`
- `docs/workflow/evalnomicon/drafts/eval/triage-rubric.md`
- `docs/active/plans/evals/eval-design.md`

---

## Prototype 1 Invariant Ledger Seeds

Create `docs/workflow/evalnomicon/src/prototype1/invariant-ledger.md` with entries like:

1. **History is authority; scheduler/report/registry are projections.**
   - Source: `prototype1_state/mod.rs`, `prototype1_state/history.rs`.
   - Status: current local authority claim, narrow/tamper-evident, not distributed consensus.

2. **Crown is lineage authority, not pid/path/branch.**
   - Crown is the capability to mutate one active lineage.
   - One Crown per lineage, not one global singleton.

3. **At most one valid `Crown<Ruling>` per lineage.**
   - During handoff there may be zero rulers.
   - Multiple runtimes may execute; execution is not Crown authority.

4. **Successor handoff is a cross-runtime typed contract.**
   - The same in-memory object does not cross process boundaries.
   - The typed protocol and sealed evidence do.

5. **Startup admission is not just “binary ran.”**
   - Target admission shape:
     - `ProducedBy(SelfRuntime, CurrentArtifact)`
     - `AdmittedBy(CurrentArtifact, Lineage, Policy, History)`

6. **Policy-bearing surface is protected.**
   - Ordinary self-improvement must not mutate `crates/ploke-eval` authority surface until explicit protocol-upgrade transition exists.

7. **Artifact surface is partitioned.**
   - `ArtifactSurface = Immutable + Mutated + Ambient`.
   - Current concrete partition: Immutable = `crates/ploke-eval`; Mutated = tool-description text files; Ambient = empty declared surface.

8. **Artifact identity is not worktree path.**
   - Branch names and worktree paths are handles, not semantic identity.
   - Dirty worktrees are provisional, not durable graph nodes.

9. **Runtime graph is richer than git ancestry.**
   - Artifact graph, runtime derivation graph, operation graph.
   - Operation coordinate = generator Runtime + target Artifact.

10. **Child self-report is evidence, not promotion.**
    - Selection/admission requires policy and independent verification surfaces.

11. **Evaluation records must name evaluator and policy.**
    - Scores without evaluator/eval-set/policy identity are not comparable evidence.

12. **Messages are typed cross-runtime obligations.**
    - Box = lock transition + unlock transition + concrete file schema.

---

## Research Source-Card Schema

For `docs/workflow/evalnomicon/src/research/source-cards.md` or individual source-card files:

- id
- title
- local path or URL
- read status: title only / abstract skim / notes skim / partial read / full read
- source group
- framework questions answered
- key mechanism
- Prototype 1 doc targets
- OM / NOM / CM relevance
- admissible evidence implications
- safety / circularity warnings
- next extraction task

First source cards to write:

1. HyperAgents
2. SWE-Gym
3. SWE-TRACE
4. AgentPRM
5. SWE-Replay
6. STOP
7. Self-Rewarding Language Models
8. Self-Refine
9. Shepherd
10. Darwin Gödel Machine
11. Strand / trustless consensus source
12. ContextBench
13. Rust issue-resolution benchmark paper

---

## Phased Work Plan

### Phase 0: Repair navigation only

Objective: make the current book navigable before adding new conceptual load.

Tasks:

1. Fix broken link in `docs/workflow/evalnomicon/src/SUMMARY.md`:
   - from `./meta-experiments/protocol-operationalization-memory.md`
   - to `./meta-experiments/01-protocol-operationalization-memory.md`

2. Fix broken link in `docs/workflow/evalnomicon/src/meta-experiments/index.md` the same way.

3. Add `docs/workflow/evalnomicon/src/protocols/noms.md` to `SUMMARY.md`.

4. Add an H1 to `src/protocols/noms.md`, probably:
   - `# Non-Obvious Metric Protocols`

5. Decide whether to include or archive:
   - `docs/workflow/evalnomicon/src/meta-experiments/02-prototing-protocol.md`

Validation:

- Check every link in `SUMMARY.md` resolves.
- If mdBook is available, run build/check for `docs/workflow/evalnomicon`.

### Phase 1: Add structure skeleton

Objective: create destinations before moving prose.

Tasks:

1. Add new `Prototype 1` section files:
   - `src/prototype1/index.md`
   - `src/prototype1/runtime-loop.md`
   - `src/prototype1/artifact-runtime-model.md`
   - `src/prototype1/runtime-authority.md`
   - `src/prototype1/history-crown.md`
   - `src/prototype1/edit-surface.md`
   - `src/prototype1/selection-and-evaluation.md`
   - `src/prototype1/invariant-ledger.md`

2. Add new `Research` section files:
   - `src/research/index.md`
   - `src/research/source-cards.md`

3. Add entries to `src/SUMMARY.md`.

Validation:

- All new pages are reachable from `SUMMARY.md`.
- No broken local links.

### Phase 2: Populate high-leverage canonical docs

Objective: create the docs that future consolidation can point to.

Priority files:

1. `src/prototype1/invariant-ledger.md`
   - Extract from code comments in `prototype1_state/mod.rs` and `history.rs`.
   - Each invariant should include status, caveat, and code anchor.

2. `src/prototype1/runtime-loop.md`
   - Distill from `drafts/runtime/loop.md` and code comments.
   - Explain Parent -> Child -> Evaluation -> Selection -> Successor handoff.

3. `src/prototype1/history-crown.md`
   - Distill from `history.rs`, `mod.rs`, and older history drafts.
   - Mark current vs intended vs not claimed.

Validation:

- Each canonical claim has either a code anchor or explicit “intended / not implemented” status.
- Avoid copying stale draft text without status.

### Phase 3: Promote protocols and selection

Objective: connect framework to operational eval/refinement work.

Priority files:

1. `src/protocols/eval-triage.md`
   - Promote from `drafts/eval/triage-rubric.md`.

2. `src/prototype1/selection-and-evaluation.md`
   - Distill from `drafts/selection/README.md` and research source cards.

3. `src/core/evidence-and-authority.md`
   - Define evidence vs authority vs projection.
   - Tie OM/NOM/CM to admissible evidence.

Validation:

- Selection doc states hard gates, oracle evidence, process evidence, child self-report limits, and circularity warnings.
- Eval triage distinguishes Harness Invalidity / Known Frontier Limit / Action-Surface Failure.

### Phase 4: Add research source cards

Objective: make external research usable without scattering paper notes.

Priority cards:

1. HyperAgents
2. SWE-Gym
3. SWE-TRACE
4. AgentPRM
5. SWE-Replay
6. STOP
7. Darwin Gödel Machine
8. Strand / trustless consensus source

Validation:

- Each card has local path or URL.
- Each card maps to one or more doc targets.
- Each card states whether it supports design, warns against a failure mode, or is future/background.

### Phase 5: Promote reference maps

Objective: provide operator-facing evidence maps after authority docs exist.

Priority files:

1. `src/reference/persistence-map.md`
   - Distill from `drafts/persistence/map-2026-05-03/synthesis.md`.

2. `src/reference/runtime-playback.md`
   - Distill from `drafts/observability/runtime-playback/README.md`.

3. `src/reference/record-surface-map.md`
   - Distill from `drafts/observability/runtime-playback/inventory/record-surface-map.md`.

Validation:

- Each map distinguishes durable authority, evidence, projection, diagnostic telemetry, and operator convenience.

---

## Risks / Tradeoffs

1. **Stale draft promotion risk**
   - Many drafts are dated and implementation-sensitive.
   - Mitigation: distill through code anchors and status labels.

2. **Overbuilding the book structure before prose exists**
   - Too many empty pages can be noisy.
   - Mitigation: create only the sections needed for P1/P2 promotions first.

3. **Research sprawl**
   - 28 PDFs can become a bibliography sink.
   - Mitigation: source-card schema with explicit doc targets and read status.

4. **Authority/projection confusion**
   - This is the central conceptual hazard.
   - Mitigation: invariant ledger first; all persistence/reference docs must point back to it.

5. **Mixing operator docs with conceptual docs**
   - Persistence maps can overwhelm the framework.
   - Mitigation: keep operator maps in reference appendices.

---

## Suggested Immediate Next Action

Start with Phase 0 and Phase 1 only:

1. Repair current book navigation.
2. Add the minimal Prototype 1 section skeleton.
3. Add the invariant ledger page with a short initial set of code-anchored invariants.

Do not bulk-copy drafts yet.
