# Prototype 1 Eval Store — Cozo Feature Map

Status: active planning note / backend capability review.

Related files:

- [`relational-data-model.md`](relational-data-model.md)
- [`database-planning-notes.md`](database-planning-notes.md)
- [`codegraph-eval-join-model.md`](codegraph-eval-join-model.md)
- [`debugging-and-research-query-workloads.md`](debugging-and-research-query-workloads.md)
- [`open-questions.md`](open-questions.md)

External docs checked:

- <https://docs.cozodb.org/en/latest/algorithms.html>
- <https://docs.cozodb.org/en/latest/queries.html>
- <https://docs.cozodb.org/en/latest/stored.html>
- <https://docs.cozodb.org/en/latest/vector.html>
- <https://docs.cozodb.org/en/latest/timetravel.html>
- <https://docs.cozodb.org/en/latest/aggregations.html>
- <https://docs.cozodb.org/en/latest/sysops.html>
- <https://docs.cozodb.org/en/latest/nonscript.html>
- <https://docs.cozodb.org/en/latest/execution.html>

Local source check: workspace `cozo = "0.7.6"`; the `cozo` crate default feature is `compact`, which includes `graph-algo`, `requests`, and SQLite storage. Native Ploke builds should therefore have the algorithms in `algorithms.html` available unless a future crate disables default features or targets WASM. Treat this as a backend capability, not as a semantic guarantee.

## Planning stance

Cozo has enough graph/search/temporal machinery that the eval store should not be shaped like the filesystem. But high-level Prototype 1 code should not depend directly on Cozo-only scripts.

Recommended split:

```text
EvalStore                 -- writes normalized evidence/ref rows
EvalQuery                 -- ordinary phase/debug/selection queries
EvalGraphQuery            -- optional graph-algorithm-backed queries
EvalSimilarityQuery       -- optional HNSW/FTS/LSH-backed queries
EvalTemporalQuery         -- optional time-travel/projection queries
EvalSnapshotStore         -- optional backup/export/import support
```

Other backends can implement these in Rust, with SQL/graph extensions, or as unsupported capability flags.

## Feature categories and Prototype 1 fit

### Datalog relations, recursion, and fixed rules

CozoScript has inline Datalog rules, recursion, aggregation, and fixed rules. Fixed rules use `<~` and include utilities/algorithms like `PageRank`, `ShortestPathDijkstra`, `ConnectedComponents`, `JsonReader`, etc.

Prototype 1 fit:

- use inline rules for ordinary joins over parent/runtime/attempt/channel/evaluation rows;
- use recursion for transitive ancestry, artifact derivation, successor lineage, and causal closure;
- use fixed graph rules for bounded analysis where the graph shape is already explicit;
- keep authority checks in Rust/typestate/History/Channel where required.

Schema implication: relation keys and edge views should make common graph traversals cheap. Do not bury graph endpoints inside JSON payloads.

### Stored relation operations and transactions

Relevant Cozo operations from `stored.html`:

- `:create`, `:replace`, `:put`, `:rm`, `:insert`, `:update`, `:delete`;
- `:ensure` and `:ensure_not` for transaction-time consistency checks;
- multi-statement chained scripts that execute atomically;
- ephemeral relations for transaction-local intermediate state;
- `:returning` for mutation receipts.

Prototype 1 fit:

- idempotent import of JSONL/channel/record rows;
- `dual-strict` writes that insert semantic rows and evidence refs in one transaction;
- append guards: ensure no duplicate event id or source cursor is being changed;
- late-result recovery guard: ensure terminal child evidence does not already exist before materialize/build re-entry;
- profile commitment admission: ensure the expected digest row exists and has not changed;
- channel/import receipt ingestion: insert message, receipt, and import row together.

Caution: Cozo does not enforce all semantic foreign keys for us. Writer/import code still owns validation and authority checks.

### Indices and key-prefix execution

Cozo stored relations are sorted by composite keys. Query execution docs emphasize key-prefix scans: bound key prefixes are cheap; full scans are expensive. Cozo also supports secondary indices as read-only reordered relations.

Prototype 1 fit:

- choose primary keys for stable uniqueness;
- add lookup relations or secondary indices for common alternate paths;
- verify query plans with `::explain` once physical DDL exists.

High-value lookup/index paths:

```text
(parent_id, event_index)
(parent_id, runtime_id)
(node_id, attempt_id)
(runtime_id, source_event_index)
(channel_id, source_cursor)
(run_id, turn_id)
(tool_call_id)
(benchmark_instance_id, profile_commitment_id)
(subject_kind, subject_id, ref_id)
```

This reinforces the current recommendation to avoid path-only identities and large nullable catch-all event tables.

### Aggregations

