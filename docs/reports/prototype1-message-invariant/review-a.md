# Prototype 1 Message Invariant Review A

## Verdict

Fail. The current `Message`/`Open`/`Packed` model is directionally structured, but it does not absolutely enforce the invariant that a message cannot be expressed as sent unless it is also received by the exact intended recipient. `Open<M>` is guarded, but `Packed<M>` and `Parent<Planned>` are not linear obligations, receive failures can drop the receiver state, and the child-plan call path creates an armed `Open` before filesystem-heavy work that can fail.

## Counterexamples

1. A caller can express "sent" without receipt. `Open::pack` returns `M::SenderClosed` and `Packed<M>` immediately after the write closure returns `Ok(())` ([inner.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/inner.rs:117), [inner.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/inner.rs:125), [inner.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/inner.rs:131)). For `ChildPlan`, `SenderClosed = Parent<Planned>` and `ReceiverOpen = Parent<Planned>` ([parent.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/parent.rs:182)). Nothing forces the returned `Packed<ChildPlan>` to be received; `Packed<M>` has no `#[must_use]`, no drop guard, and no destructor ([inner.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/inner.rs:165)). Minimal counterexample: pack successfully, keep or return `Parent<Planned>`, and drop `packed`.

2. A single sent message can be duplicated and received more than once. `Packed<M>` derives `Clone` ([inner.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/inner.rs:166)), and `Parent<S>` derives `Clone` ([parent.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/parent.rs:42)). A caller can clone both `packed` and `Parent<Planned>`, then call `receive` twice. That defeats any one-message/one-receiver reading of the invariant.

3. Receive failure consumes and loses the receiver. `Packed::receive` passes `receiver` by value into `M::ready_receiver` ([inner.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/inner.rs:183), [inner.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/inner.rs:187)). On `Err`, `ReceiveError` preserves only `packed` and `source`, not the consumed `ReceiverOpen` ([inner.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/inner.rs:190), [inner.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/inner.rs:205)). The child-plan caller then discards even the preserved `packed` while mapping the error ([cli_facing.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:340)). This means a wrong-recipient attempt can follow a successful sent transition without preserving enough state to retry or prove exact receipt.

4. Child-plan opens the message before fallible filesystem work. `run_parent_target_selection` creates `Open::<ChildPlan>` before awaiting `run_prototype1_loop_controller` ([cli_facing.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:329), [cli_facing.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:330)). If that controller returns an error from underlying filesystem access, `open` drops while still armed and `Drop` panics ([inner.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/inner.rs:155)). This is not a typed failure path and will mask the original `PrepareError`.

5. Child-plan receiver validation is not an exact identity check. `ChildPlanFiles` records `parent_node_id` and `child_generation` ([parent.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/parent.rs:56), [parent.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/parent.rs:73)), and `validate_receiver` checks only `identity.node_id` plus `identity.generation + 1` ([parent.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/parent.rs:104), [parent.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/parent.rs:112)). `ParentIdentity` contains additional identity fields, including `campaign_id`, `parent_id`, `branch_id`, and `artifact_branch` ([identity.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/identity.rs:24)). If "exact intended recipient" means the full parent identity, the receive-side check is incomplete.

## Failure Paths That Are Correct

`Open::pack` does not close the sender or create `Packed<M>` when its write closure returns `Err`; it moves the sender through `M::fail_sender`, disarms `Open`, and returns `PackError` ([inner.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/inner.rs:142), [inner.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/inner.rs:147), [inner.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/inner.rs:149)). For `ChildPlan`, that failed state is `Parent<Ready>` ([parent.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/parent.rs:185)). This part does not express sent on a reported pack failure.

The current `cli_facing.rs` happy path does immediately receive the packed child plan after pack ([cli_facing.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:332), [cli_facing.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:340)). That call-site discipline is not enforced by the type model.

## Minimal Required Fixes

1. Make sent and receipt one linear transition. Do not expose `SenderClosed` independently from an unreceived `Packed<M>`. Prefer an API that consumes `Open<M>` and `ReceiverOpen` in one operation and returns `ReceiverReady` only after both pack and receive validation succeed, or introduce a private linear carrier that cannot expose `Parent<Planned>` except to `receive`.

2. Remove `Clone` from protocol obligation carriers. At minimum, `Packed<M>` and role states such as `Parent<Planned>` must be move-only. If earlier parent states need clone-like inspection, provide explicit snapshots rather than cloning transition carriers.

3. Preserve all linear state on failure. `ReceiveError` must carry both the unreceived `Packed<M>` and the original `ReceiverOpen`, or `ready_receiver` must validate by reference and only consume the receiver after validation succeeds.

4. Give `Packed<M>` the same obligation treatment as `Open<M>` or make it impossible to hold directly. `#[must_use]` is useful but insufficient by itself; a drop guard can catch accidental loss, but the stronger fix is to avoid exposing discardable unreceived packed values.

5. Move `Open::<ChildPlan>::from_sender(parent)` until after `run_prototype1_loop_controller(input).await?`, or add an explicit typed abort path that converts `Open<ChildPlan>` back to `Parent<Ready>` before propagating controller errors.

6. If exact recipient means full parent identity, include the intended identity fields in `ChildPlanFiles` and validate them in `ChildPlan::ready_receiver`, not only in the earlier `Parent<Unchecked>::check` path.
