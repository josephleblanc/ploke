# Research Programme & Operational Plan for Evaluation-Driven Development

DRAFT: 2026-04-09

NOTE: Below document is mostly accurate, and is on-target for our goals. Some of the details (e.g. tool names) may differ from our current tool names, and some docs/dirs mentioned may already exist while others have not yet been created or are stubs. We may copy over some sections to their own dirs/docs but will keep the originals here and maintain a central design doc.

Accuracy updates as of 2026-04-09:

- Living workflow artifacts now belong under [docs/active/workflow](/home/brasides/code/ploke/docs/active/workflow).
- Durable workflow templates, schema drafts, and examples now belong under [docs/workflow](/home/brasides/code/ploke/docs/workflow).
- Current eval provenance is still split across `run.json`, `execution-log.json`, `agent-turn-summary.json`, `agent-turn-trace.json`, `indexing-status.json`, `snapshot-status.json`, and `repo-state.json`; the "immutable run manifest" in `§VIII` is the target converged schema, not a claim about today's `run.json`.
- Current provider selection may be present in `execution-log.json` even when it is absent from `run.json`.
- For active work, start from [docs/active/workflow/README.md](/home/brasides/code/ploke/docs/active/workflow/README.md), then [handoffs/recent-activity.md](/home/brasides/code/ploke/docs/active/workflow/handoffs/recent-activity.md), then return here or to the relevant phase or living artifact.
- The standalone [phased-exec-plan.md](/home/brasides/code/ploke/docs/active/plans/evals/phased-exec-plan.md) is the canonical source for phase status and exit criteria. This document remains the central design and rationale reference.

---

## I. Terminology & Conceptual Framework

The structure you're describing has well-established precedent in the philosophy of science. What you're calling "secondary hypotheses" maps most precisely onto **auxiliary hypotheses** — a concept central to the **Duhem-Quine thesis**, which states that no hypothesis can be tested in isolation; every empirical test simultaneously tests the primary hypothesis *and* the entire constellation of auxiliary assumptions required to conduct the test.

This is exactly the problem you've identified: when an eval run fails, you can't naively attribute it to "structured representations don't help" because the failure might reside in tool friction, parsing infidelity, network flakiness, or eval harness bugs. Lakatos formalized this further with his **methodology of scientific research programmes**, distinguishing:

- **Hard core**: Your primary hypothesis (LLMs perform better with structured code representations)
- **Protective belt**: The auxiliary hypotheses that must hold for the hard core to be testable (tool system effectiveness, parsing accuracy, network robustness, eval fidelity)
- **Positive heuristic**: The research direction that tells you what to build next — your roadmap
- **Negative heuristic**: The commitment not to abandon the hard core based on a single anomaly, but instead to interrogate the protective belt first

When an eval fails, your triage procedure is essentially asking: *which layer of the protective belt broke?* Only after the protective belt is robust do anomalous results bear on the hard core.

However, not all auxiliary hypotheses serve the same function. I recommend a more granular taxonomy, because the distinction matters for both engineering prioritization and eventual publication:

| Term | Role | Example |
|---|---|---|
| **Supporting / enabling hypotheses** | Claims that must hold for the intervention to have a fair chance of succeeding | "The tool interface is discoverable and recoverable enough that the model can actually use the structured representation." |
| **Diagnostic / mechanism hypotheses** | Narrower claims about specific changes or *why* the intervention works | "Adding similarity fallback to `code_item_lookup` improves recovery after misspecified queries." |
| **Measurement assumptions** | Claims about whether your data collection is sufficient to support conclusions | "Our logs let us determine whether a missing node was absent from the index or merely queried incorrectly." |
| **Threats to validity** | Sources of confounding or ambiguity; **construct validity** is the central risk — if the agent fails due to a timeout, you measured network reliability, not coding ability | "A structured-code condition appears worse, but only because timeout handling is unstable." |

Tracking these separately matters because they answer fundamentally different questions: *Why should the intervention work?* vs. *Is the system healthy enough to test it?* vs. *Can we trust the evidence?* That separation will make both engineering and publication much cleaner.

What you are undertaking is a paradigm shift toward what might be called **Evaluation-Driven Development (EDD)**: the practice of using rigorous, instrumented evaluation as the primary driver of engineering decisions, rather than feature backlogs or intuition. The entire programme can be understood as a **theory of change**:

