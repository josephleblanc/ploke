# Prototype 1 Eval Store — Code Graph Join Model

Status: follow-on planning note / code-graph overlay review; **not part of the first file-or-db storage migration pass**.

Related files:

- [`relational-data-model.md`](relational-data-model.md)
- [`database-planning-notes.md`](database-planning-notes.md)
- [`cozo-feature-map.md`](cozo-feature-map.md)
- [`debugging-and-research-query-workloads.md`](debugging-and-research-query-workloads.md)
- [`storage-plan.md`](storage-plan.md)
- [`crates/ingest/ploke-transform/src/schema/mod.rs`](../../../../crates/ingest/ploke-transform/src/schema/mod.rs)
- [`crates/ploke-db/src/helpers.rs`](../../../../crates/ploke-db/src/helpers.rs)
- [`crates/ploke-db/src/get_by_id/mod.rs`](../../../../crates/ploke-db/src/get_by_id/mod.rs)

## Purpose

This note considers what becomes possible when Prototype 1 loop records and the parsed Ploke code graph live in the same Cozo database handle.

## First-pass boundary

This note is about **follow-on code-graph overlays over eval evidence**, not the full persistence-port plan. It assumes ordinary eval evidence/refs can be queried in the same owner-scoped database as a parsed code graph, then asks how to join those facts to code nodes/snapshots. History, channels, message boxes, bootstrap, journals, and artifact mutation remain separate ports; this note may reference them only by explicit refs/evidence rows. See [`persistence-port-map.md`](persistence-port-map.md).

The first migration pass is narrower: make ordinary Prototype 1 eval evidence configurable between the current filesystem layout and a database-backed store, while preserving file output and authority boundaries.

Do **not** require the relations in this note for the first pass. The first pass should only avoid choices that would make these joins hard later:

- keep owner/artifact/scope fields explicit;
- keep raw records/logs as refs/hashes;
- avoid overloading Prototype 1 `node_id` with code graph node identity;
- avoid embedding code-node columns into every eval relation;
- keep the parent DB handle available as the eventual place for overlays.

The bridge relations here are follow-on overlay/query work after file/db parity for core eval evidence is proven.

The goal is not just co-location. The value is that eval facts can become **overlays over code graph facts**:

```text
code graph node / file / crate / relation / type edge
  <- cited, edited, validated, failed, protected, selected, reviewed, refreshed
Prototype 1 runtime/eval/trace evidence
```

This is especially valuable for the self-improving loop because the code being edited is Ploke itself. The database can answer questions about which Ploke code items were read, proposed for edit, actually changed, validated, selected, protected, repeatedly failed, or associated with provider/tool/validation regressions.

## Current code graph surfaces to join against

The current transform schema creates time-travel Cozo relations for parsed Rust nodes and edges. Important surfaces include:

- primary/associated code nodes:
  - `function`, `const`, `enum`, `macro`, `module`, `static`, `struct`, `trait`, `type_alias`, `union`, `method`;
- selected secondary/type surfaces:
  - `param`, `field`, `variant`, attributes, generic params, type nodes;
- source structure:
  - `file_mod`, `crate_context`, `workspace_metadata`;
- graph edges:
  - `syntax_edge`, `type_relation`, `type_use`, `type_contains`;
- compile/config masks:
  - `compilation_unit`, `compilation_unit_enabled_node`, `compilation_unit_enabled_edge`, `compilation_unit_enabled_file`, `compilation_unit_meta`;
- retrieval metadata:
  - `bm25_doc_meta`, embedding/HNSW relations from `ploke-db`/`ploke-embed`.

Useful existing join facts:

- code node id (`Uuid`) and relation kind;
- simple name and canonical module path;
- file path via `file_mod` and ancestor traversal;
- `span` byte offsets;
- node `tracking_hash`;
- file/module `tracking_hash` used as file hash by snippet resolution;
- namespace/crate context/root path;
- syntax/type edges for containment and impact expansion;
- compilation-unit membership for feature/target-sensitive validation.

Terminology guardrail: current Prototype 1 candidate ids like `node-...` are not code graph node ids. Use `candidate_node_id` or `p1_node_id` for Prototype 1 work-item strings, and `code_node_id` for parsed-code `Uuid`s.

## Authority and scope guardrails

### Code graph facts are projections over artifacts

