# Protocol Typing Scratch

This note connects the abstract metric/method/executor framing to a more
concrete notion of typed protocol steps.

It is intentionally rough and is meant as a staging area for ideas that may
later move into the `core` or `protocols` sections of the book.

## From Metrics To Protocols

Earlier notation distinguished:

- `m`: a metric
- `x`: a method specification for producing `m`
- `I_x`: the admissible inputs for `x`
- `e`: an executor of `x`

One useful refinement is that a method specification may itself be composed of
multiple ordered steps:

```text
x = (x_1, x_2, ..., x_n)
```

Each step may have:

- its own input domain `I_i`
- its own output domain `O_i`
- its own executor `e_i`

The method is composable when:

```text
O_i ⊆ I_(i+1)
```

That is: the output of one step is admissible input to the next.

This is useful because many review procedures are mixed-mode:

- some steps are mechanized
- some steps are LLM-adjudicated
- some steps may eventually be human validation passes

## Step-Local Executors

For many NOM-oriented protocols, executor identity is better modeled per step
than per method.

Example:

- `e_1`: deterministic CLI/data transform
- `e_2`: LLM adjudicator
- `e_3`: deterministic CLI/data transform

This is cleaner than pretending the entire method has one executor.

## Function-Like View

For a single step:

```text
Exec(e_i, x_i, i_i) = o_i
```

where:

- `x_i` is a step specification
- `i_i ∈ I_i`
- `o_i ∈ O_i`
- `e_i` is the executor for that step

At the method level, the pipeline is valid when each step's output fits the
next step's input contract.

## Rust-Style Framing

One way to think about this in Rust terms is:

```rust
trait ProtocolStep<I, O> {
    fn run(input: I) -> O;
}
```

This works well for mechanized steps.

For adjudicative steps, it is useful to distinguish the step specification from
the executor:

```rust
trait Executor<Spec, Input, Output> {
    fn execute(spec: Spec, input: Input) -> Output;
}
```

This is only pseudocode, but it captures the separation between:

- the protocol step as a specification
- the executor that carries out that step

## Worked Example: Suspicious Tool Call Review

Suppose we want a protocol that:

1. inspects tool calls for a run
2. selects a suspicious call for closer review
3. inspects that one tool call in detail

### Step 1: Mechanized extraction

```rust
struct RunId(String);

struct PrintedToolCallLine {
    index: usize,
    summary: String,
}

struct Step1InspectToolCalls;

impl ProtocolStep<RunId, Vec<PrintedToolCallLine>> for Step1InspectToolCalls {
    fn run(input: RunId) -> Vec<PrintedToolCallLine> {
        // conceptually: `ploke-eval inspect tool-calls ...`
        todo!()
    }
}
```

This step is mechanized and has a fairly clean input/output boundary.

### Step 2: Adjudicative selection

```rust
struct SuspiciousToolCallIndex(usize);

struct Step2SelectSuspiciousCall;

trait LlmJudge<I, O> {
    fn judge(input: I) -> O;
}

struct ChatGpt54;

impl LlmJudge<Vec<PrintedToolCallLine>, SuspiciousToolCallIndex> for ChatGpt54 {
    fn judge(input: Vec<PrintedToolCallLine>) -> SuspiciousToolCallIndex {
        // prompt-governed judgment
        todo!()
    }
}
```

This step is not mechanized, but it is still typed:

- input: a list of printed tool-call lines
- output: one selected suspicious index

That typing boundary is valuable even if the execution is probabilistic.

### Step 3: Mechanized detail inspection

```rust
struct ToolSummary {
    index: usize,
    detail: String,
}

struct Step3InspectToolCallDetail;

impl ProtocolStep<SuspiciousToolCallIndex, ToolSummary> for Step3InspectToolCallDetail {
    fn run(input: SuspiciousToolCallIndex) -> ToolSummary {
        // conceptually: `ploke-eval inspect tool-call {index}`
        todo!()
    }
}
```

This yields a mixed protocol:

```text
RunId
  -> Vec<PrintedToolCallLine>
  -> SuspiciousToolCallIndex
  -> ToolSummary
```

where the middle step is adjudicative and the outer steps are mechanized.

## Why This Matters

This framing helps in several ways:

1. It shows that a NOM protocol may still contain many OM-like substeps.
2. It makes the boundaries between adjudication and mechanized inspection
   explicit.
3. It provides a way to reason about protocol quality step by step rather than
   as one opaque review blob.
4. It starts to look like a typed interface, which makes later implementation
   or automation more plausible.

## Relation To Reliability

Once steps are typed, quality questions can be asked per step:

- Is the step specification explicit enough?
- Is the step bounded?
- Is the input contract clear?
- Is the executor reproducible on the same inputs?
- Is the executor calibrated against a stronger reference?

That helps isolate whether a weak protocol is failing because:

- the step is poorly specified
- the inputs are underspecified
- the executor is inconsistent
- the output type is too coarse or too loose

## Provisional Design Principle

When possible, define NOM protocols as compositions of typed steps with explicit
input/output contracts, even when one or more steps are adjudicative rather than
mechanized.

This does not remove uncertainty, but it makes uncertainty localizable.