Cozo supports ordinary aggregations (`count`, `count_unique`, `collect`, `group_count`, `latest_by`, `smallest_by`, `mean`, `sum`, `variance`, etc.) and semi-lattice aggregations (`min`, `max`, `union`, `intersection`, `min_cost`, `shortest`, etc.) that can be used recursively under constraints.

Prototype 1 fit:

- benchmark rollups by model/profile/harness version;
- provider failure rates and validation coverage;
- latest terminal evidence per attempt without trusting mutable projections;
- shortest/min-cost causal paths where fixed algorithms are too heavy;
- grouping review findings by failure domain;
- building materialized current views from append events.

Caution: use append/event facts as input. Do not model mutable scheduler state as the aggregation source of truth.

### Time travel via `Validity`

Cozo time travel works when the last key column is `Validity`. It can query facts as-of a timestamp, with `ASSERT`/`RETRACT` conveniences.

Prototype 1 fit:

- materialized current projections with “state as of” debug views;
- operator/UI status tables that preserve previous states;
- historical comparison of derived labels or annotations.

Do **not** use `Validity` as the primary causal order for runtime evidence. Prototype 1 still needs explicit stream/event ordering:

```text
source_stream_id
source_event_index
source_cursor
source_line
runtime_sequence
parent_sequence
```

Time travel is a projection facility, not a substitute for channel/journal/History ordering.

### Proximity search: HNSW, MinHash-LSH, FTS

From `vector.html`, Cozo supports:

- HNSW vector indices with distance metrics `L2`, `Cosine`, `IP`;
- MinHash-LSH for near-duplicate strings or lists;
- full-text search with query syntax including `AND`, `OR`, `NOT`, prefixes, `NEAR`, and boosts;
- tokenizer/filter options including `Simple`, `Ngram`, `Lowercase`, `Stemmer`, `Stopwords`, etc.

Ploke already uses Cozo HNSW for code embeddings in `ploke-db`/`ploke-embed` paths.

Prototype 1 fit:

- HNSW: similar agent turns, prompts, failure summaries, benchmark tasks, or code/evidence snippets;
- FTS: logs, tool-result summaries, final responses, run-review findings, provider error messages;
- LSH: near-duplicate prompts, repeated refusal/error strings, duplicated patch diffs, repeated stale-anchor loops.

Implementation stance:

- normalize query dimensions as columns first;
- keep full LLM/log payloads as refs or separate large-value relations;
- index redacted summaries or embeddings, not secret-bearing raw payloads by default;
- expose via `EvalSimilarityQuery`, not core `EvalStore`.

### System ops and schema metadata

Useful system ops:

- `::relations`, `::columns`, `::indices` for introspection;
- `::describe` to attach human/AI-readable relation descriptions;
- `::index`, `::hnsw`, `::fts`, `::lsh` for index management;
- `::explain` for query plans;
- `::running`, `::kill` for runaway query management;
- `::access_level` to mark relations `protected`, `read_only`, or `hidden`;
- `::compact` for maintenance.

Prototype 1 fit:

- migration checks can assert expected relations/columns/indices;
- relation descriptions can document authority/projection status directly in DB metadata;
- debug tooling can expose query plans for slow research queries;
- archival snapshots can mark imported historical evidence read-only after import validation.

Caution: access levels protect against programmer mistakes, not malicious actors.

### Triggers and callbacks

Cozo supports stored-relation triggers and callbacks in some host environments. Trigger docs warn that direct imports do not run triggers, and triggers do not propagate.

Prototype 1 fit:

- possible maintenance of materialized projections after live writes;
- possible UI/watch notifications for relation changes;
- possible debug counters or summary tables.

Caution:

- avoid making triggers part of authority semantics;
- avoid relying on triggers for import/backfill correctness because direct import APIs skip them;
- prefer explicit writer transactions for first slices.

### Backup/export/import

The non-CozoScript APIs include:

- export selected relations as a consistent snapshot;
- import relations transactionally;
- backup/restore whole SQLite-backed Cozo DBs;
- import selected relations from backup.

Prototype 1 fit:

- reproducible research snapshots;
- historical replay fixtures from real runs;
- run-review evidence packages;
- cross-runtime import bundles when a child or treatment run sends parent-visible evidence.

Caution:

- direct import APIs do not run triggers;
- backup fixtures are schema-coupled and should follow the repository backup-fixture guardrails;
- imported child-local code graph facts must not become parent facts without explicit artifact/import scope.

### Large values

Cozo docs recommend storing large values in a dedicated relation and keeping metadata separately, because touched large values are read into memory during queries.

Prototype 1 fit:

- keep full LLM responses, stdout/stderr, raw run records, protocol artifacts, snapshots, and binaries as refs/hashes initially;
- if inline DB storage is needed later, split metadata and blob relations:

