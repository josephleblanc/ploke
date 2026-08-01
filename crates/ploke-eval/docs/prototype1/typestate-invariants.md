# Prototype 1 Typestate Invariants

Status: source-backed synthesis for the current Prototype 1 implementation. This
page is intentionally stricter than an operator guide: every quoted source block
is included to prove one specific claim, not to provide adjacent context.

## Source priority

When sources disagree, prefer current code under
`crates/ploke-eval/src/cli/prototype1_state/`, then consolidated evalnomicon
pages, then drafts and historical reports.

## Claim 1: the outer runtime state is a product, not a flat phase enum

The `Runtime` carrier proves the R-state shape is the product of phase, role,
context, plan, children, history, evidence, continuation, and report axes.

```rust,ignore
{{#include ../../src/cli/prototype1_state/typestate/runtime.rs:prototype1_runtime_product}}
```

The `Facts` fields prove that several live values remain ordinary runtime
bookkeeping behind `Option`s, so axis names are not complete semantic proofs by
themselves.

```rust,ignore
{{#include ../../src/cli/prototype1_state/typestate/context.rs:prototype1_context_facts_fields}}
```

Consequence: an alias such as `R10` is a local judgment about a consumed runtime
value. It says the edge received a value with the expected product shape; it does
not say all filesystem, provider, policy, and History checks are globally true.

## Claim 2: artifact/runtime vocabulary is explicitly separated

The operation target enum proves the model distinguishes one artifact, patch
sets, and artifact sets instead of treating branch names as the semantic graph.

```rust,ignore
{{#include ../../src/loop_graph.rs:prototype1_operation_target}}
```

The coordinate carrier proves a runtime identity is paired with the target of a
generative or compositional action.

```rust,ignore
{{#include ../../src/loop_graph.rs:prototype1_coordinate}}
```

Consequence: the intended model is runtime/artifact succession. Current live
paths still have provenance gaps, but new authority claims should name which
runtime and which artifact/patch target they bind.

## Claim 3: R aliases document the required local shape at key points

`R8` proves that after child-plan receive the parent role is `Selectable`, the
plan axis carries `Received<ChildPlan>`, and the child set is planned.

```rust,ignore
{{#include ../../src/cli/prototype1_state/typestate/aliases.rs:prototype1_alias_r8}}
```

`R13bHandoffCommitted` proves that committed handoff is represented as a retired
parent plus advanced/sealed History and recorded continuation facts.

```rust,ignore
{{#include ../../src/cli/prototype1_state/typestate/aliases.rs:prototype1_alias_r13b_handoff_committed}}
```

The R12 branch enum proves continuation has two typed outcomes: stopped or
handoff committed.

```rust,ignore
{{#include ../../src/cli/prototype1_state/typestate/aliases.rs:prototype1_branch_r12_continuation}}
```

Consequence: `R13bHandoffCommitted` means the predecessor committed handoff and
moved to `Parent<Retired>`. It does not mean the successor completed a future
parent turn or that no other process exists globally.

## Claim 4: transition composition proves adjacency, not semantic truth

The `Step` trait contract proves an edge consumes one state and may compose only
with a next edge whose input is exactly the prior output.

```rust,ignore
{{#include ../../src/cli/prototype1_state/typestate/transition.rs:prototype1_step_trait_contract}}
```

The value-first helper proves callers can apply direct edge functions without
erasing the same adjacency requirement.

```rust,ignore
{{#include ../../src/cli/prototype1_state/typestate/transition.rs:prototype1_step_input}}
```

Consequence: typed composition catches skipped edges such as `R8 -> R12` unless
some real edge returns that type. It does not prove an edge body performed every
external check; missing evidence must still produce an error before advancing.

## Claim 5: parent readiness is a move-only local authority island

The `Parent<S>` carrier proves parent role state is an owned value with private
state, not a free string label.

```rust,ignore
{{#include ../../src/cli/prototype1_state/parent.rs:prototype1_parent_struct}}
```

The startup gate proves `Startup<Validated>` carries private lineage, parent,
generation, and History state before readiness can be admitted.

