# Consolidated Ploke-loop source inventory index

Date: 2026-06-19
Task: `t_ff11df4b`
Status: citeable first-pass index for paper, mdBook, and symbolic-proof synthesis; not final prose and not a proof.
Repo root for relative paths: `/home/team_ploke_dev/code/ploke`

## Methodology

This index merges the three source-inventory slices produced by child tasks:

- `t_319c2be8` documentation/run-note slice, originally written to `/home/team_ploke_dev/.hermes/kanban/boards/ploke/workspaces/t_319c2be8/ploke-loop-doc-source-inventory-slice.md`.
- `t_34283835` implementation-invariants slice, committed at `docs/active/agents/2026-06-19_ploke-loop-publication-docs-synthesis/implementation-invariants-source-slice.md` in commit `43ee5718`.
- `t_db9bbe4b` GraphRAG/embeddings/TUI/config slice, originally written to `/home/team_ploke_dev/.hermes/kanban/boards/ploke/workspaces/t_db9bbe4b/graphrag-embeddings-tui-config-source-inventory.md`.

The two scratch-workspace artifacts were no longer present at their original filesystem paths by consolidation time. Their full written contents were recovered from the local session database using `session_search` on the exact artifact filenames, and the committed implementation slice was read directly from the repository. This avoids silently discarding sources from completed child slices while preserving the caveat that the two workspace files themselves are not durable repo artifacts.

Additional checks performed during consolidation:

- `kanban_show(t_ff11df4b)`, `kanban_show(t_319c2be8)`, `kanban_show(t_34283835)`, `kanban_show(t_db9bbe4b)`, `kanban_show(t_6df5f941)`, and `kanban_show(t_914c9f5b)` for task scope, parent handoffs, and review dependency.
- `read_file` on `AGENTS.md`, `docs/active/agents/readme.md`, the existing Ploke-loop README, and `implementation-invariants-source-slice.md`.
- `session_search` for the two scratch artifact filenames to recover exact child-slice source rows.
- `search_files` spot-check for the run-review cluster `docs/active/agents/run-reviews/2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-*`.
- `git status --short --branch` before writing to avoid staging unrelated concurrent worker changes.

Classification values are normalized to exactly these six labels: `canonical/current`, `stale-but-useful`, `implementation-grounded`, `speculative`, `duplicate`, and `missing due to home-machine typestate changes not yet pushed`.

## Citation policy

1. For claims about current behavior, cite `canonical/current` prose only when backed by `implementation-grounded` code/test/doc anchors.
2. For proof claims, cite `canonical/current` term pages and `implementation-grounded` History/Crown/proof-fact code, while keeping `speculative` proof targets explicitly in future-work/proof-obligation language.
3. For collaborator mdBook prose, use `stale-but-useful` pages as scaffolds, not authority, unless they are refreshed against current code.
4. Treat `duplicate` entries as relationship maps: useful for drift analysis and wording, but not the preferred citation target.
5. Treat every typestate completion claim depending on absent home-machine work as `missing due to home-machine typestate changes not yet pushed` until the relevant code/docs are pushed and inspected.

## Gap and conflict summary

- The high-priority absent source is `crates/ploke-eval/src/cli/prototype1_state/typestate/**`. The parent cards and child slices repeatedly warn that final typestate transitions may be complete on the user's home machine but not pushed to this VM.
- Current VM `prototype1_state` code has authority, parent, child, successor, channel, C1-C4, and History state carriers, but `mod.rs`, `c2.rs`, `c3.rs`, and related notes show partial live wiring. Do not write final typestate prose as if the current VM contains the complete design.
- Runtime-loop, runtime-authority, History/Crown, edit-surface, and parent-child-channel narratives exist in multiple generations. Prefer concise `docs/workflow/evalnomicon/src/prototype1/*` pages plus current code; mine older drafts only for rationale and vocabulary.
- GraphRAG/RAG docs describe the intended sparse+dense+fusion+context flow, but the strongest current behavior anchors are `ploke-rag`, `ploke-db`, `ploke-embed`, and TUI command/config sources.
- The TUI command/config surface is in migration: current code still has broad `StateCommand` handling while docs/comments describe a grouped validated command target.
- Embedding hot-swap has an implementation/config divergence: `EmbeddingRuntime::activate` supports active-set swapping and DB relation updates, while TUI model-load UX guidance recommends restart for embedding backend changes.
- Cozo vectors must be documented precisely: schema uses `<F32; dims>` vector columns, and tests should expect Cozo vector values rather than plain lists.
- Active run reviews and operator logs are evidence for specific runs, not global architecture authority.

## High-priority missing or unpushed typestate-dependent sources

| Path or source | Classification | Why it matters | Downstream relevance |
| --- | --- | --- | --- |
| `crates/ploke-eval/src/cli/prototype1_state/typestate/**` | missing due to home-machine typestate changes not yet pushed | Parent cards and child slices expect a typestate directory or equivalent final transition code, but it is absent on this VM. | Blocks final typestate, successor handoff, and symbolic-proof prose. |
| Final pushed docs/code for C1-C5 transition wiring beyond current VM partial state | missing due to home-machine typestate changes not yet pushed | Current `c2.rs`, `c3.rs`, and `mod.rs` caveats show pushed VM code is not final. | Blocks any claim that the VM enforces full runtime-succession typestate. |
| Home-machine evidence for successor handoff / incoming parent acknowledgement | missing due to home-machine typestate changes not yet pushed | Current successor records exist, but final authority handoff may depend on unpushed changes. | Needed for proof-track handoff propositions. |
| Refreshed line anchors after home-machine typestate merge | missing due to home-machine typestate changes not yet pushed | Several child slices contain approximate line/module anchors from current VM state. | Needed before publication-grade citations. |

## Source index by topic

All paths are relative to `/home/team_ploke_dev/code/ploke` unless an absolute path is shown.

### 1. Meta inventory inputs and source-of-truth guidance

