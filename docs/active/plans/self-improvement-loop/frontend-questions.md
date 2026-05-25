# Frontend Questions

Questions the loop observability frontend should answer before feature work is considered complete.

## Loop Progress

- Is the run still live, quiescent, or terminal, and which typed evidence supports that state?
- What benchmark objective is this run trying to improve?
- Which generation, parent, child, artifact, and candidate does the selected result belong to?
- What changed between generations, and where is that represented as typed evidence?
- Did the run improve, regress, or fail to measure the oracle benchmark?

## Benchmark Trajectory

- What is the score trajectory over generations?
- Which score changes are mechanical, oracle-backed, or only projection-level summaries?
- What is the distribution of kept, rejected, failed, and unmeasured candidates per generation?
- Which benchmark cases repeatedly fail, regress, or stop being measured?
- Which patch or successor decision is the best-supported cause of an improvement?

## Causal Order

- What is the sealed History order for this run?
- Where do transition-journal and runtime-local phases join onto sealed History steps?
- Which steps are ordered by causal records, and which timestamps are only audit data?
- Does the successor handoff chain preserve parent -> child -> selected successor -> next parent continuity?

## Evidence And Authority

- Which facts come from sealed History records, admitted projections, runtime artifacts, or benchmark outputs?
- Which records are missing, malformed, or incompatible with the current schema?
- Can the UI show evidence strength without upgrading weak projections into authority?
- Can an operator trace a frontend row back to a persisted record path and typed record id?
- Can applicability-aware diagnostics distinguish "not applicable" from "missing" and "failed"?

## Runtime Diagnosis

- Where did the loop spend time?
- Which child attempts failed, timed out, or produced unusable artifacts?
- Which transitions occurred, and which expected transitions are absent?
- What was the stop reason for the long run?

## Benchmark Direction

- What oracle benchmark cases are driving the current loop?
- Which interventions were selected because of benchmark evidence?
- Are score changes attributable to a specific patch, generation, or selection decision?
- What evidence is still missing before claiming self-improvement?
- Which frontend view keeps benchmark movement visible even when the operator is inspecting runtime failures?

## Interaction Model

- Can the frontend browse from run -> generation -> child attempt -> artifact -> benchmark outcome?
- Can it compare two generations side by side?
- Can it filter by failure class, transition kind, evidence strength, and benchmark result?
- Can it expose unresolved questions without presenting them as facts?
- Can it show aggregate improvement first, then drill down to runtime diagnosis?
