# 2026-04-09 Eval Workflow And Research Operations Plan

- date: 2026-04-09
- task title: Operational plan for eval-driven development and hypothesis testing
- task description: Turn the current eval, replay, tool-design, setup-reliability, and observability goals into a reusable workflow that guides implementation priorities, supports long-running investigation, and produces evidence suitable for internal decisions and future publication.
- related planning files:
  - `/home/brasides/code/ploke/docs/active/agents/2026-04-06_eval-benchmarking-report.md`
  - `/home/brasides/code/ploke/docs/active/agents/2026-04-08_setup-reliability-ripgrep-parse-handoff.md`
  - `/home/brasides/code/ploke/docs/active/agents/2026-04-09_openrouter-timeout-followup-handoff.md`
  - `/home/brasides/code/ploke/docs/active/agents/2026-04-07_tool-node-kind-vocabulary-design.md`

## Why This Exists

We are not working on a single bug. We are building an evaluation and development program whose job is to answer a research question while also improving the product.

That means the work has to do two things at once:

1. improve the application
2. improve the credibility of the evidence we gather about the application

If those two concerns are not organized together, we risk doing a large amount of work that either:

- improves the system but does not strengthen the evidence for the primary hypothesis
- generates benchmark numbers that are confounded by setup failures, provider instability, tool-contract drift, or missing observability

This document defines a working structure for choosing tasks, sequencing them, validating them, and recording what they prove.

## Recommended Terminology

Your current distinction between the top-level claim and the supporting claims is good. I recommend the following terms:

- **Primary hypothesis**
  - The main research claim we want to support or refute.
- **Enabling hypotheses**
  - Claims that must hold for the primary hypothesis to be meaningfully tested.
- **Measurement hypotheses**
  - Claims about whether our harness, logs, and artifacts accurately capture the variables we care about.
- **Intervention hypotheses**
  - Claims about a specific change, such as a prompt revision, tool schema fix, retry policy change, or retrieval strategy adjustment.
- **Operational prerequisites**
  - Non-negotiable conditions that must be true before a given experiment class is worth running.

If you want a more mathematical flavor, the whole effort can be treated as a **research program** with:

- one primary hypothesis
- several enabling propositions
- several measurement propositions
- many intervention-level experiments

## Primary Hypothesis

LLMs perform better on coding tasks when they have access to structured code representations than when they operate only over unstructured text and shell-oriented editing workflows.

For now, define "perform better" as:

- higher benchmark success rate
- lower token cost
- lower wall-clock time

Secondary metrics worth keeping, but not elevating above those three:

- lower turn count
- lower tool-call error rate
- lower recovery-turn count after tool failure
- lower human postmortem ambiguity

## The Most Important Constraint

We should not run high-level experiments if lower-level prerequisites are still dominating outcomes.

In practice:

- if setup fails before turn 1, the run is not evidence about agent quality
- if provider instability dominates the runtime, the run is not strong evidence about tool design
- if tool vocabulary drift makes valid actions impossible, the run is not strong evidence about the model
- if logs do not preserve the final state clearly, the run is weak evidence for anything

This yields a strict rule:

**Do not spend substantial time on prompt, tool, or retrieval ablations until setup reliability, provider reliability, tool-contract correctness, and replay/observability are good enough that the measured differences are interpretable.**

## The Enabling Hypotheses

These are the propositions that need to be strengthened before the primary hypothesis can be tested credibly.

### H1. Setup Reliability

The eval harness can consistently:

- resolve the target repo and checkout
- parse the target codebase
- build the DB/snapshot state needed by the agent runtime
- expose the framework in a live, non-panicking state

What counts as evidence:

- setup pass-through rate is high and tracked explicitly
- setup failures fail fast and are typed
- setup failures are clearly separated from agent failures
- setup-only replays exist for historical failures

### H2. Structured Representation Fidelity

The parser, transform, embedding, and DB layers represent the target repository accurately enough for the structured tools to be meaningful.

What counts as evidence:

- targeted repros for parser/transform edge cases
- tests and probes for "does this node exist" and "does it have the expected relationships"
- replay fixtures that compare tool expectations against DB reality

### H3. Tool Contract Correctness And Agent UX

The tool layer exposes the structured representation in a way the model can predictably use.

What counts as evidence:

- tool schemas and validators are aligned
- tool recovery paths are short, strict, and self-healing where possible
- vocabulary drift is eliminated or mechanically prevented
- common failure modes are easy for a model to recover from

### H4. Provider/Runtime Reliability

The live model path is reliable enough that network/provider behavior does not dominate benchmark outcomes.

What counts as evidence:

- request failures are classified precisely
- timeout policy is understood and measured
- retries are bounded and observable
- runs cleanly distinguish provider failures from application failures

### H5. Measurement Integrity

The eval harness produces artifacts that let us explain outcomes without guesswork.

What counts as evidence:

- typed run outcomes
- phase timing
- conversation/tool/result preservation
- replayable artifacts
- clear linkage between run configuration and produced artifacts

## Core Workstreams

Treat the work as five parallel workstreams with explicit gates.