| Source | Anchor | Classification | Annotation | Downstream relevance |
| --- | --- | --- | --- | --- |
| `docs/active/agents/2026-06-19_ploke-loop-publication-docs-synthesis/README.md` | `Ploke-loop publication/docs/proof synthesis source inventory`; authority legend; duplicate/conflict sections | duplicate | Initial repo source map for this synthesis thread. It is superseded for citation-index purposes by this consolidated index, but remains useful as the first committed inventory and companion-artifact list. | Paper/mdBook/proof source-map provenance. |
| `docs/active/agents/2026-06-19_ploke-loop-publication-docs-synthesis/implementation-invariants-source-slice.md` | full slice from task `t_34283835` | duplicate | Implementation-grounded child slice merged here. Keep as audit trail and fallback if a row in this consolidated index needs its original rationale. | Reviewer and downstream audit provenance. |
| `/home/team_ploke_dev/.hermes/kanban/boards/ploke/workspaces/t_319c2be8/ploke-loop-doc-source-inventory-slice.md` | recovered from session transcript; original workspace artifact absent | duplicate | Documentation/run-note child slice merged here. The file path is retained because it was the child deliverable path, but the scratch artifact is no longer present on disk. | Provenance only; cite repo paths below instead. |
| `/home/team_ploke_dev/.hermes/kanban/boards/ploke/workspaces/t_db9bbe4b/graphrag-embeddings-tui-config-source-inventory.md` | recovered from session transcript; original workspace artifact absent | duplicate | GraphRAG/embeddings/TUI/config child slice merged here. The scratch artifact path is retained for auditability, but it is not a durable repo doc. | Provenance only; cite repo paths below instead. |
| `docs/book/src/reference/documentation-roadmap.md` | `Source-of-truth order`; `Build-out workflow`; `Section build-out plan` | canonical/current | Current mdBook maintenance/source-order policy. It ranks current source/tests above mdBook pages and generated wiki imports, and says active agent notes are leads rather than canonical truth. | Citation hygiene and mdBook refresh policy. |
| `docs/active/agents/readme.md` | `Status guide`; `Conventions`; June 19 Ploke-loop entry | canonical/current | Active-agent doc index and status guide. It identifies the Ploke-loop synthesis directory as the initial source inventory/authority map and warns how to treat active plans/reviews. | Shared doc navigation and status semantics. |

### 2. Evalnomicon canonical Prototype 1 narrative

| Source | Anchor | Classification | Annotation | Downstream relevance |
| --- | --- | --- | --- | --- |
| `docs/workflow/evalnomicon/src/prototype1/index.md` | `# Prototype 1`; source priority | canonical/current | Entry point for current Prototype 1 Evalnomicon chapter and source-priority policy. | Starting citation for Prototype 1 docs. |
| `docs/workflow/evalnomicon/src/prototype1/artifact-runtime-model.md` | `Core terms`; `Three graph views`; `Identity cautions` | canonical/current | Defines Artifact, Runtime, Tree, Lineage, and why worktree/process/branch identities are not enough. | Paper glossary, mdBook architecture, proof term setup. |
| `docs/workflow/evalnomicon/src/prototype1/history-crown.md` | `History`; `Crown`; `Handoff boundary`; `Current claim boundary` | canonical/current | Best short prose source for History as local lineage authority and Crown as bounded authority; explicitly excludes global process/path/branch/consensus claims. | Authority and limitations prose. |
| `docs/workflow/evalnomicon/src/prototype1/invariant-ledger.md` | status labels; History/Crown/successor/surface/child-self-report rows | canonical/current | Compact invariant inventory with implemented/partially implemented/intended/not-claimed distinctions. | Symbolic-proof roadmap and claim filtering. |
| `docs/workflow/evalnomicon/src/prototype1/runtime-authority.md` | `Authority shape`; role/state/lineage bounded authority | canonical/current | Concise current model of authority tokens, mutable surfaces, and transition records. | Collaborator explanation of authority boundaries. |
| `docs/workflow/evalnomicon/src/prototype1/runtime-loop.md` | `Current loop shape`; `Runtime roles`; `Why this is not a flat eval loop` | canonical/current | Best short narrative for parent -> child -> successor runtime succession and the artifact/runtime distinction. | Paper introduction and mdBook runtime chapter. |
| `docs/workflow/evalnomicon/src/prototype1/persistence-and-observability.md` | `Evidence families`; `Projection warning`; `Message boxes` | canonical/current | Clear warning that scheduler/report/registry/database/log surfaces are evidence or projections, not History authority. | Observability and proof-boundary prose. |
| `docs/workflow/evalnomicon/src/prototype1/selection-and-evaluation.md` | `Selection boundary`; `First-pass selection shape` | canonical/current | Defines child evidence versus parent-side successor selection and bounds traversal/scoring claims. | Evaluation/paper claims and successor selection docs. |
| `docs/workflow/evalnomicon/src/prototype1/edit-surface.md` | `# Edit Surface`; surface partition | canonical/current | Short current source for bounded edit surface and immutable/mutable/policy-bearing areas. | Edit-surface mdBook/proof section. |

### 3. Evalnomicon drafts, formal targets, and duplicates

| Source | Anchor | Classification | Annotation | Downstream relevance |
| --- | --- | --- | --- | --- |
| `docs/workflow/evalnomicon/drafts/runtime/authority.md` | `Purpose`; `Vocabulary`; `Core Invariants`; `Role Surfaces` | stale-but-useful | Richer authority draft with `ExecutionPath<Role, State> + Surface<Role> + Transport` vocabulary. Subordinate to `src/prototype1/runtime-authority.md` and current code. | Rationale and examples for proof/mdBook writers. |
| `docs/workflow/evalnomicon/drafts/runtime/loop.md` | `Revised Loop Shape`; `History And Journal`; `Current Implementation Status` | stale-but-useful | Long-form V2 loop draft. Useful motivation but includes old absolute paths and partially superseded implementation status. | Background for runtime succession chapter. |
| `docs/workflow/evalnomicon/drafts/runtime/loop-v1-historical.md` | historical V1 runtime-loop draft | duplicate | Historical predecessor to `drafts/runtime/loop.md` and `src/prototype1/runtime-loop.md`. | History/drift only. |
| `docs/workflow/evalnomicon/drafts/runtime/parent-child-channel.md` | `Current Shape`; `Parent To Child`; `Child To Parent`; `Intended Channel Model` | stale-but-useful | Older filesystem/channel contract; reconcile with current `prototype1_state/channel.rs` before citing implementation claims. | Channel rationale and drift map. |
| `docs/workflow/evalnomicon/drafts/runtime/artifact-runtime-lineage.md` | artifact/runtime lineage draft | speculative | Early artifact/runtime graph terminology source. Use only as conceptual lead. | Background for lineage diagrams. |
| `docs/workflow/evalnomicon/drafts/runtime/child.md` | child runtime role draft | stale-but-useful | Child role draft found in runtime directory; verify against current `child.rs` and C1-C4 paths. | Background for child role docs. |
| `docs/workflow/evalnomicon/drafts/formal/README.md` | `Formal Drafts` | canonical/current | Directory index for formal proof/callgraph material. | Proof-track navigation. |
| `docs/workflow/evalnomicon/drafts/formal/detached-process-callgraph-proof-target.md` | `Purpose`; `Source anchors`; `Terms`; `Successor exception` | speculative | Best proof-target statement for detached-process and Crown safety; not implementation proof. | Future-work/proof-obligation citations. |
| `docs/workflow/evalnomicon/drafts/formal/callgraph-implementation-design-for-detached-process-proof.md` | `Purpose`; `BuildDomain`; architecture | speculative | Designs compiler-grade extraction needed for proof. Contains implementation survey notes but remains target design. | Proof implementation roadmap. |
| `docs/workflow/evalnomicon/drafts/formal/rustc-macro-expansion-backend-plan.md` | `Thesis`; `Current VM facts`; `Backend options`; `Proposed architecture` | speculative | Forward-looking rustc/macro/build-domain extraction plan. | Macro/build.rs proof-gap explanation. |
| `docs/workflow/evalnomicon/drafts/formal/macro-buildrs-callgraph-sequencing-survey.md` | macro/build.rs sequencing survey | speculative | Additional proof sequencing source for macro and build-script limitations. | Paper appendix / proof implementation notes. |
| `docs/workflow/evalnomicon/drafts/formal/edit-surface.md` | formal edit-surface note | duplicate | Older formal note that overlaps `src/prototype1/edit-surface.md` and guided-edit-surface active notes. | Vocabulary archaeology only. |
| `docs/workflow/evalnomicon/drafts/formal/typestate-sketches.md` | typestate sketches | speculative | Conceptual sketch only; does not satisfy the missing pushed `prototype1_state/typestate/**` implementation source. | Background until home-machine typestate code is available. |
| `docs/workflow/evalnomicon/drafts/history/README.md` | history drafts index | stale-but-useful | Older History draft index. Subordinate to current `history-crown.md` and History code. | History archaeology. |
| `docs/workflow/evalnomicon/drafts/history/crown-authority-background.md` | Crown authority background | stale-but-useful | Background for Crown authority; likely superseded by current History/Crown docs. | Paper motivation only after recheck. |
| `docs/workflow/evalnomicon/drafts/history/handoff-2026-04-29.md` | History handoff | stale-but-useful | Historical handoff note around History work. | Design provenance. |

