# Eval Benchmarking Next Steps Plan

- date: 2026-04-06
- task title: Plan follow-up work for eval benchmarking design
- task description: Capture the immediate planning link for the benchmark-structure research report and define the next design stages for ploke-eval.
- related planning files:
  - `/home/brasides/code/ploke/docs/active/agents/2026-04-06_eval-benchmarking-next-steps-plan.md`

## Purpose

This planning note exists to link the benchmark-design report into the shared-agent document workflow and to anchor follow-up implementation planning.

Primary report:

- `/home/brasides/code/ploke/docs/active/agents/2026-04-06_eval-benchmarking-report.md`

## Planned Follow-Ups

1. Define the benchmark run schema:
   - run manifest extensions
   - phase/event record schema
   - summary/aggregate schema
2. Define benchmark modes:
   - ingestion baseline
   - retrieval benchmark
   - agentic task benchmark
   - regression/replay benchmark
   - config sweep benchmark
3. Decide scoring and success criteria:
   - infrastructure health
   - retrieval quality
   - task completion
   - cost and latency
4. Map existing observability hooks to required benchmark outputs:
   - indexing status
   - context plan snapshots
   - chat session reports
   - tool-call observability
5. Identify missing instrumentation and storage surfaces in `ploke-eval`.
