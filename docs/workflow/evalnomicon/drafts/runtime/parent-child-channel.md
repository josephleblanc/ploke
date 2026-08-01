# Parent/Child Runtime Channel Plan

Date: 2026-05-05

This draft records the Prototype 1 parent/child filesystem communication
surface and the intended direction for replacing the scattered file protocol
with a role-indexed runtime channel contract.

Hard rule: parent/child lifecycle state must be driven by the per-runtime
`Channel`, not by compatibility projections. Files such as the shared transition
journal, latest runner-result projections, branch registries, scheduler views,
diagnostic streams, and monitor tables are for later reconstruction,
observability, and debugging. They must not become the authority that advances
parent/child state.

## Current Shape

The live child path is partially migrated to a type-level parent/child channel.
It still uses several filesystem surfaces, each with its own producer/consumer
convention, but C3/C4 now use the per-runtime channel for ready/evaluating and
terminal child observation.

### Parent To Child

- `nodes/<node-id>/invocations/<runtime-id>.json`
  - Current role: bootstrap contract for one runtime attempt.
  - Code: `Invocation` carries `role`, `campaign_id`, `node_id`,
    `runtime_id`, and `journal_path` in
    `crates/ploke-eval/src/cli/prototype1_state/invocation.rs:93`.
  - Code: parent writes the child invocation before spawn in
    `crates/ploke-eval/src/cli/prototype1_state/c3.rs:542`.
  - Code: the child loads the invocation through the runner path in
    `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`.
  - Assessment: per-attempt and useful, but it is a launch descriptor, not a
    full role-indexed communication channel.

- `nodes/<node-id>/runner-request.json`
  - Current role: work descriptor for the child evaluation.
  - Code: path helper is
    `crates/ploke-eval/src/intervention/scheduler.rs:256`.
  - Code: child loads it in
    `crates/ploke-eval/src/cli/prototype1_process.rs:1995`.
  - Assessment: per-node, not per-attempt; it is part of the work payload.

- `nodes/<node-id>/node.json`
  - Current role: durable node summary used by parent and child.
  - Code: path helper is
    `crates/ploke-eval/src/intervention/scheduler.rs:252`.
  - Code: child loads it in
    `crates/ploke-eval/src/cli/prototype1_process.rs:1994`.
  - Assessment: per-node projection/evidence, not a channel endpoint.

- `nodes/<node-id>/worktree/`
  - Current role: child Artifact filesystem surface.
  - Assessment: artifact substrate, not runtime messaging.

- `nodes/<node-id>/bin/ploke-eval`
  - Current role: child binary created by the parent before spawn.
  - Code: parent spawns the binary with invocation args and environment in
    `crates/ploke-eval/src/cli/prototype1_state/c3.rs:578`.
  - Assessment: execution substrate. The digest-preserved `ploke-eval` surface
    is what lets parent and child share the compiled protocol contract.

- `nodes/<node-id>/streams/<runtime-id>/{stdout.log,stderr.log}`
  - Current role: process stdout/stderr redirection.
  - Code: stream paths are opened before spawn in
    `crates/ploke-eval/src/cli/prototype1_state/c3.rs:577`.
  - Assessment: per-attempt diagnostic stream, not structured protocol.

### Child To Parent

- `transition-journal.jsonl`
  - Current role: shared append-only transition projection.
  - Code: `Child<S>` stores one `journal_path` and appends lifecycle entries in
    `crates/ploke-eval/src/cli/prototype1_state/child.rs:183`.
  - Code: child transitions write `Child<Ready>`, `Child<Evaluating>`, and
    `Child<ResultWritten>` in
    `crates/ploke-eval/src/cli/prototype1_state/child.rs:144`,
    `crates/ploke-eval/src/cli/prototype1_state/child.rs:151`, and
    `crates/ploke-eval/src/cli/prototype1_state/child.rs:159`.
  - Code: C4 appends `ObserveChild(Before/After)` entries as reconstruction
    evidence after using the per-runtime channel in
    `crates/ploke-eval/src/cli/prototype1_state/c4.rs`.
  - Assessment: entries identify `runtime_id` and node refs, but the file is
    shared across children. It is a reconstruction surface and fanout race
    surface, not parent/child lifecycle authority.

- `nodes/<node-id>/results/<runtime-id>.json`
  - Current role: attempt-scoped terminal runner result.
  - Code: child writes this in
    `crates/ploke-eval/src/cli/prototype1_process.rs:1626`.
  - Code: the terminal channel `Result` embeds the runner result and, on
    success, the treatment evidence needed by parent comparison.
  - Assessment: this is useful reconstruction evidence for one attempt. It
    should not by itself advance successful parent observation because it does
    not carry treatment evidence.