> **Intervention** (structured code tools) → **Mechanism** (better localization, navigation, editing decisions) → **Enablers** (tool UX, accurate representation, robust runtime, observability) → **Evidence** (benchmark outcomes + trace-level diagnostics) → **Iteration** (hypothesis → intervention → eval → attribution → decision)

That framing is strong enough for both product development and a future paper.

---

## II. Primary Hypothesis

### Refined Statement

> **H0 (Hard Core):** For benchmarked coding tasks, agents given access to structured code representations and tools to navigate them outperform agents using primarily shell/text interaction, as measured by (a) higher task success rate, (b) lower token usage, and (c) lower wall-clock time — compared against a shell-only control condition using the same model, benchmark subset, and system prompt (modulo tool-related sections).

### Explicit Decision Rule

Hierarchy of endpoints and interpretation rubric:

1. **Primary endpoint:** Benchmark success rate (solve_rate)
2. **Secondary endpoints:** Token usage (token_cost), wall-clock time (wall_time)

| Outcome | Interpretation |
|---|---|
| Success improves, efficiency improves or neutral | **Strong support** for H0 |
| Success improves, efficiency regresses | **Partial support** — structured tools help but at a cost; investigate whether the cost is inherent or reducible |
| Success does not improve, regardless of efficiency | **Weak / negative support** — interrogate protective belt before concluding |
| Success regresses | **Evidence against** H0, pending protective belt validation |

### Falsification Criterion

A controlled comparison (same model, same benchmark subset, same system prompt modulo tool availability) shows no statistically significant improvement on the primary endpoint, or shows regression on a majority of endpoints, *after* the protective belt has been validated to acceptable thresholds.

---

## III. Hypothesis Registry

See [hypothesis registry](/home/brasides/code/ploke/docs/active/workflow/hypothesis-registry.md)

### Registry Schema for All Entries

Each hypothesis tracked with:

| Field | Description |
|---|---|
| `id` | e.g., H0, A1, D-TOOL-007 |
| `type` | primary / supporting / diagnostic / measurement / threat |
| `statement` | The claim |
| `rationale` | Why it matters to the programme |
| `metrics` | What you measure |
| `acceptance_criteria` | Pre-specified thresholds |
| `falsification` | What would disprove it |
| `status` | proposed / active / supported / rejected / inconclusive |
| `linked_runs` | Experiment IDs |
| `linked_postmortems` | Failure analysis references |
| `next_actions` | What to do based on current status |

---

## IV. Two Classes of Metrics

This distinction is critical and easy to overlook:

### Outcome Metrics (answer the primary hypothesis)
- Task success rate (solve_rate)
- Tokens consumed (token_cost)
- Wall-clock time (wall_time)

### Validity / Health Metrics (answer whether the experiment was fair)
- Provider error rate, timeout rate
- Tool-call failure rate, invalid tool-call rate
- Recovery success rate
- Index coverage rate for touched files/symbols
- Fraction of queries against nonexistent nodes
- Replay completeness rate
- % of runs with attributable failure label
- Median tool latency
- % of runs ending due to infrastructure rather than task failure

**A/B results are hard to trust if the validity metrics are unstable.** Always report both classes.

---

## V. Dependency-Ordered Implementation Layers

**Do not generate trustworthy evidence for a higher-layer hypothesis until the lower layers are solid.**

```
Layer 0: Observability & Data Capture
  ├── Structured, machine-queryable run logs
  ├── Full conversation history serialization (per turn: messages,
  │   tool calls, tool results, token counts, latencies)
  ├── DB state snapshots (or deterministic reconstruction) at each
  │   agent turn
  ├── Error taxonomy with unambiguous categorization
  └── Replay capability

Layer 1: Eval Harness Integrity (A4)
  ├── Correct patch application and test execution
  ├── Accurate pass/fail determination
  ├── Comprehensive result schema
  ├── Automated triage classification
  └── Baseline comparison infrastructure

Layer 2: Network & Provider Stability (A3)
  ├── Characterized timeout/retry behavior
  ├── Provider pinning or controlled selection
  ├── Request-level logging with full metadata
  └── Circuit breakers / graceful degradation

Layer 3: Parsing & Embedding Accuracy (A2)
  ├── Parse coverage reporting per benchmark repo
  ├── Node accuracy validation (ground truth comparison)
  ├── Query recall benchmarks (unit-level)
  └── Incremental update correctness

Layer 4: Tool System Design (A1)
  ├── Tool description refinement (A/B testable)
  ├── Error recovery and suggested-next-step design
  ├── Tool parameter design
  └── Tool set composition (ablation-ready)

Layer 5: Controlled Experiments (H0)
  ├── Baseline: shell-only agent on benchmark subset
  ├── Treatment: structured-representation agent on same subset
  ├── Controlled variables: model, temperature, system prompt
  ├── Statistical analysis
  └── Qualitative analysis
```

