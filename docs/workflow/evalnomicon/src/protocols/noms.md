
## Role of this document

For each NOM we are to define:
- the object of analysis
- the evaluative question
- why it is non-obvious
- what decisions it is supposed to support

## telos

We are running evals on `ploke`, an agentic harness for Rust codebases that exposes a structured tool surface built on parsed code, graph-aware retrieval, and graph-aware navigation/edit workflows. Our eval targets are the Rust instances in Multi-SWE-Bench, and we already have persisted run records for those evals.

The current task is not to execute more runs, but to introspect on existing run records in a way that produces actionable feedback about tool and workflow quality. The purpose of that introspection is to support intervention choice: improve the harness where the evidence suggests it is weak, and thereby improve outcome metrics such as solve rate while reducing token cost, latency, and avoidable tool churn. This also provides evidence for or against `H0`, because it helps distinguish failures caused by tool/workflow design from failures caused by unrelated factors.

## NOMs

### Tool Call Failure Modes

Possible related article or blog copy:

 We are introspecting on persisted eval traces from Multi-SWE-Bench Rust runs of the `ploke` agentic harness. The goal is to derive actionable feedback about tool and workflow quality from those traces, so we can choose better interventions and improve outcome metrics such as solve rate, token cost, and time-to-resolution.

 The first metric under consideration is a classification of tool-call failure modes. This is a NOM rather than an OM because the relevant notion of failure is not captured by surface execution status alone. A tool call may return without error and still fail semantically relative to the intent that made the call appropriate in context; conversely, an errored call may be acceptable or easily recoverable. Determining the relevant failure mode therefore requires bounded interpretation of intent, local context, tool behavior, and observed outcome.

 We want this metric to provide evidence for decisions about tool design and workflow design. In particular, it should help answer whether failures are driven by misleading tool semantics, weak parameter schemas, ineffective recovery paths, semantically unhelpful successful calls, repeated failure clusters, or likely model-strategy issues rather than tool issues.

#### What This Metric Is
This metric is a structured classification of tool-call failure modes within an eval trace.

More precisely:
> This metric classifies whether a tool call failed relative to its intended
> task contribution, and if so, what kind of failure it was.

Categories:
- transport/execution failure
- semantic failure
- workflow/recovery failure
- interface/schema failure
- misleading apparent success

#### Why is this metric non-obvious?

This is a NOM because the relevant notion of “failure” is not reducible to
surface execution status alone. A tool call may return successfully and still
fail to satisfy the intent that made the call appropriate in context.
Conversely, a call may error in a way that is locally acceptable or
recoverable. Determining the relevant failure mode therefore requires bounded
interpretation of intent, local context, tool behavior, and outcome.

For example, a search call may return results without error, yet still fail if
the query intent is clear from context and the returned results do not satisfy
that intent in any useful way.

#### What downstream support does this metric provide?

The metric is meant to provide the evidence for downstream workflow intervention decisions.

It does not provide advice on implementation or suggest which changes should be made.

This metric should provide evidence to help answer the following questions:

- Is the tool interface itself misleading or under-specified?
- Are argument schemas allowing preventable invalid inputs?
- Are recovery affordances present but ineffective?
- Are successful-looking calls semantically unhelpful?
- Are repeated failures clustered around one tool, one parameter pattern, or one workflow transition?
- Are follow-up affordances or hints being used, and when used, do they appear to help?


Non-goal: assessment of the domain that is responsible for the failure:
- Are failures primarily tool-design failures, workflow-design failures, or likely model-strategy failures?

The primary responsibility is to provide evidence on:
- what failed
- why

#### Areas for improvement

1. `Failure` needs a defined scope.
   You should probably distinguish:
   - execution failure
   - semantic failure
   - recovery failure
   - workflow/interface failure

2. `Intent` needs a bounded source.
   Otherwise this becomes too hindsight-heavy.
   You likely need to say intent is derived only from admissible local evidence:
   - tool invocation
   - nearby assistant content/reasoning if present
   - local trace context
   - task statement only if explicitly allowed

3. You should decide whether the output is:
   - one classified call
   - a set of classified calls
   - or a run-level synthesis over those classifications

I suspect the first real useful NOM is actually:
- a run-level synthesis built from local call classifications

**My Suggested Refined Version**

If I compress your draft into something more stable, I’d write:

