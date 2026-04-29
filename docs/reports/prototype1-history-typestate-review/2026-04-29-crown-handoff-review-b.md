# Prototype 1 Crown Handoff Review - Reviewer B

Date: 2026-04-29

## Scope

Reviewed protocol correctness and cross-runtime semantics for Crown, History,
and successor spawning in the current working tree. Inspected:

- `docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md`
- `crates/ploke-eval/src/cli/prototype1_state/history.rs`
- `crates/ploke-eval/src/cli/prototype1_state/parent.rs`
- `crates/ploke-eval/src/cli/prototype1_state/inner.rs`
- `crates/ploke-eval/src/cli/prototype1_state/invocation.rs`
- `crates/ploke-eval/src/cli/prototype1_state/successor.rs`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
- `crates/ploke-eval/src/cli/prototype1_process.rs`

Desired invariant under review:

```text
For one lineage, at most one valid typestate carrier may hold Crown<Ruling>.
During handoff there may be zero rulers.
A successor runtime may execute, but should only be spawned through the
transition that moves predecessor Parent to Parent<Retired> and locks Crown.
```

## Executive Verdict

The current implementation is closer than the 2026-04-27 review, but it still
does not enforce the cross-runtime Crown/History invariant end to end.

The parent-side live spawn path now consumes `Parent<Selectable>` and calls
`Parent<Selectable>::lock_crown()` before spawning the detached successor
runtime (`crates/ploke-eval/src/cli/prototype1_process.rs:859`,
`crates/ploke-eval/src/cli/prototype1_process.rs:873`). That is the right local
shape for retiring the predecessor before the successor is executable.

The remaining gap is cross-runtime authority. The successor process does not
consume `Crown<Locked>`, verify `Block<Sealed>`, or transition through
`Successor<Admitted>`. It acknowledges handoff by loading a forgeable invocation
JSON, checking mutable scheduler state, writing a ready JSON, and then entering
the normal parent path (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3050`,
`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3110`,
`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3122`). That means the
current code can support a disciplined live run, but it cannot yet prove that a
successor became parent only because the predecessor locked Crown and sealed the
prior authority epoch.

## Findings

### Critical: Successor admission still bypasses sealed History and `Successor<Admitted>`

The draft requires the successor to verify the exact sealed predecessor block
before unlocking Crown and becoming `Parent<Ruling>`
(`docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md:104`,
`docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md:118`).
`history.rs` correctly narrows the current claim: live handoff locks a
lineage-bound `Crown<Locked>` but does not seal or persist a History block, and
successor validation still consults mutable scheduler/invocation state
(`crates/ploke-eval/src/cli/prototype1_state/history.rs:104`,
`crates/ploke-eval/src/cli/prototype1_state/history.rs:195`).

The live successor entrypoint confirms that gap. `acknowledge_prototype1_state_handoff`
loads a `SuccessorInvocation` from disk, validates campaign/node/root, checks
continuation against scheduler state, records successor-ready, and returns
`Parent<Ready>` (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3061`,
`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3073`,
`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3100`,
`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3110`,
`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3122`). No block hash,
sealed block, selected-successor evidence, active-artifact hash, or locked Crown
is consumed.

Recommendation: make successor admission consume a durable handoff object that
cites `Block<Sealed>`, the locked Crown transition, selected successor runtime,
selected artifact/checkout commit, and policy decision. Reserve
`Successor<Admitted>` for the post-verification carrier; do not let a raw
invocation file promote directly to `Parent<Ready>` or future `Parent<Ruling>`.

### High: Scheduler state is still authority-adjacent in the handoff gate

The History draft says mutable scheduler JSON is a projection, not authority
(`docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md:337`).
The live validation path still uses `scheduler.last_continuation_decision` as
the successor gate: `validate_prototype1_successor_node_continuation` loads the
scheduler, reads the last decision, and accepts when it is `ContinueReady` for
the node branch (`crates/ploke-eval/src/cli/prototype1_process.rs:384`,
`crates/ploke-eval/src/cli/prototype1_process.rs:388`,
`crates/ploke-eval/src/cli/prototype1_process.rs:400`). The decision is written
immediately before handoff in the controller path
(`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3421`,
`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3427`,
`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3444`).

This leaks authority from the sealed-transition model into a mutable projection.
An invocation plus a matching mutable scheduler decision is sufficient for the
successor to acknowledge; sealed History is not consulted.

Recommendation: keep scheduler continuation as a derived control projection.
The admission check should verify a sealed decision entry inside the predecessor
block and fail closed if scheduler state disagrees with the sealed block.

### High: Invocation files are executable authority tokens, not projections