This should be understood as a **strict dependency ordering**, not as parallel workstreams. While some work at different layers can proceed concurrently when there are no blocking dependencies, the *priority rule* is: work at a lower layer always takes priority when that layer is blocking interpretation of results at a higher layer. Parallel workstreams risk the common mistake of optimizing prompts and tool variants before the measurement layer is good enough to interpret the results.

That said, within each layer, the work may touch different subsystems (networking, parsing, eval harness), so agents can often work in parallel — as long as everyone understands the priority hierarchy.

---

## VI. Failure Taxonomy

Every failure in every eval run gets classified:

| Category | Code | Description |
|---|---|---|
| Setup/environment | `SETUP_ENVIRONMENT` | Run fails before meaningful agent work because checkout, indexing, snapshot load, or benchmark environment setup is broken |
| Model strategy | `MODEL_STRATEGY` | Poor planning, incorrect reasoning, premature termination |
| Tool discovery | `TOOL_DISCOVERY` | Model failed to find or choose the right tool |
| Tool semantics | `TOOL_SEMANTICS` | Tool behavior confusing or mismatched expectations |
| Tool recovery | `TOOL_RECOVERY` | Error/recovery path insufficient |
| Index fidelity | `INDEX_FIDELITY` | Code item absent, stale, misparsed, or mislinked |
| Query design | `QUERY_QUALITY` | Retrieval/query design weak despite valid data |
| Runtime/infra | `RUNTIME_INFRA` | Provider/network/timeout/retry failure |
| Eval harness | `EVAL_HARNESS` | Harness bug or benchmark execution issue |
| Patch synthesis | `PATCH_SYNTHESIS` | Right localization, wrong edit |
| Validation gap | `VALIDATION_GAP` | Patch compiles but hidden tests fail due to incomplete fix |

(pending examples) Maintain a living document of the taxonomy with canonical examples for each category to keep triage consistent across people and time. The current working file is [docs/active/workflow/failure-taxonomy.md](/home/brasides/code/ploke/docs/active/workflow/failure-taxonomy.md), which is the canonical taxonomy if this design doc and the living taxonomy ever diverge.

---

## VII. The Introspection & Replay API

The principle: **every question we might ask about a run should be answerable programmatically, without re-running the eval.**

### Minimal API Surface

```
Run Inspection
  run.conversations()          → iterator over agent turns
  run.tool_calls()             → all tool calls with inputs, outputs, timing
  run.db_snapshots()           → DB state at each turn boundary
  run.metrics()                → computed metrics for this run
  run.failures()               → classified failure records
  run.config()                 → frozen configuration for this run
  run.manifest()               → immutable run manifest (see §VIII)

Turn-Level Inspection
  turn.messages()              → full message history at this point
  turn.tool_call()             → the tool call made (if any)
  turn.tool_result()           → what the tool returned
  turn.db_state()              → queryable DB snapshot
  turn.db_state().lookup(name) → "does this node exist at the time
                                  the agent queried for it?"

Replay
  replay(run, from_turn=N)         → re-execute from turn N with
                                     current system code but original
                                     conversation history and DB state
  replay_tool_call(turn)           → re-execute just the tool call
                                     with current tool implementation
  replay_query(turn, query)        → run an arbitrary query against
                                     the DB snapshot from this turn

Comparison
  compare(run_a, run_b)            → structured diff of metrics,
                                     failure classifications, tool
                                     usage patterns
  compare_across(experiment_set)   → aggregate comparison with
                                     statistical tests
```

Support at least three replay modes:
1. **Exact trace replay** — replay prior I/O without re-querying live systems
2. **Counterfactual replay** — same run state, one changed variable (e.g., new tool implementation, different prompt)
3. **Step replay** — inspect the run turn by turn, pausing at each decision point