```text
eval_blob_ref { blob_id => kind, content_sha256, byte_len, sensitivity, source_ref }
eval_blob_bytes { blob_id => bytes_or_text }
```

Most queries should stop at metadata and only join bytes/text at the final drilldown step.

## Algorithm page details

The Cozo algorithms are fixed rules available when the `graph-algo` feature is compiled in. They are useful once we project typed eval facts into graph-shaped edge relations.

### Utilities

| Algorithm/utility | Prototype 1 use |
| --- | --- |
| `Constant` | Small inline fixtures, query params, synthetic rows. Mostly syntax sugar. |
| `ReorderSort` | Intermediate top-k ranking inside query plans, e.g. candidate scoring before final joins. Prefer global `:sort`/`:limit` for final output. |
| `CsvReader` | Ad hoc research/backfill import from CSV. Not for authority-bearing live runtime input. |
| `JsonReader` | Backfill/import of JSON or JSONL evidence into staging queries. Useful for historical run import experiments; not a replacement for typed import validation. |

`CsvReader`/`JsonReader` are not available on WASM targets. HTTP reads require the `requests` feature; local file reads do not.

### Connectedness and acyclicity

| Algorithm | Prototype 1 use |
| --- | --- |
| `ConnectedComponents` | Group disconnected evidence islands in a campaign; find candidates/runs not connected to any parent epoch. |
| `StronglyConnectedComponent` / `SCC` | Detect cycles in derived causal graphs, artifact derivation graphs, or retry/re-entry loops. |
| `TopSort` | Produce or validate an acyclic causal/phase ordering for event-dependency graphs. |
| `MinimumSpanningForestKruskal` / `MinimumSpanningTreePrim` | Lower-priority. Possible use for minimal explanation trees, but not a first-slice need. |

High-value debugging query: “show me every evidence island not reachable from the campaign’s active parent lineage.”

### Pathfinding

| Algorithm | Prototype 1 use |
| --- | --- |
| `ShortestPathBFS` | Unweighted causal path: parent start → child request → runtime result → selection → handoff. |
| `ShortestPathDijkstra` | Weighted path by duration/cost/confidence/risk. Useful for explanation and process metrics. |
| `KShortestPathYen` | Alternative causal/explanation paths, e.g. multiple evidence chains supporting a selection decision. |
| `BreadthFirstSearch` / `BFS` | Find nearest descendant/ancestor satisfying a condition, e.g. nearest provider error before terminal failure. |
| `DepthFirstSearch` / `DFS` | Explore one branch deeply, e.g. artifact lineage or nested agent-turn cause chain. |
| `ShortestPathAStar` | Only if a valid heuristic exists. Lower priority for eval evidence; more plausible for spatial/domain graphs, not current Prototype 1. |

This suggests adding algorithm-ready edge projections such as:

```text
eval_causal_edge { graph_scope, from_id, to_id => edge_kind, weight?, evidence_ref? }
eval_artifact_edge { graph_scope, from_artifact_id, to_artifact_id => edge_kind, operation_id? }
eval_trace_edge { graph_scope, from_event_id, to_event_id => edge_kind, turn_id?, runtime_id? }
```

These can be derived/materialized from typed relations. They should not replace the typed source relations.

### Community detection

| Algorithm | Prototype 1 use |
| --- | --- |
| `ClusteringCoefficients` | Identify tightly-coupled tools/files/failure nodes in trajectory graphs. |
| `CommunityDetectionLouvain` | Cluster failures, tool-use patterns, benchmark instances, or code touch co-occurrence networks. |
| `LabelPropagation` | Fast approximate grouping of similar trajectory/failure/code-touch graphs. |

These are research and observability tools, not loop-control authority. They can help papers and dashboards answer “what families of failure dominate after this harness change?”

### Centrality

| Algorithm | Prototype 1 use |
| --- | --- |
| `DegreeCentrality` | First exploration of new graph projections: which tools/files/events connect most evidence? |
| `PageRank` | Rank influential code items, failures, review findings, or trajectory states in evidence graphs. |
| `ClosenessCentrality` | Find central states in recurrent trajectories. |
| `BetweennessCentrality` | Identify bottlenecks/bridges, e.g. a validation step connecting edit attempts to success. Expensive; use cautiously. |

Centrality should inform diagnosis/research, not selection authority, unless explicitly admitted as a scorer with validation.

### Random walk

`RandomWalk` can sample paths through a graph with optional weights.

Prototype 1 fit:

- stochastic exploration of large lineage/trajectory graphs for UI/research;
- possible future model-facing “show representative paths” query.

Do not use random-walk output as authority-bearing selection without a separate admitted policy and reproducibility story.

## Natural graph shapes for Prototype 1

The fixed algorithms require edge-shaped input relations. We should derive those edges from typed rows instead of flattening all data into one universal graph table.

Potential graph projections:

### Causal runtime graph

