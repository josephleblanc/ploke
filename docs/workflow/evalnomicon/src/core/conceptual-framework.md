# Conceptual Framework

This chapter is the home for the core conceptual framework behind eval-driven
development in this repository.

Planned topics:

- primary and supporting hypotheses
- measurement assumptions and threats to validity
- outcome metrics versus validity or health metrics
- OM / DOM / NOM distinctions
- introspection as a driver of design, not just postmortem

For the current broader rationale, see
[eval-design.md](../../../../active/plans/evals/eval-design.md).

## Logical Framing

### Metric Classes

```text
M = all currently named metrics in the framework
M_op = all currently operationalized metrics

O = obvious metrics
N = non-obvious metrics
C = conceptual metrics
D = uncaptured dimensions

M = O ∪ N ∪ C
M_op = O ∪ N

O ∩ N = ∅
O ∩ C = ∅
N ∩ C = ∅
D ∩ M = ∅

D -> C -> N -> O
```

Interpretation:

- `O`, `N`, and `C` are mutually exclusive modes of metric production at a
  given point in time.
- `D -> C -> N -> O` is a common maturation path, not a strict law.
- `D` is outside `M` because uncaptured dimensions are not yet named metrics.

### Method, Inputs, and Execution

```text
m = a metric
x = a method specification for producing metric m
I_x = the admissible input domain for method x
e = an executor of method x
v = the value produced for metric m
```

Function-like view:

```text
Exec(e, x, i) = v, where i ∈ I_x
```

Interpretation:

- `x` is a specification, not an execution.
- `I_x` is associated with `x`; it is the evidence or input contract for that
  method.
- `e` is the thing that executes `x`. Depending on context, `e` may be code, an
  LLM, or a human reviewer.

Definition:

> An executor `e` follows method `x` iff `e` applies the procedure specified by
> `x` using only admissible inputs from `I_x`.

Prog(m) = the set of candidate programmatic derivation procedures for m
Adj(m)  = the set of candidate adjudication procedures for m


  - x = a method specification
  - e = an evaluator / executor
  - x(e, I) -> v = executor e applying method x to admissible inputs I, producing value v

Rep(x, e) := x is reproducible on the same inputs for some evaluator e
Rel(x, e) := x is reliable enough for routine workflow use for some evaluator e

B(x) := x is bounded in time, scope, and effort
E(x) := x is explicit enough to be written down and followed
A(x) := x uses admissible evidence / inputs
  R := R(x) ⇔ A(x) ∧ B(x) ∧ E(x) ∧ Rep(x) ∧ Rel(x)

Then the class boundaries can be written cleanly as:

m ∈ O  ⇔  ∃ p ∈ Prog(m) such that R(p)

m ∈ N  ⇔  ¬∃ p ∈ Prog(m) such that R(p)
          ∧ ∃ a ∈ Adj(m) such that R(a)

m ∈ C  ⇔  ¬∃ p ∈ Prog(m) such that R(p)
          ∧ ¬∃ a ∈ Adj(m) such that R(a)
          ∧ m is conceptually specified

r ∈ ℝ 
m ∈ M
v ∈ m

∞ = x

or you wouldn't really say v ∈ m if m ∈ M, but you could if m ∈ M_op (I think?)

So more precisely:
m ∈ M_op
v ∈ m

Or more precisely and less easy to read:
m_op ∈ M_op
v_m_op ∈ m_op

or something?

## Definitions

### Metric

A measurement of something we care about.

The below categorizations of metrics are defined by their method of production (or lack thereof) and
of our ability to conceptualize them.

#### OM: Obvious Metric

- obvious metrics are metrics which can be programmatically derived from our records
- examples: 
  - token usage
  - tool success rate
  - number of tools
  - tool use frequency of various tools

OMs may be directly aggregated from our records. i would also include information directly derived combinations of other obvious metrics to also be obvious metrics, such as correlation of the use of a given tool with success rate or token usage on a given eval task. obvious metrics may be derived programmatically, without manual analysis once the functions for aggregation and combination have been defined. these obvious metrics, where they are not included in our ploke-eval cli tools already, are good candidates for a per-target patch summary or full-repo target summary or full batch on all repos summary. 
- Om: Obvious Metric
#### NOM: Non-Obvious Metric

- non-obvious metrics are metrics that are not possible to trivially derive programmatically
- there may or may not exist a clear protocol, loosely defined, which results in a reliable metric. 
- examples: 
  - the number of tool calls used to find the intended target code item
  - success or failure to find the intended code item
  - delta of minimal possible tool calls for successful patch (theoretical)
  - minimum possible context (in tokens) for successful patch
  - number pf tokens used on useful vs useless information
  - number of search results which are relevant to the jntended search result

NOMs are metrics which require some amount of abstract reasoning and jusgement to correctly
categorize on a case by case basis, and so are good targets for an llm skill (or set of skills). 

In the ideal case, we can define a set of repeatable steps that use our ploke-eval cli to arrive at
the non-obvious metrics by following a minimal set of tool calls and judgement procedures on a given
target eval.

#### CM: Conceptual Metric

- A metric for which there is no project-local known method for either deriving programmatically or
  adjudicating via LLM protocol.
- A metric that we can conceptually articulate, even if we cannot find an effective method to
  measure or derive it.
- There may or may not exist a method to transition this item to an OM or NOM.
- Examples:

#### UD: Unknown Dimension

- An unknown dimension that has probably matters but which we aren't aware of yet.
- Could be useful if we develop a theory that can account for unknown factors.
- Examples:
  - I don't think the concept of an example actually applies here.
  - The best analogy would be something like dark matter for physics.

#### Specification x
- what evidence may be used
- what evidence may not be used
- whether hindsight is allowed
- whether external context is allowed
- what counts as sufficient evidence

#### Reproducability Rep(x, e)
- dependent on execution outcome under an evaluator

Rep(x, e) := x is reproducible on the same inputs for some evaluator e

#### Method

#### Input/Evidence Contract
- I(x) input/evidence contract

## Boundaries

#### Metrics

- OM ∩ NOM = ∅
  - OM: effectively automated via programmatic functions
  - NOM: effectively automated via LLM adjudication
  - OM and NOM are disjoint operational classes
    - They are both operationally defined, and a given metric is either an OM or a NOM.
  - neither is a subset of the other.

- CM vs. (OM ∪ NOM)


## Reference / Shorthand

These are some conventions that are local to this project, and are helpful in using an
understandable shorthand for conceptual and planning docs.

Rather than allow them to remain vague, we define them below, enumerating where possible the cases
they refer to, but they are not key definitions in the same sense as the above, and serve more to
qualify statements in a way which makes sense to this project and its resource constraints.

#### Effectively

Example: Obvious Metrics (OMs) are *effectively* closed under programmatic operations.

Explanation: In the formal sense, the above statement is untrue, or at least uncertain. However, in
this project and under this project's resource constraints re: time to develop an algorithm or
function which could reliably identify, e.g. "success or failure to find the intended code item",
the statement is true.

## More Formal Terms
- protocol
- procedure
- specification: 
- adjudication protocol
- operationalization
- measurement procedure
- annotation protocol
- evaluation protocol
