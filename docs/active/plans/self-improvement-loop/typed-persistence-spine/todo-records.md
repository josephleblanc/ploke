Yes. I think you’re right to slow this down before implementation. The danger is that we remove `serde_json::Value` locally but only create nicer-shaped blobs, without deciding what the archive graph actually needs to answer.

The thing to answer now is not “all UI details.” It is the **answer model**: for each major UI question, what typed facts, joins, and derived computations must exist so the UI can answer it without reverse-engineering records later.

I’d do a short pre-implementation pass before slice 1.

**What We Need To Pin Down**

1. **Identity model**
   
   What are the stable ids that every later feature depends on?

   Likely core ids:

   - run id
   - runtime id
   - artifact id
   - parent artifact id
   - child artifact id
   - patch id
   - protocol coordinate (`run_id`, `subject_id`, path, procedure)
   - evaluation id
   - candidate set root/commitment
   - candidate membership id
   - selected successor id
   - tool call id
   - LLM attempt id
   - timeline span id
   - code graph node id
   - metric/score id

   This matters now because early typed records need to preserve these joins. If protocol artifacts deserialize cleanly but do not carry the ids needed to connect parent -> patch -> child -> evaluation, we will have typed records that still do not support the UI.

2. **Archive graph primitives**

   We need to decide what the graph consumes conceptually:

   - `GraphNode`: artifact/runtime/agent instance
   - `GraphEdge`: lineage, patch, candidate, selection, evaluation, merge/composability
   - `InspectorRef`: stable pointer to typed detail records
   - `TimelineSpanRef`: stable pointer to derived timeline span projections
   - badges: score delta, running/failed, selected/rejected, evidence strength, changed item count

   This is the “UI contract” layer. Earlier slices should produce records that can eventually feed these primitives.

3. **Feature answer shapes**

   For each question, we should define the answer as a typed shape, even if the Rust type comes later.

   Example:

   “How did this parent select this successor?”

   Needs:

   - parent artifact id
   - final candidate set root/commitment
   - all candidate memberships in that final set
   - selected membership id
   - evaluation/metric records used for comparison
   - decision record
   - rejected candidates with reasons or available evidence
   - link to selected child artifact/runtime

   Verification later:

   - typed replay proves selected membership belongs to the final decision candidate set.

4. **Protocol artifact role**

   This is especially important before `protocol-artifacts.decode`.

   We should know whether protocol artifacts are only inspectable payloads, or whether they also carry decision evidence for graph edges.

   Questions to answer:

   - Which protocol kinds exist?
   - For each kind, what are typed input/output/artifact payloads?
   - Which ids must be inside or adjacent to each payload?
   - Which UI panels use each protocol payload?
   - How do malformed protocol payloads deserialize: typed error record, not raw fallback?
   - Does a protocol output cite patch/evidence/candidate/evaluation ids, or is that join held elsewhere?

5. **Timeline model**

   We need a minimal span algebra before implementing timeline-related records:

   - every span has id, kind, start, end/status, parent span id, associated graph node/edge refs
   - spans may be nested or overlapping
   - tool calls and LLM attempts are spans
   - patch application and evaluation are spans
   - timeline rendering is derived from typed spans, not logs

6. **Comparison model**

   “Anything comparable should be comparable” needs some structure.

   At minimum, comparable things need:

   - type/kind
   - ids
   - metric dimensions
   - baseline/comparator cohort
   - observed deltas
   - evidence strength
   - caveat: correlation vs repeated-eval/experimental support

   This prevents later “comparison panels” from becoming bespoke reports.

7. **Patch/code graph impact model**

   Before later patch slices, we should know the desired chain:

   - patch id
   - touched files
   - hunks/ranges
   - code graph item spans
   - directly modified functions/types/modules
   - score/evaluation deltas
   - other patches touching same loci
   - compatibility/composability result

   This is what lets the UI support the “locus” analysis you described.

**Suggested Next Move**

Before coding slice 1, I would add one compact section to the existing docs, not a new sprawling doc: an “Answer Shape Checklist” in `ui-drilldown-contract.md` or `traceability-matrix.md`.

