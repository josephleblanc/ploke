# Todo 2026-03-20

Follow the instructions below.

---

## Instructions

These instructions direct you to work on tasks associated with the broader
implementation of the initially described task in section "Workspace" below.
Each instruction is added sequentially in response to documentation generated
implementing the response to the previous instructions, so if you see a
(2026-03-20/2), you may assume it supersedes a (2025-03-20/1) or
(2025-03-18/3).

These previous instructions primarily indicate the direction of the
implementation, but your focus should be on the most recently dated item, where
they follow the structure `(yyyy-mm-dd/ordering)`

### (2026-03-20/1)
For now, focus primarily on the Ingestion Pipeline instructions,
but keep an awareness of the overall direction of the desired longer term
implementation.
Put your report in docs/active/reports/ for my review, providing links to
progress or intermediate reports produced by subagents.

### (2026-03-20/2)
Review the reports referenced below, and create a plan to implement the desired functionality.
> The report is at docs/active/reports/2026-03-20_workspaces_report.md.
>
> It links the three survey reports, confirms that the parser/transform/schema/indexing
> primitives are mostly already in place, and identifies the main gap as ploke-tui
> orchestration and state: replacing the current single-directory /index start flow
> with a manifest-driven workspace flow, then adding workspace save/load/status/update/
> remove and scope-aware RAG/tool behavior.
>
> Subagent reports are here:
>
> - docs/active/agents/2026-03-workspaces/ingestion-pipeline-survey.md
> - docs/active/reports/2026-03-20_db-rag-workspace-survey.md
> - docs/active/reports/tui-workspace-survey.md
>
> No product code was changed and no tests were run.

### (2026-03-20/3)
Review the initial plan referenced below, and edit it in response to my
following points on readiness and acceptance criteria:

> The implementation plan is at docs/active/reports/2026-03-
> 20_workspaces_implementation_plan.md.
>
> It turns the survey into a phased build plan: first make ploke-tui model one loaded
> workspace plus a focused crate, then replace the current single-directory /index
> start flow with manifest-driven parse_workspace(...) +
> transform_parsed_workspace(...), then add workspace status/update, registry-backed
> save/load, and finally scope-aware RAG/tool behavior.
>
> The report also calls out the main architectural split: workspace indexing/search is
> mostly TUI orchestration, but /load crates ... and /workspace rm <crate> need new
> namespace-scoped DB export/import/remove primitives before they can be implemented
> safely. No product code was changed and no tests were run.

This plan is overall a reasonable starting point, but we need to be more
specific about the acceptance criteria and in particular Phase 1 needs to be
expanded, perhaps split into multiple phases, due to the potential complexity
of implementing the required `ploke-db` search methods implied in the Phase 1
description.

- Re: test fixtures and pre- 20_workspaces_implementation_plan.md readiness

We currently have fixture targets which are mostly just crates, not workspace
members.

As such, we will need to add new fixtures which can be used as targets to
validate both the expected `syn_parser` workspace handling and
`ploke-transform` transforms, as well provide the basis for new backup dbs that
can be used in phase 1 and later phases.

Add a readiness section to the plan for the test fixtures and behavior which
must be validated for phase 1 to begin, using the following point "Re:
Acceptance Criteria" as a starting point both for reviewing the current plan
and creating the new readiness criteria section.

- Re: Acceptance criteria

Break each of the acceptance criteria into specific, disprovable definitions or imperatives
1. new and/or changed data structures
2. implied or expected invariants
3. cross-crate contracts
4. possible failure states

For each of the items described, identify:
- required fixtures for validation
- explicit or implied properties (examples below)
- untestable properties, if any
- already present tests explicitly or implicitly validating the item described

These acceptance criteria may be summarized in the planning document, but must
be exhaustively documented in an accompanying document, with links and
backlinks to the planning document and citations to any referenced code or
tests. These acceptance criteria must be the set of properties which must be
either provably correct, or impossible to prove with clear explanation as to
why the described property is impossible to prove and/or test.