This API is both the developer's debugging tool and the eval harness's data access layer.

---

## VIII. The Immutable Run Manifest

Current-state clarification: today's run artifacts do not yet converge on a single file with all of the fields below. The draft target schema now lives at [docs/workflow/run-manifest.v0.draft.json](/home/brasides/code/ploke/docs/workflow/run-manifest.v0.draft.json).

Every eval run (not exploratory hacking, but a "real" run entered into the record) must capture:

- Experiment ID
- Benchmark issue ID, repo, commit
- Model identifier
- Provider
- Prompt version (git sha or content hash)
- Tool schema version
- Tool implementation version
- Index/database snapshot ID
- Runtime config (temperature, max_turns, etc.)
- Retry/timeout policy
- Seed (if applicable)
- Timestamps (start, end, per-turn)
- Final outcome + all computed metrics
- Failure classification

This manifest is committed and tagged. It is the anchor for all later analysis and the guarantee of reproducibility.

---

## IX. A/B Testing & Ablation Framework

The current converged draft config lives at [docs/workflow/experiment-config.v0.draft.json](/home/brasides/code/ploke/docs/workflow/experiment-config.v0.draft.json), with operating notes in [docs/workflow/ab-testing-ablation-framework.md](/home/brasides/code/ploke/docs/workflow/ab-testing-ablation-framework.md).

Treat the TOML snippets below as illustrative shape only. The JSON draft above is the current converged schema target.

Build this as a **configuration layer**, not a code-branching layer:

```toml
[experiment]
id = "exp-024-tool-recovery"
hypothesis = "A1"
description = "Test whether fuzzy fallback on code_item_lookup improves recovery_rate"

[experiment.control]
tag = "exact-match-only"
tool_config.code_item_lookup.on_miss = "error"

[experiment.treatment]
tag = "fuzzy-fallback"
tool_config.code_item_lookup.on_miss = "fuzzy_search"
tool_config.code_item_lookup.fuzzy_max_results = 3

[experiment.benchmark]
subset = "multi-swe-bench-v1-rust"
issues = ["repo-a#123", "repo-a#456", "repo-b#789"]

[experiment.controls]
model = "anthropic/claude-sonnet-4"
temperature = 0.0
system_prompt = "prompts/v3-baseline.md"
max_turns = 50
```

For ablation studies, the config supports a tool inclusion list:

TODO: adjust below example to match our real tool names (the LLM-exposed prompt names may fluctuate as we experiment, but we can keep our function names stable)
```toml
[experiment.treatment_a]
tag = "all-tools"
tools = ["code_item_lookup", "find_references", "go_to_definition",
         "semantic_search", "file_read", "file_write"]

[experiment.treatment_b]
tag = "no-semantic-search"
tools = ["code_item_lookup", "find_references", "go_to_definition",
         "file_read", "file_write"]

[experiment.treatment_c]
tag = "lookup-only"
tools = ["code_item_lookup", "file_read", "file_write"]
```

The eval harness reads this config, runs both arms, and produces a comparison using the `compare()` API. The config file is committed alongside results and forms your experiment ledger.

---

## X. Operational Workflow

### A. The Experiment Cycle

Every piece of work maps to this cycle. Whether you're fixing a parsing bug, refining a tool description, or running a full A/B test, the shape is the same:

```
1. OBSERVE  → Identify an anomaly, failure pattern, or open question
              from eval data, postmortem logs, or manual inspection.

2. ORIENT   → Locate the anomaly in the hypothesis registry. Which
              auxiliary hypothesis does it bear on? Is it known or new?
              Update the registry if needed.

3. HYPOTHESIZE → Form a specific, testable prediction.
                 "Changing X will improve metric Y by approximately Z"
                 or "The root cause of failure pattern P is Q."

4. IMPLEMENT → Make the change. One change per experiment where
               possible. If multiple changes are coupled, document
               the coupling explicitly.

5. MEASURE  → Run the relevant eval subset. Collect the metrics.
              Compare to the previous run.

6. RECORD   → Write up: what changed, what was measured, what the
              result was, what it means. This is your lab notebook
              entry and feeds the publication pipeline.

7. DECIDE   → Does the result change the status of any hypothesis?
              Does it suggest the next experiment? Update the registry
              and the priority queue.
```