### 4. Active run notes, walkthroughs, migration logs, and proof-spine notes

| Source | Anchor | Classification | Annotation | Downstream relevance |
| --- | --- | --- | --- | --- |
| `docs/active/agents/2026-06-02_prototype1-state-loop-walkthrough/README.md` | `Purpose`; glossary; command entrypoint; config planes; parent turn execution | implementation-grounded | Most complete active walkthrough of `ploke-eval loop prototype1-state`; line refs should be refreshed against current code before publication. | End-to-end mdBook and paper implementation map. |
| `docs/active/agents/2026-06-02_prototype1-state-loop-walkthrough/campaign-configs.md` | config plane overview; model/provider/route precedence; run-profile config | implementation-grounded | Strong source for campaign/run-profile config and model routing. | Config/state persistence chapter. |
| `docs/active/agents/2026-06-02_prototype1-state-loop-walkthrough/terminology-conflicts.md` | `Node`; parent id; child vs successor; runtime/run/turn; authority/projection | canonical/current | Current conflict ledger for overloaded terms. Use before prose generation. | Glossary and terminology normalization. |
| `docs/active/agents/2026-06-02_prototype1-state-loop-walkthrough/turn-live-replay.md` | turn-live artifact paths and persisted data types | implementation-grounded | Best source for replay sidecars such as traces, summaries, and response JSONL. | Observability/replay docs and experiment reproducibility. |
| `docs/active/agents/2026-06-02_prototype1-state-loop-walkthrough/edit-surface-persistence-walkthrough-2026-06-06.md` | evidence roots; process map; TUI adapter loop; backend admission | implementation-grounded | Shows how edits persist at submitted-result/admission layers without becoming promotion authority. | Edit-surface and authority-boundary prose. |
| `docs/active/agents/2026-06-02_prototype1-state-loop-walkthrough/model-api-brief.md` | execution path; live calls; config setting sites | implementation-grounded | Focused source for model/API call boundaries and route provenance. | Reproducibility and operator docs. |
| `docs/active/agents/2026-06-06_parent-successor-handoff-regression/README.md` | conclusion; hypothesis; evidence log | implementation-grounded | Negative/bug evidence around child self-evaluation and parent/successor handoff. Not a happy-path proof. | Avoiding overclaims; failure-mode section. |
| `docs/active/agents/2026-06-06_parent-successor-handoff-regression/implementation-notes-2026-06-06.md` | fixed contract; source changes; regression test; remaining live check | implementation-grounded | Compact source for broad-batch finalization contract and verification. | Regression/fix provenance. |
| `docs/active/agents/2026-06-09_prototype1-state-api-surface-migration/README.md` | contents index | canonical/current | Active migration index for API-surface cleanup. | Migration/restart navigation. |
| `docs/active/agents/2026-06-09_prototype1-state-api-surface-migration/migration.md` | downstream consumer map; PR1-PR4; post-review fixes | canonical/current | Maps DTO/graph import coupling across `ploke-records`, `ploke-tree`, and egui. | Schema migration and read-side projection docs. |
| `docs/active/agents/2026-06-09_prototype1-state-api-surface-refactor-proposal.md` | API-surface refactor proposal | speculative | Explains desired cleanup of edit-surface/headless-attempt models and harness boundary, but not proof of completed work. | Design rationale and future-work staging. |
| `docs/active/agents/2026-06-19_detached-process-crown-proof-spine/README.md` | proof-spine entry point | canonical/current | Active proof-spine index for detached-process/Crown work. | Proof-track navigation. |
| `docs/active/agents/2026-06-19_detached-process-crown-proof-spine/traceability.md` | invariant families; traceability matrix; fact-family consumer plan | stale-but-useful | Highly relevant proof traceability, but child slice observed an older HEAD than current consolidation. Revalidate before final citation. | Proof implementation map after refresh. |
| `docs/active/agents/operator-logs/2026-06-07_prototype1-selectfix-replay-live-run.md` | operator log for selectfix replay live run | implementation-grounded | Evidence for one specific live run, not global architecture. | Empirical/run evidence if needed. |
| `docs/active/agents/run-reviews/README.md` | run-review index | stale-but-useful | Locator for granular run reviews. Use only for specific evidence claims. | Experiment source selection. |
| `docs/active/agents/run-reviews/2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-baseline-eval-protocol.md` | June 9 run-review cluster | implementation-grounded | One exact member of the direct-protocol state3 run-review cluster. | Experiment evidence. |
| `docs/active/agents/run-reviews/2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-terminal-outcome.md` | June 9 run-review cluster | implementation-grounded | Terminal-outcome member of the same run-review cluster. | Experiment evidence. |
| `docs/active/agents/run-reviews/2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-broad-base.md` | June 9 run-review cluster | implementation-grounded | Broad-base member of the same run-review cluster. | Experiment evidence. |
| `docs/active/agents/run-reviews/2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-broad-r2.md` | June 9 run-review cluster | implementation-grounded | Broad-r2 member of the same run-review cluster. | Experiment evidence. |
| `docs/active/agents/run-reviews/2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-broad-r3.md` | June 9 run-review cluster | implementation-grounded | Broad-r3 member of the same run-review cluster. | Experiment evidence. |
| `docs/active/agents/run-reviews/2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-gen1-broad-base.md` | June 9 run-review cluster | implementation-grounded | Gen1 broad-base member of the same run-review cluster. | Experiment evidence. |
| `docs/active/agents/run-reviews/2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-gen1-broad-r3.md` | June 9 run-review cluster | implementation-grounded | Gen1 broad-r3 member of the same run-review cluster. | Experiment evidence. |
| `docs/active/agents/run-reviews/2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-gen1-broad-r4.md` | June 9 run-review cluster | implementation-grounded | Gen1 broad-r4 member of the same run-review cluster. | Experiment evidence. |
| `docs/active/agents/run-reviews/2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-treatment-98d590.md` | June 9 run-review cluster | implementation-grounded | Treatment member of the same run-review cluster. | Experiment evidence. |
| `docs/active/agents/run-reviews/2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-treatment-3dce62110.md` | June 9 run-review cluster | implementation-grounded | Treatment member of the same run-review cluster. | Experiment evidence. |
| `docs/active/agents/run-reviews/2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-treatment-bc17b460.md` | June 9 run-review cluster | implementation-grounded | Treatment member of the same run-review cluster. | Experiment evidence. |
| `docs/active/agents/run-reviews/2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-treatment-f7e43aba.md` | June 9 run-review cluster | implementation-grounded | Treatment member of the same run-review cluster. | Experiment evidence. |
| `docs/archive/agents/2026-05/2026-05-07-prototype1-edit-surface-implementation-plan.md` | May edit-surface plan | stale-but-useful | Earlier edit-surface plan superseded by June guided-edit-surface and evalnomicon pages. | Design provenance. |
| `docs/archive/agents/2026-05/2026-05-07-prototype1-edit-surface-handoff.md` | May edit-surface handoff | stale-but-useful | Earlier handoff source for edit-surface work. | Design provenance. |
| `docs/archive/agents/2026-05/single-ruler-readiness-2026-05-01/*` | single-ruler readiness cluster | stale-but-useful | Precursor reviews for single-ruler/Crown/History invariants. Use only if tracing concept history. | Historical background. |
| `docs/active/agents/collaboration-incidents/*` | collaboration incident ledger | stale-but-useful | Process-failure evidence for why source-verified claims matter; not architecture authority. | Methodology/process appendix only. |

