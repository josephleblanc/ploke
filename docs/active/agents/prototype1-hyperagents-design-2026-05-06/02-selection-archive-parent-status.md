# Prototype 1 Selection / Archive Parent Status

## Status

Current Prototype 1 implements **generation-local successor selection**, not a
HyperAgents-style archive selector.

`successor_selection` decides whether one completed child branch should become
the next successor coordinate, and `decide_generation` chooses within the
children produced by the active parent generation. The decision is local to the
current parent turn: first accepted child wins; if none are accepted, the best
rejected child may be selected as an exploration coordinate
(`crates/ploke-eval/src/successor_selection/mod.rs:28`,
`crates/ploke-eval/src/successor_selection/mod.rs:33`,
`crates/ploke-eval/src/successor_selection/mod.rs:46`).

There is no implemented archive/frontier-wide parent selector. The scheduler
has `frontier_node_ids`, completed/failed node lists, and a mutable
`last_continuation_decision`, but the typed parent path filters runnable
children by direct child generation and active parent id
(`crates/ploke-eval/src/intervention/scheduler.rs:198`,
`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5010`,
`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5084`,
`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5185`).

`Accepted` and `ExploreFrom` are structurally distinct successor outcomes.
Rejected archive admission and archive parent selection are not implemented as
structural concepts. `ExploreFromRejected` is a continuation policy projection
for a rejected child chosen as an exploration coordinate, not admission of a
rejected child into an archive
(`crates/ploke-eval/src/successor_selection/decision.rs:64`,
`crates/ploke-eval/src/intervention/scheduler.rs:77`,
`crates/ploke-eval/src/intervention/scheduler.rs:683`).

## Existing Pieces

`successor_selection` consumes `SelectionInput`: candidate node/branch/generation,
branch disposition, the branch evaluation artifact path, and per-instance
`RunComparison` rows containing optional parent and child `OperationalRunMetrics`
(`crates/ploke-eval/src/successor_selection/evidence.rs:9`,
`crates/ploke-eval/src/successor_selection/evidence.rs:34`).
`selection_input_from_child_report` builds that input from the selected child
node and `Prototype1BranchEvaluationReport`, preserving only baseline metrics,
treatment metrics, instance status, and overall branch disposition
(`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:7020`).

The default selection registry evaluates only the operational domain
(`crates/ploke-eval/src/successor_selection/registry.rs:19`). Future domain names
exist for protocol, patch, oracle, and adjudication, but they are not active in
the default registry (`crates/ploke-eval/src/successor_selection/domains/mod.rs:12`).
Operational evaluation skips rows missing parent or child metrics, counts
improvements/regressions, and maps `Keep` with comparable metrics and no
regressions to `Better` (`crates/ploke-eval/src/successor_selection/domains/operational.rs:15`,
`crates/ploke-eval/src/successor_selection/domains/operational.rs:39`).
`Better` becomes `SuccessorOutcome::Accepted`; mixed, worse, inconclusive, or
missing evidence stop for a single candidate
(`crates/ploke-eval/src/successor_selection/decision.rs:21`,
`crates/ploke-eval/src/successor_selection/decision.rs:27`).

Generation fanout in the typed parent path is direct-child only. The active
parent can materialize only generation `parent.generation + 1` nodes whose
`parent_node_id` matches the active parent
(`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5185`,
`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5210`).
The child plan message also records the same direct-child generation rule
(`crates/ploke-eval/src/cli/prototype1_state/parent.rs:111`,
`crates/ploke-eval/src/cli/prototype1_state/parent.rs:121`), and validation
requires the child plan's parent id, generation, and staged children to match
the current parent turn (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:569`).

Fanout width is `child_budget.min`, capped by available nodes; total planned
children are truncated to `child_budget.max` before fanout
(`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5932`,
`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5943`,
`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5427`).
`run_child_fanout` runs children in waves, sorts completed outcomes by
`plan_index`, and stops launching later waves once an accepted selection exists
(`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5453`,
`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5504`,
`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5508`).
After fanout, `generation_selection` returns the accepted child or rejected
exploration fallback (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5518`,
`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5528`).

