# Prototype 1 Record Emission Sites Audit

## Summary

I traced the `ploke-eval loop prototype1-state` write path and found no new persisted record family outside the admission map. The path still emits legacy JSONL journal entries plus mutable sidecar JSON records; the map is broadly accurate, but the code still duplicates several facts that History should treat as projections or evidence refs.

## File / Line References

- Journal envelope and append: [`crates/ploke-eval/src/cli/prototype1_state/journal.rs`](../../../../crates/ploke-eval/src/cli/prototype1_state/journal.rs:129), [`...:281`](../../../../crates/ploke-eval/src/cli/prototype1_state/journal.rs:281), [`...:598`](../../../../crates/ploke-eval/src/cli/prototype1_state/journal.rs:598)
- Parent turn and typed transition writers: [`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`](../../../../crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3229), [`c1.rs`](../../../../crates/ploke-eval/src/cli/prototype1_state/c1.rs:523), [`c2.rs`](../../../../crates/ploke-eval/src/cli/prototype1_state/c2.rs:368), [`c3.rs`](../../../../crates/ploke-eval/src/cli/prototype1_state/c3.rs:213), [`c4.rs`](../../../../crates/ploke-eval/src/cli/prototype1_state/c4.rs:274), [`child.rs`](../../../../crates/ploke-eval/src/cli/prototype1_state/child.rs:123), [`successor.rs`](../../../../crates/ploke-eval/src/cli/prototype1_state/successor.rs:67)
- Scheduler / node / runner-request / runner-result: [`scheduler.rs`](../../../../crates/ploke-eval/src/intervention/scheduler.rs:372), [`...:386`](../../../../crates/ploke-eval/src/intervention/scheduler.rs:386), [`...:515`](../../../../crates/ploke-eval/src/intervention/scheduler.rs:515), [`...:600`](../../../../crates/ploke-eval/src/intervention/scheduler.rs:600), [`...:676`](../../../../crates/ploke-eval/src/intervention/scheduler.rs:676), [`...:688`](../../../../crates/ploke-eval/src/intervention/scheduler.rs:688)
- Branch registry and evaluation summary: [`branch_registry.rs`](../../../../crates/ploke-eval/src/intervention/branch_registry.rs:202), [`...:216`](../../../../crates/ploke-eval/src/intervention/branch_registry.rs:216), [`...:352`](../../../../crates/ploke-eval/src/intervention/branch_registry.rs:352), [`...:452`](../../../../crates/ploke-eval/src/intervention/branch_registry.rs:452), [`...:682`](../../../../crates/ploke-eval/src/intervention/branch_registry.rs:682), [`process.rs`](../../../../crates/ploke-eval/src/cli/prototype1_process.rs:1186)
- Invocation / ready / completion / streams / handoff: [`invocation.rs`](../../../../crates/ploke-eval/src/cli/prototype1_state/invocation.rs:295), [`...:410`](../../../../crates/ploke-eval/src/cli/prototype1_state/invocation.rs:410), [`...:442`](../../../../crates/ploke-eval/src/cli/prototype1_state/invocation.rs:442), [`process.rs`](../../../../crates/ploke-eval/src/cli/prototype1_process.rs:306), [`...:333`](../../../../crates/ploke-eval/src/cli/prototype1_process.rs:333), [`...:466`](../../../../crates/ploke-eval/src/cli/prototype1_process.rs:466), [`...:744`](../../../../crates/ploke-eval/src/cli/prototype1_process.rs:744), [`...:851`](../../../../crates/ploke-eval/src/cli/prototype1_process.rs:851)

## Emitted Record Families

- `prototype1/transition-journal.jsonl`: `ParentStarted`, `MaterializeBranch`, `BuildChild`, `SpawnChild`, `Child`, `ChildReady`, `ObserveChild`, `Successor`, `ChildArtifactCommitted`, `ActiveCheckoutAdvanced`, `SuccessorHandoff`
- `prototype1/scheduler.json` and `prototype1/nodes/*/node.json`
- `prototype1/nodes/*/runner-request.json`
- `prototype1/nodes/*/runner-result.json` and `prototype1/nodes/*/results/<runtime-id>.json`
- `prototype1/nodes/*/invocations/<runtime-id>.json`
- `prototype1/nodes/*/successor-ready/<runtime-id>.json`
- `prototype1/nodes/*/successor-completion/<runtime-id>.json`
- `prototype1/evaluations/<branch-id>.json`
- `prototype1/branches.json`
- `prototype1/nodes/*/streams/<runtime-id>/stdout.log` and `stderr.log`

## Gaps / Uncertainties vs Admission Map

- The map says `nodes/*/results/<runtime-id>.json` should be preferred over `runner-result.json`; the code still writes both on every attempt, so the latest file is still a full mutable copy, not a pointer/projection. See [`process.rs:1047-1056`](../../../../crates/ploke-eval/src/cli/prototype1_process.rs:1047) and [`scheduler.rs:600-613`](../../../../crates/ploke-eval/src/intervention/scheduler.rs:600) versus map lines 25-26 and 82-83.
- The map wants branch registry summaries to carry a path/ref to the full evaluation artifact; `record_treatment_branch_evaluation` still stores only the inline summary in `branches.json`. See [`branch_registry.rs:682-710`](../../../../crates/ploke-eval/src/intervention/branch_registry.rs:682) versus map lines 31 and 81.
- Successor ready/completion are dual-written to standalone files and journal records. That matches the map, but the map’s caveat still stands: there is no broad typed reader path yet, so these files remain process evidence more than durable authority. See [`process.rs:306-359`](../../../../crates/ploke-eval/src/cli/prototype1_process.rs:306) and [`invocation.rs:410-457`](../../../../crates/ploke-eval/src/cli/prototype1_state/invocation.rs:410) versus map lines 28-29 and 84-85.
- Stream logs are only referenced, not normalized. `SpawnEntry` and `SuccessorHandoffEntry` carry stream paths, and `open_streams` creates the files, but the map’s “evidence refs only” treatment still looks correct. See [`journal.rs:147-152`](../../../../crates/ploke-eval/src/cli/prototype1_state/journal.rs:147) and [`process.rs:123-156`](../../../../crates/ploke-eval/src/cli/prototype1_process.rs:123) versus map line 39.
- Scheduler and node records are still mutated as durable JSON mirrors on every transition. The map’s projection/cache language is accurate, but the code does not yet enforce that distinction mechanically. See [`scheduler.rs:515-597`](../../../../crates/ploke-eval/src/intervention/scheduler.rs:515) and [`...:688-833`](../../../../crates/ploke-eval/src/intervention/scheduler.rs:688) versus map lines 30, 32, and 69.

## Recommended Follow-Ups

- Make the attempt-scoped result the only durable child result artifact, and demote `runner-result.json` to a pointer or compatibility shim.
- Add evaluation artifact path/digest refs to branch registry summaries.
- Add a typed load/validation path for successor-ready and successor-completion before treating them as reusable evidence.
- Keep History import focused on journal + evaluation + attempt result artifacts; treat scheduler, node, and stream files as projections or diagnostics only.