### 5. `ploke-eval` operator docs and Prototype 1 code anchors

| Source | Anchor | Classification | Annotation | Downstream relevance |
| --- | --- | --- | --- | --- |
| `crates/ploke-eval/docs/README.md` | `ploke-eval docs` | canonical/current | Crate-local docs entry point for Prototype 1 loop docs. | Contributor/operator navigation. |
| `crates/ploke-eval/docs/prototype1-loop-operator.md` | `The Two Roots`; `Command Surfaces`; `Parent Phase Map`; `Persisted Files`; `Authority And Debugging Rules` | implementation-grounded | Best operator-facing source for commands, persisted files, and debugging rules. | Operator mdBook section. |
| `crates/ploke-eval/docs/prototype1-proof-ladder.md` | `Rule`; `Current Rungs`; `Remaining Rungs` | implementation-grounded | Frames implementation progress as proof rungs; update before publication because live attempts may have moved. | Proof/progress narrative. |
| `crates/ploke-eval/docs/guides/prototype1-step-execution-path.md` | prerequisites; CLI dispatch; step controller; runtime context; source anchors | implementation-grounded | Deep source for `prototype1-step` execution path and parent identity creation. | Contributor deep-dive and source citation. |
| `crates/ploke-eval/docs/prototype1-child-plan-authority/README.md` | `Prototype 1 Child-Plan Authority` | canonical/current | Entry point for child-plan authority docs. | Child-plan authority navigation. |
| `crates/ploke-eval/docs/prototype1-child-plan-authority/prototype1-record-inventory.md` | `Prototype 1 record inventory for diagram small multiples` | implementation-grounded | Diagram and record-surface inventory for child-plan authority. | mdBook diagrams and record inventory. |
| `crates/ploke-eval/src/cli/prototype1_state/mod.rs` | module docs | implementation-grounded | Primary source-code anchor for Configuration, Artifact, Runtime, Journal, History, Crown, and design constraints. Explicitly says typed states are partially wired. | Code-backed domain model and typestate caveat. |
| `crates/ploke-eval/src/cli/prototype1_state/authority.rs` | module docs; `ActiveRoot`; `ChildRoot`; capability carriers | implementation-grounded | Capability-carrier/root-validation source. Has dead-code/stale-date caveats, so do not overstate live use. | Authority-as-capability proof evidence. |
| `crates/ploke-eval/src/cli/prototype1_state/parent.rs` | `Parent<Unchecked>`; `Parent<Checked>`; `Parent<Ready>`; `Parent<Planned>`; `Parent<Selectable>` | implementation-grounded | Current parent role/state carriers and child-plan surfaces. | Parent typestate and handoff docs. |
| `crates/ploke-eval/src/cli/prototype1_state/child.rs` | child role state carriers | implementation-grounded | Child-side role source referenced by documentation slice as current VM typestate-like carrier. Verify before final line-cited prose. | Child role docs and typestate audit. |
| `crates/ploke-eval/src/cli/prototype1_state/successor.rs` | successor handoff records/states | implementation-grounded | Successor is incoming Parent before handoff acknowledgement, not a separate live controller role. | Child/successor terminology and handoff proof. |
| `crates/ploke-eval/src/cli/prototype1_state/channel.rs` | role-indexed parent/child channel | implementation-grounded | Current code successor to older parent-child-channel draft; separates protocol authority from transport mechanics. | Runtime channel docs. |
| `crates/ploke-eval/src/cli/prototype1_state/invocation.rs` | module docs; `Role` | canonical/current | Persisted bootstrap contract for child/successor runtime attempts. | Runtime attempt identity and handoff docs. |
| `crates/ploke-eval/src/cli/prototype1_state/identity.rs` | parent identity file semantics | implementation-grounded | `.ploke/prototype1/parent_identity.json` is checkout-local coordinate, not process ID or standalone authority proof. | Identity and authority boundary prose. |
| `crates/ploke-eval/src/cli/prototype1_state/journal.rs` | append-only journal and replay helpers | implementation-grounded | Durable transition event stream, distinct from sealed History authority. | Observability/replay/projection prose. |
| `crates/ploke-eval/src/cli/prototype1_state/history/mod.rs` | module docs; History/Crown invariants | canonical/current | Strongest current code source for History/Crown authority and projection warnings. | Proof and authority citations. |
| `crates/ploke-eval/src/cli/prototype1_state/history/seal/mod.rs` | `Entry<S>`; `OpenBlock`; entry hash; typed entry/block states | canonical/current | Implements typed History entries and open block fields; comments name proof gaps around global append position, Merkle lineage maps, manifest digests, and uniform carriers. | Symbolic-proof details. |
| `crates/ploke-eval/src/cli/prototype1_state/history/projection/mod.rs` | read-only History projection; `History::candidates` | implementation-grounded | Projection over verified sealed blocks; candidate payloads carry provenance and membership proofs but do not grant append authority. | GraphRAG/History adapter seam. |
| `crates/ploke-eval/src/cli/prototype1_state/history/stored/mod.rs` | append-only storage port; lineage heads | implementation-grounded | `append` is the semantic operation that can advance lineage head; heads files are projections. | Persistence/proof boundary. |
| `crates/ploke-eval/src/cli/prototype1_state/c1.rs` | C1 -> C2 materialization | implementation-grounded | Child workspace/artifact materialization transition. Final status depends on unpushed typestate completion. | Transition proof/source row. |
| `crates/ploke-eval/src/cli/prototype1_state/c2.rs` | C2 build transition | implementation-grounded | Build-child transition; child slices note live controller caveats. | Typestate caveat and transition source. |
| `crates/ploke-eval/src/cli/prototype1_state/c3.rs` | C3 spawn/ready transition | implementation-grounded | Runtime handoff scaffold; child slices note child-ready journal append was not fully wired in pushed VM version. | Typestate caveat and transition source. |
| `crates/ploke-eval/src/cli/prototype1_state/c4.rs` | C4/C5 observe child | implementation-grounded | Parent-side child terminal observation and evidence carrier. | Parent-observes-child proposition. |
| `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs` | module docs; `GeneratorSurfaceVersion`; adapter boundary | canonical/current | Eval-owned adapter boundary for TUI edit proposals, material span validation, and all-applied/rejected evidence admission. | Guided edit-surface docs. |
| `crates/ploke-eval/src/cli/prototype1_state/backend/surface_admission.rs` | `GitWorktreeBackend::validate_edit_surface_candidate` | canonical/current | Hard checked edit-carrier admission path: path/surface/hash/span/policy validation. | Bounded mutation proof and mdBook examples. |
| `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs` | `BroadHarnessRequest`; `RequestAdmissionBinding` | implementation-grounded | Request/admission binding seam; aliasing indicates recent migration pressure. | Harness request diagrams. |
| `crates/ploke-eval/src/cli/prototype1_state/evidence.rs` | module docs | canonical/current | Child-evidence grouping invariant: loose JSON/path inference must not feed child evidence, metrics, selection, scoring, or authority. | Evidence/admission proof prose. |
| `crates/ploke-eval/src/cli/prototype1_state/evidence_inventory.rs` | `HistoryCommitmentLane`; `InventoryRow` | canonical/current | Best current schema for evidence-surface inventory: projection-only, sealed History, and ingress-that-may-seal lanes. | Publication evidence inventory. |
| `crates/ploke-eval/src/cli/prototype1_state/profile.rs` | `AntiAttractorPolicy` | implementation-grounded | Run-profile policy for surface/skeleton attractors and surface-freshness bias. | Config/policy docs. |
| `crates/ploke-eval/src/cli/prototype1_state/run/mod.rs` | parent-run execution boundary docs | implementation-grounded | Current live execution-path inventory and extraction rule; continuation gate is immediate safety boundary. | Runtime-core extraction map. |
| `crates/ploke-eval/src/cli/prototype1_state/run/core.rs` | `EffectiveRunControl`; `DiagnosedPhase`; `ActiveParentStatus`; `RuntimeContext` | implementation-grounded | Pushed run-core status/preflight structures, still importing live path functions from `cli_facing`. | Run-core extraction caveat. |
| `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs` | broad live controller path | implementation-grounded | Current broad live VM behavior surface; too large for unspecific prose and not the desired long-term authority boundary. | Requires targeted re-audit before final citations. |
| `crates/ploke-eval/src/cli/prototype1_state/prototype1_generation_audit.md` | `Core Issue`; `Hard Pre-Flight Invariants` | implementation-grounded | Focused audit on parent identity/generation and remaining risks. | Proof-gap source. |
| `crates/ploke-eval/src/cli/prototype1_state/prototype1_successor_rehydration_review.md` | `Findings`; `Verification`; `Orchestrator Follow-Up` | implementation-grounded | Focused review of successor rehydration. | Handoff proof details. |
| `crates/ploke-eval/src/inner/HANDOFF.md` | `Read First`; `Prototype 1 loop filesystem`; `Guardrails`; `Resume Here` | implementation-grounded | Local work-resumption handoff; useful but not stable architecture prose. | Contributor restart only. |

