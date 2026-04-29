# Prototype 1 History/Crown Typestate Review - Reviewer A

## Executive Verdict

The current implementation does not yet enforce the strongest History/Crown claims. The safe claim is narrower: Prototype 1 has a partially typed parent/child/successor scaffold, attempt-scoped invocation files, and an append-only JSONL transition journal, but it does not have a concrete `Crown<Locked>`, `Block<Sealed>`, successor admission type, ingress policy, block hash, entry hash chain, or compiler-enforced block verification gate.

Some local typestate transitions are real and useful, especially `Child<Starting> -> Child<Ready> -> Child<Evaluating> -> Child<ResultWritten>` and the C1-C5 move-only transition path. They are weakened by crate-wide constructors, raw journal append surfaces, public(crate) record constructors, cloneable authority inputs, and live controller paths that emit authority records without consuming the corresponding role/state capability.

## Claims Reviewed

- History is a chain of sealed Blocks, with entries written during a Crown epoch and late observations routed through ingress: `docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md:26`, `docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md:42`, `docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md:286`.
- The near-term claim is local tamper-evident, lineage-scoped, transition-checked History: `docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md:74`.
- Successor admission must verify the sealed block before becoming `Parent<Ruling>`: `docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md:104`, `crates/ploke-eval/src/cli/prototype1_state/mod.rs:156`.
- Advanced states must be hard to construct, transitions should consume prior state, and durable records should be projections of allowed transitions: `docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md:122`, `AGENTS.md:7`.
- Mutable JSON buffers are not sealed History: `docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md:337`, `crates/ploke-eval/src/cli/prototype1_state/mod.rs:613`.

## Findings

### Critical: Crown and Block are documentation-only, not enforced objects

There is no concrete `Block<Open>`, `Block<Sealed>`, ingress store, block hash, entries root, previous block hash, or successor admission type in the reviewed Rust modules. `inner::Crown<L>` exists only as an unconstructed private-field token, and `LockBox` is a trait with no implementation in the reviewed path: `crates/ploke-eval/src/cli/prototype1_state/inner.rs:42`, `crates/ploke-eval/src/cli/prototype1_state/inner.rs:54`.

The module docs accurately admit part of this gap: live handoff still uses invocation and ready files, and the concrete Crown box is missing: `crates/ploke-eval/src/cli/prototype1_state/mod.rs:164`, `crates/ploke-eval/src/cli/prototype1_state/mod.rs:619`. The stronger draft language around sealed blocks and successor verification is therefore aspirational, not implemented: `docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md:117`, `docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md:390`.

Concrete violation: a successor can be recorded as spawned, ready, timed out, or completed through `SuccessorRecord` journal entries without any `Crown<Locked>` or `Block<Sealed>` value existing in memory or on disk. See direct successor record append in `crates/ploke-eval/src/cli/prototype1_process.rs:166`, ready/completion writes in `crates/ploke-eval/src/cli/prototype1_process.rs:306`, and spawn/handoff writes in `crates/ploke-eval/src/cli/prototype1_process.rs:906`.

### High: Raw journal append is a crate-wide authority bypass

`PrototypeJournal::new` is `pub(crate)` and `RecordStore::append` accepts any `JournalEntry`: `crates/ploke-eval/src/cli/prototype1_state/journal.rs:258`, `crates/ploke-eval/src/cli/prototype1_state/journal.rs:551`. Many `JournalEntry` payloads have public(crate) variants or public fields: `JournalEntry` at `crates/ploke-eval/src/cli/prototype1_state/journal.rs:235`, `ReadyEntry` at `crates/ploke-eval/src/cli/prototype1_state/journal.rs:131`, and successor `Record` fields at `crates/ploke-eval/src/cli/prototype1_state/successor.rs:57`.

Concrete bypass:

```rust
let mut journal = PrototypeJournal::new(path);
journal.append(JournalEntry::ChildReady(ReadyEntry {
    runtime_id,
    recorded_at: RecordedAt::now(),
    generation,
    refs,
    paths,
    pid,
}))?;
```

That writes a child-ready witness without consuming `Child<Starting>` or calling `Child<Starting>::ready()`. Similar direct appends can forge `SuccessorRecord::ready`, `SuccessorRecord::completed`, or `JournalEntry::SuccessorHandoff`. This violates the claim that durable records are projections of allowed typed transitions rather than arbitrary status writes.

### High: Successor/Crown records are constructors, not move-only transitions

`successor::Record` exposes constructors such as `selected`, `checkout`, `spawned`, `ready`, `timed_out`, `exited_before_ready`, and `completed`: `crates/ploke-eval/src/cli/prototype1_state/successor.rs:67`. These constructors do not consume a `Parent<Ruling>`, `Crown<Locked>`, `Block<Sealed>`, or `Successor<Admitted>` capability. The live path calls them directly from process helpers and CLI code: `crates/ploke-eval/src/cli/prototype1_process.rs:528`, `crates/ploke-eval/src/cli/prototype1_process.rs:908`, `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3114`.

This weakens both move-only authority transfer and durable record provenance. The journal records that something happened, but Rust does not require the claimed authority state to exist before the record can be emitted.

### High: Authority tokens are cloneable or mintable enough to undermine exclusivity

`authority.rs` claims private-field, move-only authority tokens: `crates/ploke-eval/src/cli/prototype1_state/authority.rs:5`. The fields are private, but the inputs used to mint authority are cloneable: `ActiveRoot`, `SharedRoot`, and `VerifiedActive` derive `Clone`: `crates/ploke-eval/src/cli/prototype1_state/authority.rs:178`, `crates/ploke-eval/src/cli/prototype1_state/authority.rs:197`, `crates/ploke-eval/src/cli/prototype1_state/authority.rs:293`. `Parent::from_verified` is `pub(crate)` and consumes only a `VerifiedActive`, `SharedRoot`, and `RuntimeId`: `crates/ploke-eval/src/cli/prototype1_state/authority.rs:391`.