examples:
- if a workspace is present in db, then all crate nodes have an edge to that workspace
- for all search functions `ploke-rag` that operate correctly on a target crate
A to return results a_1, a_2, ..., a_n, there exists a corresponding search
function which operates correctly on multiple target crates A, B, .., such that
if a_1 in A and b_1 in B are more similar than a_1 is to all other items a_n in
A, the corresponding search function returns results a_1, b_1, ...
- all items in the database have a deterministically generated, unique identifier

### (2026-03-20/4)

Review the following referenced reports, acceptance criteria, and associated
docs from the last planning update:
> The workspace planning docs are updated.
>
> The revised implementation plan is at docs/active/reports/2026-03-
> 20_workspaces_implementation_plan.md. It now adds a formal Phase 1 readiness gate for
> fixtures and validation, splits the early work into smaller phases, and moves
> workspace-scoped retrieval and crate-subset DB operations into later, explicit phases
> instead of implying them up front.
>
> The exhaustive companion acceptance document is at docs/active/reports/2026-03-
> 20_workspaces_acceptance_criteria.md. It breaks the work into disprovable criteria
> with:
>
> - new or changed data structures
> - invariants
> - cross-crate contracts
> - failure states
> - required fixtures
> - existing tests already covering the area
> - properties that are not currently provable, with reasons
>
> Notable corrections in the acceptance doc:
>
> - it replaces the “workspace edge” example with the property the current schema can
>   actually prove: workspace_metadata.members consistency
> - it calls out that exact global cross-crate ranking is not generally provable for
>   approximate HNSW retrieval, so the acceptance target is scope correctness plus
>   controlled-fixture hit behavior
>
> Supporting survey docs used for the revision are:
>
> - docs/active/agents/2026-03-workspaces/phase1-readiness-ingest-fixtures.md
> - docs/active/agents/2026-03-workspaces/db-rag-acceptance-survey.md
> - docs/active/agents/2026-03-workspaces/tui-phase-plan-survey.md
>
> No product code was changed and no tests were run.

The focus of your review should be on correctness, evaluating the "acceptance"
criteria as propositions, and determine whether the acceptance criteria in
sequential phases are building a logically valid and provably correct
implementation.

If the acceptance criteria do form a logically coherent structure, evaluate
whether the built claims are necessary, sufficient, or neither for the
accomplishment of the defined goals of the overall task.

If the acceptance criteria do not form a logically coherent structure, revise
and expand the acceptance criteria to form a set of claims which are provably
correct, in the spirit of the Howard-Curry Correspondence or a mathematical
proof in the spirit of LADR or Birkhoff and Maclane's Algebra.

If you require access to information outside the documents referenced in the
quote above, create sub-agents to provide brief and targeted information to
supply that information instead of reading files or executing tests directly.

### (2026-03-20/5)
For this task, do not explore other crates, supporting code docs, or referenced
files beyond those directly referenced in the quote below. Assume all
referenced items contain the contents and supporting evidence for the claims or
descriptions which reference them. However, do not make assumptions beyond what
is claimed in the documents.

> Revised the workspace docs for (2026-03-20/4).
> 
> Updated:
> 
> - docs/active/reports/2026-03-20_workspaces_acceptance_criteria.md
> - docs/active/reports/2026-03-20_workspaces_implementation_plan.md
> - docs/active/reports/2026-03-20_workspaces_acceptance_review.md
> 
> The review conclusion is: the phase order was logically coherent, but the prior
> acceptance set was necessary rather than sufficient. I tightened it by adding
> explicit cross-phase proof obligations for session coherence, authoritative workspace
> membership plus drift handling, a Phase 1 snapshot-coherence proof
> (workspace_metadata.members vs crate_context), /index atomicity, registry-vs-snapshot
> authority on load, and pre-truncation scope enforcement for dense/BM25/hybrid
> retrieval.
> 
> Targeted survey notes used for the revision are here:
> 
> - docs/active/agents/2026-03-workspaces/indexing-state-coherence-survey.md
> - docs/active/agents/2026-03-workspaces/retrieval-scope-proof-survey.md
> - docs/active/agents/2026-03-workspaces/save-load-update-correctness-survey.md
> 
> No product code was changed and no tests were run.

Review @docs/active/reports/2026-03-20_workspaces_acceptance_criteria.md for
logical validity, in the spirit of the Howard-Curry Correspondence or a
mathematical proof in the spirit of LADR or Birkhoff and Maclane's Algebra.
Evaluate each claim, provable or unprovable assertion in regard to logical
structure and adherence to the stated goals.