```rust,ignore
{{#include ../../src/cli/prototype1_state/parent.rs:prototype1_parent_startup_gate}}
```

The checkout check proves the active checkout is validated through the backend
before the carrier can proceed.

```rust,ignore
{{#include ../../src/cli/prototype1_state/parent.rs:prototype1_parent_check_validation}}
```

The readiness transition proves startup evidence is validated against the parent
identity before admitting readiness.

```rust,ignore
{{#include ../../src/cli/prototype1_state/parent.rs:prototype1_parent_ready_validation}}
```

Consequence: callers cannot normally mint `Parent<Ready>` from invocation JSON
or a path alone. The guarantee is still local/module-scoped, not distributed
consensus.

## Claim 6: message boxes model a sender obligation, durable box, and receiver obligation

The generic `MessageBox` trait proves each message names exactly one lock edge
and one unlock edge over a static file schema.

```rust,ignore
{{#include ../../src/cli/prototype1_state/inner.rs:prototype1_message_box_trait}}
```

The `Message` trait proves the message implementation owns sender-close,
sender-fail, and receiver-validation logic for that box.

```rust,ignore
{{#include ../../src/cli/prototype1_state/inner.rs:prototype1_message_trait}}
```

The carrier-only `Open<M>` excerpt proves an open message holds an armed sender
and body before durable packing.

```rust,ignore
{{#include ../../src/cli/prototype1_state/inner.rs:prototype1_message_open_carrier}}
```

The `Open<M>::from_sender` excerpt proves opening consumes the only valid sender
role/state for the message.

```rust,ignore
{{#include ../../src/cli/prototype1_state/inner.rs:prototype1_message_open_from_sender}}
```

The success arm of `Open<M>::lock` proves successful packing closes the sender,
disarms the drop guard, and returns `Locked<M>` around the body written to the box.

```rust,ignore
{{#include ../../src/cli/prototype1_state/inner.rs:prototype1_message_open_lock_success}}
```

The failure arm proves a write failure becomes the message-specific
`SenderFailed` state instead of silently dropping the open obligation.

```rust,ignore
{{#include ../../src/cli/prototype1_state/inner.rs:prototype1_message_open_lock_failure}}
```

The drop guard excerpt proves an armed `Open<M>` panics if it is dropped before
lock or typed failure.

```rust,ignore
{{#include ../../src/cli/prototype1_state/inner.rs:prototype1_message_open_drop_guard}}
```

The carrier-only `Locked<M>` excerpt proves the durable-box state binds the box
address and body.

```rust,ignore
{{#include ../../src/cli/prototype1_state/inner.rs:prototype1_message_locked_carrier}}
```

The `Locked<M>::from_box` excerpt proves a restarted process can reconstruct the
locked state only by reading the body at the typed box address.

```rust,ignore
{{#include ../../src/cli/prototype1_state/inner.rs:prototype1_message_locked_from_box}}
```

The `Locked<M>::unlock` excerpt proves receipt calls message-specific receiver
validation before returning the next receiver state and `Received<M>`.

```rust,ignore
{{#include ../../src/cli/prototype1_state/inner.rs:prototype1_message_locked_unlock}}
```

The carrier-only `Received<M>` excerpt proves receipt is represented by a typed
capability over the consumed box and body.

```rust,ignore
{{#include ../../src/cli/prototype1_state/inner.rs:prototype1_message_received_carrier}}
```

Consequence: a projection file may help reconstruction, but the authority claim
is the typed lock/read/unlock path for the message box. The child-plan concrete
instance is detailed in [Zero Admission Flow](child-plan-authority/zero-admission-flow.md).

## Claim 7: handoff authority retires the predecessor through Crown and sealed History

The Crown boundary proves only `Parent<Selectable>` can seal a block with
artifact admission and retire into `Parent<Retired>`.

```rust,ignore
{{#include ../../src/cli/prototype1_state/inner.rs:prototype1_parent_seal_block_with_artifact}}
```

The store append excerpt proves a sealed block is hash-verified, checked against
the expected lineage state, and only then written to heads/projections.

```rust,ignore
{{#include ../../src/cli/prototype1_state/history/stored/mod.rs:prototype1_fs_block_store_append}}
```