The critical discipline is **step 6**: if it isn't recorded, it didn't happen. The secondary goal of publication makes this discipline self-reinforcing — every recorded cycle is material for the article.

### B. The Micro-Sprint Eval Loop (Daily Operational Unit)

For day-to-day work, operate in tight feedback loops:

1. **Select a failure cohort**: Identify 5–10 SWE-bench failures sharing an error bucket (e.g., "Agent failed to resolve Rust trait usages")
2. **Formulate a diagnostic hypothesis**: *"If we change the `find_usages` tool to fall back to exact-string-match when AST query yields zero results, the agent will recover and succeed."*
3. **Write an Experiment Decision Record (see §XI)** for planned A/B tests, ablations, or other materially diagnostic changes. Routine implementation work can stay in the lab book and handoffs.
4. **Implement the change**
5. **Replay on the failure cohort** using the replay API — this is fast and cheap
6. **Run wider subset eval** to check for regressions
7. **Merge and log**: link the PR to the EDR with updated telemetry data when an EDR exists

### C. The Priority Queue

At any given time, maintain a priority queue. Priority is determined by:

The live priority queue belongs in [docs/active/workflow/priority-queue.md](/home/brasides/code/ploke/docs/active/workflow/priority-queue.md).

1. **Layer violation**: Work at a lower layer always takes priority when the lower layer is blocking. If your eval harness produces false negatives, nothing else matters until that's fixed.
2. **Evidence value**: Among items at the same layer, prefer the item producing the most informative result (confirms or denies a hypothesis definitively).
3. **Cost**: Among items with similar evidence value, prefer the cheaper one.
4. **Coupling**: Prefer items decoupled from other changes. If you must make coupled changes, batch them explicitly and document why.

### D. The Run Protocol

```
PRE-RUN
  □ Configuration is committed and tagged (git sha)
  □ Experiment ID is generated
  □ Hypothesis being tested is identified (from registry)
  □ Expected outcome is written down BEFORE the run
  □ Benchmark subset is specified and frozen for this comparison
  □ Provider health check passed
  □ Validity metrics from recent runs are within acceptable bounds

RUN
  □ Full telemetry is captured (Layer 0)
  □ Run completes or fails gracefully with full state dump

POST-RUN
  □ Automated metrics are computed and stored
  □ Automated triage classifies failures
  □ Results are compared to expected outcome
  □ Cycle step 6 (RECORD) is completed
  □ If result is surprising, a postmortem is triggered
```

### E. The Postmortem Protocol

When a result is surprising or a failure is unclear:

```
1. Identify the specific failing case(s)
2. Replay the conversation history against current system state
3. For each failure, walk the decision tree:
   → Did the agent receive accurate information from the DB?
      (No → A2 issue, investigate parsing/embedding)
   → Did the agent use tools correctly?
      (No → A1 issue, investigate tool design)
   → Did tool calls return correct results given correct inputs?
      (No → could be A1/A2/A3, investigate further)
   → Did the agent reason correctly given accurate information?
      (No → model limitation, document; consider prompt engineering)
   → Did the eval harness correctly assess the result?
      (No → A4 issue, investigate harness)
4. Classify the failure and update the registry
5. If the failure reveals a new category, create a new auxiliary
   hypothesis or refine an existing one
```

---

## XI. Record-Keeping for Publication

### The EDR (Experiment Decision Record) System

Keep the durable template and examples in [docs/workflow/edr](/home/brasides/code/ploke/docs/workflow/edr) and store active EDRs in [docs/active/workflow/edr](/home/brasides/code/ploke/docs/active/workflow/edr). Every A/B test or ablation study gets an EDR:

> **EDR-012: Follow-up Queries vs. Hard Failures in `code_item_lookup`**
> - **Date:** 2025-07-16
> - **Hypothesis ID:** D-TOOL-007
> - **Statement:** Adding similarity fallback improves recovery-to-relevant-symbol rate.
> - **Method:** A/B test on 50 SWE-bench issues; paired comparison.
> - **Acceptance Criteria:** +15% recovery rate, no >5% token regression.
> - **Results:** Auto-follow-up saved 1,200 tokens avg, reduced wall-clock by 15s. Recovery rate +22%.
> - **Conclusion/Action:** Adopt. Merge PR #347. Update hypothesis D-TOOL-007 status to "supported."

