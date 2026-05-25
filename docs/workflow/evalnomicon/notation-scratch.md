# Metric Operationalization Notation Scratch

This note is a staging area for notation and definitions that may later move
into `src/core/conceptual-framework.md`.

## Metric Classes

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

## Method, Inputs, And Execution

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

## Method-Spec Properties

These are properties of the method specification itself:

```text
B(x)   := x is bounded in time, scope, and effort
E(x)   := x is explicit enough to be written down and followed
Adm(x) := x specifies an admissible input domain I_x
```

Interpretation:

- `B(x)` asks whether the method can be carried out within a predictable amount
  of work.
- `E(x)` asks whether the method is concrete enough to support consistent
  execution.
- `Adm(x)` asks whether the method makes clear what evidence may and may not be
  used.

## Execution Properties

These are properties of an executor applying a method:

```text
Rep(e, x) := executor e produces consistent results when following x on the same inputs
Rel(e, x) := executor e applies x reliably enough for routine workflow use
Cal(e, x) := executor e is acceptably calibrated relative to a stronger reference for x
```

Interpretation:

- `Rep(e, x)` is about repeatability on fixed inputs.
- `Rel(e, x)` is about practical workflow trustworthiness.
- `Cal(e, x)` is about agreement with a stronger baseline such as a
  higher-effort human review or a more trusted derivation procedure.

## Operational Reliability

```text
R(e, x) := B(x) ∧ E(x) ∧ Adm(x) ∧ Rep(e, x) ∧ Rel(e, x)
```

Notes:

- `Cal(e, x)` is intentionally kept outside `R(e, x)` for now.
- This allows calibration to be tracked separately from whether a method is good
  enough for routine use.
- If calibration later becomes a hard requirement for some class of metrics, it
  can be folded into a stronger predicate.

## Class Membership By Operationalization

Loose classification sketch:

```text
m ∈ O iff there exists a programmatic executor e and method x for m such that R(e, x)

m ∈ N iff there does not yet exist a satisfactory programmatic operationalization
         for m, but there exists an adjudicative executor e and method x for m
         such that R(e, x)

m ∈ C iff m is conceptually specified, but no acceptable operationalization yet exists
```

Interpretation:

- `O` is mechanized operationalization.
- `N` is adjudicated operationalization.
- `C` is named but not yet operationalized.

## Why This Split Matters

This decomposition separates three things that were previously collapsing into
one another:

1. the metric itself
2. the method specification used to produce it
3. the executor that actually carries out that method

That distinction matters because many quality properties belong to different
layers:

- boundedness and explicitness belong mainly to the method specification
- reproducibility and reliability belong mainly to executor-plus-method
- calibration requires comparison against a stronger reference procedure

This makes it easier to reason about where a proposed metric or protocol is
weak: the problem may be in the metric definition, the method design, the input
contract, the executor, or the calibration process.