### 6. Passive records, proof facts, read-side graph, and projection tests

| Source | Anchor | Classification | Annotation | Downstream relevance |
| --- | --- | --- | --- | --- |
| `crates/ploke-records/src/history.rs` | module docs; passive History record mirror | canonical/current | Passive record mirror boundary: records mirror persisted shapes for projection/UI and do not prove hashes, authorize successor startup, or advance lineage heads. | Projection vs authority claims. |
| `crates/ploke-records/src/proof_facts.rs` | module docs; proof IDs and DTOs | canonical/current | Stable DTO vocabulary for proof-grade call/effect graph extraction: BuildDomain, ExpansionBoundary, CallSite, Definition, EffectSeed, AuthorityFact, Blocker identities. | Detached-process proof data model. |
| `crates/ploke-records/src/proof_authority.rs` | module docs; `AuthorityExtractionConfig` | implementation-grounded | Conservative source extractor for authority-shaped calls; exact-but-untrusted or malformed calls remain blocked. | Proof fact extraction implementation. |
| `crates/ploke-db/src/proof_graph.rs` | `ProofGraphStore`; `ProofInvariantStatus`; schema creation | canonical/current | Database/query surface for proof-useful facts and invariant checker findings; blocker/provenance rows are retained. | Proof GraphRAG/symbolic consumer bridge. |
| `crates/ploke-tree/src/browser.rs` | `RunExecutionGraph`; `ExecutionNodeKind`; `ExecutionEdgeKind` | implementation-grounded | Renderer-neutral browsing boundary over typed records/projections; does not decide History authority. | Read-side graph/chapter diagrams. |
| `crates/ploke-tree/src/graph/artifact_tree.rs` | `Tree`; `Marks`; `Diagnostics` | implementation-grounded | Artifact-first projection over `Graph`, with traversal indexes and diagnostics. | Artifact tree and lineage visualization docs. |
| `crates/ploke-tree/src/tests.rs` | `scheduler_nodes_become_mutable_projection_tree_nodes`; `parent_child_relationships_use_only_explicit_scheduler_parent_fields` | canonical/current | Executable projection invariants: scheduler nodes are mutable projections, and parent/child relationships use explicit scheduler parent fields. | Proof examples and regression evidence. |
| `crates/ploke-tree/src/tests.rs` | ignored diagnostic `real_run_run_forest_node_ids_are_not_artifact_tree_ids` | implementation-grounded | Real-run diagnostic for identity pitfall: RunForest node IDs and ArtifactTree artifact IDs differ. | Identity/glossary caution. |

### 7. GraphRAG, embeddings, database, schema, TUI, and config