### Six Living Artifacts

To make the programme sustainable, maintain:

1. **Programme Charter** — The big picture: primary hypothesis, intervention definition, outcome metrics, decision rules, workstreams (see §XII)
2. **Hypothesis Registry** — The living list of claims and tests (see §III)
3. **Experiment Decision Records** — One per planned A/B or ablation (see above)
4. **Evidence Ledger** — A running record of what you now believe and why, updated after each experiment cycle
5. **Failure Taxonomy + Canonical Examples** — Keeps triage consistent across people and time (see §VI)
6. **Lab Book** — Chronological narrative of changes made, evals run, surprises, dead ends, and lessons learned

That last one is especially valuable for publications, because many of the most interesting insights are process insights.

Current artifact locations:

- [docs/active/workflow/programme_charter.md](/home/brasides/code/ploke/docs/active/workflow/programme_charter.md)
- [docs/active/workflow/hypothesis-registry.md](/home/brasides/code/ploke/docs/active/workflow/hypothesis-registry.md)
- [docs/active/workflow/edr](/home/brasides/code/ploke/docs/active/workflow/edr)
- [docs/active/workflow/evidence-ledger.md](/home/brasides/code/ploke/docs/active/workflow/evidence-ledger.md)
- [docs/active/workflow/failure-taxonomy.md](/home/brasides/code/ploke/docs/active/workflow/failure-taxonomy.md)
- [docs/active/workflow/lab-book](/home/brasides/code/ploke/docs/active/workflow/lab-book)

### Publication Pipeline

The experiment records are your raw material. Plan a multi-part publication series:

1. **Paper 1: Evaluation Methodology & Construct Validity** — How you built the deterministic harness, replay semantics, triage systems, and separated infrastructure noise from signal. This contributes enormously to the community, which struggles with eval noise.
2. **Paper 2: Tool UX Design for LLM Coding Agents** — Results of A/B and ablation tests. What makes low-friction tools for structured code?
3. **Paper 3: Structured vs. Unstructured Code Representations** — The definitive comparison addressing H0. Final controlled experiments, ablation studies, statistical analysis.

A potential fourth angle: the "often obscured process" — each significant postmortem, each surprising result, each methodological insight is a potential standalone piece about what the Duhem-Quine problem looks like in practice (i.e., the failure triage stories).

---

## XII. Programme Charter (Draft)

```
PROGRAMME GOAL
Determine whether structured code representations improve LLM
coding-task performance relative to shell/text-centric interaction.

PRIMARY HYPOTHESIS (H0)
Agents with structured-code tools achieve higher benchmark success
rates, with lower or comparable token usage and wall-clock time.

PRIMARY ENDPOINT
  - Benchmark success rate (solve_rate)

SECONDARY ENDPOINTS
  - Token usage (token_cost)
  - Wall-clock time (wall_time)

DECISION RULE
  - Strong support: success improves, efficiency neutral or improves
  - Partial support: success improves, efficiency regresses
  - Weak/negative: success does not improve
  - Against: success regresses (after protective belt validated)

ENABLING HYPOTHESES
  A1. Tool interfaces are understandable and recoverable
  A2. Structured representations are sufficiently accurate and fresh
  A3. Runtime/provider behavior is stable enough for fair evaluation
  A4. Eval instrumentation attributes failure causes correctly
  A5. Replay and introspection support rapid debugging and iteration

THREATS TO VALIDITY
  - Provider instability; stale or incomplete code index
  - Prompt/tool version drift; inconsistent benchmark slices
  - Insufficient trace detail; incorrect failure attribution
  - Benchmark overfitting (Goodhart's Law)
  - Model-specific findings that don't generalize

WORKSTREAMS
  - Observability & data capture
  - Eval harness integrity
  - Network/provider robustness
  - Parsing & embedding fidelity
  - Tool UX & agentic mechanics
  - Controlled experiments & research synthesis

ADOPTION POLICY
A change is adopted when it improves the primary outcome or yields
meaningful diagnostic evidence without degrading experiment validity.
```

---

## XIII. Phased Execution Plan

### Phase 1: Foundations — Make Results Trustworthy (Layers 0–1)

**Goal**: Produce a single eval run with full telemetry, accurate results, and queryable data.