### Workstream A. Setup And Corpus Preparation

Goal:

- make "agent reached a valid starting state" boring and reliable

Includes:

- parser/setup fixes
- indexing health
- snapshot creation/load
- preflight validation
- typed setup failures

Exit criteria:

- setup pass-through is tracked and high enough that batch results are not dominated by setup loss
- historical setup failures have replay coverage
- setup failures persist explicit diagnostics and do not crash the headless runtime

### Workstream B. Tool Contract And Structured-Access UX

Goal:

- make structured tools predictable, discoverable, and recoverable

Includes:

- canonical schemas and vocabularies
- better recovery errors
- lower-friction tool descriptions
- follow-up query behavior where appropriate
- insertion/edit affordance design

Exit criteria:

- major known tool mismatches are covered by replay tests
- model-facing schemas and runtime semantics cannot drift silently
- tool failure triage is significantly more precise than "lookup failed"

### Workstream C. Provider And Runtime Reliability

Goal:

- prevent transport/provider behavior from invalidating eval conclusions

Includes:

- request telemetry
- timeout semantics
- retry policy
- provider selection/failover
- live-run health classification

Exit criteria:

- provider failures are typed and measurable
- repeated network issues do not masquerade as model or tool quality problems
- live-run retry behavior has sane bounds for eval mode

### Workstream D. Eval Observability, Replay, And Analysis

Goal:

- make every important failure explainable and every important success inspectable

Includes:

- clearer run summaries
- typed outcome taxonomy
- replay APIs and fixtures
- step-through semantics
- low-friction DB/log probes

Exit criteria:

- one can answer common postmortem questions quickly
- replay can isolate setup, tool, DB, and model issues separately
- artifacts support both debugging and batch aggregation

### Workstream E. Experiment Design And Research Output

Goal:

- turn stable infrastructure into interpretable experiments and publishable evidence

Includes:

- A/B prompt tests
- tool-description tests
- tool functionality and recovery tests
- ablations over tool sets
- model/provider comparisons
- writeups and figures

Exit criteria:

- experiment specs are explicit
- confounders are tracked
- result tables are reproducible from stored artifacts

## Stage Gates

Use stage gates to decide what work is worth doing next.

### Gate 0. Setup-Credible

Required before interpreting any agent benchmark result:

- setup failures are typed
- setup-only replays exist
- setup succeeds often enough for the target batch
- parser/indexing failures do not panic the runtime

### Gate 1. Tool-Credible

Required before blaming or praising the model for structured-tool behavior:

- schemas, validators, and DB semantics align
- major replayed tool failures have targeted regressions
- recovery paths are model-usable

### Gate 2. Runtime-Credible

Required before interpreting token/time comparisons from live runs:

- provider failures are classifiable
- retry behavior is bounded and measured
- network instability is not dominating outcomes

### Gate 3. Measurement-Credible

Required before doing broad sweeps or writing strong conclusions:

- artifacts preserve outcome, config, and key intermediate states
- runs can be grouped and compared cleanly
- replay and introspection support failure attribution

### Gate 4. Experiment-Credible

At this point it is worth spending meaningfully on:

- prompt A/B tests
- tool description experiments
- functionality ablations
- structured vs unstructured comparisons

## Operational Loop For Each Task

Use the same loop for bugs, design changes, and experiment prep.

### 1. Write The Proposition

Before implementing, state:

- what claim is being tested
- what would count as success
- what would falsify the claim
- which gate or workstream it belongs to

Examples:

- "The historical ripgrep setup failure is caused by Rust 2015 keyword handling."
- "Adding canonical node-kind enums will remove schema/runtime drift for `method`."
- "Current OpenRouter failures are response-body timeouts rather than malformed requests."

### 2. Choose The Smallest Runner

Always prefer the narrowest executable surface that can prove the proposition:

- unit test
- parser repro
- DB probe
- setup-only replay
- recorded tool-call replay
- full live agent run
- batch eval

This keeps costs and ambiguity low.

### 3. Preserve Or Create A Repro Artifact

Before changing behavior, create or confirm one of:

- a minimal fixture
- a historical replay
- a structured log target
- a DB snapshot
- a request/response capture

If the issue is real but not reproducible, the first task is improving reproducibility.

### 4. Add Just Enough Instrumentation

Instrumentation should answer the next question, not every question.

Preferred forms:

- typed status artifact
- narrow tracing around the failing branch
- nested diagnostics extraction
- one helper query for DB or replay inspection

### 5. Implement The Narrowest Fix

Bias toward:

- local changes
- feature-gated experiments when uncertainty is high
- strict semantics with better recovery, not silent coercion
- one clearly testable behavior change per slice

### 6. Verify At Two Levels

Each task should ideally have:

- one narrow proving test
- one more realistic replay or integration check

This is the pattern that worked for the ripgrep setup/parser fix and should be reused.

### 7. Record What Changed And What It Proves

After each slice, update a handoff or plan doc with:

- conclusion
- evidence
- remaining unknowns
- next most valuable question

This is how the implementation trail becomes research material later.

## Experiment Taxonomy

Once the stage gates are met, classify experiments explicitly.