| Source | Anchor | Classification | Annotation | Downstream relevance |
| --- | --- | --- | --- | --- |
| `docs/active/agents/2026-06-19_ploke-loop-publication-docs-synthesis/graphrag-history-adapter-note.md` | GraphRAG / prototype History adapter evidence note | implementation-grounded | Prior note connecting code graph/RAG observations to Prototype 1 History candidate/evidence payloads. Safe seam: export observations as evidence, do not mint continuation authority. | GraphRAG-History adapter design. |
| `docs/book/src/architecture/code-understanding-pipeline.md` | parse/transform/store/embed/index | stale-but-useful | Short mdBook pipeline source. Useful as collaborator orientation, but thin relative to current implementation. | Pipeline chapter scaffold. |
| `docs/book/src/architecture/index.md` | components; system diagram; data flow; decisions | stale-but-useful | Imported/generated architecture scaffold; verify against current source. | Collaborator overview draft. |
| `docs/book/src/architecture/runtime-flow.md` | startup; workspace indexing; chat turn; passive records | stale-but-useful | Short runtime-flow scaffold, not enough for publication-quality docs. | mdBook runtime chapter starting point. |
| `docs/book/src/architecture/eval-and-projection-plane.md` | purpose; main crates; authority rule | stale-but-useful | Thin but conceptually correct slot for eval/projection vs authority. | Eval/projection mdBook chapter. |
| `docs/book/src/crate-guide/ploke-rag.md` | crate responsibilities | stale-but-useful | High-level RAG guide; implementation claims need `ploke-rag/src` checks. | Public crate guide scaffold. |
| `docs/book/src/crate-guide/ploke-embed.md` | crate responsibilities | stale-but-useful | High-level embedding guide; less precise than runtime/indexer/provider code. | Public crate guide scaffold. |
| `docs/book/src/crate-guide/ploke-db.md` | crate responsibilities | stale-but-useful | High-level DB guide. Refresh against current multi-embedding/HNSW/BM25 code. | Public crate guide scaffold. |
| `docs/book/src/crate-guide/ploke-tree.md` | responsibilities | stale-but-useful | Public read-side projection summary. Pair with current `ploke-tree` code for claims. | Projection docs. |
| `docs/book/src/crate-guide/retrieval-and-llm-crates.md` | retrieval flow; RAG/DB/IO/LLM crates | stale-but-useful | Lightweight scaffold for GraphRAG/RAG pipeline docs. | Retrieval chapter scaffold. |
| `docs/book/src/user-guide/commands-and-modes.md` | model/embedding/indexing/persistence commands | stale-but-useful | User-facing command index candidate; verify against current command parser. | Operator/user guide. |
| `docs/book/src/reference/glossary.md` | glossary | stale-but-useful | Glossary slot to reconcile with `terminology-conflicts.md`. | Glossary target. |
| `PROPOSED_ARCH_V3.md` | older Ploke architecture proposal | stale-but-useful | Useful high-level parsing -> graph -> embedding -> RAG -> TUI vision, but aspirational/older. | Intro/motivation after source refresh. |
| `docs/testing/TYPE_RESOLUTION_COVERAGE.md` | type-resolution coverage map | implementation-grounded | Current-ish parser/DB/RAG/TUI type-resolution coverage and gaps. | GraphRAG/type-resolution evidence. |
| `crates/ploke-rag/src/lib.rs` | crate root docs / exports | canonical/current | Declares GraphRAG-style architecture and exports `RagService`, retrieval strategies, budget/policy, BM25 status, and fusion utilities. | RAG API entry point. |
| `crates/ploke-rag/src/core/mod.rs` | `RagService`; `RetrievalStrategy`; `SearchParams`; `RagConfig`; `TypeContextConfig` | canonical/current | Primary RAG orchestration: BM25 sparse search, HNSW dense search, hybrid RRF/MMR, type-context gating, timeouts/retries. | RAG behavior citation. |
| `crates/ploke-rag/src/context/mod.rs` | `TokenBudget`; `AssemblyPolicy`; `assemble_context` | implementation-grounded | Current context assembly with dedup, DB path/snippet retrieval, token budgets, and explicit limitations around range normalization/stitching and metadata. | Context assembly chapter and limitations. |
| `crates/ploke-rag/src/fusion/mod.rs` | `ScoreNorm`; `RrfConfig`; `rrf_fuse`; `mmr_select` | canonical/current | Pure fusion utilities for normalization, weighted RRF, and deterministic MMR. | Retrieval algorithm citation. |
| `crates/ploke-tui/src/rag/search.rs` | BM25/hybrid/sparse/dense command handlers | implementation-grounded | TUI exposes retrieval commands through `state.rag`; commands are gated on RAG service availability. | User command and RAG path docs. |
| `crates/ploke-tui/src/rag/context.rs` | `process_with_rag` | implementation-grounded | TUI prompt-construction path: scan completion, config/chat snapshot, loaded-workspace retrieval, context stats, fallback. | Lifetime-of-user-message docs. |
| `crates/ingest/ploke-embed/src/lib.rs` | crate root | canonical/current | Embedding crate entry point. | Embedding implementation navigation. |
| `crates/ingest/ploke-embed/src/runtime.rs` | `EmbeddingRuntime` | canonical/current | Single active embedding-set/embedder handle; `activate` persists DB metadata/relation before swapping runtime state. | Embedding runtime and hot-swap caveat. |
| `crates/ingest/ploke-embed/src/config.rs` | embedding configuration | implementation-grounded | Embedding/provider/model config source read by child slice. | Config docs and provider behavior. |
| `crates/ingest/ploke-embed/src/indexer/mod.rs` | `EmbeddingProcessor`; `EmbeddingSource`; `IndexerTask`; `IndexingStatus` | implementation-grounded | Backend abstraction, batching, cancellation, dimensions, deterministic mocks, and placeholder Cozo remote behavior. | Embedding/indexing behavior. |
| `crates/ingest/ploke-embed/src/providers/openrouter.rs` | `OpenRouterBackend` | implementation-grounded | OpenRouter embedding backend with dimensions, input type, provider order/fallback, semaphore, RPS limiter, retry, and cancellation. Do not record raw credentials. | Provider/model docs and redaction warning. |
| `crates/ingest/ploke-embed/README.md` | core components; process flow; dataflow | stale-but-useful | Best crate-local embedding pipeline prose, but verify against code. | Embedding chapter scaffold. |
| `crates/ingest/ploke-embed/docs/better_index.md` | better indexing notes | speculative | Design notes for better indexing, not current contract. | Future-work discussion. |
| `crates/ingest/ploke-embed/embedding_test_survey.md` | pipeline snapshot; test inventory; gaps; invariants | implementation-grounded | Useful coverage/gap source for embedding pipeline tests. | Evaluation/test coverage notes. |
| `crates/ploke-core/src/embeddings.rs` | `EmbeddingSet`; `EmbeddingShape`; provider/model IDs; relation names | canonical/current | Core typed embedding identity/shape model and deterministic embedding-set IDs. Comments contain planning references, so separate structs from future relation semantics. | Embedding schema glossary. |
| `crates/ploke-db/src/database.rs` | `Database`; `active_embedding_set`; typed relation helpers | implementation-grounded | Main Cozo DB wrapper, active embedding set, typed type graph relation recognition, namespace import/export, snippet context conversion. | DB/schema and retrieval docs. |
| `crates/ploke-db/src/multi_embedding/schema.rs` | `EmbeddingVector`; relation scripts; `validate_embedding_vec` | canonical/current | Current vector schema uses `<F32; dims>` and embedding-set metadata relations. Be precise about Cozo vector values vs list-shaped helper inputs. | Vector schema citation. |
| `crates/ploke-db/src/multi_embedding/db_ext.rs` | `EmbeddingExt`; vector relation operations | implementation-grounded | Registering embedding sets, active set changes, vector relation writes/retractions. | Embedding persistence docs. |
| `crates/ploke-db/src/multi_embedding/hnsw_ext.rs` | HNSW helper operations | implementation-grounded | HNSW relation creation/checking over embedding relations. | Dense retrieval docs. |
| `crates/ploke-db/src/index/hnsw.rs` | HNSW index query/types | implementation-grounded | Dense vector index/search support. | Dense retrieval docs. |
| `crates/ploke-db/src/bm25_index/mod.rs` | `CodeTokenizer`; `TOKENIZER_VERSION` | canonical/current | Code-aware BM25 tokenizer/indexer: identifiers, comments, symbols, metadata. | Sparse retrieval docs. |
| `crates/ploke-db/src/get_by_id/mod.rs` | `GetNodeInfo::paths_from_id`; `NodePaths`; `COMMON_FIELDS_EMBEDDED` | implementation-grounded | Converts node IDs to canonical paths/file paths for context assembly. | Retrieval context provenance. |
| `crates/ploke-db/COZO_HNSW.md` | key learnings; test fix checklist | implementation-grounded | Concrete Cozo HNSW/vector gotchas: fields param, `<F32; DIM>`, F32 requirement, `vec()` casting. | DB/vector caveat appendix. |
| `crates/ploke-db/docs/m1_readiness_report.md` | M1 readiness assessment | implementation-grounded | Database readiness/evidence report. Refresh before maturity claims. | Status/evidence source. |
| `crates/ploke-rag/docs/sota_v2.md` | executive summary; staged retrieval; overlay; sandbox | speculative | Strategy/proposal source for GraphRAG/RAG roadmap, not implementation proof. | Future-work/research direction. |
| `crates/ploke-rag/docs/point2_context_assembly_review.md` | context assembly review | stale-but-useful | Review/proposed API for context assembly, token budgeting, dedup, stitching, and deterministic policy. | Context-assembly improvement notes. |
| `crates/ploke-tui/src/lib.rs` | runtime initialization | canonical/current | Wires user config, DB, embedding runtime, active embedding-set handle, and `RagService::new_full`. | TUI startup and subsystem wiring. |
| `crates/ploke-tui/src/app_state/core.rs` | `AppState`; `RuntimeConfig`; `SystemStatus`; embedding/RAG config fields | canonical/current | Main runtime state container for chat/config/system state, DB, embedding runtime, optional RAG, token budget, and command/config state. | TUI state/config docs. |
| `crates/ploke-tui/src/app/events.rs` | app event loop/input handling | implementation-grounded | TUI app event path source read by child slice. | UI event-flow diagrams. |
| `crates/ploke-tui/src/app_state/events.rs` | `SystemMutation`; `SystemEvent` | implementation-grounded | Typed system mutations/events for history, model switch, tool calls, DB load/backup, re-index, pwd change, etc. | Event model docs. |
| `crates/ploke-tui/src/event_bus/mod.rs` | `EventBus`; `run_event_bus`; priorities; error events | implementation-grounded | Broadcast event bus and index event translation with lag warning containment. | Runtime event architecture. |
| `crates/ploke-tui/src/app_state/commands.rs` | `StateCommand`; `IndexCmd`; `LoadCmd`; `Validate` note | implementation-grounded | Current command/state-command surface; comments document migration from flat enum toward grouped validated commands. | Command architecture migration caveat. |
| `crates/ploke-tui/src/app_state/dispatcher.rs` | `StateCommand` dispatch | implementation-grounded | Runtime dispatcher for model/config/DB/index/RAG/edit/provider/chat mutations. | Command-to-state path docs. |
| `crates/ploke-tui/src/app/commands/parser.rs` | slash command parser | implementation-grounded | Structured parser for slash commands. | User command reference source. |
| `crates/ploke-tui/src/app/commands/mod.rs` | command registry/help surface | implementation-grounded | Command help/aliases and routing. | User command examples. |
| `crates/ploke-tui/src/app/commands/exec.rs` | command executor; model/config handlers | implementation-grounded | UI executor forwards commands; validation belongs in AppState. Includes model/router/load/save/provider strictness and embedding search. | Command UX/config docs. |
| `crates/ploke-tui/src/user_config.rs` | `UserConfig`; model registry; embedding/RAG/user config | implementation-grounded | User config serialization/persistence. Must redact raw API keys/tokens. | Config/state persistence docs. |
| `crates/ploke-tui/src/app/view/components/config_overlay.rs` | config overlay UI | implementation-grounded | TUI UI surface for config editing. | Config UX docs. |
| `crates/ploke-tui/src/app/input/config_overlay.rs` | config overlay input | implementation-grounded | Input handling for config overlay. | Config UX docs. |
| `crates/ploke-tui/src/chat_history.rs` | `UpdateFailedEvent`; hierarchical messages | implementation-grounded | Current chat history has parent/child message semantics and validation-failure events. | Lifetime-of-user-message docs. |
| `crates/ploke-tui/src/tools/request_code_context.rs` | request schema; type-context degraded warnings | implementation-grounded | User-facing code-context retrieval schema and degraded typed-graph neighbor warning. | Tool/RAG docs. |
| `crates/ploke-tui/docs/data-flows/select-embedding-model.md` | embedding model selection data-flow note | stale-but-useful | Child slices identify this as a stub/gap, not enough for final embedding-selection docs; current behavior must be cited from implementation/config sources instead. | Embedding docs gap. |
| `crates/ploke-tui/README.md` | core concepts; architecture; pipelines; RAG pipeline | stale-but-useful | High-level TUI architecture prose; verify against current code and mdBook imports. | Collaborator intro wording. |
| `crates/ploke-tui/docs/archive/workflows/message_and_tool_call_lifecycle.md` | components; glossary; scenario; persistence; tool-calling path | stale-but-useful | Detailed but archived message/tool lifecycle narrative. | Lifetime-of-user-message source after refresh. |
| `crates/ploke-tui/docs/archive/workflows/request-response.md` | messaging lifecycle | stale-but-useful | Additional archived lifecycle source, likely duplicate. | Historical background. |
| `crates/ploke-tui/docs/archive/reports/user_config_loading_detailed.md` | config architecture; loading sequence; source priority | stale-but-useful | Archived config-loading source. | Config doc scaffold after source check. |
| `crates/ploke-tui/docs/archive/reports/event_flow_summaries.md` | query flow; model selection flow; startup sequence; event priority | stale-but-useful | Archived event-flow summaries. | Event-flow diagrams after source check. |
| `docs/design/adrs/proposed/ADR-022-restore-active-embedding-set-on-backup-load.md` | proposed ADR | speculative | Proposed behavior for active embedding-set restoration on backup load; not accepted. | Config/backup future-work. |
| `docs/design/adrs/accepted/ADR-013-typed-node-ids.md` | accepted typed node IDs ADR | canonical/current | Accepted identity/schema ADR relevant to code graph and typed IDs. | Schema/identity proof background. |
| `docs/design/adrs/accepted/ADR-012-state-bearing-types.md` | accepted state-bearing types ADR | canonical/current | Accepted ADR relevant to typed state boundaries. | Typestate/design background. |
| `docs/design/adrs/accepted/ADR-025-module-tree-staged-file-duplicate-definitions.md` | accepted module-tree duplicate definitions ADR | canonical/current | Accepted ADR relevant to parser/module-tree correctness and duplicate definitions. | Parsing/schema correctness docs. |
| `docs/dependency_details/cozo/notes/vectors.md` | Cozo vector notes | stale-but-useful | Dependency note for vectors. Check current `ploke-db` tests before citing. | DB appendix. |
| `docs/dependency_details/cozo/notes/datavalue-cozo.md` | Cozo DataValue notes | stale-but-useful | Dependency note for Cozo values. | DB appendix. |
| `docs/dependency_details/cozo/notes/overview.md` | Cozo overview notes | stale-but-useful | Dependency background. | DB appendix. |
| `docs/dependency_details/cozo/types/time-travel.md` | Cozo time-travel notes | stale-but-useful | Dependency background on time-travel semantics. | DB appendix. |

