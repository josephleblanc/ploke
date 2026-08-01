# Protocol Operationalization Memory

This note preserves the conceptual thread behind the first `ploke-protocol`
bootstrap so a cold restart does not reduce the work to "we added a crate and a
command."

## Why `ploke-protocol` Exists

The core design move was to separate:

- the metric itself
- the method specification for producing that metric
- the executor that carries out that method

That separation became necessary because NOM work is not just "ask an LLM to
judge a trace." A workable NOM procedure needs:

- bounded admissible inputs
- a method specification
- one or more typed protocol steps
- an executor per step
- a typed output
- a way to reason about reliability, reproducibility, and later calibration

`ploke-protocol` is the first implementation of that split.

## Core Distinctions

### Metric Classes

We converged on a practical maturity ladder:

```text
D -> C -> N -> O
```

Where:

- `D`: uncaptured dimensions
- `C`: conceptual metrics
- `N`: non-obvious metrics
- `O`: obvious metrics

This is a common maturation path, not a law. The point is that a metric can
move from vague concern, to named concept, to adjudicable protocol, to
mechanized derivation.

### Metric vs Value

One useful clarification was:

- `m ∈ M`: `m` is a metric in the framework
- `v ∈ Val(m)`: `v` is a value in the value domain of metric `m`

This mirrors the distinction between a typed quantity and one particular value
of that quantity. It matters because methods and protocols produce values for
metrics; they do not produce metrics themselves.

### Method vs Executor

We tightened the earlier notation to:

- `x`: a method specification for producing metric `m`
- `I_x`: the admissible input domain for method `x`
- `e`: an executor of method `x`

Function-like view:

```text
Exec(e, x, i) = v, where i ∈ I_x
```

This lets us say:

- boundedness and explicitness belong mainly to the method specification
- reproducibility and reliability belong mainly to executor-plus-method
- calibration is a further property of executor performance against a stronger
  reference

That distinction is one of the main conceptual reasons the crate boundary is
useful.

### Typed Protocol Steps

The other key move was to think of NOM procedures as compositions of typed
steps, not as one opaque blob.

Conceptually:

```text
x = (x_1, x_2, ..., x_n)
```
Each step may have:

- its own input domain
- its own output domain
- its own executor

This matters because many useful NOM procedures are mixed-mode:

- mechanized extraction/preparation
- adjudicative LLM step
- mechanized parsing/validation

The value of typing these steps is not theoretical neatness. It is that it
makes uncertainty and failure localizable.

## What The Current Code Encodes

The current `ploke-protocol` bootstrap already encodes several of these
commitments:

- [core.rs](/home/brasides/code/ploke/crates/ploke-protocol/src/core.rs)
  - `Metric`
  - `Protocol`
  - `ProtocolStep`
  - `Executor`
  - `ExecutorKind`
  - `Confidence`

- [llm.rs](/home/brasides/code/ploke/crates/ploke-protocol/src/llm.rs)
  - a one-shot JSON adjudication wrapper over `ploke-llm`

- [tool_call_review.rs](/home/brasides/code/ploke/crates/ploke-protocol/src/tool_call_review.rs)
  - a first bounded protocol around reviewing one indexed tool call

- [crates/ploke-eval/src/cli.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs)
  - `ploke-eval protocol tool-call-review`

That command is important because it proves the architectural path:

1. read real eval artifacts
2. build a typed subject
3. run a bounded one-shot adjudicative step
4. parse typed output
5. print a usable result

## What Is Intentionally Not Solved Yet

The current implementation is a bootstrap, not the full framework.

Still missing or intentionally thin:

- persisted protocol artifacts
- explicit calibration against stronger references
- richer input packets for tool-call review
- aggregation from many local reviews into higher-level NOMs
- protocol registries or reusable protocol manifests
- a second protocol that proves the architecture generalizes

## Why The First Protocol Is Local

A major design decision was to start with a local bounded protocol rather than a
whole-run NOM.

That choice was deliberate:

- whole-run NOMs quickly introduce hindsight policy, repeated adjudication,
  ambiguous search loops, and larger context windows
- a local protocol can still be reused upward later
- a local protocol gives us a cleaner place to test typed inputs, structured
  outputs, replayability, and eventual calibration

So the first command is not the destination. It is the first review atom that
larger NOM procedures can later compose.

## Immediate Next Moves

The next steps that best preserve the architecture are:

1. persist protocol-run artifacts, not just stdout output
2. enrich the tool-call review input packet so the LLM sees more than a compact
   summary line
3. decide which second bounded protocol best tests generality without requiring
   open-ended loops

Good candidates for a second bounded protocol:

- search-result relevance for one search-producing call
- target-set contact for one localized review slice
- tool-call misuse classification for one indexed call with richer local context

## Source Trail

If nuance is lost in a restart, the relevant nearby sources are:

- [notation-scratch.md](/home/brasides/code/ploke/docs/workflow/evalnomicon/notation-scratch.md)
- [protocol-typing-scratch.md](/home/brasides/code/ploke/docs/workflow/evalnomicon/protocol-typing-scratch.md)
- [conceptual-framework.md](/home/brasides/code/ploke/docs/workflow/evalnomicon/src/core/conceptual-framework.md)
- [2026-04-13 cleaned chat export](/home/brasides/code/ploke/docs/workflow/evalnomicon/chat-history/2026-04-13_codex-session-019d8a3a_cleaned.md)

Those are the primary source materials for the abstractions that became
`ploke-protocol`.