Child admission evidence exists as observation evidence, not archive admission.
Parent-side observation records `ObservedChildResult::Succeeded` with evaluation
artifact path and overall disposition, or `ObservedChildResult::Failed` with
runner disposition/detail/exit code
(`crates/ploke-eval/src/cli/prototype1_state/journal.rs:165`).
Successful child observation loads the evaluation report and records the
successful result (`crates/ploke-eval/src/cli/prototype1_state/c4.rs:436`,
`crates/ploke-eval/src/cli/prototype1_state/c4.rs:466`).

History/Crown provides the authority substrate for admitted lineage facts, but
current docs explicitly distinguish History from scheduler, branch registry,
CLI reports, metrics, and other mutable projections
(`crates/ploke-eval/src/cli/prototype1_state/history.rs:52`,
`crates/ploke-eval/src/cli/prototype1_state/history.rs:203`).
Live handoff seals/appends a minimal History block before successor spawn
(`crates/ploke-eval/src/cli/prototype1_process.rs:1154`,
`crates/ploke-eval/src/cli/prototype1_process.rs:1200`,
`crates/ploke-eval/src/cli/prototype1_process.rs:1232`), but the module docs
still say existing journals and reports must be imported as evidence with
explicit degraded provenance rather than treated as sealed History
(`crates/ploke-eval/src/cli/prototype1_state/history.rs:360`).

## Gaps

There is no `Archive` object, no archive admission result, no score-child
record, and no parent selector over an archive/frontier. Current selection ranks
children only inside the active parent generation and only from operational
metrics already copied into a branch evaluation artifact.

`Accepted` currently means "operational domain judged this child better enough
to continue"; it does not mean oracle-verified, adjudicated, protocol-safe, or
History-admitted. `ExploreFrom` currently means "no accepted child in this
generation, choose a rejected child as an exploration coordinate"; it is not a
separate archive state (`crates/ploke-eval/src/successor_selection/mod.rs:49`).

Rejected archive admission is absent. The only nearby admission-shaped enum is
History ingress import disposition, and it has accepted import modes only
(`crates/ploke-eval/src/cli/prototype1_state/history.rs:2216`). Failed or
rejected child evidence remains child observation/selection evidence unless a
future policy imports it into History or an archive.

Parent selection is also absent. The current parent is determined by the
running successor/parent handoff path, not by choosing an archive member as the
next parent. The typed path enforces direct child lineage for the current
parent, and History docs warn not to reconstruct authority from generation,
branch name, or scheduler frontier
(`crates/ploke-eval/src/cli/prototype1_state/history.rs:396`).

## Recommended Next Slice

After child admission evidence exists, add the smallest durable archive-facing
object before changing parent selection:

1. Define an append-only `ChildScore` or `ArchiveCandidate` record produced from
   one `ObservedChildResult` plus one `SuccessorDecision`.
2. Include node id, branch id, generation, parent node id, evaluation artifact
   path, operational metric summary, decision outcome, explicit evaluated domain
   set, and explicit absent domains (`oracle`, `adjudication`, `protocol`).
3. Store accepted and rejected/exploration candidates in the same archive-shaped
   ledger with distinct states, rather than overloading `SuccessorOutcome`.
4. Add a read-only archive/frontier projection that can rank candidates across
   generations and parents without authorizing handoff.
5. Only after that projection is stable, introduce a typed parent-selection
   decision that chooses an archive candidate to hydrate as `Parent<Ready>` or
   whatever later structural state replaces the current generation-local
   successor path.

That slice keeps the current successor path intact while creating the missing
HyperAgents substrate: scored child evidence admitted to an archive-like ledger,
then a later parent selector over that ledger.