**Deliverables**:
- Define and implement the run data schema (conversations, tool calls, DB snapshots, metrics, failure classifications)
- Implement the immutable run manifest
- Implement the introspection API (minimum: `run.conversations()`, `turn.tool_call()`, `turn.db_state().lookup()`)
- Implement the automated triage decision tree using the failure taxonomy
- Validate the eval harness against known-good and known-bad patches (manually verify a sample to establish A4 baseline)
- Implement basic replay: `replay_tool_call(turn)` for debugging
- Implement version identifiers for prompts, tools, index, and runtime config

**Exit criterion**: You can run the benchmark, inspect any failure, and determine with confidence which layer the failure belongs to. You trust the pass/fail determination.

### Phase 2: Baselines & Controls (Layers 2–3)

**Goal**: Establish baseline measurements and validate that A2 and A3 are at acceptable levels.

**Deliverables**:
- Characterize and harden network behavior (A3). Pin provider if needed. Log and measure request reliability.
- Run parse coverage and node accuracy checks against benchmark repos (A2). Identify and fix highest-impact parsing gaps.
- Create a **probe suite** for index validation: known symbol lookups, cross-reference traversals, rename/move/stale cases, ambiguous symbol cases, macro/generated-code edge cases, partial-name/fuzzy cases. This gives you a non-benchmark validation layer for the representation itself.
- Run the shell-only baseline on a frozen benchmark subset. Record all H0 metrics. This is your control condition.
- Run the structured-representation agent on the same subset. This is your first treatment measurement. **Do not optimize yet** — just measure. The gap and failure classifications tell you where to focus.

**Exit criterion**: You have a baseline, a first treatment measurement, and a prioritized list of issues derived from the failure classification breakdown.

### Phase 3: Iterative Improvement (Layer 4, cycling through Layers 0–3 as needed)

**Goal**: Systematically improve the treatment condition by addressing failures in priority order.

This is where you spend most of your time. Each cycle follows the experiment workflow from §X.A and the micro-sprint eval loop from §X.B. Typical early work:
- Tool description rewrites (measured by tool_misuse_rate change)
- Error recovery improvements (measured by recovery_rate change)
- Parsing fixes for specific failure cases (measured by query_recall and downstream solve_rate)
- System prompt refinements (measured by overall solve_rate, token_cost)

Every 1–2 weeks, re-run the full benchmark comparison to track aggregate H0 progress. Write EDRs for planned A/B tests, ablations, or other materially diagnostic changes.

**Exit criterion**: H0 metrics are stable (not improving with further changes), or you've exhausted current improvement ideas. Either way, you have a rich dataset.

### Phase 4: Controlled Experiments (Layer 5)

**Goal**: Produce the definitive comparison for publication.

**Deliverables**:
- Freeze all configurations
- Run the final shell-only vs. structured-representation comparison on the full benchmark
- Run ablation studies on tool subsets
- Run A/B tests on the most impactful tool design decisions identified in Phase 3
- Statistical analysis: significance tests, confidence intervals, effect sizes
- Qualitative analysis: categorize and discuss interesting cases from trace review

**Exit criterion**: Sufficient evidence to write a clear results section with pre-specified decision rule applied.

### Phase 5: Publication (Continuous from Phase 1)

This isn't a separate phase — it's a continuous output. The EDRs, evidence ledger, and research notebook from every cycle are your raw material. Every recorded cycle is article material.

---

## XIV. Addressing Known Risks and Open Considerations

Several important concerns deserve explicit attention as you proceed:

### Statistical Power

With curated benchmark subsets, sample sizes may be small. Before running formal comparisons (especially in Phase 4), perform a **power analysis**: given an expected effect size (your Phase 2 baseline-vs-treatment gap gives you an estimate), how many benchmark issues do you need to detect that effect with 95% confidence? If your benchmark is too small, consider:
- Running multiple seeds/trials per issue (if stochasticity is high)
- Expanding the benchmark subset
- Using bootstrapping or permutation tests rather than parametric tests
- At minimum, reporting confidence intervals alongside point estimates

### Cost Management