## Duplicate mapping

| Duplicate/conflict family | Prefer | Mine for context | Relationship |
| --- | --- | --- | --- |
| Runtime loop narrative | `docs/workflow/evalnomicon/src/prototype1/runtime-loop.md` plus `crates/ploke-eval/src/cli/prototype1_state/run/mod.rs` and `cli_facing.rs` for checked implementation | `docs/workflow/evalnomicon/drafts/runtime/loop.md`; `loop-v1-historical.md`; active walkthrough README | `src/prototype1/runtime-loop.md` is concise current prose; V2 draft is richer but older; V1 is historical. |
| Runtime authority narrative | `docs/workflow/evalnomicon/src/prototype1/runtime-authority.md`; `prototype1_state/authority.rs`; `parent.rs`; `invocation.rs`; `history/mod.rs` | `docs/workflow/evalnomicon/drafts/runtime/authority.md` | Current prose plus code should win; draft supplies examples/vocabulary. |
| History/Crown narrative | `docs/workflow/evalnomicon/src/prototype1/history-crown.md`; `prototype1_state/history/**` | `docs/workflow/evalnomicon/drafts/history/**`; run notes | Current page summarizes; code implements; drafts/history is older background. |
| Parent-child channel | `crates/ploke-eval/src/cli/prototype1_state/channel.rs` | `docs/workflow/evalnomicon/drafts/runtime/parent-child-channel.md` | Draft explains intended filesystem/channel protocol; code is current implementation anchor. |
| Persistence/observability vs authority | `persistence-and-observability.md`; `journal.rs`; `history/stored/mod.rs`; `history/projection/mod.rs`; `evidence_inventory.rs` | active run reviews and June 6 regression notes | Logs, journals, DB rows, and run reviews are evidence/projection unless sealed/admitted through explicit authority path. |
| Candidate-generation/edit-surface model | `edit_surface/tui.rs`; `backend/surface_admission.rs`; `evidence.rs`; `evidence_inventory.rs` | June 9 API-surface refactor proposal and migration log; May archive plans | Current code gives admission boundary; proposals explain cleanup but are not completed design. |
| General architecture/RAG pipeline | `ploke-rag/src/*`; `ploke-db/src/*`; `ploke-embed/src/*`; `ploke-tui/src/*` | `PROPOSED_ARCH_V3.md`; mdBook architecture/crate-guide pages; TUI README/archive docs | Public docs are useful scaffolds but can be stale; detailed claims need current code anchors. |
| TUI command architecture | `app_state/commands.rs`; `dispatcher.rs`; `app/commands/{parser,mod,exec}.rs` | mdBook command guide; archived event-flow docs | Code still has broad flat command handling while comments/docs describe grouped command target. |
| GraphRAG/History adapter | `graphrag-history-adapter-note.md`; `history/projection/mod.rs`; `evidence_inventory.rs`; RAG context/search code | `ploke-rag/docs/sota_v2.md` | Safe seam is evidence/candidate payloads, not authority minting. |
| Typestate completion | no current VM path; expected `prototype1_state/typestate/**` | current `parent.rs`, `child.rs`, `successor.rs`, `c1.rs`-`c4.rs`, `authority.rs`, sketches | Current VM sources are partial/implementation-grounded; final typestate claims wait for home-machine push. |

