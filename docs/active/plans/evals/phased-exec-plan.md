# Phased Execution Plan

If you are about to start real work, begin with [README.md](../../workflow/README.md), [readiness-status.md](../../workflow/readiness-status.md), and [recent-activity.md](../../workflow/handoffs/recent-activity.md), then use this file to understand which phase and exit criteria the work belongs to.

This file is the canonical source for phase status and exit criteria. Keep supporting rationale in [eval-design.md](./eval-design.md).

Each phase (or iteration for Phase 3) gains its own doc once started, with each deliverable annotated with:
- (not started)
- ([in progress](links-tracking-doc))
- ([complete](links-tracking-doc))

When phase-specific tracking starts, keep those docs under `docs/active/plans/evals/` next to this file. Use [phase-tracking-template.md](../../../workflow/phase-tracking-template.md) as the starting point.

## Phase 1: Foundations — Make Results Trustworthy (Layers 0–1)

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

## Phase 2: Baselines & Controls (Layers 2–3)

**Goal**: Establish baseline measurements and validate that A2 and A3 are at acceptable levels.

**Deliverables**:
- Characterize and harden network behavior (A3). Pin provider if needed. Log and measure request reliability.
- Run parse coverage and node accuracy checks against benchmark repos (A2). Identify and fix highest-impact parsing gaps.
- Create a **probe suite** for index validation: known symbol lookups, cross-reference traversals, rename/move/stale cases, ambiguous symbol cases, macro/generated-code edge cases, partial-name/fuzzy cases. This gives you a non-benchmark validation layer for the representation itself.
- Run the shell-only baseline on a frozen benchmark subset. Record all H0 metrics. This is your control condition.
- Run the structured-representation agent on the same subset. This is your first treatment measurement. **Do not optimize yet** — just measure. The gap and failure classifications tell you where to focus.
- Treat these Phase 2 measurements as diagnostic baselines unless the lower-layer validity conditions are already within guardrails.

**Exit criterion**: We have a baseline, a first treatment measurement, and a prioritized list of issues derived from the failure classification breakdown.

## Phase 3: Iterative Improvement (Layer 4, cycling through Layers 0–3 as needed)

**Goal**: Systematically improve the treatment condition by addressing failures in priority order.

This is where we spend most of our time. Each cycle follows the experiment workflow from §X.A and the micro-sprint eval loop from §X.B of [eval-design.md](./eval-design.md). Typical early work:
- Tool description rewrites (measured by tool_misuse_rate change)
- Error recovery improvements (measured by recovery_rate change)
- Parsing fixes for specific failure cases (measured by query_recall and downstream solve_rate)
- System prompt refinements (measured by overall solve_rate, token_cost)

Every 1–2 weeks, re-run the full benchmark comparison to track aggregate H0 progress. Write EDRs for planned A/B tests, ablations, or other materially diagnostic changes.

**Exit criterion**: H0 metrics are stable (not improving with further changes), or we've exhausted current improvement ideas. Either way, we have a rich dataset.

## Phase 4: Controlled Experiments (Layer 5)

**Goal**: Produce the definitive comparison for publication.

**Deliverables**:
- Freeze all configurations
- Run the final shell-only vs. structured-representation comparison on the full benchmark
- Run ablation studies on tool subsets
- Run A/B tests on the most impactful tool design decisions identified in Phase 3
- Statistical analysis: significance tests, confidence intervals, effect sizes
- Qualitative analysis: categorize and discuss interesting cases from trace review

**Exit criterion**: Sufficient evidence to write a clear results section with pre-specified decision rule applied.

## Phase 5: Publication (Continuous from Phase 1)

This isn't a separate phase — it's a continuous output. The EDRs, evidence ledger, and research notebook from every cycle are your raw material. Every recorded cycle is article material.