For each major UI question, record:

`question -> graph primitive -> typed facts -> required joins -> derived computation -> acceptance test -> slices that provide it`

That gives implementation a target. Then when we do `protocol-artifacts.decode`, the goal is not merely “remove `Value`.” The goal is: “typed protocol payloads now provide the facts and ids needed for these specific archive graph answers.”

That is the missing bridge. Without it, I agree we risk doing tidy local typing work that later fails to compose.

## First Pass Identity Spine Map

Status: first validation pass complete. This section records the identity
answers that already appear in docs or record DTOs, plus the narrower questions
that still affect early typed-persistence slices.

Purpose: make the early typed-persistence slices preserve graph/UI joins rather
than only replacing local `serde_json::Value` staging.

### Known Identity Carriers

| Concept | Existing carrier | Current home | UI role | Status |
|---|---|---|---|---|
| Runtime instance | `RuntimeId` | `ploke-records::ids`; older local mirror in `ploke-eval::loop_graph` | runtime node, timeline span owner, tool/LLM/eval context | Carrier established. New shared persisted DTOs should prefer `ploke-records::ids`; keep the eval mirror as live/local compatibility only until narrow adapters replace callers. |
| Recoverable artifact state | `ArtifactId`; History `ArtifactRefRecord` | `ploke-records::ids`, `ploke-records::history`; older local mirror in `ploke-eval::loop_graph` | primary archive graph node, patch base, selected successor target | Carrier established. Current canonical relation is string identity between transparent carriers; add typed helper/projection joins later instead of inventing a second artifact identity. |
| Patch record | `PatchId` | `ploke-records::ids`; older local mirror in `ploke-eval::loop_graph` | patch/self-modification edge, diff inspector ref, composability input | Carrier established. Exact parent -> patch -> child edge identity remains deferred until patch/source-record slices. |
| Runtime operation coordinate | `Coordinate { runtime_id, target: OperationTarget }` | `ploke-records::ids`; older local mirror in `ploke-eval::loop_graph` | connects runtime action to artifact, patch set, or artifact set | Carrier established as passive DTO vocabulary; validate live adoption before widening active eval code. |
| Campaign / scheduler node / branch / candidate / instance | `CampaignId`, `SchedulerNodeId`, `BranchId`, `CandidateId`, `InstanceId`, `SourceStateId` | `ploke-records::ids` | scheduler/frontier identity and evaluation context | Passive carriers exist. Live selection spine uses occurrence/membership/set commitment rather than `CandidateId`. |
| Candidate occurrence | `CandidateOccurrenceId` | `ploke-records::ids`, History selection payload | observed candidate event in History/fine playback | Established. Must stay distinct from candidate-set membership. |
| Candidate membership | `CandidateMembershipId`; `CandidateSetRecord`; `CandidateSetMembershipRecord`; `CandidateSetCommitment` | `ploke-records::ids`, `ploke-records::history::payload`, `ploke-eval` History/selection code | selection edge identity: candidate belongs to a candidate set | Established concept. There is no standalone `CandidateSetId`; persisted candidate-set identity is root/commitment plus memberships. |
| History entry/block/lineage | `EntryId`, `BlockId`, `LineageId`, `BlockHash`, `HistoryStateRoot` | `ploke-records::ids`, `ploke-records::history` | lineage, sealed history spine, genesis/backward replay | Established for sealed playback. Joins to artifact/runtime records are implementation work, not a new identity question. |
| Parent identity | `ParentIdentityRecord`, `ParentIdentityRefRecord` | `ploke-records::identity`, `ploke-records::history` | parent-capable artifact identity and parent node label | Carrier established. It anchors parent/artifact identity through node, parent, campaign, branch, and optional artifact branch fields; runtime joins come from adjacent runtime-bearing records. |
| Selected successor | `SuccessorRefRecord { runtime, artifact }`; History selection payload with selected occurrence/membership and candidate set | `ploke-records::history`, `ploke-records::history::payload` | selected successor edge and active artifact transition | Established pieces. UI answer must combine successor ref with selection payload to prove "selected from which set". |
| Protocol artifact | `Artifact { subject_id, run_id, created_at_ms, procedure_name, model/provider, body }`; `ArtifactFile { path, artifact }`; `ProtocolArtifactSummaryRecord` | `ploke-records::protocol`, `ploke-records::history::payload`; live eval protocol paths use run-scoped strings | protocol inspector ref and decision evidence panel | Typed payload carriers exist, but decode still stages through values. Use compound protocol coordinate now; later joins come from adjacent History/eval/tool/artifact/patch records. No standalone protocol-artifact id is required now. |
| Tool call | current `ToolCallRecord.arguments` and execution records | `ploke-eval`, target `ploke-records` or owning crate typed DTOs | tool-call inspector, timeline segment, child self-eval action | Answered shape: replace argument `Value`/raw argument text with a closed typed tool-argument enum/record keyed by tool name plus typed parse-failure records. |
| LLM attempt | request/response/error/timeout/timeline records | `ploke-llm`, `ploke-eval` | provider attempt inspector, timeline segment, tool bridge | Answered shape: keep typed provider attempts, add typed response/error/timeout/tool-bridge projections for Ploke-owned reads; do not let UI/replay walk provider JSON directly. |
| Code graph item | file path, byte range/span, code graph node id | `ploke-tree` / code graph crates | patch impact, locus comparison, composability | Answered shape: later patch-scoped impact projection over patch id, artifact refs, touched files, ranges, hashes, code graph item refs, and diff inspector refs. |
| Timeline span | derived `TimelineSpan` / `TimelineSpanRef` projection | playback/projection layer | bottom timeline strip and concurrency view | Answered shape: derived projection, not persisted authority; causal order and typed source refs primary, timestamps/latencies refine rendering. |

