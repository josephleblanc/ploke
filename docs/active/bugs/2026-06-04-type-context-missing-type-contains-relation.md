# Bug: Type context queries can run against DBs without `type_contains`

## Status

Open. The observed error was reported from a live TUI/error log line, but the
exact command, campaign, and database artifact have not yet been joined to this
report.

Observed symptom:

```text
ERROR ploke_tui::error: crates/ploke-tui/src/error.rs:86: Error: Db(Cozo("Cannot find requested stored relation 'type_contains'"))
```

## Broken Contract

Any TUI/RAG path that enables typed type-context expansion must either run
against a database that contains the typed type graph stored relations, including
`type_contains`, or fail preflight before issuing Cozo queries against those
relations.

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

The fixture/import layer has a split that can produce a DB without the typed
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
relation error.

## Source Trace

Probable trace:

```text
TUI tool/search path
-> ploke-rag::core::RagService::expand_hits_with_type_context
-> Database::type_targets_reachable_from_owner / Database::expand_type_context
-> Cozo query references *type_contains
-> current DB lacks stored relation type_contains
-> Db(Cozo("Cannot find requested stored relation 'type_contains'"))
-> ploke_tui::error logs Error
```

The bug is not that Cozo rejects the query. The bug is that the typed
type-context caller reached Cozo without first proving that the active database
has the relation set required by that feature path.

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

Existing tests cover the happy typed graph fixture path:

```text
cargo test -p ploke-db --features typed_type_graph unit::type_graph_queries -- --nocapture
```

Existing RAG tests cover type-context expansion over typed graph corpus fixtures:

```text
cargo test -p ploke-rag --features typed_type_graph corpus_type_shape_matrix -- --nocapture
```

Those tests prove typed graph queries work when the DB contains typed graph
relations. They do not prove that TUI/RAG refuses or degrades safely when type
context is enabled over a plain/active local embedding DB.

## Missing Repro / Validation

Add a focused regression that loads a DB through the same path that triggered
the TUI error, enables type-context expansion, and asserts the boundary behavior.

The smallest likely repro is:

```text
typed_type_graph build
-> load fixture_nodes_local_embeddings through the normal RAG/headless TUI fixture path
-> run request_code_context or RagService search with type_context enabled
-> assert no raw Cozo missing-relation error reaches the TUI tool result
```

The test should also print or assert the active relation inventory so the failure
is unambiguous:

```text
has type_contains: false
type_context.enabled: true
fixture: fixture_nodes_local_embeddings
```

If the active live run has a campaign artifact or DB path for this error, add it
here before closing the report.

## Fix Direction

Fix the authority boundary that decides whether type-context expansion is
available for the active DB.

Acceptable directions:

1. At DB/RAG initialization, detect whether the required typed graph relations
   exist and disable typed type-context expansion with an explicit degraded
   diagnostic when they do not.
2. At fixture/campaign setup, require a typed graph searchable fixture whenever
   `typed_type_graph` type-context expansion is enabled.
3. Add a DB preflight method for typed graph relation availability and call it
   before `expand_hits_with_type_context` can issue typed graph queries.

Non-fixes:

1. Do not make Cozo missing-relation errors disappear by broadly swallowing
   `DbError::Cozo`.
2. Do not import plain fixtures as if they have typed graph coverage.
3. Do not create empty `type_contains` relations as a silent compatibility shim
   unless the caller also records that type-context evidence is unavailable; an
   empty typed graph can make retrieval look semantically complete when it is
   not.

## Related Reports

- [`2026-05-24-request-code-context-silent-stale-snippet-skip.md`](./2026-05-24-request-code-context-silent-stale-snippet-skip.md)
- [`2026-06-04-prototype1-post-apply-stale-snippet-indexing.md`](./2026-06-04-prototype1-post-apply-stale-snippet-indexing.md)