### Type A. Reliability Experiments

Question:

- can the system complete the run path consistently enough to be benchmarked?

Examples:

- setup pass-through
- provider timeout policy
- replay determinism

### Type B. Fidelity Experiments

Question:

- does the structured representation accurately reflect the codebase?

Examples:

- parser edge-case handling
- DB existence checks
- edge/relation coverage

### Type C. Interface Experiments

Question:

- can the model effectively use the structured tools?

Examples:

- schema wording A/B
- recovery prompt A/B
- strict failure vs suggestive failure

### Type D. Capability Experiments

Question:

- do structured tools outperform the baseline on coding tasks?

Examples:

- structured-tool agent vs shell-only/text-only baseline
- ablations over retrieval and tool subsets

## Recommended Run Outcome Taxonomy

Use a stable run taxonomy so batches do not collapse distinct failure classes into one bucket.

Minimum categories:

- `failed_pre_turn`
- `failed_setup`
- `failed_provider`
- `failed_tool_contract`
- `failed_db_fidelity`
- `aborted_pre_patch`
- `completed_without_patch`
- `completed_with_patch`
- `validated_local_only`
- `validated_benchmark_pass`
- `validated_benchmark_fail`

Not every category needs to exist in code immediately, but this should be the target vocabulary for summaries and analysis.

## Required Artifact Families

To support both debugging and research output, each serious run should be able to answer:

- what was attempted
- what environment/config was used
- what the model saw
- what tools were offered
- what tools were called
- what failed first
- what recovered
- what patch was produced
- what validation concluded

That implies five artifact families:

### 1. Run Identity

- task id
- dataset item id
- repo/base sha
- `ploke` sha
- config fingerprints
- tool schema fingerprint
- prompt fingerprint

### 2. Phase Ledger

- setup start/end
- indexing start/end
- prompt assembly start/end
- turn timings
- tool timings
- validation timings

### 3. Interaction Record

- prompt/context plan
- final assembled prompt
- conversation turns
- tool calls and results
- final assistant message

### 4. Failure And Recovery Record

- first error
- first recovered error
- parse diagnostics
- provider failure classification
- validation failure detail

### 5. Outcome Record

- local patch presence
- local build/test result
- benchmark verdict
- token counts
- wall-clock
- turn count

## Recommended Prioritization Rule

When choosing the next task, prefer the item that maximizes:

- reduction in ambiguity
- improvement in run validity
- improvement in reproducibility
- leverage across future experiments

This usually means the order is:

1. remove a confounder
2. improve attribution
3. improve the system behavior itself
4. only then compare interventions at scale

## Suggested Near-Term Sequence

This is the current recommended order of operations.

### Phase 1. Finish Reliability Prerequisites

1. Close remaining setup reliability gaps.
2. Improve provider/runtime telemetry and timeout semantics.
3. Finish the most damaging tool-contract correctness issues.

Why:

- these are still the dominant confounders

### Phase 2. Build Measurement Credibility

1. strengthen run summaries and typed outcomes
2. improve replay APIs and low-friction DB probing
3. make postmortem questions easy to answer from artifacts

Why:

- this turns fixes into evidence instead of anecdotes

### Phase 3. Define Controlled Comparison Modes

1. structured-tools mode
2. text/shell baseline mode
3. ablation mode over subsets of structured tools

Why:

- this is the first point where structured-vs-unstructured comparisons become interpretable

### Phase 4. Run Small Controlled Batches

Start with:

- same task subset
- same model/provider where possible
- fixed budget
- repeated runs where variance matters

Track:

- setup pass-through
- patch production rate
- benchmark pass rate
- token usage
- wall-clock time
- turn count
- tool failure count

### Phase 5. Promote To Research Narrative

Once the experimental path is stable, maintain a parallel record of:

- design decisions
- failed approaches
- key turning points
- methodology constraints
- examples of misleading metrics and how they were corrected

That material is the seed of a useful paper or article series.

## Working Norms For Long-Running Tasks

To keep multi-turn work aligned, use these norms:

- Every substantial task gets a handoff doc or is appended to an existing one.
- Every task states which workstream and gate it belongs to.
- Every test or replay should say what proposition it proves.
- Setup-only, replay, and live-model paths should stay conceptually separate.
- Do not silently weaken correctness to make evals look better.
- If a change improves recovery by relaxing semantics, treat that as a deliberate design decision and document the tradeoff.

## Immediate Next Questions

Based on current state, the best next questions are:

1. Can we classify OpenRouter failures precisely enough to stop provider noise from polluting eval interpretation?
2. Which remaining tool-contract mismatches still block valid structured-tool usage in historical failure cases?
3. What artifact and replay API additions would most reduce postmortem ambiguity for the next batch?
4. What is the smallest controlled baseline comparison between structured tools and non-structured tooling that would already produce interpretable data?

## Resume Prompt

Continue from `docs/active/agents/2026-04-09_eval-workflow-and-research-operations-plan.md`.
Pick one workstream and one gate, state the proposition being tested, choose the smallest runner that can test it, and update the document or a linked handoff with concrete evidence and next steps.
