# Decision Registry

This file is a legacy planning copy. The active hypothesis registry for workflow use lives at [docs/active/workflow/hypothesis-registry.md](/home/brasides/code/ploke/docs/active/workflow/hypothesis-registry.md).

Explicit decision rule:


1. **Primary endpoint:** Benchmark success rate (solve_rate)
2. **Secondary endpoints:** Token usage (token_cost), wall-clock time (wall_time)

| Outcome | Interpretation |
|---|---|
| Success improves, efficiency improves or neutral | **Strong support** for H0 |
| Success improves, efficiency regresses | **Partial support** — structured tools help but at a cost; investigate whether the cost is inherent or reducible |
| Success does not improve, regardless of efficiency | **Weak / negative support** — interrogate protective belt before concluding |
| Success regresses | **Evidence against** H0, pending protective belt validation |

## H0 (Hard Core / Primary)
  Statement: LLMs produce higher-quality patches, using fewer tokens,
  in less wall-clock time, when provided structured code representations
  and tools to navigate them, compared to shell-only interaction.

  Metrics:
    - solve_rate: fraction of benchmark issues producing a patch that
      compiles and passes hidden tests
    - token_cost: total input + output tokens consumed
    - wall_time: elapsed seconds from issue presentation to patch submission

  Decision rule: See §II above.

  Falsification: Controlled comparison shows no significant improvement
  on primary endpoint after protective belt is validated.



## A1 (Supporting — Tool System Effectiveness)
  Statement: The tool-calling interface exposes structured code
  operations with sufficiently low friction that model performance
  is not bottlenecked by tool misuse, misunderstanding, or
  unrecoverable error states.

  Metrics:
    - tool_misuse_rate: fraction of tool calls syntactically or
      semantically malformed
    - recovery_rate: fraction of tool errors after which the agent
      recovers productively
    - tool_call_efficiency: ratio of productive tool calls to total
    - abandon_rate: fraction of runs where agent gives up or loops
      after tool errors
    - first_useful_tool_call_turn: turns until first productive tool use

  Falsification: tool_misuse_rate or abandon_rate exceeds thresholds
  that make H0 untestable.



## A2 (Supporting — Parsing & Embedding Fidelity)
  Statement: The code intelligence pipeline accurately models the
  target codebase at the required granularity: functions, types,
  traits, impls, modules, and their relationships.

  Metrics:
    - parse_coverage: fraction of source files successfully parsed
    - node_accuracy: fraction of code items correctly identified
    - query_recall: for known-good queries, fraction returning
      correct item
    - staleness_rate: fraction of queries where DB doesn't reflect
      current source state

  Falsification: parse_coverage or node_accuracy below threshold,
  or staleness_rate above threshold.



## A3 (Supporting — Network & Provider Robustness)
  Statement: API interactions are reliable and well-characterized;
  failures don't silently corrupt results or introduce confounds.

  Metrics:
    - request_failure_rate
    - retry_success_rate
    - provider_variance: measurable output differences across
      providers for same model
    - silent_failure_rate: 200 responses with truncated/malformed content

  Falsification: request_failure_rate confounds results, or
  provider_variance introduces uncontrolled variation.



## A4 (Measurement — Eval Harness Fidelity)
  Statement: The evaluation infrastructure produces accurate,
  comprehensive, queryable data that correctly assesses patch
  validity and attributes failures.

  Metrics:
    - harness_false_negative_rate
    - harness_false_positive_rate
    - data_completeness: fraction of runs with full telemetry
    - triage_accuracy: validated by manual review of sample

  Falsification: false rates above threshold or data_completeness
  below threshold.
