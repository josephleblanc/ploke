# Prototype 1 Message Invariant Review B

Scope: `crates/ploke-eval/src/cli/prototype1_state/inner.rs`, `parent.rs`, and `cli_facing.rs`.

Required invariant: after a sender packs a message, code must not be able to claim, record, send, or advance as if the message crossed runtimes unless the exact intended receiver consumes the exact packed message. File read/write errors must produce typed failure, not silent loss or ambiguous sent state.

Verdict: the current protocol does not prove the stronger invariant. It has a useful `Open<M>` obligation, but `Packed<M>` is not linear, is easy to discard or duplicate, and is not the required capability for later advancement. The concrete receiver path immediately consumes the message in the same runtime and then throws away the resulting type state. Subsequent advancement is driven from scheduler/registry JSON inference, not from a consumed packed message.

## Counterexamples

### 1. Same-runtime self-receive satisfies the type transition

`run_parent_target_selection` opens a `ChildPlan` from `Parent<Ready>`, runs target selection, packs the plan, and immediately calls `packed.receive(parent)` with the `Parent<Planned>` returned by the same `pack` call:

- `cli_facing.rs:329`: `Open::<ChildPlan>::from_sender(parent)`
- `cli_facing.rs:332-339`: `open.pack(...)`
- `cli_facing.rs:340-345`: `packed.receive(parent)`
- `parent.rs:183-187`: `ChildPlan` defines `SenderClosed = Parent<Planned>` and `ReceiverOpen = Parent<Planned>`

This proves only that the sender can consume its own packed message. It does not prove that another runtime read the persisted buffer. `Parent<Selectable>` is bound to `_parent` and dropped immediately, so the later controller path does not require the receiver-ready state.

Recommended change: split the sender-closed state from the receiver-open state. The sender should receive something like `Parent<PlanWritten>` plus a durable `Sent<ChildPlan>` handle. The receiver should be a separately loaded `Parent<AwaitingChildPlan>` or `Runtime<Receiver, Awaiting<ChildPlan>>`, and only that receiver should be able to consume a loaded, validated packed message into `Parent<Selectable>`.

### 2. Later advancement bypasses `Packed<ChildPlan>` entirely

The actual candidate selection path uses scheduler/registry state to infer runnable nodes:

- `cli_facing.rs:2227-2299`: `runnable_candidate_nodes` reads scheduler and registry JSON and returns `CandidateNode`s.
- `cli_facing.rs:2363-2421`: `resolve_next_candidate_node_id` returns a `String` node id.
- `cli_facing.rs:2394-2421`: if candidates already exist, no `ChildPlan`, no `Packed`, and no receive are involved.
- `cli_facing.rs:2400-2410`: if no candidates exist, target selection is run, its typed receive result is ignored, then candidate resolution is repeated from files.

Concrete bypass: create or leave a scheduler entry for generation `parent.generation + 1` with `parent_node_id` equal to the active parent. The parent can advance to `C1::load` using the inferred node id without possessing a `Packed<ChildPlan>` or a `Parent<Selectable>` derived from receipt of that exact message.

Recommended change: make the selector require a received capability, for example `select_child(parent: Parent<Selectable>, receipt: Received<ChildPlan>) -> Result<Child<Candidate>, ...>`. Scheduler and registry reads should validate the received message, not replace it.

### 3. `Packed<M>` is duplicable and droppable

`Packed<M>` derives `Clone` and has no `Drop` guard or `must_use` obligation:

- `inner.rs:166-171`: `#[derive(Debug, Clone, PartialEq, Eq)] pub(crate) struct Packed<M>`
- `inner.rs:183-198`: `receive(self, ...)` consumes only one clone.
- `parent.rs:43-48`: `Parent<S>` also derives `Clone`.

After one successful pack, code in the crate can duplicate both the planned parent and the packed message:

```rust
let (planned, packed) = open.pack(files, write)?;
let _first = packed.clone().receive(planned.clone())?;
let _second = packed.receive(planned)?;
```

That is two successful receives from one packed message value. The type system therefore does not encode exactly-once consumption.

Recommended change: remove `Clone` from `Packed<M>` and from linear role carriers such as `Parent<Planned>` and `Parent<Selectable>`, or store a private non-`Clone` capability in those states. Add `#[must_use]` to `Packed<M>`. If dropping an unreceived packed message is illegal in this proof model, give it the same armed-drop treatment as `Open<M>` or represent acknowledged discard as an explicit typed transition.

### 4. The packed buffer can be extracted, cloned, or reconstructed as ordinary data

