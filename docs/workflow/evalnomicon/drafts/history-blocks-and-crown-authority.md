# History Blocks And Crown Authority

This draft pins the next Prototype 1 History concept after the first working
three-generation trampoline run. The goal is not to add another report format.
The goal is to define the durable substrate that lets later analysis answer:

```text
what happened
under whose authority
in what operational environment
by which procedure or transition
over which evidence
producing which result
and why that result was allowed to affect the next action
```

## Reduction To Avoid

History is not `scheduler.json`, `branches.json`, a CLI inspection view, a
mutable report, or a side table in Cozo. Those may be useful projections.
They are not the trust source.

The stronger object is:

```text
History = chain of sealed Blocks
Block = authority epoch for one active lineage
Entry = provenance-bearing fact inside an epoch
Ingress = append-only late/backchannel observations outside the sealed epoch
Projection = disposable view or index derived from History
```

Cozo can later index History. It should not become the authority layer unless
it stores or verifies the same immutable block and entry identities.

## Crown Boundary

The Crown is the authority to mutate one active lineage and choose the next
Parent for that lineage. A Block should align with that authority epoch:

```text
Parent<Ruling> opens Block<Open>
Parent<Ruling> records entries while it has the Crown
Parent<Ruling> installs the selected Artifact
Parent<Ruling> locks Crown<Locked>
Crown<Locked> seals Block<Sealed>
Successor<Admitted> verifies Block<Sealed>
Successor<Admitted> imports admissible Ingress
Successor<Admitted> becomes Parent<Ruling>
Parent<Ruling> opens the next Block<Open>
```

Pinning the block boundary to `Crown<Locked>` gives us a concrete authority
cut: no later Parent can honestly claim that a decision belonged to the prior
epoch unless it appears in the sealed block or is imported from ingress with a
recorded policy.

What this gives today:

- a precise place to stop accepting Parent-authored decisions for one epoch
- a validation target for the successor before it becomes `Parent<Ruling>`
- a way to separate sealed evidence from late observations
- a place to check that selection, active checkout, runtime identity, and
  artifact identity agree

What this does not give today:

- distributed consensus
- protection against a compromised process that can rewrite all local files
- proof that an LLM judgment is correct
- proof that every current JSON buffer already has block semantics
- global authority across unrelated lineages

Later cryptographic signatures, remote witnesses, replicated logs, or
multi-Ruler consensus may strengthen the model. The current claim should stay
local: tamper-evident, lineage-scoped, transition-checked History, not a
distributed ledger.

## Block Signing

For the near term, "signed" means content-addressed and sealed by the authority
transition that locks the Crown. A sealed block should have a stable hash over
its header and entries. Recomputing that hash should detect local mutation of
the block contents.

A later signature can be added without changing the semantic object:

```text
block_hash = hash(block_header_without_hash, entries_root)
signature = sign(runtime_signing_identity, block_hash)
```

Today, the minimum useful block seal is:

- deterministic serialization for the header
- deterministic serialization for entries
- per-entry payload hashes
- an entries Merkle root or equivalent ordered content hash
- previous block hash
- writer/runtime identity
- Crown lock transition identity
- block hash computed over the complete sealed block

Cryptographic signing should be treated as an extension. The important near
term invariant is that the successor verifies the exact sealed block before it
can unlock the Crown and become `Parent<Ruling>`.

## Type Safety Claim

The type-safety claim must be narrow and auditable. We should not claim that
Rust types make the whole execution environment trustworthy. We should claim
only what the implementation actually enforces.

The intended shape is:

```text
Parent<Ruling> -> Crown<Locked> -> Block<Sealed>
Crown<Locked> + Block<Sealed> + SuccessorEvidence -> Successor<Admitted>
Successor<Admitted> -> Parent<Ruling>
```

To make that real, advanced states must be hard to construct accidentally:

- state marker fields are private or sealed
- constructors for authoritative states are private or module-scoped
- transitions consume the prior state and return the next state
- transition methods emit durable records as projections of the transition
- public APIs do not accept arbitrary strings such as `"ruling"` or
  `"sealed"` as status writes