### Immediate Questions Before `protocol-artifacts.decode`

Status: answered enough for slice 1 and promoted to
`traceability-matrix.md`. Keep this section as scratch context only.

1. Should slice 1 codify a rule that new shared persisted DTOs use
   `ploke-records::ids`, while `ploke-eval::loop_graph` remains a local mirror
   until a later cleanup?

   Answer: yes. New shared persisted DTOs should use `ploke-records::ids`.
   Existing eval-local mirrors are legacy/local until a cleanup slice replaces
   or narrows them.

2. What is the canonical join from a protocol artifact to the archive graph:
   `run_id`, `subject_id`, `ArtifactFile.path`,
   `ProtocolArtifactSummaryRecord.path`, History entry id, parent/child
   artifact ids, patch id, evaluation id, tool call id, or some typed
   combination?

   Answer for slice 1: the protocol coordinate is `run_id`, `subject_id`,
   artifact file path or summary path, `procedure_name`, and metadata such as
   `created_at_ms`, model, and provider. Graph joins to History, evaluation,
   tool, patch, and parent/child artifacts remain explicit follow-up work.

3. Are protocol artifacts only inspectable detail records, or can their typed
   payloads provide facts used directly by parent -> patch -> child reasoning?

   Answer: protocol payloads are typed inspectable facts. They may support
   reasoning only through explicit typed citations or adjacent typed records;
   slice 1 must not infer lineage from filenames, newest-artifact ordering, or
   reports.

4. If a protocol payload is malformed for its `procedure_name`, what typed
   error record should preserve the failed payload boundary without falling back
   to production `serde_json::Value` field walking?

   Answer: add a named typed parse/error record carrying the protocol
   coordinate, expected payload kind, and structured error detail. Exact Rust
   shape belongs to `protocol-artifacts.decode`.

5. Do existing protocol artifact bodies carry enough ids for
   `ui.protocol.outputs`, or is the join held by adjacent run registry,
   aggregate, History, or evaluation records? Current validation says the
   enforced joins are run/subject checks and aggregate selection by procedure
   plus newest artifact metadata, not History/evaluation/patch/tool joins.

   Answer: existing bodies are enough to type the protocol output panel, but
   not enough to complete archive-graph lineage joins. Adjacent records must
   carry those joins in later slices.