The code graph is a queryable projection of an artifact checkout, not the artifact authority itself. Git/worktree content, patch/application evidence, and sealed History/handoff facts remain their own domains.

A row that joins eval evidence to a code graph node says:

```text
this event referred to this parsed code item under this graph/artifact snapshot
```

It does not by itself prove that an edit was applied, accepted, selected, or sealed.

### Snapshot scope must be explicit

Most existing code graph queries use `@ 'NOW'`. That is fine for live RAG, but dangerous for historical eval records. A run-review query should not silently re-resolve a May run against a June code graph.

Every eval-to-code reference should carry one of:

- `code_snapshot_id`, pointing to a recorded graph/artifact snapshot;
- `code_validity`, when using Cozo `Validity` as an as-of coordinate;
- enough redundant resolution data to re-check: `code_node_id`, relation, canonical path, relpath, span, node hash, file hash, artifact id.

### Parent graph and child graph are not automatically the same graph

For ordinary self-improvement over Ploke:

- the parent DB has the parent artifact code graph and parent-owned eval overlay rows;
- a child may produce a changed artifact that is not yet parsed into the parent DB;
- treatment runs may have their own code graphs over benchmark targets or child artifacts;
- a selected successor should reindex, inherit, or validate overlays under its new artifact.

Do not merge child-local code graph facts into the parent code graph just because both are Cozo rows. Import must be explicit and artifact-scoped.

## Natural bridge model

The bridge should be many-to-many. An eval subject can reference many code items, and a code item can be referenced by many eval subjects.

### `eval_code_snapshot`

A graph/artifact coordinate. This avoids relying on `@ 'NOW'` for historical joins.

```text
eval_code_snapshot {
  snapshot_id: String =>
  campaign_id: String?,
  artifact_id: String?,
  store_scope: String,          -- parent | child_runtime | treatment_run | imported
  namespace: String?,           -- code graph namespace when known
  crate_id: String?,            -- crate_context id when known
  root_path: String?,
  git_commit: String?,
  code_validity: String?,       -- Cozo Validity/as-of coordinate if used
  schema_version: String?,
  created_at: String?
}
```

This is a pointer to a code graph state, not a replacement for the code graph relations.

### `eval_code_ref`

A normalized reference to a code location or parsed code item.

```text
eval_code_ref {
  code_ref_id: String =>
  campaign_id: String,
  artifact_id: String?,
  snapshot_id: String?,
  code_node_id: Uuid?,
  node_kind: String?,           -- function | method | module | file | unknown | ...
  crate_id: String?,
  namespace: String?,
  canonical_path: [String]?,
  relpath: String?,
  span: [Int; 2]?,
  node_hash: String?,
  file_hash: String?,
  resolution: String,           -- exact | path_span | path_only | text_match | unresolved | ambiguous | stale
  source_ref: String?,
  recorded_at: String?
}
```

Store redundant fields intentionally. They make historical joins auditable after reindexing or artifact change.

### `eval_code_link`

Links any eval subject to a code ref.

```text
eval_code_link {
  subject_kind: String,
  subject_id: String,
  code_ref_id: String =>
  ref_role: String,             -- cited | retrieved | edited | proposed | applied | validated | failed | protected | selected | denied
  event_id: String?,
  confidence: Float?,
  evidence_ref: String?,
  recorded_at: String?
}
```

Examples of `subject_kind`:

```text
model_exchange | tool_call | edit_event | patch | apply_event | validation_event |
selection_decision | review_finding | policy_surface | attempt | run
```

This relation keeps `eval_code_ref` from becoming a giant nullable table of every possible reference role.

### `eval_code_touch`

Patch/application-specific touch facts. This should be derived from patch/apply records and may reference parsed nodes where resolution is possible.

```text
eval_code_touch {
  touch_id: String =>
  campaign_id: String,
  patch_id: String?,
  apply_id: String?,
  code_ref_id: String?,
  touch_kind: String,           -- add | modify | delete | move | format | unknown
  relpath: String,
  pre_hash: String?,
  post_hash: String?,
  hunk_ref: String?,
  recorded_at: String?
}
```

This is where post-image code can be represented even before the child artifact is parsed. `code_node_id` may be null for newly-added items until a successor or child graph import binds it.

### Retrieval and context rows

Prompt/RAG context should be separately queryable because it is a major causal input to agent behavior.