- validation is part of the transition into the authoritative state

The record is not the transition. The record is evidence that an allowed
transition happened.

## Weekly Audit Policy

Prototype 1 History and Crown authority must be audited at least once per week
while this architecture is active. The audit should be a combined human and
LLM review of documentation claims against the actual implementation.

The audit must check:

- whether any doc claims more than the current code enforces
- whether `Parent<Ruling>`, `Crown<Locked>`, `Block<Sealed>`, and successor
  admission states can be forged through public constructors or public fields
- whether transition methods are move-only where authority transfer requires it
- whether the emitted durable records are projections of typed transitions
- whether mutable JSON buffers are being mistaken for sealed History
- whether a late observation can influence a decision without an import policy
- whether proposer, recorder, admitting authority, and ruling authority remain
  distinct in persisted events

Any mismatch should be handled as one of three outcomes:

```text
fix the implementation
narrow the claim
record the gap explicitly before depending on it
```

This audit policy exists because the runtime is self-modifying and eventually
may evaluate or edit parts of its own assessment machinery. The stronger the
History claim becomes, the more important it is that the code, documentation,
and operator expectations do not drift apart quietly.

## Entry Shape

Every History entry should preserve chain of custody. At minimum:

- `entry_id`
- `entry_kind`
- `subject`
- `procedure`, `transition`, or `policy`
- `executor`
- `operational_environment`
- `observer`
- `recorder`
- `proposer`
- `ruling_authority`
- `admitting_authority` when imported or accepted under policy
- `input_refs`
- `output_refs`
- `occurred_at`
- `observed_at`
- `recorded_at`
- `authority_context`
- `payload_hash`
- `previous_entry_hash` or explicit block-local ordering

Important roles must not be collapsed:

- origin authority: the authority under which the event originated
- observer: the runtime or instrumentation that observed the event
- submitter: the actor or process that submitted it for History
- admitting authority: the authority that accepted it into a block
- committer: the runtime that sealed the containing block
- ruler: the current lineage authority when the entry was written

This distinction matters when a late or external event was submitted under one
authority but admitted by another. The imported entry must cite the original
event and the policy that justified importing it.

## Entry Kinds

The first useful classes are:

- `Observation`
  A fact observed by a runtime or monitor. It does not itself decide anything.
- `ProcedureRun`
  A bounded procedure execution, including mechanized steps, LLM calls,
  fork/merge protocol artifacts, inputs, outputs, and executor identity.
- `Judgment`
  An evaluated claim such as branch comparison, metric interpretation, or
  LLM adjudication.
- `Decision`
  A policy-bearing choice that is allowed to affect control flow.
- `Transition`
  An allowed typed state transition such as successor handoff, Crown lock, or
  successor admission.
- `Projection`
  A generated view over History, included only when the projection itself must
  be cited as an artifact.

Procedure-generated analysis should retain the structure that produced it. For
example, a tool-call review should not collapse into a single score if it was
produced by segmentation, three adjudication branches, and a mechanized merge.

## Operational Environment

Operational environment is first-class. It is not decorative metadata.

An entry should distinguish:

```text
occurred_in = environment where the action happened
observed_by = environment or instrumentation that recorded it
```

Useful environment fields include:

- runtime id
- artifact id, commit, tree, or source-state id
- binary path and build identity
- tool description artifact versions
- tool registry or schema version
- prompt and procedure version
- LLM provider, model, and relevant generation config
- code graph or database snapshot where applicable
- eval campaign and oracle task identity
- worktree or repo root as a handle, not semantic identity
- recording process and journal path

Without this, we cannot later tell whether a datum came from a mechanized run,
an LLM adjudication, a human review, a stale binary, a different tool surface,
or an imported late observation.

## Block Header

A block header should be small and mechanical:

