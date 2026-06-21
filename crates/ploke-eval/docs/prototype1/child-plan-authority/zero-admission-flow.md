# Prototype 1 Child-Plan Authority: Zero-Admission Flow

Status: source-grounded note for the zero-admission child-plan bug class. This
page focuses on concrete child-plan message-box mechanics: how a parent publishes
a `ChildPlan`, how retry reads it, and why a below-minimum batch must still write
an authority-bearing child-plan message.

## The bug shape

A parent can attempt broad-harness child planning and admit fewer children than
`child_budget.min`. The fixed behavior is **not** to call that batch successful.
The fixed behavior is to persist a `ChildPlan` message with zero or fewer-than-min
runnable children plus `rejected_surface_attempts`, then return the error. On
retry, the parent can receive that existing message instead of minting a fresh
request batch.

The authority surface is:

```text
Parent<Ready>
  -> Open<ChildPlan>
  -> Locked<ChildPlan> at prototype1/messages/child-plan/<parent-node-id>.json
  -> Parent<Planned>
  -> Received<ChildPlan>
  -> Parent<Selectable>
```

A runtime child `Channel` starts later, after runnable `ChildFiles` exist. The
zero-admission bug happens before child runtime spawn, so runner results and
channel messages must not substitute for the parent-owned `ChildPlan` box.

## Concrete child-plan box

The child-plan body proves the durable payload binds the box path, parent node,
child generation, runnable children, and rejected surface attempts.

```rust,ignore
{{#include ../../../src/cli/prototype1_state/parent.rs:prototype1_child_plan_body_carrier}}
```

The file-schema excerpt proves the durable address is
`prototype1/messages/child-plan/<parent-node-id>.json` and binds that address to
the child-plan lock/unlock transitions.

```rust,ignore
{{#include ../../../src/cli/prototype1_state/parent.rs:prototype1_child_plan_box_schema}}
```

The lock transition proves only `Parent<Ready>` can become `Parent<Planned>` by
locking this box.

```rust,ignore
{{#include ../../../src/cli/prototype1_state/parent.rs:prototype1_child_plan_lock_transition}}
```

The unlock transition proves the receiver side advances from `Parent<Planned>`
to `Parent<Selectable>`.

```rust,ignore
{{#include ../../../src/cli/prototype1_state/parent.rs:prototype1_child_plan_unlock_transition}}
```

The receiver method proves receipt rejects the wrong box path, wrong parent
node, or wrong child generation before casting the receiver.

```rust,ignore
{{#include ../../../src/cli/prototype1_state/parent.rs:prototype1_child_plan_ready_receiver}}
```

## Zero-admission write path

When broad-harness selection returns below minimum, the controller must finish a
parent-owned message transition before returning the below-minimum error:

```text
Parent<AwaitingHarnessPlan>
  -> accept_harness_plan() -> Parent<Ready>
  -> Open<ChildPlan>::from_sender(parent, files)
  -> open.lock(at, write_child_plan_message)
  -> ChildPlanFiles { children: [], rejected_surface_attempts: [...] }
  -> error: admitted fewer than child_budget.min
```

The accept transition proves a request-bound parent returns to `Parent<Ready>`
before it can publish the durable child-plan message.

```rust,ignore
{{#include ../../../src/cli/prototype1_state/parent.rs:prototype1_parent_accept_harness_plan}}
```

Within `Open<M>::lock`, the success arm proves the successful write closes the
sender, disarms the open obligation, and returns a `Locked<M>` body.

```rust,ignore
{{#include ../../../src/cli/prototype1_state/inner.rs:prototype1_message_open_lock_success}}
```

The below-minimum branch proves the controller accepts the harness response,
persists the rejected plan, and only then returns the minimum-child error.

```rust,ignore
{{#include ../../../src/cli/prototype1_state/cli_facing.rs:prototype1_broad_harness_below_min_persist_rejected_plan}}
```

The rejected-plan helper proves the persisted body uses an empty `children` list,
keeps `rejected_surface_attempts`, and still goes through `Open<ChildPlan>::lock`.

```rust,ignore
{{#include ../../../src/cli/prototype1_state/cli_facing.rs:prototype1_persist_rejected_child_plan}}
```

## Retry receive path

On retry, the existing authority file should be consumed rather than starting a
fresh broad-harness batch:

```text
resolve_child_plan
  -> ChildPlanFile exists
  -> Locked::<ChildPlan>::from_box(...)
  -> Parent<Ready>::planned_from_locked_child_plan()
  -> locked.unlock(planned)
  -> Parent<Selectable> + Received<ChildPlan>
```

The replay constructor proves retry reconstructs `Locked<ChildPlan>` by reading
from the typed box address.

```rust,ignore
{{#include ../../../src/cli/prototype1_state/inner.rs:prototype1_message_locked_from_box}}
```

The replay bridge proves retry recreates the `Parent<Planned>` receiver state
through an observed `LockChildPlan` replay transition, not by treating raw JSON
as final authority.

```rust,ignore
{{#include ../../../src/cli/prototype1_state/parent.rs:prototype1_parent_planned_from_locked_child_plan}}
```

The generic unlock method proves receiver-specific validation runs before the
next receiver state and `Received<M>` are returned.

```rust,ignore
{{#include ../../../src/cli/prototype1_state/inner.rs:prototype1_message_locked_unlock}}
```

After `unlock`, normal parent-side selection can observe that there are no
runnable children and use the rejected-attempt evidence to diagnose/report the
failed child-plan phase.

## Boundaries to keep separate

- `prototype1/messages/child-plan/<parent-node-id>.json` is the parent-owned
  child-plan authority box.
- Scheduler, branch, request, and runner-result files are projections or later
  evidence unless admitted by a typed transition.
- Runtime child `Channel` terminal results begin after `ChildFiles` exist; they
  cannot prove a zero-admission child plan.
- A malformed or wrong-parent child-plan file should fail the typed read/unlock
  path instead of falling back to a fresh batch.

## Regression coverage

The relevant tests should keep proving these contracts:

- below-minimum broad-harness admission persists a child-plan body with rejected
  attempts before returning the error;
- retry consumes that existing body and does not mint fresh request slots;
- wrong-parent and malformed child-plan files fail at the typed receive boundary;
- positive broad-harness admission still writes and receives runnable children.