- `nodes/<node-id>/runner-result.json`
  - Current role: latest node-level result projection.
  - Code: latest result path helper is
    `crates/ploke-eval/src/intervention/scheduler.rs:260`.
  - Code: attempt result write also updates this projection in
    `crates/ploke-eval/src/cli/prototype1_process.rs:1629`.
  - Assessment: per-node mutable projection. It is for reconstruction and
    operator display only; it must not drive parent/child state.

- `evaluations/<branch-id>.json`
  - Current role: branch comparison report written by the parent after a
    successful channel observation carries treatment evidence.
  - Assessment: comparison evidence and later reconstruction surface, not a
    child-to-parent lifecycle channel.

- `branches.json`
  - Current role: branch registry plus latest evaluation summaries.
  - Assessment: shared mutable projection. It should not be treated as a
    child-to-parent channel.

### Parent-Owned Candidate Publication

- `messages/child-plan/<parent-node-id>.json`
  - Current role: typed parent-owned message describing the candidate set.
  - Code: `ChildPlanFiles` body contains scheduler, branch registry, parent,
    child generation, and child node/request addresses in
    `crates/ploke-eval/src/cli/prototype1_state/parent.rs:100`.
  - Code: `ChildPlanFile` resolves to
    `prototype1/messages/child-plan/<parent-node-id>.json` in
    `crates/ploke-eval/src/cli/prototype1_state/parent.rs:219`.
  - Code: `LockChildPlan` and `UnlockChildPlan` move
    `Parent<Ready> -> Parent<Planned> -> Parent<Selectable>` in
    `crates/ploke-eval/src/cli/prototype1_state/parent.rs:203`.
  - Assessment: this is the closest current example of the desired typed
    message-box pattern, but it is not the live per-child parent/child channel.

## Problem

The current filesystem protocol splits one logical child attempt across
bootstrap files, work files, process streams, a shared journal, attempt results,
latest-result projections, evaluation reports, scheduler state, and branch
registry state.

This causes three recurring problems:

- There are too many communication surfaces, so the controller can accidentally
  treat projections as protocol facts.
- Fanout creates shared-file races, especially for `transition-journal.jsonl`,
  `scheduler.json`, `branches.json`, and node mirrors.
- The authority to use a communication surface is not consistently carried by a
  role/state type. Some paths are type-shaped, but many are just filesystem
  conventions passed through invocation records or environment variables.

The repair direction is deliberately narrow: projections may be written and
read for later reconstruction, but they must not decide live lifecycle
advancement. Any path that advances parent/child state from a projection should
be treated as migration debt or a bug unless a typed transition explicitly
converts channel evidence into that projection.

## Intended Channel Model

The desired object is not just a transport trait. It is a role-indexed runtime
channel whose authority is produced by typed transitions.

The channel exists as shared compiled protocol surface. Parent and child may use
it because both are admitted runtimes preserving the same `ploke-eval` protocol
digest. The concrete file or socket remains untrusted external bytes crossing a
runtime boundary.

In shorthand:

```text
Channel<Role>
  = typed authority to use one side of a parent/child communication contract

Transport
  = backend mechanics for moving bytes, such as files or sockets

Envelope
  = serialized message with runtime ids, message kind, sequence/id, body hash,
    and payload

Projection
  = scheduler, branch registry, monitor table, latest-result file, etc.
```

The important invariant is:

```text
Parent<ChildLinked> may write ParentToChild and read ChildToParent.
Child<LinkedToParent> may write ChildToParent and read ParentToChild.
No Child<LinkedToParent> carrier gives write authority to a sibling child's
endpoint.
```

The file paths are deterministic projections of the typed channel contract. The
file path does not confer authority by itself.

## Expected Communication Pattern

This section describes the protocol shape the implementation should converge
on. It is intentionally written as a contract between runtimes, not as a list of
current helper functions.

### Authority, Evidence, And Projections

The parent/child runtime protocol should keep four categories separate:

- `Invocation`
  - Attempt-scoped bootstrap contract. It tells a fresh runtime which role it
    has, which campaign/node/runtime it belongs to, and where the per-runtime
    channel endpoints live.
  - It is not terminal evidence and it is not a mutable status file.

