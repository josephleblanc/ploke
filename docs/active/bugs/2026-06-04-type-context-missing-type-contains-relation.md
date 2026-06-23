# Bug: Type context queries can run against DBs without `type_contains`

## Status

**Resolved (2026-06-06).** Joined evidence from the live failure, the stale starting-DB
cache artifact, and the code path below. Fix landed as a runtime capability gate in
`ploke-rag`/`ploke-db`, a model-visible degrade note in `request_code_context`, and
starting-DB cache invalidation (`STARTING_DB_CACHE_VERSION` 3 + `typed_type_graph`
discriminator).

### Joined live failure evidence

| Field | Value |
|-------|-------|
| Campaign | `p1-admissionfix-g35flash-p25flash-20260606-053302` |
| Run | `run-1780751388442-structured-current-policy-c545d2b2` |
| Stale cache snapshot | `744264bb0078d14a2abe45a9cafda922a677afaf7619206f543bc88e466924c6` (mtime 2026-05-22) |
| Symptom | `Db(Cozo("Cannot find requested stored relation 'type_contains'"))` |
| Protocol segment review (blocked) | `1780751792263_tool_call_segment_review_BurntSushi__ripgrep-2209.json` |
| Blocked tool call | `[0] request_code_context` (`search_term: "printer replacement"`) |
| Recoverability verdict | `no_clear_recovery` (UI: blocked), confidence high |

Protocol adjudication for segment 0 (`LocateTarget`) marked call `[0]` as
recoverability-blocked: the only call in the segment failed with an internal
compilation error and the visible packet offered no agent-recoverable next step.

```text
request_code_context: Internal compiler error: DB error: Database error: Cannot find requested stored relation 'type_contains'
```

**Systemic mechanism:** A `typed_type_graph` binary reused a pre-typed-graph starting-DB
cache snapshot because `starting_db_cache_key` omitted any typed-graph schema
discriminator. The snapshot was built before typed-graph relations existed in the eval
cache pipeline, so `type_contains` was never registered. Type-context expansion still
ran because enablement was compile-time-only (`cfg!(feature = "typed_type_graph")`).

Observed symptom:

```text
ERROR ploke_tui::error: crates/ploke-tui/src/error.rs:86: Error: Db(Cozo("Cannot find requested stored relation 'type_contains'"))
```

## Broken Contract

Any TUI/RAG path that enables typed type-context expansion must either run
against a database that contains populated typed type graph stored relations, including
`type_contains`, or degrade preflight with an explicit diagnostic before issuing Cozo
queries against those relations.

## Evidence

The TUI error logger prints database errors at:

```text
crates/ploke-tui/src/error.rs
```

The missing relation is a real typed type graph relation, not an arbitrary table
name. It is defined by the transform schema behind the `typed_type_graph`
feature:

```text
crates/ingest/ploke-transform/src/schema/edges.rs
```

`TypeContainsSchema` creates:

```text
:create type_contains {
    parent_type_id: Uuid,
    child_type_id: Uuid,
    kind: String,
    position: Int,
    at: Validity
}
```

RAG type-context expansion calls DB APIs that require `type_contains`:

```text
crates/ploke-rag/src/core/mod.rs
crates/ploke-db/src/type_graph.rs
```

For example, `expand_hits_with_type_context` calls
`type_targets_reachable_from_owner` and `expand_type_context`, and those DB
queries traverse `*type_contains`.

The fixture/import layer has a split that can produce a DB without populated typed
relations. In `crates/ploke-db/src/database.rs`,
`prior_rels_for_plain_backup_import` explicitly removes typed type graph
relations under the `typed_type_graph` feature:

```text
relations.retain(|r| !is_typed_type_graph_relation(r));
```

The fixture loader then imports typed relations only for fixtures marked
`FixtureStatus::TypedTypeGraph`:

```text
crates/test-utils/src/fixture_dbs.rs
```

`fixture_nodes_local_embeddings`, which is used by RAG and the headless TUI
harness, is currently an active local-embedding fixture rather than a typed type
graph fixture:

```text
id: "fixture_nodes_local_embeddings"
status: FixtureStatus::Active
import_mode: FixtureImportMode::BackupWithEmbeddings
```

That means a typed build can legitimately load an otherwise healthy searchable
fixture DB that does not claim typed type graph coverage. If type-context
expansion is then enabled, the later query can fail with Cozo's missing stored
relation error (stale cache) or return BM25/dense-only results without telling
the model (plain fixture with empty typed-graph relation shells).

## Source Trace

Probable trace:

```text
TUI tool/search path
-> ploke-rag::core::RagService::expand_hits_with_type_context
-> Database::type_targets_reachable_from_owner / Database::expand_type_context
-> Cozo query references *type_contains
-> current DB lacks stored relation type_contains (stale cache) OR relations are empty shells (plain import)
-> Db(Cozo("Cannot find requested stored relation 'type_contains'")) OR silent BM25-only results
-> ploke_tui::error logs Error (stale cache case)
```

The bug is not that Cozo rejects the query. The bug is that the typed
type-context caller reached Cozo without first proving that the active database
has populated typed graph data required by that feature path.

## Docs/Policy Expectation

The typed type-resolution coverage docs treat `type_contains` as part of the
typed type graph contract:

```text
docs/testing/TYPE_RESOLUTION_COVERAGE.md
```

The backup fixture docs distinguish active local fixtures from typed graph
fixtures and describe typed graph fixture validation separately:

```text
docs/testing/BACKUP_DB_FIXTURES.md
```

So an active local embedding fixture should not be silently treated as a typed
graph fixture just because the binary was compiled with typed type graph
support.

## Current Repro Coverage

Regression tests added with the fix:

```text
cargo test -p ploke-rag type_context_disabled_safely_when_relations_absent
cargo test -p ploke-tui request_code_context_degrades_on_non_typed_db
cargo test -p ploke-eval starting_db_cache_key_differs_by_typed_graph_surface
cargo test -p ploke-eval doctor_flags_stale_starting_db_missing_typed_graph_relations
```

Existing tests cover the happy typed graph fixture path:

```text
cargo test -p ploke-db unit::type_graph_queries -- --nocapture
cargo test -p ploke-rag corpus_type_shape_matrix -- --nocapture
```

## Resolution

1. **`Database::has_typed_type_graph_relations`** — probes that required relations
   are registered **and** `type_contains` has rows (plain-import fixtures register
   empty schema shells but must not count as typed-graph capable).
2. **`RagService` construction gate** — disables type-context expansion and sets
   `type_context_degraded` when the probe fails; emits `tracing::warn!`.
3. **`request_code_context`** — surfaces degraded type-context as a `note`/`next_steps`
   entry so the model knows results are BM25/dense-only.
4. **Eval starting-DB cache** — `StartingDbCacheMetadata.typed_type_graph` +
   `STARTING_DB_CACHE_VERSION` 3 invalidates stale snapshots; optional
   `prototype1-doctor` check flags cached starting DBs missing typed-graph relations
   under a typed-graph build.

Stale snapshot `744264bb...` removed from `~/.ploke-eval/cache/starting-dbs/` as
explicit cleanup (version bump already invalidates by key).

## Related Reports

- [`2026-05-24-request-code-context-silent-stale-snippet-skip.md`](./2026-05-24-request-code-context-silent-stale-snippet-skip.md)
- [`2026-06-04-prototype1-post-apply-stale-snippet-indexing.md`](./2026-06-04-prototype1-post-apply-stale-snippet-indexing.md)