`SuccessorInvocation::new` creates the bootstrap record with campaign, node,
runtime id, journal path, and active parent root
(`crates/ploke-eval/src/cli/prototype1_state/invocation.rs:218`,
`crates/ploke-eval/src/cli/prototype1_state/invocation.rs:221`). The record is
persisted as ordinary JSON (`crates/ploke-eval/src/cli/prototype1_state/invocation.rs:341`,
`crates/ploke-eval/src/cli/prototype1_state/invocation.rs:345`) and later loaded
and classified by the `role` field (`crates/ploke-eval/src/cli/prototype1_state/invocation.rs:322`,
`crates/ploke-eval/src/cli/prototype1_state/invocation.rs:327`). The runner CLI
correctly refuses to execute a successor invocation through the child runner
path (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1338`), but
`loop prototype1-state --handoff-invocation` still treats that JSON as the
successor token.

Because the invocation does not carry an anchored block hash or predecessor
Crown-lock evidence, it is not just a bootstrap hint. It is part of the live
authority path.

Recommendation: make invocation files attempt-scoped launch descriptors only.
They should cite sealed handoff evidence but not themselves authorize
admission. Loading an invocation should produce `Successor<Unchecked>` or an
equivalent non-authoritative carrier, and only sealed evidence verification
should produce the admitted successor state.

### High: Ready-file polling can accept the wrong acknowledgement

The predecessor wait loop treats the presence of the ready file as success. It
loads the record but discards it (`crates/ploke-eval/src/cli/prototype1_process.rs:826`,
`crates/ploke-eval/src/cli/prototype1_process.rs:832`,
`crates/ploke-eval/src/cli/prototype1_process.rs:833`,
`crates/ploke-eval/src/cli/prototype1_process.rs:835`). It does not compare the
record's campaign, node id, runtime id, or pid against the expected invocation.
On success it records `SuccessorHandoffEntry` for the runtime it expected
(`crates/ploke-eval/src/cli/prototype1_process.rs:936`,
`crates/ploke-eval/src/cli/prototype1_process.rs:940`).

That is a correctness hole even before sealed History exists: any local writer
that can create syntactically valid JSON at the expected ready path can cause
the predecessor to record an acknowledged handoff.

Recommendation: validate the loaded ready record against the exact
`SuccessorInvocation`, use exclusive creation for ready/completion files, and
ultimately make ready an ingress or transition fact admitted against sealed
handoff evidence rather than a path-presence signal.

### High: `Crown<Ruling>` is not actually held by a live parent carrier

`inner::Crown` has private fields and state markers, and `Crown<crown::Ruling>`
can be consumed into `Crown<crown::Locked>`
(`crates/ploke-eval/src/cli/prototype1_state/inner.rs:42`,
`crates/ploke-eval/src/cli/prototype1_state/inner.rs:60`,
`crates/ploke-eval/src/cli/prototype1_state/inner.rs:88`). But the live parent
does not hold a `Crown<Ruling>` field. Instead, `Parent<Selectable>::lock_crown`
mints a fresh lineage Crown from `campaign_id` and immediately locks it
(`crates/ploke-eval/src/cli/prototype1_state/parent.rs:423`,
`crates/ploke-eval/src/cli/prototype1_state/parent.rs:429`,
`crates/ploke-eval/src/cli/prototype1_state/parent.rs:430`).

This is better than a public string status write, but it does not prove "at most
one valid typestate carrier may hold `Crown<Ruling>`" because the ruling carrier
is not part of the live parent state. The current invariant is closer to:
only `Parent<Selectable>` can ask this module to create a locked Crown.

Recommendation: introduce a real ruling authority carrier, e.g.
`Parent<Ruling>` or `Parent<Selectable>` with a private `Crown<Ruling>` field.
Opening/writing a block should require that carrier, and locking should consume
it into `(Parent<Retired>, Crown<Locked>, Block<Sealed>)`.

### Medium: The parent-side spawn path has the right local move, but only locally

The current spawned-successor path installs/builds the selected artifact, then
locks Crown by consuming `Parent<Selectable>`, then writes the invocation and
spawns the successor (`crates/ploke-eval/src/cli/prototype1_process.rs:867`,
`crates/ploke-eval/src/cli/prototype1_process.rs:873`,
`crates/ploke-eval/src/cli/prototype1_process.rs:884`,
`crates/ploke-eval/src/cli/prototype1_process.rs:915`). That satisfies the
local shape requested for this review: the predecessor is moved to
`Parent<Retired>` before `spawn()`.

The gap is that the locked Crown is not sealed into History or passed into a
verifiable handoff artifact. It is logged only through debug output and dropped
after proving lineage locally (`crates/ploke-eval/src/cli/prototype1_process.rs:873`,
`crates/ploke-eval/src/cli/prototype1_process.rs:874`). `history.rs` has the
sealing API for `Crown<Locked>::seal(Block<Open>, SealBlock)`
(`crates/ploke-eval/src/cli/prototype1_state/history.rs:1218`), but live handoff
does not call it.

Recommendation: make `spawn_and_handoff_prototype1_successor` accept/open the
current `Block<Open>`, consume `Crown<Locked>` into `Block<Sealed>`, append it
through `BlockStore`, and put the resulting block hash in the successor
invocation.

### Medium: Stale `Successor<Admitted>` semantics remain aspirational and should be labeled so

The draft presents `Successor<Admitted>` as the verifier/importer that becomes
the next `Parent<Ruling>`
(`docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md:47`,
`docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md:118`).
`history.rs` now accurately says `Successor<Admitted>` is not a live handoff
gate (`crates/ploke-eval/src/cli/prototype1_state/history.rs:172`,
`crates/ploke-eval/src/cli/prototype1_state/history.rs:195`). The live
`successor.rs` type is a journal projection with states such as `Selected`,
`Spawned`, `Ready`, `TimedOut`, and `Completed`
(`crates/ploke-eval/src/cli/prototype1_state/successor.rs:18`,
`crates/ploke-eval/src/cli/prototype1_state/successor.rs:57`). It is not a
typestate successor carrier and does not enforce admission.

Recommendation: keep `successor::Record` as evidence/projection, but avoid
describing it as admission. Update docs/comments so `Successor<Admitted>` is
explicitly future work until a real carrier consumes sealed predecessor evidence.

### Medium: Legacy handoff records can still be mistaken for authority

The journal module explicitly warns that `SuccessorHandoffEntry` is legacy
evidence, not the future `Crown<Locked> -> Successor<Admitted> -> Parent<Ruling>`
carrier (`crates/ploke-eval/src/cli/prototype1_state/journal.rs:252`,
`crates/ploke-eval/src/cli/prototype1_state/journal.rs:257`). The live path
still writes both structured successor records and a legacy
`SuccessorHandoffEntry` on ready (`crates/ploke-eval/src/cli/prototype1_process.rs:923`,
`crates/ploke-eval/src/cli/prototype1_process.rs:938`). This is acceptable as
transitional evidence, but it should not be interpreted by reports or future
History import as proof of Crown transfer.

Recommendation: mark imported `SuccessorHandoffEntry` facts as degraded
pre-History evidence. Require the actual Crown lock transition and sealed block
hash before a handoff record can be admitted as an authority transition.

### Medium: A stale testing shortcut can affect successor selection policy

The controller still contains a temporary comment dated 2026-04-26 saying any
completed child evaluation is treated as successor-eligible even when evaluator
disposition is Reject, with a stated revert deadline of 2026-04-26 14:41:53 PDT
(`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3406`). As of this
review date, 2026-04-29, that comment is stale. The surrounding code calls
`decide_node_successor_continuation` and may then spawn a successor
(`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3427`,
`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3452`).

This is not directly a Crown typestate bug, but it weakens the claim that the
selected successor reflects the current policy/evidence boundary.

Recommendation: either remove the shortcut or record it as an explicit degraded
policy in History/projection output so future audits do not confuse testing
handoff behavior with production continuation semantics.

## Positive Controls

- `history.rs` now implements real `Block<Open>`, `Block<Sealed>`, deterministic
  block hashing, `BlockStore::append`, and `Crown<Locked>::seal` with lineage
  checking (`crates/ploke-eval/src/cli/prototype1_state/history.rs:1002`,
  `crates/ploke-eval/src/cli/prototype1_state/history.rs:1151`,
  `crates/ploke-eval/src/cli/prototype1_state/history.rs:1218`,
  `crates/ploke-eval/src/cli/prototype1_state/history.rs:468`).
- `Parent<Selectable>::lock_crown` is unavailable on earlier parent states and
  consumes the parent into `Parent<Retired>` (`crates/ploke-eval/src/cli/prototype1_state/parent.rs:423`).
- The live parent-side successor spawn path calls `lock_crown()` before
  `spawn()` (`crates/ploke-eval/src/cli/prototype1_process.rs:873`,
  `crates/ploke-eval/src/cli/prototype1_process.rs:915`).
- Child-plan messaging is a useful pattern for cross-runtime protocol boxes:
  sender/receiver states are typed, and the receiver validates identity and
  generation before unlock (`crates/ploke-eval/src/cli/prototype1_state/parent.rs:264`,
  `crates/ploke-eval/src/cli/prototype1_state/parent.rs:286`).

## Concrete Recommendations

1. Add a live authority spine:
   `Parent<Ruling> -> (Parent<Retired>, Crown<Locked>, Block<Sealed>)`, then
   `Crown<Locked> + Block<Sealed> + SuccessorEvidence -> Successor<Admitted> ->
   Parent<Ruling>`.
2. Make the current block store part of live handoff. The predecessor should
   append the sealed block before writing/spawning the successor invocation.
3. Put the sealed block hash, selected successor runtime id, selected artifact
   identity, active checkout commit, and policy entry id into successor handoff
   evidence.
4. Move successor validation off `scheduler.last_continuation_decision`; treat
   scheduler state as a projection that must agree with sealed History.
5. Downgrade invocation JSON to a launch descriptor. It should never be the
   authority token that admits a successor.
6. Validate ready/completion files against the expected invocation and use
   exclusive create semantics. Longer term, record them as ingress or transition
   facts with payload hashes and admitting policy.
7. Rename or document `successor::Record` as projection/evidence only. Do not let
   `Ready` or `SuccessorHandoffEntry` stand in for `Successor<Admitted>`.
8. Remove or explicitly policy-label the stale rejected-evaluation successor
   shortcut before using these runs as evidence of protocol correctness.