Running many A/B tests, ablations, and repeated trials across LLM APIs is expensive. Establish a **cost budget** per experiment phase. Mitigation strategies:
- Use the replay API and micro-sprint eval loops (5–10 issue cohorts) for rapid iteration; reserve full benchmark runs for periodic checkpoints
- Track cost-per-run as a validity metric
- Use cheaper models for testing infrastructure changes; reserve expensive models for formal comparisons
- Cache deterministic parts of the pipeline

### Benchmark Overfitting (Goodhart's Law)

As you iterate on the system using SWE-bench results as your guide, you risk overfitting to the specific benchmark. Mitigations:
- Maintain a held-out set of benchmark issues never used during iteration (only for Phase 4)
- Periodically validate findings on qualitatively different tasks (different repos, different issue types)
- Track whether improvements are driven by general capabilities vs. benchmark-specific patterns
- Document this risk explicitly in publications

### Model Generalization

Results from one LLM may not transfer to others. At minimum:
- Run your final Phase 4 comparison on at least two substantially different models
- Report model-specific results separately
- Note model-specific tool-use behaviors in your EDRs during Phase 3

### Rust-Specific Parsing Challenges

Rust introduces significant parsing complexity that will likely drive many A2 failures:
- **Procedural macros**: Code generated at compile time may not appear in your AST
- **Trait resolution and type inference**: Full type information may require compilation, not just parsing
- **Generated code**: Build scripts, derive macros
- **Module system**: Re-exports, glob imports, conditional compilation (`#[cfg(...)]`)

Build explicit test cases for each of these in your probe suite. Decide early what granularity is "good enough" — you don't need to replicate the compiler, but you need to understand and document the boundaries of your representation's fidelity.

### Team Coordination

If multiple people are working on this, establish:
- Clear ownership of each workstream/layer
- A shared understanding of the priority hierarchy (lower layers take precedence when blocking)
- A regular cadence (weekly) for reviewing triage buckets and deciding as a group whether infrastructure failures should halt feature work
- The hypothesis registry and evidence ledger as the coordination artifacts, not just status meetings

### Cold-Start Problem

In early phases, you'll have very few data points to guide prioritization. Handle this by:
- Using the manual triage of your first ~10 failure cases (see "What to Do First" below) as your initial signal
- Making cheap, reversible decisions early and expensive ones later
- Accepting that early phases are exploratory — the micro-sprint eval loop is designed for this

### Benchmark Reproducibility

The SWE-bench repos themselves need deterministic build environments:
- Pin dependency versions, Rust toolchain versions, and OS/container images
- Document any patches needed to make benchmark repos build reliably
- Track build environment as part of the run manifest
- If a benchmark issue's test suite is flaky (passes/fails nondeterministically), flag it and exclude it from formal comparisons

---

## XV. What to Do First

Tomorrow. Concretely.

1. **Review and fill in the hypothesis registry** at [docs/active/workflow/hypothesis-registry.md](/home/brasides/code/ploke/docs/active/workflow/hypothesis-registry.md). Bring the live entry shapes and acceptance criteria up to the intended schema from §III.

2. **Define the run data schema**. What fields does a run record contain? What fields does a turn record contain? Don't implement storage yet — just define the shape. This forces you to decide what you're capturing.

3. **Implement `turn.db_state().lookup(name)`** — because you specifically mentioned wanting to answer "does this node exist at the time the agent queried for it?" Build the smallest possible thing that answers that one question. This gives you a concrete foothold for the introspection API.

4. **Manually triage 10 failure cases** from your existing eval runs using the postmortem protocol from §X.E and store durable writeups under [docs/active/workflow/postmortems](/home/brasides/code/ploke/docs/active/workflow/postmortems). Classify each one against the failure taxonomy. This will immediately tell you which layer of the protective belt is weakest and therefore where implementation effort should go next. This also solves the cold-start problem — you'll have data to prioritize with.

Those four items bootstrap the entire system. Everything else follows from having a hypothesis registry to reference, a data schema to fill, an introspection API to grow, and a triage breakdown to prioritize against.

---

## XVI. The Operational Mantra

If the above is too much to hold in your head at any given moment, here is the shortest version:

> **Version everything.**
> **Instrument every turn.**
> **Replay every important failure.**
> **Classify failure causes.**
> **Run paired experiments.**
> **Record everything in a living evidence ledger.**
> **Fix the measurement layer before optimizing the intervention.**

That is the backbone. Everything in this document is an elaboration of those seven principles.