## Downstream citation bundles

### Paper / experiment-plan bundle

- Use `artifact-runtime-model.md`, `runtime-loop.md`, `selection-and-evaluation.md`, and `history-crown.md` for the conceptual story.
- Use `invariant-ledger.md`, `evidence.rs`, `evidence_inventory.rs`, `history/**`, and `crates/ploke-tree/src/tests.rs` for claim boundaries and executable/projection evidence.
- Use active run-review files only for specific empirical claims; do not generalize them into architecture authority.
- Keep `detached-process-callgraph-proof-target.md` and callgraph backend plans in future-work/proof-obligation sections.

### Collaborator mdBook bundle

- Start with `docs/book/src/reference/documentation-roadmap.md` and `docs/book/src/SUMMARY.md` for structure.
- For architecture: use `PROPOSED_ARCH_V3.md` only as stale context, then cite current crate sources.
- For code understanding: cite `docs/book/src/architecture/code-understanding-pipeline.md`, `ploke-rag/src/*`, `ploke-db/src/*`, `ploke-embed/src/*`, and TUI RAG/config sources.
- For runtime/config: cite the June 2 walkthrough, campaign configs, model API brief, TUI `user_config.rs`, and command parser/executor/dispatcher sources.
- For glossary: start from `terminology-conflicts.md` and `artifact-runtime-model.md`.

### Symbolic-proof bundle

- Use `invariant-ledger.md`, `history-crown.md`, `runtime-authority.md`, `history/**`, `parent.rs`, `invocation.rs`, `successor.rs`, and `channel.rs` for current proof vocabulary.
- Use `proof_facts.rs`, `proof_authority.rs`, and `proof_graph.rs` for proof-fact DTO/storage implementation.
- Use `detached-process-callgraph-proof-target.md`, `callgraph-implementation-design-for-detached-process-proof.md`, and `rustc-macro-expansion-backend-plan.md` as target/gap sources only.
- Do not promote detached-process or full typestate claims until `prototype1_state/typestate/**` or equivalent final home-machine changes are pushed and re-indexed.

## Verification notes for this consolidated index

- This document consolidates all exact source paths named in the recovered `t_319c2be8`, committed `t_34283835`, and recovered `t_db9bbe4b` slices. Where child slices named a wildcard cluster, this index either names the cluster plus exact representative files or expands the exact files found during consolidation.
- Classification labels are normalized to the six allowed values.
- Sources that are obsolete, duplicated, speculative, absent, or scratch-artifact-only are retained rather than discarded, with their downstream status made explicit.
- No Rust or product source code was changed by this task.