- `Channel<Role<State>, Transport>`
  - Live per-runtime communication authority.
  - The role/state parameter is the authority token. The transport address is
    only the backend used to move bytes.
  - Parent and child may use the channel because both runtimes were constructed
    with the same compiled protocol contract.

- attempt result
  - Reconstruction evidence for one concrete runtime attempt.
  - For the file transport migration this is
    `nodes/<node-id>/results/<runtime-id>.json`.
  - A `ResultWritten` channel message points at this evidence for compatibility
    readers. It is not the lifecycle message for new parent observation.

- projections
  - Shared journal entries, scheduler updates, latest-node runner results,
    branch registry summaries, monitor tables, and diagnostic streams.
  - These are views or compatibility surfaces derived from protocol evidence.
    They should not become the authority for a child attempt. Their job is
    reconstruction, observability, and post-run analysis.

- evaluation telemetry
  - Structured observations emitted by work that happens inside child
    evaluation, such as per-chat-step provider attempts from
    `crates/ploke-llm/src/manager/session.rs`.
  - These records are useful for operators and monitors because they show live
    child activity before the child writes its terminal attempt result.
  - They are not parent/child lifecycle authority. The parent may read them to
    report progress or diagnose slow children, but child selection and terminal
    classification should still flow through attempt results and explicit
    protocol messages.

### Normal Flow

The expected parent-to-child flow is:

```text
Parent:
  materialize child artifact/worktree
  build child binary
  write ChildInvocation {
    role,
    campaign_id,
    node_id,
    runtime_id,
    channel_root,
    ...
  }
  spawn child with the invocation path
```

The expected child startup flow is:

```text
Child<Starting>:
  load invocation
  construct Channel<Child<Starting>, Transport>
  send ToParent::Ready
  enter Child<Ready>
```

The expected evaluation flow is immediate after readiness:

```text
Parent:
  observe ToParent::Ready

Child<Ready>:
  send ToParent::Evaluating
  enter Child<Evaluating>
```

There is no parent `StartEvaluation` gate in the current protocol. The
`Ready` message tells the parent the child runtime is observable; it does not
wait for a second permission message before starting evaluation.

The expected terminal flow is:

```text
Child<Evaluating>:
  run evaluation
  write attempt result
  send ToParent::Result { runner_result, treatment? }
  exit

Parent:
  observe ToParent::Result from the per-runtime Channel
  classify child outcome
  compare treatment evidence for successful children
  write journal/scheduler/history projections
```

The ordering matters. `ToParent::Result` is the lifecycle message. Attempt
results, compatibility `ResultWritten { result_ref }` messages, and old shared
journal entries are reconstruction surfaces. New execution handoff should use
the direct `Result` payload so successful children cannot be observed without
treatment evidence.

In the current child runner, treatment evidence is assembled after treatment
eval/protocol closure from `closure-state.json` plus complete run records. The
complete-treatment gate runs before a success result is built. If any treatment
instance lacks metrics, the child writes a failed runner result and sends a
terminal `ToParent::Result` without a `treatment` payload. A result file or
`Child<ResultWritten>` projection is therefore never enough to prove successful
treatment output.

During `run evaluation`, the child may call subsystems that emit their own
structured external observations. For example, the LLM chat-step path records
`ProviderAttempt` telemetry through tracing when HTTP attempts complete. These
events let a monitor show that a child is still active, waiting on a provider,
retrying, or making progress through chat steps. They should be modeled as an
observer-readable telemetry stream associated with the child runtime, not as
the child-to-parent lifecycle channel itself.

### Failure And Termination Rules

The channel protocol should make child failure observable without requiring the
parent to infer everything from timeouts:

```text
Child<Starting | Ready | Evaluating>:
  on known local failure:
    send ToParent::Failed { detail }
    write terminal attempt result when possible
    exit

Child<Starting | Ready | Evaluating>:
  before or during normal process shutdown:
    send ToParent::Exited { status } when possible
```

The parent should handle terminal conditions in this order of authority:

```text
1. child channel reports Result/Failed/Exited for this runtime
2. bounded observation timeout expires
3. child process status says the process exited
4. compatibility projections are read only to reconstruct what happened
```

The exact implementation may poll more than one surface in the same loop, but
only the per-runtime channel should advance parent/child lifecycle state. It
should not wait indefinitely after the child process exits. A post-ready child
that exits without a channel terminal message should become durable failed child
evidence through an explicit transition, not through ad hoc projection reads.

Telemetry may influence operator display, timeout diagnostics, and later
adaptive policy, but it should not by itself advance the child lifecycle state.
For example, a recent provider-attempt event can explain why a child has not
finished yet, but it is not equivalent to `Child<Evaluating>` or
`Child<ResultWritten>`.