```text
eval_retrieval {
  retrieval_id: String =>
  campaign_id: String,
  turn_id: String?,
  exchange_id: String?,
  query_text: String?,
  method: String,               -- bm25 | hnsw | hybrid | exact | tool_lookup
  scope_ref: String?,
  recorded_at: String?
}

eval_retrieval_hit {
  retrieval_id: String,
  rank: Int =>
  code_ref_id: String?,
  score: Float?,
  snippet_hash: String?,
  visible_to_model: Bool,
  reason: String?
}
```

This enables questions like “did successful children see the right functions before editing?”

### Validation coverage rows

Validation must be joined to code and changed paths, not only to tool lifecycle status.

```text
eval_validation_event {
  validation_id: String =>
  campaign_id: String,
  turn_id: String?,
  tool_call_id: String?,
  command: String,
  cwd: String?,
  manifest_path: String?,
  package: String?,
  exit_code: Int?,
  semantic_status: String,      -- passed | failed | setup_failed | timed_out | weak | unknown
  model_visible: Bool,
  recorded_at: String?
}

eval_validation_cover {
  validation_id: String,
  code_ref_id: String =>
  cover_kind: String,           -- changed_file | changed_crate | workspace | unrelated | unknown
  evidence_ref: String?
}
```

This directly addresses the recurring “cargo completed but did not validate the changed crate/files” problem.

### Index/refresh rows

Post-apply code graph freshness should be queryable.

```text
eval_refresh_event {
  refresh_id: String =>
  campaign_id: String,
  artifact_id: String?,
  trigger_id: String?,          -- apply/edit/tool event
  relpath: String?,
  owner_crate: String?,
  refreshed_crate: String?,
  snapshot_id: String?,
  status: String,               -- scheduled | completed | skipped | wrong_owner | failed
  evidence_ref: String?,
  recorded_at: String?
}
```

The wrong-crate refresh bug becomes a query instead of a log grep:

```text
changed relpath -> expected owner crate from file_mod/crate_context
refresh_event.refreshed_crate != owner_crate
later stale-anchor failure references same relpath
```

## High-value query families enabled by same-DB joins

### 1. Code item trajectory

Input: `code_node_id`, canonical path, or relpath.

Answer:

- Which turns retrieved or cited this item?
- Which tools proposed edits against it?
- Which patches touched it?
- Which validations covered it?
- Which attempts succeeded/failed after touching it?
- Was it selected, rejected, protected, or denied?
- Did later successor reindex preserve its tracking hash?

Sketch:

```text
code node -> eval_code_ref -> eval_code_link
  -> model/tool/edit/apply/validation/selection/review subjects
```

### 2. Candidate impact radius

Input: `patch_id`, `apply_id`, or child candidate.

Answer:

- Which code graph nodes/files were touched?
- Which containing modules/crates and compilation units are affected?
- Which syntax/type neighbors might need validation?
- Which tests or cargo packages should be run?
- Did the candidate validate the affected area or only an unrelated focused manifest?

Code graph edges and compilation-unit masks make this much stronger than path-only validation.

### 3. Retrieval effectiveness

Input: campaign/profile/model/benchmark.

Answer:

- Which retrieval hits were shown to the model?
- Which cited code items were later edited?
- Which retrieval methods correlate with successful edits?
- Which failed turns never retrieved the owning module/type?
- Which code items are repeatedly retrieved but never useful?

This can feed RAG tuning and paper metrics.

### 4. Failure localization

Input: failure domain or bug class.

Answer:

- Which code graph nodes/modules are associated with provider/model/tool/validation failures?
- Are stale-anchor failures concentrated in certain crates or same-file edit loops?
- Are `code_item_lookup` failures mostly `impl` nodes, methods, imports, or missing relation fields?
- Which validation failures are tied to changed code versus setup/harness issues?

This can turn bug triage into graph queries.

### 5. Selection explanation with code context

Input: selection decision.

Answer:

- What did the selected candidate touch?
- What did rejected candidates touch?
- Did the selected candidate affect a protected or high-centrality code area?
- Which code refs were used in evaluation findings?
- Was the selection based on code-relevant validation or provider/harness artifacts?

This improves both operator trust and research reporting.

### 6. Successor overlay migration

Input: selected artifact / successor parent.

Answer:

- Which policy labels must be carried forward?
- Which code refs still resolve by `code_node_id`/hash/path under the successor snapshot?
- Which protected/mutable labels became stale, missing, or ambiguous?
- Which candidate post-image nodes are now first-class code graph nodes after reindex?

