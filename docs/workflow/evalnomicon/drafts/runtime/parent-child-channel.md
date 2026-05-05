# Parent/Child Runtime Channel Plan

Date: 2026-05-05

This draft records the current Prototype 1 parent/child filesystem communication
surface and the intended direction for replacing the scattered file protocol
with a role-indexed runtime channel contract.

## Current Shape

The live child path does not currently communicate through one type-level
parent/child channel. It uses several filesystem surfaces, each with its own
producer/consumer convention.

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
  - Current role: shared append-only transition stream.
  - Code: `Child<S>` stores one `journal_path` and appends lifecycle entries in
    `crates/ploke-eval/src/cli/prototype1_state/child.rs:183`.
  - Code: child transitions write `Child<Ready>`, `Child<Evaluating>`, and
    `Child<ResultWritten>` in
    `crates/ploke-eval/src/cli/prototype1_state/child.rs:144`,
    `crates/ploke-eval/src/cli/prototype1_state/child.rs:151`, and
    `crates/ploke-eval/src/cli/prototype1_state/child.rs:159`.
  - Code: parent discovers result paths by reading the shared journal in
    `crates/ploke-eval/src/cli/prototype1_state/c4.rs:227`.
  - Assessment: entries identify `runtime_id` and node refs, but the file is
    shared across children. This is the main fanout race surface.

- `nodes/<node-id>/results/<runtime-id>.json`
  - Current role: attempt-scoped terminal runner result.
  - Code: child writes this in
    `crates/ploke-eval/src/cli/prototype1_process.rs:1626`.
  - Code: parent loads it after journal discovery in
    `crates/ploke-eval/src/cli/prototype1_state/c4.rs:311`.
  - Assessment: this is the cleanest current per-child/per-attempt result
    surface.

- `nodes/<node-id>/runner-result.json`
  - Current role: latest node-level result projection.
  - Code: latest result path helper is
    `crates/ploke-eval/src/intervention/scheduler.rs:260`.
  - Code: attempt result write also updates this projection in
    `crates/ploke-eval/src/cli/prototype1_process.rs:1629`.
  - Assessment: per-node mutable projection, not the best evidence for a
    concrete attempt.

- `evaluations/<branch-id>.json`
  - Current role: branch comparison report written during child evaluation and
    loaded by the parent when the runner result succeeded.
  - Code: parent loads the evaluation artifact after loading the runner result
    in `crates/ploke-eval/src/cli/prototype1_state/c4.rs:328`.
  - Assessment: evaluation evidence referenced by the terminal result, not the
    primary child lifecycle channel.

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
    Bootstrap,
    StartEvaluation,
    Cancel,
}

enum ChildToParent {
    Ready,
    Evaluating,
    ResultWritten { result_ref: ResultRef },
    Failed,
    Exited,
}
```

The attempt result payload can remain a separate content/evidence artifact at
first. `ChildToParent::ResultWritten` should point at it by ref/path/hash rather
than inline all evaluation output.

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
   `Ready`, `Evaluating`, `ResultWritten`, `Failed`, `Exited`.

4. Keep writing `transition-journal.jsonl` as a projection during migration,
   but make it downstream of channel messages rather than the primary
   communication surface.

5. Move `Invocation` toward an initial channel/bootstrap envelope or a narrow
   compatibility projection of that envelope.

6. Keep `nodes/<node-id>/results/<runtime-id>.json` as the attempt result
   payload/ref. Prefer it over `runner-result.json` for selection and timing.

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

