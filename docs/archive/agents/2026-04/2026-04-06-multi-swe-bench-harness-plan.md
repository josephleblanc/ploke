# 2026-04-06

## Task Title
Multi-SWE-Bench Harness Plan

## Task Description
Plan the first benchmark-oriented harness flow for driving the existing agentic loop with live model requests, observing tool activity, and capturing the resulting patch/application outcome.

## Related Planning Files
- This document
- [2026-04-06-multi-swe-bench-harness-design.md](./2026-04-06-multi-swe-bench-harness-design.md)

## Plan
1. Confirm the current prompt -> context -> llm -> tool -> patch flow in the modular harness and current runtime.
2. Identify the minimal runtime settings needed for autonomous edit application and stable observation.
3. Define a benchmark runner shape that uses the existing loaded-workspace and event-subscription primitives.
4. Define the event recording model for prompt construction, tool lifecycle, assistant output, and applied patch artifacts.
5. Call out the minimal code additions required to make turn completion explicit and deterministic.