### Projection Questions Answered For Slice Planning

Status: answered as projection constraints and promoted to
`traceability-matrix.md` / `ui-drilldown-contract.md`. Do not block the
protocol slice on these; implement each in its named later slice.

- `GraphNode`: projection over artifact/runtime/agent-instance identity; exact
  Rust shape deferred to `evaluation-oracle-targets.identity-joins` and
  `runtime-artifact-lineage`.
- `GraphEdge`: role-preserving projection over patch, candidate membership,
  successor selection, History lineage, evaluation, and merge/composability
  relations; do not flatten into one authoritative parent-child edge.
- `CandidateSetId`: do not add now. Root/commitment plus role-specific
  membership ids is enough for current replay proofs.
- Timeline span identity: causal order and typed source refs are primary;
  timestamps are audit/rendering facts. The span carrier is a derived playback
  projection, not a persisted authority record.
- Patch hunk/range/code graph item identity: missing first-class impact
  projection; close in `patch.diff-code-graph-impact` as a patch-scoped read
  projection over artifact refs, touched files, ranges, hashes, code graph item
  refs, and diff inspector refs.
- Comparison cohort and evidence-strength identity: needs a role-aware
  comparison envelope; close in `score.value-locus-analysis` and
  `patch.child-composability`. The envelope must distinguish observed
  correlation, baseline/repeated-eval support, and future causal claims.

### Validation Assignments

Use bounded validation rather than fresh design. Each validator should report:

- existing source paths and line ranges;
- what is already answered;
- what remains open;
- which implementation slices are affected;
- smallest verification command or targeted search.

Suggested splits:

1. `records-identity`: `ploke-records::ids`, `identity`, `history`,
   `selection`, `protocol`.
2. `eval-identity`: `ploke-eval` live use of runtime/artifact/patch/candidate
   ids, especially `loop_graph`, History store, successor selection, and
   protocol aggregate paths.
3. `docs-identity`: durable design docs and UI contract consistency:
   artifact/runtime/parent semantics, candidate membership, successor
   selection, archive graph primitives.

### Validation Findings

Records validator:

- Confirmed the shared passive carriers in `ploke-records::ids` and the
  History/identity/protocol DTOs.
- Corrected protocol artifact shape: outer protocol `Artifact` has
  `subject_id`, `run_id`, `created_at_ms`, optional model/provider,
  `procedure_name`, and `body`; there is no `sequence`.
- Confirmed selection payloads already include selected occurrence,
  selected membership, candidate set, considered rows, traversal, and
  decision.
- Confirmed `CandidateSetRecord` / `CandidateSetMembershipRecord` exist, but
  there is no standalone `CandidateSetId`.
- Confirmed `ProtocolArtifactSummaryRecord` / `ProtocolArtifactsRecord`
  already model path and metadata summary joins.

Eval validator:

- Confirmed `ploke-eval::loop_graph` mirrors the shared runtime/artifact/patch
  target vocabulary.
- Confirmed live selection code uses `CandidateOccurrenceId`,
  `CandidateMembershipId`, and `CandidateSetCommitment`; scoped eval files did
  not use `CandidateId` as the selection-spine identity.
- Confirmed protocol identity in live eval is currently run-scoped strings:
  `run_id`, `subject_id`, path, and procedure/newest-artifact selection.
- Found no canonical link yet from protocol artifact to History entry,
  evaluation id, tool-call id, parent/child artifact id, or patch id.

Docs validator:

- Confirmed artifact provenance, artifact-surface measurement, and successor
  succession are distinct axes.
- Confirmed the archive graph contract already requires parent/child/patch,
  evaluation, successor lineage, candidate frontier, selected membership, and
  lineage back to genesis.
- Recommended promoting stable identity/join claims to
  `traceability-matrix.md` first, then mirroring UI answer shapes in
  `ui-drilldown-contract.md`; keep this file as the validation draft.