This is central to safe self-editing.

## Cozo algorithm leverage over combined graphs

Once `eval_code_link` and code graph edges are present, several algorithm-backed projections become useful.

### Causal + code path

Build an edge projection containing both eval and code edges:

```text
eval event -> code_ref -> code_node -> syntax/type neighbor -> validation event -> selection
```

Use BFS/Dijkstra/Yen to find:

- shortest explanation from terminal failure to changed code;
- alternate evidence paths supporting a selection;
- nearest provider/model/tool failure upstream of a candidate reject.

### Connected components / SCC

Use connected components to find evidence islands:

- code touches that never connect to any validation;
- tool failures disconnected from any model-visible feedback;
- candidate workspaces or code refs disconnected from parent selection.

Use SCC/toposort to detect cycles or invalid causal order in derived event graphs, especially around destructive re-entry or repeated same-file loops.

### Centrality and community detection

Use centrality and community detection for research/dashboard analysis:

- code items/modules central to successful improvements;
- bottleneck functions/modules associated with many failures;
- communities of tools, files, benchmarks, and failure domains;
- before/after harness-change clusters.

These should remain research/diagnostic projections, not hidden selection authority.

### FTS/HNSW/LSH over joined evidence

Same DB enables mixed retrieval:

- code embedding search over code nodes;
- FTS over redacted logs/review findings/tool summaries;
- HNSW over trajectory/failure embeddings;
- LSH for near-duplicate prompts, patches, and error strings.

A future query could ask:

```text
Find prior turns similar to this failure, restricted to code refs in crates/ploke-eval
or neighbors of the touched function.
```

## Follow-on implementation priorities

The first storage pass should remain focused on configurable file-or-db persistence for ordinary eval evidence. After that parity exists, the first code-graph overlay slices would be:

1. `eval_code_snapshot` for parent code graph snapshot identity.
2. Richer `eval_code_ref` with `code_node_id`, path/span/hash, resolution, and snapshot.
3. `eval_code_link` as the generic subject-to-code bridge.
4. `eval_code_touch` from patch/apply evidence.
5. `eval_validation_event` / `eval_validation_cover` for cargo/test semantics.
6. `eval_retrieval` / `eval_retrieval_hit` once prompt/RAG events are easy to capture.
7. `eval_refresh_event` for post-apply reindex/freshness bugs.

Still defer:

- importing full child code graphs into the parent DB;
- centrality/community ranking in live decisions;
- storing full snippets/logs/LLM payloads inline;
- automatic policy migration without explicit successor validation.

## Example diagnostic queries

### Wrong-crate refresh

```text
apply_event -> code_touch(relpath)
code_touch.relpath -> file_mod/crate_context owner
refresh_event(trigger_id = apply_event) -> refreshed_crate
where refreshed_crate != owner
then join stale-anchor tool failures on relpath
```

### Weak final validation

```text
selected candidate -> code_touch -> owner crate / changed files
candidate -> validation_event -> validation_cover
where cover_kind in [unrelated, unknown] or semantic_status != passed
```

### Tool output used before edit

```text
tool_event(result, model_visible=true) -> message_event(next assistant)
message_event quotes/summarizes failure -> edit_event -> code_link(edited)
validation_event after edit
```

### Code item improvement history

```text
code_node_id -> code_ref -> code_link(ref_role in edited/applied/validated/selected)
join attempts/evaluations/selections over time
aggregate by model/profile/harness version
```

### Selection code-diff comparison

```text
selection_decision -> candidate set
candidate -> patch/apply -> code_touch -> code graph node/module/crate
candidate -> evaluation/validation/provider domain
compare selected vs rejected code areas and evidence strength
```

## Open design questions

- What should create the first `eval_code_snapshot`: setup, first DB restore, first `EvalStore` construction, or explicit code-graph admission?
- Can we capture a reliable Cozo `Validity` coordinate for the code graph at the same time as eval rows, or do we need a separate snapshot id/digest?
- Which event writers can cheaply resolve code refs immediately, and which should emit path/span refs for later backfill?
- Should retrieval hits store snippet text hashes even when code refs resolve exactly?
- How should policy labels bind across successor reindex: code node id, tracking hash, canonical path, or a validation rule using all three?
- Do we need separate relation prefixes for benchmark-target code graphs versus Ploke self-edit code graphs?