`Packed<M>` exposes the underlying buffer:

- `inner.rs:174-179`: `buffer()` and `into_buffer()`

For `ChildPlan`, the buffer is reconstructable:

- `parent.rs:55-62`: `ChildPlanFiles` is a plain cloneable value.
- `parent.rs:64-86`: `ChildPlanFiles::for_parent` rebuilds it from `manifest_path`, `ParentIdentity`, and `Prototype1NodeRecord`s.

The receiver validation does not bind to exact file contents or exact child entries:

- `parent.rs:104-120`: receiver validation checks only parent node id and generation.
- `cli_facing.rs:349-410`: pack-time validation checks scheduler path, branch path, parent id, generation, and that at least one staged child matches. It does not compare `ChildPlanFiles.children` against the report or verify durable file contents/digests.

Concrete replacement: two independently constructed `ChildPlanFiles` values with the same parent id and generation but different child file lists can both satisfy receiver validation. A caller can also consume `Packed<ChildPlan>` with `into_buffer()` and continue using raw paths as proof material.

Recommended change: make the message envelope opaque and content-bound. `ChildPlan` should carry a message id, sender runtime id, intended receiver identity, exact child node ids, file paths, and content digests or version stamps. `receive` should read and verify the exact persisted files and return a `Received<ChildPlan>` projection, not the raw buffer.

### 5. Pack does not require a transport write/read proof

`Open::pack` accepts any closure named `write`:

- `inner.rs:117-124`: `F: FnOnce(&M::Buffer) -> Result<(), E>`
- `inner.rs:125-140`: success closes the sender and constructs `Packed<M>`.

At the call site, the closure only validates a report-derived `ChildPlanFiles` value:

- `cli_facing.rs:331-335`: `ChildPlanFiles::for_parent(...)` followed by `validate_child_plan(...)`

The actual scheduler/registry writes happen earlier inside `run_prototype1_loop_controller`, outside the typed pack transition. If that controller partially mutates files before a later error, the type system has no `Parent<PlanWriteFailed>`/`PlanPartiallyWritten>` state tied to those files. Conversely, a successful `pack` does not prove that a receiver can read the files, parse them, or that their contents still match the packed message.

Recommended change: make packing use a protocol transport trait rather than an arbitrary closure. The transport should perform atomic write, durable commit, readback or digest capture, and typed error construction. The resulting `Packed<M>` should be created only by that transport after the durable message has been written and identified.

### 6. Typed failure states are available but dropped at the boundary

`PackError` and `ReceiveError` preserve useful typed state:

- `inner.rs:262-264`: `PackError` stores `M::SenderFailed`.
- `inner.rs:205-207`: `ReceiveError` stores the original `Packed<M>`.

The call site immediately discards both:

- `cli_facing.rs:336-339`: `let (_parent, source) = err.into_parts(); source`
- `cli_facing.rs:340-345`: `let (_packed, source) = err.into_parts(); ...`

This returns an error to the CLI, but it loses the typed failure object and records no durable failed transition. That is weaker than the requested invariant because a file or receiver failure is not represented as an admissible protocol state transition.

Recommended change: map pack/receive failures into explicit terminal or retryable protocol states, and require the transition method to journal them before returning a CLI-level `PrepareError`. Do not destructure and drop the failed sender or packed message until the failure has been durably projected.

## Recommended Type Shape

The stronger invariant wants a linear, content-bound handoff:

```rust
Parent<Ready>
  -> pack_child_plan(transport)
  -> Result<(Parent<PlanWritten>, Sent<ChildPlan>), Parent<PlanWriteFailed>>

Runtime<Receiver, Awaiting<ChildPlan>>
  -> load_sent(sent_address)
  -> Result<Packed<ChildPlan>, ReceiveLoadFailed<ChildPlan>>
  -> receive(packed)
  -> Result<(Parent<Selectable>, Received<ChildPlan>), ReceiveRejected<ChildPlan>>
```

Key properties:

- `Sent`, `Packed`, `Received`, and linear parent states are not `Clone`.
- `Packed` cannot expose or return a raw buffer as the proof object.
- `receive` verifies the intended receiver, message id, sender identity, exact child list, paths, and file content/version evidence.
- Candidate selection and child materialization require `Parent<Selectable>` plus `Received<ChildPlan>`, not a node id inferred from scheduler JSON.
- Pack/read/write failures are typed states with required journal projections.

The reduction to avoid is treating persisted scheduler/registry JSON as equivalent to a consumed cross-runtime message. Those files are useful evidence, but the proof object has to be the exact message envelope consumed by the exact receiver.