For any key terms influencing the validity of the document's claims, provide a
definition that is supported by the limited documentation you have access to,
and point out any terms which are not sufficiently defined for the claims in
the document, with attached questions and/or requirements to resolve in order
for the terms to be clearly defined.

For any poorly defined terms or claims, identify the assumptions made, and
provide clarifying questions to resolve or make such assumptions explicit.

Provide a review and analysis to identify gaps, shortcomings, or overall
compositional pitfalls that could be revised, expanded, or pruned out.

### (2026-03-20/6)
Use the review produced in the previous step, located in
docs/active/reports/2026-03-20_workspaces_acceptance_criteria-review.md,
to revise the acceptance criteria in line with:
- the "Overall" recommendations
- the point-by-point critique

Each critique by revising the described section, terms, or structure

For each undefined term and question, provide an answer with references to the
code base and documentation, citing any code or tests referenced.

As you revise the original document under review,
docs/active/reports/2026-03-20_workspaces_acceptance_criteria.md, add notes to
the 2026-03-20_workspaces_acceptance_criteria-review.md identifying exactly how
each point is updated or addressed in the revision.

You are not limited as the original review in (2026-03-20/6) was, but use
sub-agents whenever possible to survey, investigate claims, or explore the code
base for appropriate evidence, which you can then check for accuracy if you
plan to reference it in the revised acceptance criteria document.

---

## Workspaces

### Ingestion Pipeline

We have been working on expanding our database schema to include workspace metadata
on the parsed targets, in an effort to bring workspace-wide search/edit capability
to our `ploke-tui` application. So far we have implemented workspace parsing in
`syn_parser`, and introduced a transform function to insert the intermediate data
structure for the workspace metadata as a relation in the cozo database.

I would like you to evaluate what will need to be done before we have indexing (e.g.
using `/index ...` in ploke-tui commands), semantic search, and semantic edit
capability in `ploke-tui`.

Use sub-agents to perform surveys of the target crates, using them in parallel where
possible. Limit your own direct examination of files to only reading or searching
those files after you have received a report from a sub-agent. Instruct the sub-
agents to use the new `docs/active/agents/2026-03-workspaces` directory to have
the agents share links and/or brief reports so you can use regex search on the
reports as well.

The purpose of this phase is to identify the necessary changes and updates to
bring our code base to using basic workspace-wide indexing and semantic
search/edit capability, not to introduce inter-crate linking or dependency
graphs, which will be implemented at a later date.

The end desired behavior is for the user to be able to run ploke-tui, use the `/index <workspace>` command or `/index` command in a workspace, and for all crates in the workspace to be:
- parsed
- transformed into the db,
- generate vector embeddings for those crates,
- index those embeddings with hnsw,
- optionally use bm25 indexing,

### TUI functionality and commands

#### Functionality

TUI uses automatic rag search on user input and exposes tools for the LLM that utilize database queries and semantic search. The workspace-aware versions should:
- Expand on crate-specific search (all that is currently implemented) with
optional crate specification
- Exact work on the above point will be expanded upon review of Ingestion Pipeline

#### Commands

Once the workspace is indexed, the code graph database should have the ability for pre-existing save/load commands to have the following expanded functionality:

- `/save db` when a workspace is loaded to save all crates to the user's config dir and populate a workspace config file for ploke with the processed crate names and backup db locations.
- `/load <workspace>` to load all crates in the named workspace. The named workspaces will be saved in the user's config dir
- `/load crates <crate1> <crate 2>` to load multiple crate backups into the currently loaded database if any, performing validation to identify potential conflicts (e.g. currently loaded crate has same path as target crate to be loaded, prompt user if update is desired)

New commands to manage the loaded crate should be implemented. Initial candidates:

- `/workspace status`
  - currently loaded/not-loaded crates
  - uses file hashes to check against file hashes saved in db for change detection
- `/workspace update` updates stale crates in loaded crates
- `/workspace rm <crate>`
  - checks if crate is backed up, if so removes it from loaded db.
  - if not backed up, prompt user to ask if it should be saved, otherwise just remove.