Concrete bypass:

```rust
let p1 = Parent::from_verified(verified.clone(), shared.clone(), runtime_a);
let p2 = Parent::from_verified(verified, shared, runtime_b);
```

That creates two parent authority values for the same active root. There is no lineage lease, Crown value, journal emission, or unique handoff transition preventing duplicate authority. `Bootstrap::from_selected` and `Bootstrap::acknowledge` are also in-memory only and do not verify a sealed block: `crates/ploke-eval/src/cli/prototype1_state/authority.rs:530`, `crates/ploke-eval/src/cli/prototype1_state/authority.rs:553`.

### High: The child-plan box has a direct state advancement escape hatch

The message-box design is structurally sound in `inner::Open<M>::lock` and `Locked<M>::unlock`: `crates/ploke-eval/src/cli/prototype1_state/inner.rs:179`, `crates/ploke-eval/src/cli/prototype1_state/inner.rs:278`. However, `Parent<Ready>::planned_from_locked_child_plan` publicly advances `Parent<Ready>` to `Parent<Planned>` without requiring a `Locked<ChildPlan>` or received capability: `crates/ploke-eval/src/cli/prototype1_state/parent.rs:398`.

Concrete bypass:

```rust
let planned = ready_parent.planned_from_locked_child_plan();
```

This bypasses the intended `Parent<Ready> -> Parent<Planned>` lock precondition documented in `mod.rs`: `crates/ploke-eval/src/cli/prototype1_state/mod.rs:198`. It is used in a test for wrong-receiver validation, but as `pub(crate)` it is available to the crate, not just the test module.

### Medium: Invocation classification trusts mutable JSON more than the type claim allows

The persisted invocation wire record has public fields and is loaded from JSON into an authority classification by `role`: `crates/ploke-eval/src/cli/prototype1_state/invocation.rs:90`, `crates/ploke-eval/src/cli/prototype1_state/invocation.rs:322`. `load_authority` does not validate `schema_version`; successor validation checks scheduler continuation and active root later, but not a sealed block or Crown lock: `crates/ploke-eval/src/cli/prototype1_process.rs:369`, `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:2793`.

This is acceptable as a transitional bootstrap file, but it is not equivalent to `Crown<Locked> + Block<Sealed> + SuccessorEvidence -> Successor<Admitted>`.

### Medium: The JSONL journal is append-only by API convention, not tamper-evident History

`PrototypeJournal::append` uses `OpenOptions::append` and `sync_data`: `crates/ploke-eval/src/cli/prototype1_state/journal.rs:563`. That gives an append path for normal writers, but entries do not carry payload hashes, previous-entry hashes, block-local ordering hashes, entries roots, or block hashes. The draft requires those for the minimum useful block seal: `docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md:93`.

A local process can still rewrite `transition-journal.jsonl`; replay can detect some duplicate or impossible phase combinations, but not mutation against a sealed digest.

## Visibility Analysis

- Module exposure is broad. `prototype1_state` is private to `cli.rs`, but its submodules are `pub(crate)`: `crates/ploke-eval/src/cli.rs:33`, `crates/ploke-eval/src/cli/prototype1_state/mod.rs:647`. Any code in the crate can name these types and many constructors.
- State marker traits in C1 are sealed, which prevents third-party state markers: `crates/ploke-eval/src/cli/prototype1_state/c1.rs:48`. Existing marker values and many carrier fields remain constructible inside the crate due to public(crate) fields: `crates/ploke-eval/src/cli/prototype1_state/c1.rs:131`, `crates/ploke-eval/src/cli/prototype1_state/c1.rs:147`, `crates/ploke-eval/src/cli/prototype1_state/c1.rs:162`.
- `Parent<S>` in `parent.rs` has private fields and no public constructor for advanced states, which is good: `crates/ploke-eval/src/cli/prototype1_state/parent.rs:44`. The direct `planned_from_locked_child_plan` method undermines that boundary.
- `Child<S>` in `child.rs` has private fields and move-only transition methods that emit records before returning advanced states: `crates/ploke-eval/src/cli/prototype1_state/child.rs:119`, `crates/ploke-eval/src/cli/prototype1_state/child.rs:147`, `crates/ploke-eval/src/cli/prototype1_state/child.rs:155`. The raw journal append surface can still forge equivalent records.
- `authority.rs` has private fields, but cloneable inputs and `pub(crate)` authority constructors make uniqueness a convention rather than a type guarantee.

## Recommendations

1. Narrow documentation claims immediately: describe the current state as typed transition scaffolding plus unsealed JSONL records, not sealed or tamper-evident History.
2. Implement the missing Crown/Block path before relying on Crown claims: `Parent<Ruling>` should consume authority into `Crown<Locked>` and produce `Block<Sealed>` with deterministic entry hashes, previous block hash, and block hash.
3. Make successor admission consume `Crown<Locked>`, `Block<Sealed>`, and validated successor evidence before producing the next `Parent<Ruling>`.
4. Restrict raw journal append access. Prefer module-private append methods for each transition family so records can be emitted only from allowed move-only transitions.
5. Remove or tighten `planned_from_locked_child_plan`; require the locked/received child-plan capability rather than a direct cast.
6. Make authority-minting inputs non-Clone where exclusivity matters, or introduce a real lineage authority value that cannot be duplicated and is consumed at handoff.
7. Keep invocation and ready/completion files as bootstrap buffers until the Crown box exists, but explicitly mark them as non-authoritative projections in docs and monitor output.