The live R12 handoff branch proves the continuation edge delegates handoff and
then constructs the `HandoffCommitted` branch with the retired parent.

```rust,ignore
{{#include ../../src/cli/prototype1_state/live_edges.rs:prototype1_live_edge_r12_handoff_branch}}
```

The handoff block field builder proves the committed block is opened from the
current lineage state and active installed artifact.

```rust,ignore
{{#include ../../src/cli/prototype1_process.rs:prototype1_handoff_block_fields}}
```

The successor startup validation excerpt proves the successor rechecks sealed
History, selected runtime, selected artifact, and parent identity before
admitting the next parent identity.

```rust,ignore
{{#include ../../src/cli/prototype1_process.rs:prototype1_successor_startup_validation}}
```

The artifact-tree verifier proves the incoming runtime recomputes and verifies
the current checkout's clean tree against the sealed artifact claim.

```rust,ignore
{{#include ../../src/cli/prototype1_state/history/seal/mod.rs:prototype1_history_block_verify_current_artifact_tree}}
```

The surface verifier proves startup does not trust invocation JSON for surface
roots; it compares the current surface to the sealed block.

```rust,ignore
{{#include ../../src/cli/prototype1_state/history/seal/mod.rs:prototype1_history_block_verify_current_surface}}
```

Consequence: predecessor ready wait is only a transport/observability seam. The
stronger authority check is the successor's sealed-History startup validation.

## Working invariant ledger

### Ordering invariants

- Parent identity precedes parent loading.
- Startup validation precedes `Parent<Ready>`.
- Child-plan authority precedes child schedule and selection strategy.
- Child fanout or rejected-only projection precedes report facts.
- Selection material and continuation policy precede handoff.
- History seal/append precedes successor ready wait.
- Successor startup validation precedes successor admission in the next process.

### Authority invariants

- Child self-report is evidence, not promotion.
- Only a ready parent may publish a child-plan message.
- Only a planned parent may receive that child-plan message.
- Only a selectable parent may lock lineage authority for handoff.
- After handoff the predecessor is retired.
- Invocation, ready, completion, channel, and stream files are transport/debug
  evidence unless admitted by a typed transition or sealed block.

### Artifact/runtime invariants

- Worktree paths and branch names are handles, not artifact identity.
- Temporary child worktrees are evaluation/build surfaces, not successor homes.
- The selected artifact should be installed into the stable active checkout
  before successor spawn.
- A binary from one checkout should not be treated as authority-bearing for a
  different active checkout.

## Correctness strength matrix

| Layer | Current guarantee | Boundary / gap |
| --- | --- | --- |
| `Step` / `StepInput` | Edge composition consumes and produces matching R aliases. | Does not prove edge bodies performed semantic checks. |
| Strong carriers | Callers cannot normally mint parent readiness, message receipt, or Crown states from raw strings. | Local/module-scoped, not distributed authority. |
| Outer R aliases | Later edges can demand phase-shaped states such as `R8` or `R13b`. | Many facts still live in `context::Facts` behind `Option`s. |
| Child-plan box | Producer/receiver must cross concrete lock/unlock transitions for one file schema. | Other cross-runtime buffers are not all typed boxes yet. |
| Sealed History append | Hash/state-root/expected-head append is checked before lineage head projection advances. | Not consensus, process uniqueness, or proof of model judgment. |
| Reconstruction and walk gates | Strict evidence families and operator flags reduce accidental live effects. | Projections and operator discipline are not authority sources. |

## Known caveats to preserve

- R alias constructors such as `from_collected_parent` are crate-visible
  migration seams.
- `context::Facts` carries several semantic values as `Option`s checked by
  later edge bodies.
- Ready acknowledgement is evidence that the successor reached its startup path;
  do not describe predecessor ready wait as an independent identity proof.
- `driver/reconstruct.rs` is strict and side-effect-free, but it is not pure
  sealed-ledger replay; it reads multiple evidence families.
- History is local, lineage-scoped, and tamper-evident. It does not claim global
  process uniqueness, distributed consensus, or correctness of LLM judgment.
