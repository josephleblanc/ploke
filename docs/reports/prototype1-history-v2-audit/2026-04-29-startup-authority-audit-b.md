# Prototype 1 Startup Authority Audit B

Date: 2026-04-29

Scope: operator brief and module docs against the current runtime startup and
successor handoff implementation path. I read the requested files and only
targeted nearby code for parent identity, invocation, successor records, journal
labels, backend checkout checks, and History/Crown sealing boundaries.

## Summary

The operator brief mostly keeps the right boundary: current code has useful
typed handoff scaffolding and a History-shaped authority model, but it does not
yet perform Crown/History startup admission. The live successor becomes the next
bounded parent after checking artifact-carried identity, active checkout shape,
handoff invocation fields, and scheduler continuation state. It does not verify
a sealed History head, Tree key, predecessor block, or Crown-sealed admission
before entering the parent turn.

The main correction needed is wording discipline. Docs that describe
`Startup<Validated> -> Parent<Ruling>` and sealed-head verification should keep
that language explicitly intended/prescriptive. Docs that describe the live path
should say it is currently `Parent<Unchecked> -> Parent<Checked> -> Parent<Ready>`
plus successor invocation/ready records, not Crown-backed admission.

## Checked Claims

| Claim | Classification | Evidence |
| --- | --- | --- |
| Weekly audit must compare claims to implementation and inspect type barriers. | Intended/prescriptive. | [AGENTS.md:25](../../../AGENTS.md#L25), [AGENTS.md:27](../../../AGENTS.md#L27), [AGENTS.md:29](../../../AGENTS.md#L29) |
| Role/state structure must not be flattened into names or public status writes. | Intended/prescriptive. | [AGENTS.md:5](../../../AGENTS.md#L5), [AGENTS.md:6](../../../AGENTS.md#L6), [AGENTS.md:7](../../../AGENTS.md#L7) |
| The brief says live startup validation does not yet gate `Parent<Ruling>` on sealed History. | Implemented/descriptive, and accurate. | [prototype1-history-metrics-agent-brief.md:115](../../../docs/workflow/evalnomicon/drafts/prototype1-history-metrics-agent-brief.md#L115), [prototype1-history-metrics-agent-brief.md:116](../../../docs/workflow/evalnomicon/drafts/prototype1-history-metrics-agent-brief.md#L116), [prototype1-history-metrics-agent-brief.md:118](../../../docs/workflow/evalnomicon/drafts/prototype1-history-metrics-agent-brief.md#L118) |
| The brief's target sequence derives Tree key, verifies sealed History head, then enters `Parent<Ruling>`. | Intended/prescriptive. | [prototype1-history-metrics-agent-brief.md:183](../../../docs/workflow/evalnomicon/drafts/prototype1-history-metrics-agent-brief.md#L183), [prototype1-history-metrics-agent-brief.md:187](../../../docs/workflow/evalnomicon/drafts/prototype1-history-metrics-agent-brief.md#L187), [prototype1-history-metrics-agent-brief.md:188](../../../docs/workflow/evalnomicon/drafts/prototype1-history-metrics-agent-brief.md#L188), [prototype1-history-metrics-agent-brief.md:190](../../../docs/workflow/evalnomicon/drafts/prototype1-history-metrics-agent-brief.md#L190) |
| The brief says cross-runtime handoff is not one in-process state machine and does not claim OS-process uniqueness. | Intended/prescriptive, clearly qualified. | [prototype1-history-metrics-agent-brief.md:198](../../../docs/workflow/evalnomicon/drafts/prototype1-history-metrics-agent-brief.md#L198), [prototype1-history-metrics-agent-brief.md:201](../../../docs/workflow/evalnomicon/drafts/prototype1-history-metrics-agent-brief.md#L201), [prototype1-history-metrics-agent-brief.md:207](../../../docs/workflow/evalnomicon/drafts/prototype1-history-metrics-agent-brief.md#L207) |
| `mod.rs` says the full startup admission check is not implemented. | Implemented/descriptive, and accurate. | [mod.rs:85](../../../crates/ploke-eval/src/cli/prototype1_state/mod.rs#L85), [mod.rs:93](../../../crates/ploke-eval/src/cli/prototype1_state/mod.rs#L93), [mod.rs:96](../../../crates/ploke-eval/src/cli/prototype1_state/mod.rs#L96) |
| `prototype1_process.rs` describes intended successor handoff with `Parent<Selectable> -> Parent<Retired>`. | Mostly implemented/descriptive for the local move-only handoff; not History admission. | [prototype1_process.rs:39](../../../crates/ploke-eval/src/cli/prototype1_process.rs#L39), [prototype1_process.rs:43](../../../crates/ploke-eval/src/cli/prototype1_process.rs#L43), [prototype1_process.rs:52](../../../crates/ploke-eval/src/cli/prototype1_process.rs#L52), [parent.rs:423](../../../crates/ploke-eval/src/cli/prototype1_state/parent.rs#L423), [prototype1_process.rs:875](../../../crates/ploke-eval/src/cli/prototype1_process.rs#L875) |
| History module says current live handoff locks a Crown carrier but does not seal/persist a block. | Implemented/descriptive, and accurate for the code inspected. | [history.rs:6](../../../crates/ploke-eval/src/cli/prototype1_state/history.rs#L6), [history.rs:7](../../../crates/ploke-eval/src/cli/prototype1_state/history.rs#L7), [history.rs:159](../../../crates/ploke-eval/src/cli/prototype1_state/history.rs#L159), [history.rs:163](../../../crates/ploke-eval/src/cli/prototype1_state/history.rs#L163) |
| History `Crown<Locked>::seal` requires same-lineage Crown carrier. | Implemented/descriptive for the History API, not wired into live startup. | [history.rs:1455](../../../crates/ploke-eval/src/cli/prototype1_state/history.rs#L1455), [history.rs:1461](../../../crates/ploke-eval/src/cli/prototype1_state/history.rs#L1461), [history.rs:1466](../../../crates/ploke-eval/src/cli/prototype1_state/history.rs#L1466) |

## Findings

1. **The brief is clear on the main startup authority boundary.**

   The brief explicitly says live startup validation does not gate
   `Parent<Ruling>` on a sealed History head, and that live Crown sealing does
   not persist the block the next runtime must verify
   ([prototype1-history-metrics-agent-brief.md:115](../../../docs/workflow/evalnomicon/drafts/prototype1-history-metrics-agent-brief.md#L115),
   [prototype1-history-metrics-agent-brief.md:116](../../../docs/workflow/evalnomicon/drafts/prototype1-history-metrics-agent-brief.md#L116),
   [prototype1-history-metrics-agent-brief.md:118](../../../docs/workflow/evalnomicon/drafts/prototype1-history-metrics-agent-brief.md#L118)). The inspected code matches: startup loads artifact-carried identity, checks the active checkout and scheduler/node facts, acknowledges any handoff invocation, appends `ParentStarted`, then proceeds to a turn
   ([cli_facing.rs:3208](../../../crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs#L3208),
   [cli_facing.rs:3212](../../../crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs#L3212),
   [cli_facing.rs:3221](../../../crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs#L3221),
   [cli_facing.rs:3229](../../../crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs#L3229)). No sealed History head or Tree key is consulted on that path.

2. **Current successor admission is scheduler/invocation/checkout validation, not Crown/History admission.**

   Successor handoff validates that the invocation campaign, node, and active
   root match the current command/identity, then calls scheduler-continuation
   validation and writes a ready record
   ([cli_facing.rs:3073](../../../crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs#L3073),
   [cli_facing.rs:3082](../../../crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs#L3082),
   [cli_facing.rs:3100](../../../crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs#L3100),
   [cli_facing.rs:3110](../../../crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs#L3110),
   [cli_facing.rs:3111](../../../crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs#L3111)). The continuation validator checks only `scheduler.last_continuation_decision`, `ContinueReady`, and selected branch id
   ([prototype1_process.rs:388](../../../crates/ploke-eval/src/cli/prototype1_process.rs#L388),
   [prototype1_process.rs:390](../../../crates/ploke-eval/src/cli/prototype1_process.rs#L390),
   [prototype1_process.rs:400](../../../crates/ploke-eval/src/cli/prototype1_process.rs#L400),
   [prototype1_process.rs:401](../../../crates/ploke-eval/src/cli/prototype1_process.rs#L401)). Any doc phrase like "continuation authority" for this path is ambiguous/overclaiming unless it says "current scheduler/invocation authority" rather than Crown/History authority.

3. **The live handoff now has a real local `Parent<Selectable> -> Parent<Retired>` barrier, but the locked Crown is not consumed by live History.**

   The predecessor installs the selected Artifact, builds the active successor
   binary, locks Crown by consuming `Parent<Selectable>`, creates a successor
   invocation from `Parent<Retired>`, spawns the successor, and waits for a ready
   file
   ([prototype1_process.rs:861](../../../crates/ploke-eval/src/cli/prototype1_process.rs#L861),
   [prototype1_process.rs:869](../../../crates/ploke-eval/src/cli/prototype1_process.rs#L869),
   [prototype1_process.rs:875](../../../crates/ploke-eval/src/cli/prototype1_process.rs#L875),
   [prototype1_process.rs:886](../../../crates/ploke-eval/src/cli/prototype1_process.rs#L886),
   [prototype1_process.rs:919](../../../crates/ploke-eval/src/cli/prototype1_process.rs#L919),
   [prototype1_process.rs:941](../../../crates/ploke-eval/src/cli/prototype1_process.rs#L941)). The type barrier is meaningful because `lock_crown` is only on `Parent<Selectable>` and `SuccessorInvocation::from_retired_parent` takes `&Parent<Retired>`
   ([parent.rs:418](../../../crates/ploke-eval/src/cli/prototype1_state/parent.rs#L418),
   [parent.rs:429](../../../crates/ploke-eval/src/cli/prototype1_state/parent.rs#L429),
   [invocation.rs:242](../../../crates/ploke-eval/src/cli/prototype1_state/invocation.rs#L242),
   [invocation.rs:249](../../../crates/ploke-eval/src/cli/prototype1_state/invocation.rs#L249)). However, `locked_crown` is only logged in this path and is not passed to `Crown<Locked>::seal` or a live `BlockStore`, so any claim that live handoff is sealed History remains overclaiming
   ([prototype1_process.rs:875](../../../crates/ploke-eval/src/cli/prototype1_process.rs#L875),
   [prototype1_process.rs:876](../../../crates/ploke-eval/src/cli/prototype1_process.rs#L876),
   [history.rs:159](../../../crates/ploke-eval/src/cli/prototype1_state/history.rs#L159),
   [history.rs:161](../../../crates/ploke-eval/src/cli/prototype1_state/history.rs#L161),
   [history.rs:163](../../../crates/ploke-eval/src/cli/prototype1_state/history.rs#L163)).

4. **Checkout validation is concrete but weaker than intended Tree-key admission.**

   The live parent check validates a clean checkout, current branch against
   `parent_identity.artifact_branch`, HEAD commit message, and identity file in
   HEAD
   ([backend.rs:1037](../../../crates/ploke-eval/src/cli/prototype1_state/backend.rs#L1037),
   [backend.rs:1042](../../../crates/ploke-eval/src/cli/prototype1_state/backend.rs#L1042),
   [backend.rs:1050](../../../crates/ploke-eval/src/cli/prototype1_state/backend.rs#L1050),
   [backend.rs:1062](../../../crates/ploke-eval/src/cli/prototype1_state/backend.rs#L1062),
   [backend.rs:1073](../../../crates/ploke-eval/src/cli/prototype1_state/backend.rs#L1073)). `clean_tree_key` exists but is not used in the inspected startup path
   ([backend.rs:1108](../../../crates/ploke-eval/src/cli/prototype1_state/backend.rs#L1108),
   [backend.rs:1117](../../../crates/ploke-eval/src/cli/prototype1_state/backend.rs#L1117)). The brief correctly reserves Tree-key sealed-head admission for the target sequence
   ([prototype1-history-metrics-agent-brief.md:187](../../../docs/workflow/evalnomicon/drafts/prototype1-history-metrics-agent-brief.md#L187),
   [prototype1-history-metrics-agent-brief.md:188](../../../docs/workflow/evalnomicon/drafts/prototype1-history-metrics-agent-brief.md#L188)).

5. **Stale Successor-as-literal-typestate risk is mostly controlled, but a few headings/phrases can still mislead.**

   Current code models successor as an invocation role and journal projection,
   not as a literal `Successor<State>` authority carrier
   ([invocation.rs:83](../../../crates/ploke-eval/src/cli/prototype1_state/invocation.rs#L83),
   [invocation.rs:115](../../../crates/ploke-eval/src/cli/prototype1_state/invocation.rs#L115),
   [successor.rs:3](../../../crates/ploke-eval/src/cli/prototype1_state/successor.rs#L3),
   [successor.rs:18](../../../crates/ploke-eval/src/cli/prototype1_state/successor.rs#L18)). The top-level `mod.rs` phrase "Selected successor -> Successor bootstrap seam" and the `successor.rs` title "Typed successor runtime role-state records" are ambiguous because they can read like an implemented typestate role rather than projected handoff records
   ([mod.rs:4](../../../crates/ploke-eval/src/cli/prototype1_state/mod.rs#L4),
   [mod.rs:5](../../../crates/ploke-eval/src/cli/prototype1_state/mod.rs#L5),
   [successor.rs:1](../../../crates/ploke-eval/src/cli/prototype1_state/successor.rs#L1)). The body of `successor.rs` mitigates this by saying the records project the handoff path
   ([successor.rs:3](../../../crates/ploke-eval/src/cli/prototype1_state/successor.rs#L3),
   [successor.rs:4](../../../crates/ploke-eval/src/cli/prototype1_state/successor.rs#L4)).

6. **Flattened journal names remain in live records but are documented as legacy evidence, not History ontology.**

   Live handoff still appends `SuccessorHandoffEntry` and
   `ActiveCheckoutAdvancedEntry`
   ([prototype1_process.rs:943](../../../crates/ploke-eval/src/cli/prototype1_process.rs#L943),
   [prototype1_process.rs:945](../../../crates/ploke-eval/src/cli/prototype1_process.rs#L945),
   [prototype1_process.rs:573](../../../crates/ploke-eval/src/cli/prototype1_process.rs#L573),
   [prototype1_process.rs:575](../../../crates/ploke-eval/src/cli/prototype1_process.rs#L575)). This is structural naming debt, but the journal docs explicitly classify those names as legacy storage labels and require normalization before History admission
   ([journal.rs:20](../../../crates/ploke-eval/src/cli/prototype1_state/journal.rs#L20),
   [journal.rs:21](../../../crates/ploke-eval/src/cli/prototype1_state/journal.rs#L21),
   [journal.rs:37](../../../crates/ploke-eval/src/cli/prototype1_state/journal.rs#L37),
   [journal.rs:235](../../../crates/ploke-eval/src/cli/prototype1_state/journal.rs#L235),
   [journal.rs:252](../../../crates/ploke-eval/src/cli/prototype1_state/journal.rs#L252),
   [journal.rs:273](../../../crates/ploke-eval/src/cli/prototype1_state/journal.rs#L273)).

## Actionable Corrections

1. In the operator brief, keep the current "Not implemented" bullets near any
   startup/handoff target sequence. The existing boundary is good; do not split
   [prototype1-history-metrics-agent-brief.md:113](../../../docs/workflow/evalnomicon/drafts/prototype1-history-metrics-agent-brief.md#L113) through
   [prototype1-history-metrics-agent-brief.md:130](../../../docs/workflow/evalnomicon/drafts/prototype1-history-metrics-agent-brief.md#L130) away from the target sequence at
   [prototype1-history-metrics-agent-brief.md:183](../../../docs/workflow/evalnomicon/drafts/prototype1-history-metrics-agent-brief.md#L183).

2. Narrow live-path wording from "continuation authority" or "successor admission"
   to "scheduler/invocation/checkout continuation validation" wherever referring
   to current code, because the live validator is
   [prototype1_process.rs:377](../../../crates/ploke-eval/src/cli/prototype1_process.rs#L377) through
   [prototype1_process.rs:417](../../../crates/ploke-eval/src/cli/prototype1_process.rs#L417), not sealed History.

3. Rephrase stale successor typestate wording in future doc edits: prefer
   `SuccessorInvocation`, `successor::Record { state: ... }`, or "successor
   bootstrap attempt" for current code; reserve `Successor<Admitted>` or similar
   for intended History admission carriers. The potentially confusing current
   phrases are at [mod.rs:4](../../../crates/ploke-eval/src/cli/prototype1_state/mod.rs#L4) and
   [successor.rs:1](../../../crates/ploke-eval/src/cli/prototype1_state/successor.rs#L1).

4. When live History admission is implemented, wire the existing
   `Crown<Locked>::seal` boundary into the successor startup path and require the
   successor to verify a sealed predecessor block before `Parent<Ready>` or the
   future `Parent<Ruling>`. Current code has the sealing API at
   [history.rs:1455](../../../crates/ploke-eval/src/cli/prototype1_state/history.rs#L1455), but startup currently enters through
   [cli_facing.rs:3208](../../../crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs#L3208) and
   [cli_facing.rs:3221](../../../crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs#L3221).