Nodes:

```text
parent_epoch, attempt, runtime, invocation, channel_message, transition_event,
evaluation, selection_decision, successor, evidence_warning
```

Edges:

```text
started_by, spawned, observed, timed_out, result_after_timeout,
compared_by, selected_by, handed_off_to, contradicted_by
```

Queries:

- shortest path from failure to provider error;
- disconnected runtime attempts;
- cycles caused by destructive re-entry;
- all evidence supporting a terminal status.

### Artifact and operation graph

Nodes:

```text
artifact, surface, patch, operation, binary, runtime, benchmark_instance
```

Edges:

```text
derived_from, patch_applied_to, built_into, hydrated_runtime,
evaluated_on, selected_successor_artifact
```

Queries:

- artifact ancestry;
- selected-child provenance;
- stale parent identity or surface-root mismatch paths;
- central files touched by successful versus failed branches.

### Agent turn graph

Nodes:

```text
model_exchange, tool_call, tool_result, edit_proposal, apply_event,
validation_event, assistant_message, code_ref
```

Edges:

```text
requested, emitted_tool_call, executed, returned_to_model,
quoted_by_next_message, proposed_edit, applied_to, validated_by, cited_code
```

Queries:

- tool output → next model action → edit → validation chains;
- repeated same-file loops;
- model-visible versus hidden failures;
- validation-after-edit coverage.

### Research/failure graph

Nodes:

```text
campaign, profile_commitment, model_config, provider_route, benchmark_instance,
failure_domain, review_finding, code_area, harness_version
```

Edges:

```text
observed_in, grouped_as, affected_by, uses_model, uses_profile,
failed_on, reviewed_as, touched_area
```

Queries:

- failure communities before/after harness changes;
- provider failure rates by route/model;
- benchmark improvement trends excluding provider outages;
- similar failures or near-duplicate run reviews.

## Backend trait implications

Do not expose algorithm names directly in high-level loop code. Prefer workload-specific traits.

```rust
trait EvalGraphQuery {
    fn causal_path(&self, from: EvalNodeRef, to: EvalNodeRef) -> Result<Vec<EvalEdgeRef>>;
    fn disconnected_evidence(&self, campaign: CampaignId) -> Result<Vec<EvidenceIsland>>;
    fn artifact_ancestry(&self, artifact: ArtifactId) -> Result<Vec<ArtifactEdge>>;
    fn centrality(&self, graph: EvalGraphKind, metric: CentralityMetric) -> Result<Vec<ScoredNode>>;
}

trait EvalSimilarityQuery {
    fn similar_turns(&self, turn: TurnId, limit: usize) -> Result<Vec<ScoredTurn>>;
    fn search_evidence_text(&self, query: &str, limit: usize) -> Result<Vec<EvidenceHit>>;
    fn near_duplicate_prompts(&self, prompt: PromptId, limit: usize) -> Result<Vec<ScoredPrompt>>;
}

trait EvalTemporalQuery {
    fn current_projection_as_of(&self, subject: EvalSubject, at: EvalTime) -> Result<ProjectionView>;
}
```

The Cozo backend can implement these with fixed rules, indices, FTS/HNSW/LSH, and time travel. A filesystem backend can implement a subset with replay and Rust graph algorithms.

## Implementation priority

First file-or-db storage pass:

1. **Transactions with `:ensure` / `:ensure_not`** for idempotent import and destructive re-entry guards.
2. **Secondary indices / lookup relations** for parent/runtime/node/turn/tool query paths.
3. **Aggregations** for doctor summaries and provider-vs-merit rollups over normalized eval evidence.
4. **Backup/export/import** for reproducible storage snapshots and replay fixtures.
5. **Time travel** only for materialized current projections after append/event rows are stable.

Follow-on query/research pass after storage parity:

6. **FTS over redacted summaries** for logs, provider errors, review findings, and tool-result summaries.
7. **Graph path algorithms** over materialized causal/artifact/turn/code edge projections.
8. **HNSW/LSH** for similar-turn and near-duplicate-prompt/failure analysis.

Lower priority or caution:

- triggers for first-slice authority paths;
- random walk for anything authority-bearing;
- storing full LLM/log blobs inline;
- centrality/community detection in live loop decisions before research validation.

## Open questions added by this feature review

- Which edge projections should be materialized first: causal runtime, artifact operation, agent turn, or research/failure graph?
- Should `eval_causal_edge` be a physical relation, a query rule, or a backend-specific view?
- What relation descriptions should be attached via `::describe` so DB snapshots remain self-documenting?
- Which FTS/LSH inputs are safe to index after redaction?
- Should `dual-strict` use `:ensure_not` on deterministic event ids or compare full row hashes before `:put`?
- Which queries need `::explain` baselines before we call them from `doctor` or `walk`?