- `schema_version`
- `block_id`
- `lineage_id`
- `generation`
- `parent_block_hash`
- `opened_by_runtime`
- `opened_from_artifact`
- `ruling_authority`
- `crown_lock_transition`
- `selected_successor_runtime`
- `selected_successor_artifact`
- `policy_ref`
- `opened_at`
- `sealed_at`
- `entry_count`
- `entries_root`
- `ingress_root` or imported ingress summary when applicable
- `block_hash`
- `signature` when implemented

The header should not carry large reports. Large artifacts should be referenced
by content hash or stable path plus hash.

## Ingress

During `Crown<Locked>`, there may be no `Parent<Ruling>`, but observation
should not stop. Late child status, process exits, diagnostics, and monitor
events may still arrive.

Ingress is the append-only holding area for those observations.

Allowed while the Crown is locked:

- observation
- diagnostic submission
- terminal status submission

Not allowed while the Crown is locked:

- new Parent decision
- new branch selection
- silent mutation of the sealed block
- authority transition other than successor unlock or authorized recovery

The next admitted Parent may import ingress under policy. Import must preserve
the original event:

- `ingress_id`
- `ingress_payload_hash`
- `ingress_observed_at`
- `ingress_observed_by`
- `ingress_prior_block_hash`
- `imported_at`
- `imported_by_runtime`
- `imported_into_block`
- `import_policy`
- `import_disposition`

Possible dispositions include:

- `accepted_as_observation`
- `accepted_as_late_terminal_status`
- `accepted_as_diagnostic_only`
- `rejected_stale`
- `rejected_wrong_lineage`
- `rejected_hash_mismatch`
- `rejected_after_timeout`

If a late event was required for the selection decision, the Parent locked the
Crown too early. Required evidence belongs in the sealed block for that epoch.
Late observations can affect the next epoch only after import.

## Mapping Current Records

The current persisted state should be treated as transitional:

- `transition-journal.jsonl`
  Early unsealed transition stream. It is closest to History, but it is not
  yet a sealed block chain.
- `scheduler.json`
  Mutable projection and work queue. Useful for control, not authority.
- `branches.json`
  Mutable branch registry and candidate buffer. Useful evidence source, not
  final History.
- `evaluations/*.json`
  Judgment evidence if it carries evaluated artifact/runtime, oracle, metrics,
  and policy identity.
- `protocol-artifacts/`
  ProcedureRun evidence. These should cite executor kind, prompt/procedure
  version, model config, input refs, output refs, and merge structure.
- `messages/child-plan/*.json`
  Typed message box for Parent-owned candidate publication.
- `nodes/*/invocations/*.json`
  Attempt-scoped bootstrap contracts.
- `nodes/*/results/*.json`
  Attempt-scoped runtime results.
- `successor-ready` and `successor-completion`
  Successor handoff evidence, currently standing in for the missing concrete
  Crown box.

The cleanup direction is to decide which fields become History entries, which
remain mutable buffers, and which are projections. Do not add another parallel
status document to paper over the split.

## Flywheel Trace

The intended self-improvement flywheel should be readable as a History trace:

```text
external oracle task
-> runtime/tool operational environment
-> tool call trace
-> intent segmentation
-> adjudication branches
-> mechanized merge
-> issue detection
-> intervention synthesis
-> candidate patch
-> child artifact/runtime
-> evaluation
-> successor decision
-> Crown handoff
```

Every arrow should be represented as an `Observation`, `ProcedureRun`,
`Judgment`, `Decision`, or `Transition` with explicit provenance.

## Design Implications

1. History is append-only and block-sealed.
2. Mutable JSON files are buffers or projections unless they are admitted into
   a sealed block.
3. Compiler-enforced authority requires private construction, move-only
   transitions, and validation at transition boundaries.
4. The successor must verify the sealed block before unlocking the Crown.
5. Backchannel observations must go through ingress and import policy.
6. Proposer, observer, recorder, admitting authority, committer, and ruler must
   remain separable in the data model.
7. Weekly audit is part of the operational protocol until the implementation
   and claims are boringly aligned.