### File Transport Shape

The first transport projection remains file-backed:

```text
nodes/<node-id>/channels/<runtime-id>/
  parent-to-child.jsonl
  child-to-parent.jsonl
```

Those files are per-runtime buffers. A child runtime gets write authority only
to its own `child-to-parent.jsonl`; it does not get authority to write sibling
child endpoints. The parent may read all child-to-parent endpoints for children
it spawned and may write parent-to-child endpoints according to its current
role/state.

The file transport should validate envelope identity on read:

```text
schema_version
direction
campaign_id
node_id
runtime_id
body_hash
```

The body hash is part of the boundary check. If it is present in the envelope,
readers should recompute and validate it rather than treating it as decorative
metadata.

## Candidate Type Shape

This is the direction to refine, not final API.

```rust
pub(crate) struct Channel<R, T> {
    transport: T,
    _role: PhantomData<R>,
    _private: Private,
}

pub(crate) trait CanSend<M> {}
pub(crate) trait CanRecv<M> {}

pub(crate) trait Transport {
    type Error;

    fn write(&self, endpoint: &Endpoint, envelope: &[u8]) -> Result<Receipt, Self::Error>;
    fn read(&self, endpoint: &Endpoint) -> Result<Option<Vec<u8>>, Self::Error>;
}
```

Allowed operations should be available only under role bounds:

```rust
impl<R, T> Channel<R, T>
where
    R: CanSend<ChildToParent>,
    T: Transport,
{
    fn send_child_to_parent(&self, message: ChildToParent) -> Result<Receipt, T::Error>;
}
```

This keeps transport generic while making authority role-indexed.

## Message Families

The initial protocol can absorb the current scattered child surfaces into a
small set of messages:

```rust
enum ParentToChild {
    Cancel,
}

enum ChildToParent {
    Ready,
    Evaluating,
    Result {
        runner_result: RunnerResult,
        treatment: Option<TreatmentEvidence>,
    },
    ResultWritten { result_ref: ResultRef },
    Failed,
    Exited,
}
```

The attempt result payload can remain a separate content/evidence artifact at
first. `ChildToParent::Result` is the lifecycle message. `ResultWritten` is a
compatibility/reconstruction notification only and should not drive
parent/child state.

## File Transport Projection

The first backend should be file-backed because it matches the current runtime
surface and can be migrated incrementally.

One possible projection:

```text
nodes/<node-id>/channels/<runtime-id>/
  parent-to-child.jsonl
  child-to-parent.jsonl
```

Optional later projection:

```text
prototype1/channels/broadcast/<parent-runtime-id>.jsonl
```

The broadcast surface is parent-write, child-read. Children should not get a
role carrier that can write to it.

## Migration Plan

1. Define the role-indexed channel types and a file transport adapter.
   Do not change live behavior yet.

2. Add tests proving that `Channel<ParentRole>` and `Channel<ChildRole>` expose
   only the appropriate send/receive directions.

3. Move child lifecycle communication from direct shared-journal appends to the
   per-attempt channel:
   `Ready`, `Evaluating`, terminal `Result { runner_result, treatment }`,
   `Failed`, `Exited`. Compatibility `ResultWritten` records may still point at
   result artifacts, but must not be lifecycle authority.

4. Keep writing `transition-journal.jsonl` as a projection during migration,
   but make it downstream of channel messages rather than the primary
   communication surface.

5. Move `Invocation` toward an initial channel/bootstrap envelope or a narrow
   compatibility projection of that envelope.

6. Keep `nodes/<node-id>/results/<runtime-id>.json` as the attempt result
   payload/ref. Read it for reconstruction and post-observation analysis, not
   as a substitute for the terminal channel result.

7. Treat `scheduler.json`, `branches.json`, and `runner-result.json` as
   projections. They should not be required for parent/child communication once
   the channel path is live.

8. After file transport is stable, add a socket transport implementing the same
   role-indexed channel contract.

## Preservation Checks

- The channel is not trusted because the file or socket is trusted.
- The authority to use the channel is carried by role/state types.
- The child can only use the channel if it was constructed through the admitted
  runtime path preserving the shared `ploke-eval` protocol surface.
- The transport backend moves untrusted bytes; receive paths must validate
  envelope identity, runtime ids, message kind, sequence/hash, and expected
  receiver.
- History/Crown authority remains separate. Channel messages are evidence until
  admitted or imported under explicit policy.
