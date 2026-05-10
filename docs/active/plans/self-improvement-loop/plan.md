# Self-Improvement Loop Plan

Updated: 2026-05-10

## Goal

Make multi-generation Prototype 1 runs inspectable as typed records that can be roundtripped, evaluated against oracle benchmark progress, and projected into a frontend without turning rendered output into source truth.

## Causal Chain

`oracle benchmark target -> parent runtime -> child attempts -> typed transitions and artifacts -> persisted records -> playback projection -> frontend views -> next parent decision`

## Ordered Work

1. Establish the restart spine.
   - Use [`handoffs.md`](handoffs.md) as the shared list of handoff docs for this task track.
   - Treat the typed observability plan as a contract, not the latest operational status.
   - Status: done for this planning pass.

2. Locate current implementation state.
   - Read the two current handoffs through sub-agent summaries first.
   - Follow up with targeted reads of exact files and line ranges reported by agents.
   - Status: records/playback/browser-model/egui shell are implemented enough for focused verification. See [`implementation-status.md`](implementation-status.md).

3. Evaluate the recent long loop run.
   - Find the current transition/journal/playback document or command path.
   - Do not use `scheduler.json`; it is not authoritative for this track.
   - Identify what typed records exist, what is missing, and which projections can be trusted.
   - Status: sealed playback and transition-journal routes are identified. See [`long-run-evaluation.md`](long-run-evaluation.md).

4. Update shared record/playback crates.
   - Patch `ploke-records`, `ploke-tree`, or adjacent crates only after the missing schema/roundtrip boundary is clear.
   - Preserve passive record authority: records model facts; CLI and frontend render projections.
   - Current read: no records/playback patch is justified yet. The reported open proof is browser bundling/serve verification, plus live-status authority cleanup.

5. Roundtrip and test.
   - Add or run focused tests for record serialization, playback loading, and projection compatibility.
   - Prefer splice tests from recorded or mock upstream output into the downstream consumer contract.
   - Next tests: bounded `cargo test` commands listed in [`implementation-status.md`](implementation-status.md), then WASM/browser verification once the `trunk build` invocation issue is isolated.

6. Reconnect the loop to benchmark improvement.
   - Identify where oracle benchmark score, generation lineage, intervention, and outcome are represented.
   - Track missing evidence explicitly instead of building UI affordances around absent facts.
   - Missing: aggregate benchmark trajectory and attribution are not first-class enough in the frontend plan.

7. Shape the frontend direction.
   - Use typed playback and benchmark evidence as the frontend model.
   - Keep frontend feature questions in [`frontend-questions.md`](frontend-questions.md).
   - Add live state, causal order, evidence tier, and benchmark trajectory as first-class UI questions before generic diagnostics.

8. Reconcile long-horizon planning.
   - Locate and compare the long-horizon doc with this plan.
   - Update this directory if the long-horizon plan names additional invariants, milestones, or evidence requirements.

## Preservation Checks

- Playback remains a read/projection layer over persisted typed records.
- Frontend views do not infer source facts from CLI text, logs, or stale scheduler files.
- Parent/child protocol state remains structural rather than flattened into event strings.
- Benchmark improvement is represented as evidence in the loop, not as a later narrative attached to runs.
- Causal order comes from sealed History, transition append order, invocation/runtime order, and typed artifacts; timestamps are audit data.
- Frontend aggregation must preserve evidence strength rather than merging projection, typed record, admitted History, and sealed History facts into one undifferentiated timeline.
