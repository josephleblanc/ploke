# experiment on our protocol implementation

tracks the protocol that was developed in `ploke-protocol` around commit:

feb937ce: Rewrite ploke-protocol with typed procedure model

## Initial impressions

### Positive
New protocol implementation looks much better than the last one. More type-safety, better data model that seems to more faithfully implement the conceptual framework re: metric, admissable evidence contract.

One very positive note is that it bakes in the idea of a split, which is good, so branching becomes part of the type structure and supports possibly multiple concurrent threads of execution, which makes a lot of sense for a command that might be useful with a concept of "forking" an stateful sequence that may not depend on state updates for each sequential action, e.g.

Roughly, let `->` indicate "modifies state relevant to"
```text
A -> B, C, D
B ->       D
C ->       D

Then we can compose procedures:
  concurrently, effectively "forking"
    A -> B
    A -> C

  then "joining" or "merging"
      B, C -> D
```

This is a powerful abstraction, which I would like to see continue.

### Negative

The type-state is awesome, but I don't think it looks very composable as a way of defining the complex workflow that the composition would enable.

I don't know if this is a fundamental limitation of the type system, or if there is another way to express something like this, but I don't see a 10-step procedure being easy to maintain if just a couple elements together look like:

```rust
use crate::core::{ SequenceArtifact, ProcedureArtifact };
use crate::step::MechanizedProvenance;
use crate::llm::JsonLlmProvenance;
pub type ToolCallReviewArtifact = ProcedureArtifact<
    SequenceArtifact<
        StepArtifact<trace::Trace, Evidence, MechanizedProvenance>,
        StepArtifact<Evidence, Judgment, JsonLlmProvenance>,
    >,
>;
```

Or maybe this isn't so bad? I'm not sure. At the least, we probably want to have intermediate or convenience types to represent the different classes, or type aliases for things like `Evidence, Judgment, JsonLlmProvenance`

I did get a bit of pushback about the idea of using iterators, but honestly I was too relieved that the LLM was giving a more conceptually rigorous architecture to inquire further. I'm not sure how we can compose these kinds of types in a reasonable way, but I'm curious and want to see how this experiment continues to develop.

## Comparative information value to previous inspect tool-calls workflow

The LLM had a really, really good distilled way of framing the protocol, which I'll have them put below:

> A protocol is not valuable just because it is typed or LLM-backed. It is
> valuable if it produces a value for a metric we care about, under an
> admissible evidence contract, in a form we can actually use. If the output is
> not useful, then either the metric is not actually one we care about, the
> protocol is underspecified or badly shaped, or the protocol is too weakly
> connected to the useful evidence surface.
>
> The old workflow is currently a useful diagnostic procedure, even if it is
> informal. The new protocol does not need to copy it exactly, but it does need
> to compete with it in epistemic value. If it cannot get there, then we should
> change the protocol rather than preserve it out of attachment to the current
> implementation.
>
> So the practical goal is not to finish persistence first. It is to get one
> small protocol working live, pressure-test it against useful existing
> practice, and use that comparison to decide what a genuinely worthwhile first
> machine-readable non-obvious metric should be.
