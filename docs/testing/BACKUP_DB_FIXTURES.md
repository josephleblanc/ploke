# Backup DB Fixtures

Last reviewed: 2026-07-10
Last updated: 2026-07-16

This document is the current inventory for backup database fixtures under
the shared DB snapshot fixture directory. It records which source targets
produced each fixture, which tests consume it, whether those tests expect
mutable or immutable access, and the DB-level assumptions that make those tests
valid.

Runtime fixture loading uses `$XDG_CONFIG_HOME/ploke/db_snapshot_fixtures`, or
`~/.config/ploke/db_snapshot_fixtures` when `XDG_CONFIG_HOME` is unset. Set
`PLOKE_DB_SNAPSHOT_FIXTURE_DIR` to override this location for tests or unusual
local setups. The files under `tests/backup_dbs/` are committed seed artifacts;
they are not the normal runtime load location.

Backup DB fixture identity has two parts:

- the registry entry in
  [crates/test-utils/src/fixture_dbs.rs](../../crates/test-utils/src/fixture_dbs.rs)
- the fixture path scope

Checkout-local fixtures may contain absolute source roots, so registry loads
prefer a generated root-scoped file under `tests/backup_dbs/local/` when one
exists. Their committed registered path is a fallback and compatibility anchor,
not proof that the DB is valid for every worktree. Operational path-only
consumers must call `FixtureDb::checked_path()` so the effective path is loaded
and validated before it is copied or staged.

## Review cadence

- Review this document whenever a new backup is added, removed, renamed, or its
  schema expectations change.
- If this document is more than 7 days old, agents should remind the user that a
  fixture review is due before making more backup-fixture changes.

## Lifecycle commands

Use the registry-backed `xtask` commands for fixture health checks and
recreation guidance:

- `cargo xtask verify-backup-dbs`
  - validates active registered backup fixtures using their configured import
    mode and contract checks
- `cargo xtask verify-backup-dbs --fixture <id>`
  - scopes validation to one fixture
- `cargo xtask recreate-backup-db --fixture <id>`
  - recreates automated checkout-local fixtures to a deterministic root-scoped
    filename under `tests/backup_dbs/local/`
  - recreates automated shared-snapshot fixtures to a new dated filename under
    the shared DB snapshot fixture directory
  - recreates automated legacy registered-path fixtures to a new dated filename
    under `tests/backup_dbs/`
  - prints exact manual recreation steps for fixtures that are not hermetic yet
- `cargo xtask fixtures ensure --snapshots`
  - ensures current active and typed graph fixtures are available without
    sharing checkout-local DB rows
  - runs active fixture validation, then validates typed graph fixtures in the same baseline profile
  - creates or repairs checkout-local active fixtures under
    `tests/backup_dbs/local/`
  - stages committed seed artifacts from `tests/backup_dbs/` only when a
    shared-snapshot fixture is missing
  - refuses to overwrite differing shared snapshots
  - validates staged snapshots strictly, so stale schema seeds fail loudly
- `cargo xtask fixtures ensure --typed`
  - prepares typed GitHub corpus source checkouts under
    `<db_snapshot_fixtures>/_source_cache` unless `PLOKE_FIXTURE_HOME` is set
  - stages typed graph seed snapshots into the shared DB snapshot fixture
    directory
- `cargo xtask fixtures regenerate --all`
  - regenerates every automated active and typed graph fixture into its
    scope-specific output location
  - writes checkout-local fixtures under `tests/backup_dbs/local/`
  - writes shared-snapshot fixtures under the shared DB snapshot fixture
    directory using the registered filenames
  - skips manual legacy/orphaned snapshots
  - runs active fixtures, then regenerates typed graph fixtures in the same baseline profile
  - use `--active` or `--typed` instead of `--all` for narrower regeneration
- `cargo xtask repair-backup-db-schema --fixture <id>`
  - repairs a stale legacy backup in place when it is missing the current
    `workspace_metadata` relation
  - use this for the specific schema-drift failure surfaced by
    `verify-backup-dbs`, not as a substitute for full regeneration

Operator workflow details live in
[docs/how-to/recreate-backup-db-fixtures.md](../how-to/recreate-backup-db-fixtures.md).

## Shared helper API

Immutable backup consumers should load fixtures through the registry-backed
helpers in
[crates/test-utils/src/fixture_dbs.rs](../../crates/test-utils/src/fixture_dbs.rs):

- `shared_backup_fixture_db(&FIXTURE_...)`
  - loads from the shared DB snapshot fixture directory, validates, and caches
    an immutable `Arc<Database>` for reuse
- `fresh_backup_fixture_db(&FIXTURE_...)`
  - creates a fresh in-memory `Database` from a registered fixture while still
    enforcing the registry’s import mode, embedding expectations, and index
    setup
- `load_backup_fixture_db(&FIXTURE_...)`
  - returns both the checked effective path and the loaded `Database`
- `FIXTURE_....checked_path()`
  - validates the effective backup path for path-only consumers such as fixture
    staging or test registry snapshots

The registry file is the source of truth for fixture metadata and filenames.
Test code should reference fixture constants there instead of hard-coding backup
paths.

Because the default shared snapshot directory is global across local worktrees,
the registry helper compares a same-named default shared snapshot with the
committed seed in the current worktree when that seed exists. If the bytes
differ, the helper loads the worktree seed instead of importing a stale snapshot
produced by another checkout. The same protection applies when
`PLOKE_DB_SNAPSHOT_FIXTURE_DIR` is explicitly pinned to that default home cache,
which some tests do while moving `XDG_CONFIG_HOME`. A non-default
`PLOKE_DB_SNAPSHOT_FIXTURE_DIR` remains authoritative.

Registry status note:

- `Active` fixtures are expected to exist on disk and are validated by
  `cargo xtask verify-backup-dbs`.
- `Planned` fixtures record a source-pinned, reproducible fixture contract
  before the dated DB backup is committed. They may be recreated explicitly with
  `cargo xtask recreate-backup-db --fixture <id>`, but they are not part of the
  default verification set until promoted to `Active`.
- `TypedTypeGraph` fixtures are current-schema typed type graph backups. They
  are intentionally excluded from default backup verification because plain
  fixture imports intentionally exclude typed graph relations. Verify them with
  `cargo xtask verify-backup-dbs --fixture <id>`.
- `Legacy` and `Orphaned` fixtures remain outside the default active validation
  set unless explicitly selected.

One exception currently remains for `ploke-db` lib-unit tests: because
`ploke-test-utils` depends on `ploke-db`, those unit-test modules cannot consume
`shared_backup_fixture_db(...)` directly without hitting a duplicate-crate type
split for `Database`. In that case, use the shared registry constant (for
example `FIXTURE_NODES_CANONICAL.checked_path()?`) with a crate-local loader.

Test isolation note:

- Tests that mutate DB state loaded from a backup fixture must use
  `fresh_backup_fixture_db(&FIXTURE_...)` or a harness built on top of it.
  Do not use `shared_backup_fixture_db(...)` for tests that approve edits,
  refresh file hashes, write embeddings, update indexes, or otherwise change
  fixture-backed DB relations. Shared fixtures are for read-only consumers.
- Tests that mutate checked-in source fixtures associated with a backup DB
  also need a restore guard around the source file or tree. The DB and source
  isolation are separate: a fresh DB prevents cross-test database state leaks,
  while the restore guard prevents concurrent source-file edits from racing.
  A module-local pattern that has worked is:

```rust
use std::sync::{Mutex, MutexGuard, OnceLock};

struct FixtureRestoreGuard {
    _lock: MutexGuard<'static, ()>,
}

fn fixture_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

impl FixtureRestoreGuard {
    fn new() -> Self {
        let lock = fixture_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        restore_fixture();
        Self { _lock: lock }
    }
}

impl Drop for FixtureRestoreGuard {
    fn drop(&mut self) {
        restore_fixture();
    }
}
```

  Keep this guard scoped to the test duration. If multiple modules or test
  binaries mutate the same checked-in fixture tree, move the lock helper to a
  shared test utility instead of duplicating per-module locks.
- Tests that temporarily override `XDG_CONFIG_HOME` to point
  `WorkspaceRegistry::default_registry_path()` at a temp directory must
  serialize that mutation across the full workspace test run.
- This is test-only env mutation and does not change normal app behavior.
- Prefer a shared test lock/helper over per-module mutexes when multiple test
  binaries need to write the workspace registry.

## Fixture Summary

| Fixture | Parsed target(s) | Primary usage | Last update |
| --- | --- | --- | --- |
| `fixture_nodes_canonical_2026-05-17.sqlite` | `tests/fixture_crates/fixture_nodes` | canonical parsed `fixture_nodes` backup | 2026-05-17 |
| `fixture_nodes_local_embeddings_2026-05-17.sqlite` | `tests/fixture_crates/fixture_nodes` | local-embedding `fixture_nodes` backup | 2026-05-17 |
| `fixture_nodes_multi_embedding_schema_v1_bfc25988-15c1-5e58-9aa8-3d33b5e58b92` | `tests/fixture_crates/fixture_nodes` | legacy multi-embedding schema snapshot | 2026-03-20 |
| `ploke_db_primary_2026-05-06.sqlite` | `crates/ploke-db` | current-schema `ploke-db` graph backup | 2026-05-06 |
| `ws_fixture_01_canonical_2026-05-17.sqlite` | `tests/fixture_workspace/ws_fixture_01` | canonical plain backup of committed multi-member workspace fixture | 2026-05-17 |
| `ws_fixture_01_member_single_2026-05-06.sqlite` | `tests/fixture_workspace/ws_fixture_01/member_root` | single-member slice of workspace fixture | 2026-05-06 |
| `corpus_semver_type_graph_2026-05-17.sqlite` | `github:dtolnay/semver@8591f2344b52b31d85b538de58b76a676fe9ff90` | typed graphRAG type traversal corpus backup | 2026-05-17 |
| `corpus_semver_openrouter_embeddings_2026-05-17.sqlite` | `github:dtolnay/semver@8591f2344b52b31d85b538de58b76a676fe9ff90` | OpenRouter-searchable corpus backup for type-context matrix tests | 2026-05-17 |
| `corpus_memchr_type_graph_2026-05-17.sqlite` | `github:BurntSushi/memchr@24f5daa5257e00e87007c936761600e034827905` | typed graphRAG type traversal corpus backup | 2026-05-17 |
| `corpus_memchr_call_graph_2026-07-15.sqlite` | `github:BurntSushi/memchr@24f5daa5257e00e87007c936761600e034827905` | plain corpus backup for real-target call graph query contracts | 2026-07-15 |
| `corpus_memchr_openrouter_embeddings_2026-05-17.sqlite` | `github:BurntSushi/memchr@24f5daa5257e00e87007c936761600e034827905` | OpenRouter-searchable corpus backup for type-context matrix tests | 2026-05-17 |
| `corpus_generic_array_type_graph_2026-05-17.sqlite` | `github:fizyk20/generic-array@80bab87431c2e29823dc551a3311324812838a23` | typed graphRAG type traversal corpus backup | 2026-05-17 |
| `corpus_generic_array_call_graph_2026-07-15.sqlite` | `github:fizyk20/generic-array@80bab87431c2e29823dc551a3311324812838a23` | plain corpus backup for real-target call graph query contracts | 2026-07-15 |
| `corpus_generic_array_openrouter_embeddings_2026-05-17.sqlite` | `github:fizyk20/generic-array@80bab87431c2e29823dc551a3311324812838a23` | OpenRouter-searchable corpus backup for type-context matrix tests | 2026-05-17 |
| `corpus_chrono_type_graph_2026-05-17.sqlite` | `github:chronotope/chrono@120686c82c5da90377e815edb82c9a80b6b4f2be` | typed graphRAG type traversal corpus backup | 2026-05-17 |
| `corpus_chrono_call_graph_2026-07-15.sqlite` | `github:chronotope/chrono@120686c82c5da90377e815edb82c9a80b6b4f2be` | plain corpus backup for real-target call graph query contracts | 2026-07-15 |
| `corpus_chrono_openrouter_embeddings_2026-05-17.sqlite` | `github:chronotope/chrono@120686c82c5da90377e815edb82c9a80b6b4f2be` | OpenRouter-searchable corpus backup for type-context matrix tests | 2026-05-17 |
| `corpus_axum_type_graph_2026-05-17.sqlite` | `github:tokio-rs/axum@a3446d68bc03d61fb8e7513052bad2825d0c0db1` | typed graphRAG workspace-member corpus backup for type-context matrix tests | 2026-05-17 |
| `corpus_axum_call_graph_2026-07-16.sqlite` | `github:tokio-rs/axum@a3446d68bc03d61fb8e7513052bad2825d0c0db1` | plain workspace-member corpus backup for real-target call graph query contracts | 2026-07-16 |
| `corpus_axum_openrouter_embeddings_2026-05-17.sqlite` | `github:tokio-rs/axum@a3446d68bc03d61fb8e7513052bad2825d0c0db1` | OpenRouter-searchable workspace-member corpus backup for type-context matrix tests | 2026-05-17 |
| `ploke-db_af8e3a20-728d-5967-8523-da8a5ccdae45` | `crates/ploke-db` | currently orphaned snapshot | 2026-03-20 |

## 2026-07-16 Active Call-Graph Fixture Refresh

The active fixture set was regenerated with
`cargo run -p xtask --features call_graph -- fixtures regenerate --active`.
The regenerated shared call-graph corpus snapshots were copied into
`tests/backup_dbs/` as committed seed artifacts.

Post-regeneration verification:

- `cargo run -p xtask --features call_graph -- fixtures regenerate --active`
  completed for all active registered fixtures.
- `cargo run -p xtask --features call_graph -- verify-backup-dbs` passed for
  all registered active fixtures after copying the regenerated shared snapshots
  into `tests/backup_dbs/`.
- Current committed seed checksums:
  - `corpus_memchr_call_graph_2026-07-15.sqlite`:
    `4cbe8fb2b19d693e4a52c4e5aeffb5df63772e9e9f2632f2b5a1a69af6d23f0c`
  - `corpus_generic_array_call_graph_2026-07-15.sqlite`:
    `f3ba6aa6a22cdb6e783007e3f97028754c66ae3ce0a5d4c739733b5d625c676c`
  - `corpus_chrono_call_graph_2026-07-15.sqlite`:
    `d3f905af425282a7d202590882992a1d0a06cfce07a38f77bbf61ca9ccd76f63`
  - `corpus_axum_call_graph_2026-07-16.sqlite`:
    `eddbb6d7a51ca004f0ab573b119062e9b4161a86d80cf7662e388f326dc0edca`

## 2026-07-15 Active Call-Graph Fixture Refresh

The real-corpus active call-graph fixture set was refreshed with per-fixture
`cargo run -p xtask --features call_graph -- recreate-backup-db --fixture <id>`
commands after the stored forwarded returned-async-future proof slice.
The regenerated shared call-graph corpus snapshots were copied into
`tests/backup_dbs/` as committed seed artifacts.

Post-regeneration verification:

- `cargo run -p xtask --features call_graph -- recreate-backup-db --fixture <id>`
  completed for each registered real-corpus call-graph fixture:
  `corpus_memchr_call_graph`, `corpus_generic_array_call_graph`,
  `corpus_chrono_call_graph`, and `corpus_axum_call_graph`.
- `cargo run -p xtask --features call_graph -- verify-backup-dbs` passed for
  all registered active fixtures after copying the regenerated shared snapshots
  into `tests/backup_dbs/`.
- Committed seed checksums at that refresh:
  - `corpus_memchr_call_graph_2026-07-15.sqlite`:
    `491a19eb509c310774b041a72d18c382f82405a44603e67ec30cead29a1a9ccb`
  - `corpus_generic_array_call_graph_2026-07-15.sqlite`:
    `e1c51181cde66b0e6226901da5e91cb0231943c42a2bf85395f87e9019b6bafa`
  - `corpus_chrono_call_graph_2026-07-15.sqlite`:
    `f7c4ce66ac5cdd7bbb5d4505f7e7de77fb6e6fe6fbaeb7d6ec84e0f81144f23a`
  - `corpus_axum_call_graph_2026-07-15.sqlite`:
    `26a1fa515ea7686283fccbe04eccbc9fcc93a9a9e3ad198778de2b9f0ba20592`

## 2026-07-14 Active Call-Graph Fixture Refresh

The active call-graph fixture set was refreshed with
`cargo run -p xtask --features call_graph -- fixtures regenerate --active`
after adding durable parameter binding rows to the call-graph projection.
The regenerated shared call-graph corpus snapshots were copied into
`tests/backup_dbs/` as committed seed artifacts.

Post-regeneration verification:

- `cargo run -p xtask --features call_graph -- verify-backup-dbs` passed for
  all registered active fixtures.
- Current committed seed checksums:
  - `corpus_memchr_call_graph_2026-07-13.sqlite`:
    `58db4494c176048706998731d93e2bcfc7aaa24c990c8cc20c55aa4795d01ad4`
  - `corpus_generic_array_call_graph_2026-07-11.sqlite`:
    `97411284d0ca770ae87bcfd03a30648ccda0e03039654a0dd9366632868aebef`
  - `corpus_chrono_call_graph_2026-07-11.sqlite`:
    `a49f61343943a897d9ac489ed221f101f6008afb656e55e86535e009e00b2b2c`
  - `corpus_axum_call_graph_2026-07-13.sqlite`:
    `3f061e0b86df2c3e17a827dcae945f4ea2e83fbe68a6e4120401a0ed9212b01f`

## 2026-07-13 Active Call-Graph Fixture Refresh

The active call-graph fixture set was refreshed with
`cargo run -p xtask --features call_graph -- fixtures regenerate --active`.
The regenerated shared call-graph corpus snapshots were copied into
`tests/backup_dbs/` as committed seed artifacts.

Post-regeneration verification:

- `cargo run -p xtask --features call_graph -- verify-backup-dbs` passed for
  all registered active fixtures.
- Committed seed checksums at this refresh:
  - `corpus_memchr_call_graph_2026-07-13.sqlite`:
    `50be4a07ec89800a84c4ade833d2db222acac35a972708ed3a4d6deba484e020`
  - `corpus_generic_array_call_graph_2026-07-11.sqlite`:
    `ca138fa607b54155f967c0f0e1f69db8d48c3c7c34c781a69304aa95606e8162`
  - `corpus_chrono_call_graph_2026-07-11.sqlite`:
    `89902c879b878d63bf6e1098d2781173092a20139870c6204754fed1e53fa918`
  - `corpus_axum_call_graph_2026-07-13.sqlite`:
    `9f6be1735e56b27bbf595167d7f672c82e591bd0a7d5c6da985d3f9ea06b2a93`

## 2026-07-13 Axum Tuple Extractor Generated Impl Refresh

The active fixture set was regenerated with
`cargo run -p xtask --features call_graph -- fixtures regenerate --active`
after bounded, module-specific `all_the_tuples!(impl_from_request)` modeling
began projecting generated tuple extractor impl methods in
`axum-core/src/extract/tuple.rs`.

Post-regeneration verification:

- The regenerated `corpus_axum_call_graph_2026-07-13.sqlite` shared snapshot was
  copied into `tests/backup_dbs/` as the committed seed artifact.
- `cargo run -p xtask --features call_graph -- fixtures regenerate --active`
  completed with all registered active fixtures roundtripping successfully.
- Current seed checksum:
  `550b6695199b98eb8e02b9cd73de1268f287205a5f565795814ace2ec450088c`.
- `axum-core/src/extract/tuple.rs:18-77` now projects generated
  `FromRequestParts` and `FromRequest` tuple impl owners for arities 1 through
  16. Their generated extractor rows resolve through generated where-clause
  proof to the axum-core trait method bindings.
- Target-centered caller queries now include those tuple extractor rows:
  `FromRequest::from_request` has 34 callers and
  `FromRequestParts::from_request_parts` has 517 callers in the regenerated
  axum fixture.

## 2026-07-13 Axum Composite Rejection Generated Delegation Refresh

The `corpus_axum_call_graph` fixture was recreated with
`cargo run -p xtask --features call_graph -- recreate-backup-db --fixture corpus_axum_call_graph`
after bounded, module-specific `composite_rejection!` modeling began
projecting generated rejection enums and their `IntoResponse` delegation impls.
Generated zero-span method-call identity was also made occurrence-aware so
repeated generated match arms keep distinct parser-local call-site IDs without
weakening duplicate relation validation.

Post-regeneration verification:

- The recreated `corpus_axum_call_graph_2026-07-13.sqlite` shared snapshot was
  copied into `tests/backup_dbs/` as the committed seed artifact.
- Current seed checksum:
  `550b6695199b98eb8e02b9cd73de1268f287205a5f565795814ace2ec450088c`.

## 2026-07-12 Axum Middleware Service Generated Frontier Refresh

The `corpus_axum_call_graph` fixture was recreated with
`cargo run -p xtask --features call_graph -- recreate-backup-db --fixture corpus_axum_call_graph`
after bounded, module-specific `all_the_tuples!(impl_service)` modeling began
projecting generated middleware service-body frontier rows for
`axum/src/middleware/from_fn.rs` and `axum/src/middleware/map_request.rs`.

Post-regeneration verification:

- The regenerated `corpus_axum_call_graph_2026-07-12.sqlite` shared snapshot was
  copied into `tests/backup_dbs/` as the committed seed artifact.
- `cargo run -p xtask --features call_graph -- verify-backup-dbs --fixture corpus_axum_call_graph`
  passed.
- Current seed checksum:
  `e2e0d8fbd76cf4924560e80723fff6812de29309c5d796bdc2c27ea29b1f37d0`.
- The axum real-target matrix asserted thirty-four external targetless
  `std::mem::replace` rows: the existing hand-written `error_handling` and
  `response/sse` rows plus sixteen generated `from_fn` rows and sixteen
  generated `map_request` rows. The 2026-07-13 refresh below supersedes this
  count with bounded `map_response` generated rows.

## 2026-07-13 Axum MapResponse Middleware Generated Frontier Refresh

The active call-graph fixtures were regenerated with
`cargo run -p xtask --features call_graph -- fixtures regenerate --active`
after bounded, module-specific `impl_service!(...)` modeling began projecting
generated `Service::call` frontier rows for
`axum/src/middleware/map_response.rs`.

Post-regeneration verification:

- `cargo run -p xtask --features call_graph -- verify-backup-dbs` passed for
  all registered active fixtures.
- The axum real-target matrix now asserts fifty-one external targetless
  `std::mem::replace` rows: the existing hand-written `error_handling` and
  `response/sse` rows plus sixteen generated `from_fn` rows, sixteen generated
  `map_request` rows, and seventeen generated `map_response` rows.
- Current committed seed checksums are listed in the 2026-07-13 active
  call-graph fixture regeneration block above.

## 2026-07-12 Memchr Generated IFunc Call Refresh

The `corpus_memchr_call_graph` fixture was recreated with
`cargo run -p xtask --features call_graph -- recreate-backup-db --fixture corpus_memchr_call_graph`
after the parser began projecting the bounded `unsafe_ifunc!` generated
`core::mem::transmute::<Fn, RealFn>(fun)(...)` source oracle rows. It was
recreated again after shorthand self-field function-pointer initializer proof
began preserving cfg-visible candidate sets for memchr `Searcher.call` and
`Prefilter.call`; cfg-gated architecture helpers that are not visible in the
fixture remain omitted from the candidate set.

Post-regeneration verification:

- The recreated `corpus_memchr_call_graph_2026-07-12.sqlite` shared snapshot was
  copied into `tests/backup_dbs/` as the committed seed artifact.
- Current seed checksum:
  `ad8127dd511d5ce6698d9fa2eb0f743c637319a9289cff9c359ce5a58599b93e`.
- The memchr real-target matrix can now assert the generated inner transmute path
  row and outer returned-path dynamic row as external targetless frontiers rather
  than treating the source oracle as fully absent.
- The memchr real-target matrix can also assert finite ambiguous
  `DynamicFunction` candidate sets for `Searcher.call` and `Prefilter.call`
  instead of preserving those function-pointer field rows as targetless.

## 2026-07-13 Memchr Associated Constructor Receiver Refresh

The `corpus_memchr_call_graph` fixture was recreated with
`cargo run -p xtask --features call_graph -- recreate-backup-db --fixture corpus_memchr_call_graph`
after method receiver resolution began handling associated-function path-call
results such as `crate::tests::substring::Runner::new().fwd(...)`.

Post-regeneration verification:

- The recreated `corpus_memchr_call_graph_2026-07-13.sqlite` shared snapshot was
  copied into `tests/backup_dbs/` as the committed seed artifact.
- Current seed checksum:
  `75bd8608c4d9f98ebab74d630a3c5262100fab5a07d8437a6e3eef4c420dd431`.
- The memchr `Runner::new().fwd(...)` and `Runner::new().rev(...)` setter
  method-call rows now resolve to local methods when the receiver path is
  crate-qualified or module-qualified from the caller scope.
- The `Runner::run` `fwd(...)` and `rev(...)` callable trait-object field rows
  remain targetless. The field proof is still conservative when not every
  setter caller argument can be resolved into a finite callable candidate set.

## 2026-07-13 Axum Cross-Crate Receiver Guard Refresh

The `corpus_axum_call_graph` fixture was recreated with
`cargo run -p xtask --features call_graph -- recreate-backup-db --fixture corpus_axum_call_graph`
after associated-constructor receiver chaining was limited to methods present in
the current parsed graph. Cross-crate associated constructor receiver chains now
remain unsupported until call resolution carries the target method's graph
context instead of attempting to read the foreign method's return type from the
current graph.

Post-regeneration verification:

- The recreated `corpus_axum_call_graph_2026-07-13.sqlite` shared snapshot was
  copied into `tests/backup_dbs/` as the committed seed artifact.
- Current seed checksum:
  `550b6695199b98eb8e02b9cd73de1268f287205a5f565795814ace2ec450088c`.

## 2026-07-12 Axum Opaque Future Generated Constructor Refresh

The `corpus_axum_call_graph` fixture was recreated with
`cargo run -p xtask --features call_graph -- recreate-backup-db --fixture corpus_axum_call_graph`
after bounded item-position `opaque_future!` modeling began projecting the
generated `IntoServiceFuture` inherent constructor item.

Post-regeneration verification:

- The recreated `corpus_axum_call_graph_2026-07-12.sqlite` shared snapshot was
  copied into `tests/backup_dbs/` as the committed seed artifact.
- `axum/src/handler/service.rs:174`
  `super::future::IntoServiceFuture::new(future)` now resolves to the generated
  inherent `new` method projected from `axum/src/handler/future.rs:11-18` and
  `axum/src/macros.rs:19-20`.
- The admitted `opaque_future!` expansion-boundary summary remains linked to
  the callsite as proof metadata without keeping a callsite-level
  `type_resolution_missing` blocker.

## 2026-07-12 Axum Top-Level Handler Generated Function Refresh

The `corpus_axum_call_graph` fixture was recreated with
`cargo run -p xtask --features call_graph -- recreate-backup-db --fixture corpus_axum_call_graph`
after bounded item-position `top_level_handler_fn!` modeling began projecting
the generated axum routing handler function item for `post`.

Post-regeneration verification:

- The recreated `corpus_axum_call_graph_2026-07-12.sqlite` shared snapshot was
  copied into `tests/backup_dbs/` as the committed seed artifact.
- `axum/src/routing/method_routing.rs:445`
  `top_level_handler_fn!(post, POST)` now projects a generated
  `routing::method_routing::post` function whose generated body calls the local
  `on(...)` helper.
- The 23 inspected real-corpus `post(...)` callsites in
  `axum/src/json.rs`, `axum/src/routing/method_routing.rs`, and
  `axum/src/routing/tests/mod.rs` now resolve to that generated function. The
  multipart source-oracle rows remain absent in the current fixture.
- The admitted `routing::post` expansion-boundary summary remains linked to
  the callsite as proof metadata without keeping a callsite-level
  `type_resolution_missing` blocker.

## 2026-07-12 Axum Top-Level Service Generated Function Refresh

The `corpus_axum_call_graph` fixture was recreated with
`cargo run -p xtask --features call_graph -- recreate-backup-db --fixture corpus_axum_call_graph`
after bounded item-position `top_level_service_fn!` modeling began projecting
the generated axum routing service function items.

Post-regeneration verification:

- The recreated `corpus_axum_call_graph_2026-07-12.sqlite` shared snapshot was
  copied into `tests/backup_dbs/` as the committed seed artifact.
- `axum/src/routing/method_routing.rs:335-343` now projects generated
  `*_service` functions whose generated bodies call the local `on_service(...)`
  helper.
- The inspected real-corpus `get_service(...)`, `delete_service(...)`,
  `patch_service(...)`, and `post_service(...)` path callsites now resolve to
  those generated functions. Chained `.post_service(...)` method calls remain
  outside this top-level free-function slice.

## 2026-07-12 Axum Function-Pointer Self-Field Refresh

The `corpus_axum_call_graph` fixture was recreated with
`cargo run -p xtask --features call_graph -- recreate-backup-db --fixture corpus_axum_call_graph`
after dynamic self-field resolution began admitting bare function-pointer
fields initialized by a unique local struct-field closure.

Post-regeneration verification:

- The recreated `corpus_axum_call_graph_2026-07-12.sqlite` shared snapshot was
  copied into `tests/backup_dbs/` as the committed seed artifact.
- `axum/src/boxed.rs:85` now resolves `(self.into_route)(self.handler, state)`
  to the unique closure initializer recorded from
  `BoxedIntoRoute::from_handler`.
- The remaining real-corpus dynamic field blockers at
  `axum/src/boxed.rs:120` and `axum/src/serve/listener.rs:236` remain
  unsupported and targetless.
- `axum/src/boxed.rs:159` and `:163` now preserve finite ambiguous
  `DynamicClosure` candidates for `(self.layer)(...)` from the recorded
  `MethodRouter::{layer,route_layer}` closure bindings, without admitting a
  local traversal edge.

## 2026-07-12 Axum Handler Tuple Generated Extraction Refresh

The `corpus_axum_call_graph` fixture was recreated with
`cargo run -p xtask --features call_graph -- recreate-backup-db --fixture corpus_axum_call_graph`
after bounded item-position `all_the_tuples!(impl_handler)` modeling began
projecting generated `Handler::call` impl bodies for extractor tuple arities 1
through 16, and workspace trait import/re-export traversal began resolving the
generated impl where-clause traits through the same local re-export chains used
by workspace type import proof.

Post-regeneration verification:

- The recreated `corpus_axum_call_graph_2026-07-12.sqlite` shared snapshot was
  copied into `tests/backup_dbs/` as the committed seed artifact.
- `axum/src/handler/mod.rs:262` now projects generated `Handler::call`
  async-block owners whose extraction rows use stable generic names such as
  `T1::from_request_parts` and `T2::from_request` rather than macro
  metavariable names.
- The generated `Handler::call` method owners are recorded under generated
  `impl Handler<...> for F` trait impl rows instead of being modeled as
  inherent impl methods.
- Those generated associated-path rows now resolve through generated impl
  where-clause proof and the local `crate::extract::{FromRequest,
  FromRequestParts}` re-export chain to the axum-core trait method bindings.
- At this refresh, target-centered caller queries included the generated
  handler extractor rows: `FromRequest::from_request` had 18 callers and
  `FromRequestParts::from_request_parts` had 125 callers. See the
  2026-07-13 tuple extractor refresh above for the current active counts.

## 2026-07-12 Axum HandleError Impl Service Refresh

The active fixture set was regenerated with
`cargo run -p xtask --features call_graph -- fixtures regenerate --active`
after bounded, module-specific item-position `impl_service!` modeling began
projecting generated `HandleError<S, F, T>` service impl methods in
`axum/src/error_handling/mod.rs`.

Post-regeneration verification:

- The regenerated `corpus_axum_call_graph_2026-07-12.sqlite` shared snapshot was
  copied into `tests/backup_dbs/` as the committed seed artifact.
- `cargo run -p xtask --features call_graph -- verify-backup-dbs --fixture corpus_axum_call_graph`
  passed.
- `axum/src/error_handling/mod.rs:207-222` now projects sixteen generated
  `Service::call` method owners for `HandleError<S, F, T>`.
- The generated owners preserve the extractor path rows needed for proof:
  `Tn::from_request_parts(&mut parts, &()).await` resolves through generated
  `FromRequestParts<()>` where-clause proof to the axum-core
  `FromRequestParts::from_request_parts` trait method binding.
- At this refresh, target-centered caller queries included both generated
  handler and generated HandleError service extractor rows:
  `FromRequestParts::from_request_parts` had 261 callers. See the 2026-07-13
  tuple extractor refresh above for the current active count.

## 2026-07-12 Axum Body From Impl Generated Conversion Refresh

The active fixture set was regenerated with
`cargo run -p xtask --features call_graph -- fixtures regenerate --active`
after bounded item-position `body_from_impl!` modeling began projecting the
generated `impl From<T> for Body` conversion methods in
`axum-core/src/body.rs`.

Post-regeneration verification:

- The regenerated `corpus_axum_call_graph_2026-07-12.sqlite` shared snapshot was
  copied into `tests/backup_dbs/` as the committed seed artifact.
- `cargo run -p xtask --features call_graph -- verify-backup-dbs --fixture corpus_axum_call_graph`
  passed.
- `axum-core/src/body.rs:120-138` now projects seven generated
  `From<T> for Body::from` method owners whose
  `Self::new(http_body_util::Full::from(buf))` rows resolve to
  `Body::new` through one associated-function edge.

## 2026-07-12 Axum Tap Inner Transparent Source Block Refresh

The `corpus_axum_call_graph` fixture was recreated with
`cargo run -p xtask --features call_graph -- recreate-backup-db --fixture corpus_axum_call_graph`
after bounded transparent-source extraction for the reviewed `tap_inner!`
macro began projecting callsites from the invocation block.

Post-regeneration verification:

- The regenerated `corpus_axum_call_graph_2026-07-12.sqlite` shared snapshot was
  copied into `tests/backup_dbs/` as the committed seed artifact.
- The committed seed and regenerated shared snapshot both have SHA-256
  `ef91b92ad0411c9b63fc19d53756239dff64df3caabdfb2a12fbf7c41223f5a4`.
- `axum/src/routing/mod.rs:304-308` invokes `map_inner!` with a reviewed
  source-visible expression whose `catch_all_fallback` field contains
  `this.catch_all_fallback.map(|route| route.layer(layer))`. The regenerated
  fixture now preserves both the targetless `map(...)` row and the nested
  closure-owned targetless `route.layer(layer)` row while admitting no traversal
  edge for either unsupported receiver frontier.
- `axum/src/routing/mod.rs:398` invokes `tap_inner!` with a source-visible block
  whose nested closure owners call `take_route_or_internal_error` at
  `routing/mod.rs:410,430`; both rows now resolve to the same-module helper at
  `routing/mod.rs:63`.
- The debug-only `routing/tests/mod.rs:56,59`
  `super::take_route_or_internal_error` rows remain absent from the normal-build
  fixture profile.

## 2026-07-11 Axum Module-Qualified Constructor Refresh

The active fixture set was regenerated with
`cargo run -p xtask --features call_graph -- fixtures regenerate --active`
after local constructor resolution started using module-qualified type paths
such as `private::ServeFuture(...)`.

Post-regeneration verification:

- `corpus_axum_call_graph_2026-07-11.sqlite` was copied from the regenerated
  shared snapshot into `tests/backup_dbs/` as the committed seed artifact.
- `cargo run -p xtask --features call_graph -- verify-backup-dbs --fixture corpus_axum_call_graph`
  passed.
- Axum `axum/src/serve/mod.rs:389` and `:541`
  `private::ServeFuture(...)` rows now resolve to the tuple struct
  `ServeFuture` at `axum/src/serve/mod.rs:672`.

## 2026-07-11 Generated Local Item Macro Refresh

The active fixture set was regenerated with
`cargo run -p xtask --features call_graph -- fixtures regenerate --active`
after adding the fixture-backed bounded generated local-item macro case.

Post-regeneration verification:

- The regenerated shared call-graph corpus snapshots for memchr,
  generic-array, chrono, and axum were copied into `tests/backup_dbs/` as the
  committed seed artifacts.
- `cargo run -p xtask --features call_graph -- verify-backup-dbs` passed for all
  registered active fixtures.
- The local `fixture_call_graph` in-memory DB test
  `fixture_context_projects_macro_generated_local_fn_call_to_local_item_target`
  proves the generated local `fn` owner is traversable while the macro
  invocation remains targetless and unsupported.

## 2026-07-11 Generic Array Method-Result Receiver Refresh

The `corpus_generic_array_call_graph` fixture was recreated with
`cargo run -p xtask --features call_graph -- recreate-backup-db --fixture corpus_generic_array_call_graph`
after the method-result local-binding receiver proof for
`let iter = iter.into_iter(); iter.size_hint()` was promoted in the shared
real-corpus matrix.

Post-regeneration verification:

- The recreated `corpus_generic_array_call_graph_2026-07-11.sqlite` shared
  snapshot was copied into `tests/backup_dbs/` as the committed seed artifact.
- The generic-array guarded match rows at `src/lib.rs:1239` and `src/lib.rs:1276`
  now classify as external targetless method-result local receiver frontiers
  instead of unsupported receiver-shape gaps.

## 2026-07-11 Chrono and Memchr Call-Site Schema Refresh

The `corpus_chrono_call_graph` and `corpus_memchr_call_graph` fixtures were
recreated with the current call-graph projection schema so shared real-target
matrix tests can query `call_site.generic_arg_count` through the same owner
context API as the newer axum and generic-array fixtures.

Post-regeneration verification:

- The recreated `corpus_chrono_call_graph_2026-07-11.sqlite` and
  `corpus_memchr_call_graph_2026-07-11.sqlite` shared snapshots were copied into
  `tests/backup_dbs/` as committed seed artifacts.
- No resolver expectation changed for these two fixtures in this refresh; the
  update keeps their persisted relation shape aligned with the current DB query
  helpers.

## 2026-07-11 Axum Initialized Receiver Refresh

The `corpus_axum_call_graph` fixture was recreated with
`cargo run -p xtask --features call_graph -- recreate-backup-db --fixture corpus_axum_call_graph`
after initialized local receiver resolution began using associated constructor
return types as receiver evidence.

Post-regeneration verification:

- The recreated `corpus_axum_call_graph_2026-07-11.sqlite` shared snapshot was
  copied into `tests/backup_dbs/` as the committed seed artifact.
- `CountingCloneableState::new()` resolves as a local exact associated
  function call in the axum routing state-clone tests.
- Six initialized local receiver call rows now resolve locally:
  `state.clone()` and `state.setup_done()` in
  `state_isnt_cloned_too_much`, `state_isnt_cloned_too_much_in_layer`, and
  `state_isnt_cloned_too_much_with_fallback`.

## 2026-07-10 Axum Dynamic Self-Field Callee Evidence Refresh

The `corpus_axum_call_graph` fixture was recreated with
`cargo run -p xtask --features call_graph -- recreate-backup-db --fixture corpus_axum_call_graph`
after dynamic call extraction began preserving `self`-field callable paths for
unsupported targetless rows.

Post-regeneration verification:

- The recreated `corpus_axum_call_graph_2026-07-10.sqlite` shared snapshot was
  copied into `tests/backup_dbs/` as the committed seed artifact.
- `axum/src/boxed.rs:{85,120,159}` and
  `axum/src/serve/listener.rs:236` dynamic callable field rows preserve
  `call_site.path` while remaining unsupported and targetless.

## 2026-07-10 Axum Awaited Method-Call Receiver Refresh

The `corpus_axum_call_graph` fixture was recreated with
`cargo run -p xtask --features call_graph -- recreate-backup-db --fixture corpus_axum_call_graph`
after awaited method-call receivers began preserving the awaited inner method
name as `AwaitMethodCallResult`.

Post-regeneration verification:

- The recreated `corpus_axum_call_graph_2026-07-10.sqlite` shared snapshot was
  copied into `tests/backup_dbs/` as the committed seed artifact.
- `cargo run -p xtask --features call_graph -- verify-backup-dbs --fixture corpus_axum_call_graph`
  passed with `roundtrip=ok`.
- `axum/src/serve/listener.rs:143`
  `self.sem.clone().acquire_owned().await.unwrap()` now persists the outer
  `unwrap` receiver as `AwaitMethodCallResult(["acquire_owned"])` while
  remaining unsupported and targetless.

## 2026-07-10 Call Callee Evidence Regeneration

`cargo xtask fixtures regenerate --active` was rerun after adding the
`call_callee_evidence` call-graph relation. The regenerated shared snapshots
were copied back to the committed seed files for:

- `corpus_memchr_call_graph_2026-07-07.sqlite`
- `corpus_generic_array_call_graph_2026-07-07.sqlite`
- `corpus_chrono_call_graph_2026-07-07.sqlite`
- `corpus_axum_call_graph_2026-07-09.sqlite`

`cargo xtask verify-backup-dbs` passed after the seed refresh. Direct relation
inspection confirmed `call_callee_evidence` is present in the refreshed axum
seed.

## 2026-07-09 Active Corpus Seed Promotion

The active fixture set was regenerated with
`cargo run -p xtask --features call_graph -- fixtures regenerate --active`
after the axum impl Trait parameter external-frontier refresh.

Post-regeneration verification:

- The regeneration command roundtripped all active checkout-local fixtures and
  shared call-graph corpus snapshots successfully.
- Registry-backed verification passed with
  `cargo run -p xtask --features call_graph -- verify-backup-dbs`.
- The regenerated shared call-graph corpus snapshots for memchr,
  generic-array, chrono, and axum were copied into `tests/backup_dbs/` as the
  committed seed artifacts.

## 2026-07-09 Axum Tuple-Return Receiver Refresh

The `corpus_axum_call_graph` fixture was recreated with
`cargo run -p xtask --features call_graph -- recreate-backup-db --fixture corpus_axum_call_graph`
after call graph resolution began resolving
`axum-core/src/ext_traits/request_parts.rs:164`
`parts.extract_with_state::<State<String>, String>(&state)` through the exact
external tuple-return summary for `http::Request::into_parts`.

Post-regeneration verification:

- The recreated `corpus_axum_call_graph_2026-07-09.sqlite` shared snapshot was
  copied into `tests/backup_dbs/` as the committed seed artifact.
- Registry metadata now points at the 2026-07-09 axum call-graph snapshot so
  DB, RAG, and TUI real-corpus rows see the resolved
  `RequestPartsExt for Parts::extract_with_state` edge.

## 2026-07-10 Active Corpus Seed Refresh

The active fixture set was regenerated with
`cargo run -p xtask --features call_graph -- fixtures regenerate --active`
after the latest call-graph real-corpus refresh.

Post-regeneration verification:

- The regeneration command roundtripped all active checkout-local fixtures and
  shared call-graph corpus snapshots successfully.
- The regenerated shared call-graph corpus snapshots for memchr,
  generic-array, chrono, and axum were copied into `tests/backup_dbs/` as the
  committed seed artifacts.
- `cargo run -p xtask --features call_graph -- verify-backup-dbs` passed for
  all registered backup DB fixtures.

## 2026-06-27 Active Fixture Review

The active checkout-local fixtures were regenerated with
`cargo xtask fixtures regenerate --active`, then regenerated again with
`cargo run -p xtask --features call_graph -- fixtures regenerate --active`
after feature-gated TUI checks showed that the non-feature regeneration lacked
the gated call-graph relations.

Post-regeneration verification:

- `cargo xtask verify-backup-dbs` passed for all active registered fixtures in
  the default fixture profile.
- `cargo run -p xtask --features call_graph -- verify-backup-dbs` passed for
  all active registered fixtures with call graph projection relations present
  (`relations=64`, or `relations=65` for the local-embedding fixture).
- `cargo test -p ploke-rag test_search -- --nocapture` passed after previously
  failing to materialize `use_all_const_static` from stale
  `fixture_nodes/src/const_static.rs` rows.
- `cargo test --workspace --exclude ploke-eval --no-fail-fast` passed.
- `cargo test -p ploke-tui --features call_graph request_code_context -- --nocapture`
  passed after previously failing on missing `call_relation`.

Checkout-local outputs remain under `tests/backup_dbs/local/` and are ignored
local artifacts; the committed registry paths were not changed for this
worktree-only refresh.

## 2026-07-03 Active Fixture Review

The active fixture set was regenerated with
`cargo xtask fixtures regenerate --active` after adding the `call_body_owner`
projection relation for executable-local call owners.

Post-regeneration verification:

- `cargo xtask verify-backup-dbs` passed for all active registered fixtures.
- Call-graph corpus fixtures use backup-aware import relation selection so
  empty call graph schema relations are not requested from backups that do not
  serialize them.
- `cargo test -p ploke-db call_graph_fixture_queries -- --nocapture` passed
  after updating the generic-array guarded match oracle to expect two
  targetless unsupported `size_hint` rows.

Checkout-local outputs remain under `tests/backup_dbs/local/` and are ignored
local artifacts. Shared corpus snapshots were refreshed under the configured DB
snapshot fixture directory.

## 2026-07-05 Active Fixture Review

The active fixture set was regenerated with
`cargo run -p xtask --features call_graph -- fixtures regenerate --active`
after local closure-binding `as fn` cast calls began resolving to closure
executable owners.

Post-regeneration verification:

- The regeneration command roundtripped all active checkout-local fixtures and
  shared call-graph corpus snapshots successfully.
- Checkout-local outputs remain under `tests/backup_dbs/local/`.
- Shared call-graph corpus snapshots were refreshed under the configured DB
  snapshot fixture directory.
- The regenerated `corpus_axum_call_graph_2026-07-05.sqlite` shared snapshot
  was copied into `tests/backup_dbs/` as the committed seed artifact.

## 2026-07-06 Axum Call Graph Fixture Refresh

The `corpus_axum_call_graph` fixture was recreated with
`cargo run -p xtask --features call_graph -- recreate-backup-db --fixture corpus_axum_call_graph`
after call graph resolution began classifying direct external parameter
receiver method calls such as `req.extensions_mut()` when owner parameter type
proof identifies `http::Request`.

Post-regeneration verification:

- `cargo run -p xtask --features call_graph -- fixtures regenerate --active`
  roundtripped all active checkout-local fixtures and shared call-graph corpus
  snapshots successfully.
- The regenerated `corpus_axum_call_graph_2026-07-06.sqlite` shared snapshot
  was copied into `tests/backup_dbs/` as the committed seed artifact.
- Real-target query tests now assert the split between external
  `mut req: Request<_>` parameter receivers and still-unresolved borrowed or
  generic receiver rows.

## 2026-07-06 Active Call Graph Baseline Refresh

The active fixture set was regenerated again with
`cargo run -p xtask --features call_graph -- fixtures regenerate --active`
after the call graph rollout gate had already been removed. The `call_graph`
feature is now a compatibility alias; current default-profile regeneration also
includes baseline call graph relations.

Post-regeneration verification:

- The regeneration command roundtripped all active checkout-local fixtures and
  shared call-graph corpus snapshots successfully.
- No tracked registry, documentation, or committed seed fixture changed during
  this refresh.
- Checkout-local outputs remain under `tests/backup_dbs/local/`.
- Shared call-graph corpus snapshots were refreshed under the configured DB
  snapshot fixture directory.

2026-07-07 follow-up verification:

- `cargo run -p xtask --features call_graph -- fixtures regenerate --active`
  roundtripped all active checkout-local fixtures and shared call-graph corpus
  snapshots again, with no tracked fixture drift.
- `cargo test -p ploke-db real_target_matrix -- --nocapture` passed in the
  default profile with `75 passed`.
- `cargo test -p ploke-rag real_corpus -- --nocapture` was inconclusive under a
  15 minute verification limit: no failing assertions were emitted before the
  run was stopped, but several real-corpus tests were still running.
- `cargo test -p ploke-tui code_item -- --nocapture` was inconclusive under the
  bounded verification run: no failing assertions were emitted before the run
  was stopped, and the visible long-running `Json::from_bytes` lookup check had
  completed successfully.

2026-07-08 active refresh:

- `cargo xtask fixtures regenerate --active` roundtripped the nine active
  fixture registrations with no tracked fixture drift.
- `cargo xtask verify-backup-dbs` passed for all registered backup DBs.
- Focused real-corpus effect propagation checks passed through DB, RAG, and
  exact TUI tool surfaces:
  `cargo test -p ploke-db axum_usage_questions_report_reachable_effect_seed_for_task_spawn -- --nocapture`,
  `cargo test -p ploke-rag call_effects_exact_reads_axum_task_spawn_seed -- --nocapture`,
  `cargo test -p ploke-tui --test integration code_item_lookup_returns_real_corpus_reachable_effects -- --nocapture`,
  and
  `cargo test -p ploke-tui --test integration code_item_edges_returns_real_corpus_reachable_effects -- --nocapture`.

## 2026-07-09 Axum Impl Trait Parameter Frontier Refresh

The active fixture set was regenerated with
`cargo run -p xtask --features call_graph -- fixtures regenerate --active`
after call graph resolution began classifying
`axum-core/src/error.rs:14` `error.into()` as an external targetless frontier
through the source-visible `error: impl Into<BoxError>` parameter bound.

Post-regeneration verification:

- The regeneration command roundtripped all active checkout-local fixtures and
  shared call-graph corpus snapshots successfully.
- `cargo run -p xtask --features call_graph -- verify-backup-dbs` passed for
  all registered backup DB fixtures.
- The regenerated `corpus_axum_call_graph_2026-07-07.sqlite` shared snapshot
  was copied into `tests/backup_dbs/` as the committed seed artifact.
- The isolated regenerated snapshot passed
  `cargo test -p ploke-db axum_real_target_impl_trait_into_parameter_is_external_frontier -- --nocapture`
  before seed promotion.
- Follow-up broad verification passed with
  `cargo test --workspace --exclude ploke-eval --no-fail-fast`, including DB,
  RAG, TUI tool, transform, parser, and doctest surfaces.

## 2026-07-06 Axum Route Oneshot Frontier Refresh

The `corpus_axum_call_graph` fixture was recreated with
`cargo run -p xtask --features call_graph -- recreate-backup-db --fixture corpus_axum_call_graph`
after call graph resolution began classifying the two Route `oneshot` receiver
rows in `axum/src/routing/route.rs` as external targetless frontiers when
`tower::ServiceExt` import evidence is visible.

Post-regeneration verification:

- `cargo run -p xtask --features call_graph -- verify-backup-dbs --fixture corpus_axum_call_graph`
  passed with `roundtrip=ok`.
- The regenerated `corpus_axum_call_graph_2026-07-06.sqlite` shared snapshot
  was copied into `tests/backup_dbs/` as the committed seed artifact.

## 2026-07-07 Call Graph Unsafe Block Schema Refresh

The Axum, Chrono, Generic Array, and Memchr call-graph corpus fixtures were
recreated with
`cargo run -p xtask --features call_graph -- recreate-backup-db --fixture <id>`
after `call_site.unsafe_block` became part of the projected call-site schema.
Earlier `fixtures regenerate --active` runs only roundtripped the existing
shared snapshots and left some seeds with the previous 12-column `call_site`
shape.

Post-regeneration verification:

- The recreated `corpus_axum_call_graph_2026-07-07.sqlite` shared snapshot was
  copied into `tests/backup_dbs/` as the committed seed artifact.
- The recreated `corpus_chrono_call_graph_2026-07-07.sqlite` shared snapshot
  replaced the existing seed artifact.
- The recreated `corpus_generic_array_call_graph_2026-07-07.sqlite` shared
  snapshot was copied into `tests/backup_dbs/` as the committed seed artifact.
- The recreated `corpus_memchr_call_graph_2026-07-07.sqlite` shared snapshot
  was copied into `tests/backup_dbs/` as the committed seed artifact.
- Registry metadata now points at 2026-07-07 call-graph snapshots so
  owner-centered call graph queries decode `unsafe_block` strictly.

## 2026-07-06 Axum cfg-unix Body Visibility Refresh

The active fixture set was regenerated with
`cargo run -p xtask --features call_graph -- fixtures regenerate --active`
after cfg parsing began treating bare `#[cfg(unix)]` / `#[cfg(windows)]` as
target-family atoms and accepting `target_family = ...` plus
`target_arch = ...` name-value atoms.

Post-regeneration verification:

- The regeneration command roundtripped all active checkout-local fixtures and
  shared call-graph corpus snapshots successfully.
- The regenerated `corpus_axum_call_graph_2026-07-06.sqlite` shared snapshot
  was copied into `tests/backup_dbs/` as the committed seed artifact.
- The axum real-corpus query matrix now asserts both
  `axum/src/serve/listener.rs:41` and `:61` `Self::accept(self).await` rows as
  visible, external, targetless path calls rather than treating the
  UnixListener impl as a body-owner gap.

## 2026-07-06 Active Corpus Seed Promotion

The active fixture set was regenerated with
`cargo run -p xtask --features call_graph -- fixtures regenerate --active`
before continuing the call-graph matrix work.

Post-regeneration verification:

- The regeneration command roundtripped all active checkout-local fixtures and
  shared call-graph corpus snapshots successfully.
- Registry-backed verification passed for each shared call-graph corpus
  fixture:
  - `corpus_memchr_call_graph`
  - `corpus_generic_array_call_graph`
  - `corpus_chrono_call_graph`
  - `corpus_axum_call_graph`
- The regenerated `corpus_chrono_call_graph_2026-07-06.sqlite` and
  `corpus_axum_call_graph_2026-07-06.sqlite` shared snapshots were copied into
  `tests/backup_dbs/` as committed seed artifacts. The memchr and
  generic-array regenerated snapshots matched their existing committed seeds.

## `fixture_nodes_canonical_2026-05-17.sqlite`

- File: `tests/backup_dbs/fixture_nodes_canonical_2026-05-17.sqlite`
- Parsed target(s): `tests/fixture_crates/fixture_nodes`
- Expected DB config:
  - plain backup import
  - normal type-resolution profile fixture; under current typed graph baseline workspace
    builds, active fixture loaders import the same code graph while leaving
    typed graph relations empty rather than treating this as typed graph corpus
    coverage
  - primary HNSW index must be created after import by the caller
  - no embedding model contract is assumed by default
  - used as the canonical parsed graph fixture for `fixture_nodes`
- Tests using this fixture:
  - `ploke-db`
    - [crates/ploke-db/src/utils/test_utils.rs](../../crates/ploke-db/src/utils/test_utils.rs): shared mutable `Arc<Mutex<Database>>`
    - [crates/ploke-db/src/bm25_index/mod.rs](../../crates/ploke-db/src/bm25_index/mod.rs): shared immutable `Arc<Database>` via crate-local loader keyed by `FIXTURE_NODES_CANONICAL`
    - [crates/ploke-db/src/index/hnsw.rs](../../crates/ploke-db/src/index/hnsw.rs): fresh mutable DB per test
    - [crates/ploke-db/src/multi_embedding/hnsw_ext.rs](../../crates/ploke-db/src/multi_embedding/hnsw_ext.rs): fresh mutable DB per test
    - [crates/ploke-db/benches/resolver_bench.rs](../../crates/ploke-db/benches/resolver_bench.rs): immutable benchmark input
  - `ploke-rag`
    - legacy direct-path use removed; immutable consumers should use the local-embedding fixture helper instead
  - `ploke-tui`
    - [crates/ploke-tui/src/test_harness.rs](../../crates/ploke-tui/src/test_harness.rs): shared mutable app harness DB
    - [crates/ploke-tui/src/app/commands/exec_real_tools_live_tests.rs](../../crates/ploke-tui/src/app/commands/exec_real_tools_live_tests.rs): shared mutable DB for live-tool scaffolding
    - [crates/ploke-tui/src/rag/tests/apply_code_edit_tests.rs](../../crates/ploke-tui/src/rag/tests/apply_code_edit_tests.rs): fixture dependency note, immutable expectations
- Notes:
  - older backups remain on disk (for example `fixture_nodes_canonical_2026-05-06.sqlite`) but the active registry entry points at the 2026-05-17 snapshot

## `fixture_nodes_local_embeddings_2026-05-17.sqlite`

- File: `tests/backup_dbs/fixture_nodes_local_embeddings_2026-05-17.sqlite`
- Parsed target(s): `tests/fixture_crates/fixture_nodes`
- Expected DB config:
  - import with `Database::import_backup_with_embeddings`
  - default local embedding set expected:
    - provider: `local`
    - model: `sentence-transformers/all-MiniLM-L6-v2`
    - dims: `384`
    - dtype: `f32`
  - vectors must be present for the default local set
  - callers generally rebuild the primary index after import
  - recreation currently forces CPU device selection but still uses the default
    local model revision
- Tests using this fixture:
  - `ploke-rag`
    - [crates/ploke-rag/src/core/unit_tests.rs](../../crates/ploke-rag/src/core/unit_tests.rs): shared immutable DB plus fresh immutable imports via `fresh_backup_fixture_db`
    - [crates/ploke-rag/tests/integration_tests.rs](../../crates/ploke-rag/tests/integration_tests.rs): shared immutable DB via `shared_backup_fixture_db`
  - `ploke-tui`
    - [crates/ploke-tui/src/test_utils/new_test_harness.rs](../../crates/ploke-tui/src/test_utils/new_test_harness.rs): shared immutable headless harness DB via `shared_backup_fixture_db`; mutating edit tests use a fresh harness DB via `fresh_backup_fixture_db`
    - [crates/ploke-tui/tests/get_code_edges_regression.rs](../../crates/ploke-tui/tests/get_code_edges_regression.rs): shared immutable DB via harness
    - [crates/ploke-tui/tests/tool_ui_payload_fixture.rs](../../crates/ploke-tui/tests/tool_ui_payload_fixture.rs): shared immutable DB via harness
- Notes:
  - older backups remain on disk (for example `fixture_nodes_local_embeddings_2026-05-06.sqlite`) but the active registry entry points at the 2026-05-17 snapshot
  - 2026-06-12 review: regenerated the checkout-local effective backup after
    validation reported a stale schema missing `type_contains`; strict
    validation passed with `relations=61` and `roundtrip=ok`

## `fixture_nodes_multi_embedding_schema_v1_bfc25988-15c1-5e58-9aa8-3d33b5e58b92`

- File: `tests/backup_dbs/fixture_nodes_multi_embedding_schema_v1_bfc25988-15c1-5e58-9aa8-3d33b5e58b92`
- Parsed target(s): `tests/fixture_crates/fixture_nodes`
- Expected DB config:
  - legacy schema snapshot
  - no active consumers found during the 2026-03-20 review
- Tests using this fixture:
  - none
- Notes:
  - only a commented-out reference remains in [crates/ploke-rag/src/core/unit_tests.rs](../../crates/ploke-rag/src/core/unit_tests.rs)
  - keep under review until explicitly removed or reintroduced

## `ploke_db_primary_2026-05-06.sqlite`

- File: `tests/backup_dbs/ploke_db_primary_2026-05-06.sqlite`
- Parsed target(s): `crates/ploke-db`
- Expected DB config:
  - plain backup import
  - primary index created after import
  - recreated from the real `crates/ploke-db` source graph via `setup_db_full_crate("ploke-db")`
- Important tradeoff:
  - this is now a current-schema source-backed fixture, not a frozen user-repro snapshot
  - its contents will move with `crates/ploke-db` as that crate changes over time
- Tests using this fixture:
  - `ploke-tui`
    - [crates/ploke-tui/tests/get_code_edges_regression.rs](../../crates/ploke-tui/tests/get_code_edges_regression.rs): shared immutable DB via `shared_backup_fixture_db`

## `ws_fixture_01_canonical_2026-05-17.sqlite`

- File: `tests/backup_dbs/ws_fixture_01_canonical_2026-05-17.sqlite`
- Parsed target(s): `tests/fixture_workspace/ws_fixture_01`
- Expected DB config:
  - plain backup import
  - primary HNSW index must be created after import by the caller
  - no embedding model contract is assumed by default
  - generated from the committed multi-member workspace fixture via the shared
    workspace-fixture recreation path
- Tests using this fixture:
  - `ploke-test-utils`
    - [crates/test-utils/src/fixture_dbs.rs](../../crates/test-utils/src/fixture_dbs.rs): registry lookup and strict-load witness for the active workspace fixture
- Notes:
  - this fixture is the canonical plain workspace backup required by the
    workspace rollout readiness gate
  - the filename is dated `2026-05-17` because `cargo xtask recreate-backup-db`
    stamps outputs with UTC date

## `ws_fixture_01_member_single_2026-05-06.sqlite`

- File: `tests/backup_dbs/ws_fixture_01_member_single_2026-05-06.sqlite`
- Parsed target(s): `tests/fixture_workspace/ws_fixture_01/member_root`
- Expected DB config:
  - plain backup import
  - primary HNSW index must be created after import by the caller
  - no embedding model contract is assumed by default
  - generated from the workspace fixture, but only includes the `member_root` crate
    plus workspace metadata (excluding `nested/member_nested`)
- Tests using this fixture:
  - `ploke-tui` command decision tree tests:
    - Single workspace member scenarios (focused crate)
    - "db already loaded and is: single crate and workspace" paths
- Notes:
  - this fixture simulates a workspace where only one member has been indexed
  - used for testing focused-crate operations within a multi-member workspace context
  - the filename is dated `2026-05-06` because `cargo xtask recreate-backup-db`
    stamps outputs with UTC date

## Corpus Type Graph Fixtures

These fixtures are source-pinned targets for DB-backed graphRAG type traversal
contracts. The checkout identity, backup stem, and test intent live in one
registry-backed place instead of in local symlinks under
`tests/fixture_github_clones/corpus`.

Run `cargo xtask recreate-backup-db
--fixture <id>` to clone or reuse the pinned checkout, check out the recorded
commit, parse and transform the crate with typed type graph relations enabled,
write the dated backup under the fixture's configured shared-snapshot path, and
verify that the generated backup imports with the current schema. Copy a
reviewed snapshot into `tests/backup_dbs/` only when it should become a
committed seed artifact.

The `corpus_*_openrouter_embeddings` variants use the same source-pinned
checkouts, then run the OpenRouter embedding indexer before backup. They are
intended for RAG and TUI tests that must exercise production search fixtures
without parsing or embedding during normal test execution.

Expected searchable corpus embedding config:

- import with `Database::import_backup_with_embeddings`
- provider: `openrouter`
- model: `mistralai/codestral-embed-2505`
- dims: `1536`
- dtype: `f32`
- vectors must be present and the active embedding set metadata must match the
  registry entry
- regeneration requires `OPENROUTER_API_KEY`; ordinary tests load the saved
  backup and do not call the remote provider
- regeneration batches corpus snippets in groups of `16` to stay below
  OpenRouter request-level token limits for large real crates

### `corpus_semver_type_graph_2026-05-17.sqlite`

- Status: typed type graph
- File: `tests/backup_dbs/corpus_semver_type_graph_2026-05-17.sqlite`
- Parsed target: `github:dtolnay/semver@8591f2344b52b31d85b538de58b76a676fe9ff90`
- Checkout slug: `tests/fixture_github_clones/corpus/dtolnay__semver`
- Expected DB config:
  - plain backup import
  - no embedding model contract
  - no primary vector index required by the type graph query contracts
  - current typed type-use coordinate schema, including deterministic
    `type_use.id` and role-specific coordinate relations
- Tests using this fixture:
  - [crates/ploke-db/tests/unit/type_graph_queries/corpus_contracts.rs](../../crates/ploke-db/tests/unit/type_graph_queries/corpus_contracts.rs)
  - shared `TypeShapeCase` matrix coverage in DB/RAG/TUI tests
  - graphRAG traversal from `matches_req` to `VersionReq` and `Version`
  - traversal from `VersionReq.comparators: Vec<Comparator>` to `Comparator`

### `corpus_semver_openrouter_embeddings_2026-05-17.sqlite`

- Status: typed type graph
- File: `tests/backup_dbs/corpus_semver_openrouter_embeddings_2026-05-17.sqlite`
- Parsed target: `github:dtolnay/semver@8591f2344b52b31d85b538de58b76a676fe9ff90`
- Checkout slug: `tests/fixture_github_clones/corpus/dtolnay__semver`
- Expected DB config:
  - backup import with embeddings
  - OpenRouter searchable corpus embedding config described above
- Tests using this fixture:
  - shared `TypeShapeCase` RAG and TUI coverage for named paths, references,
    and semver model targets

### `corpus_memchr_type_graph_2026-05-17.sqlite`

- Status: typed type graph
- File: `tests/backup_dbs/corpus_memchr_type_graph_2026-05-17.sqlite`
- Parsed target: `github:BurntSushi/memchr@24f5daa5257e00e87007c936761600e034827905`
- Checkout slug: `tests/fixture_github_clones/corpus/BurntSushi__memchr`
- Expected DB config:
  - plain backup import
  - no embedding model contract
  - no primary vector index required by the type graph query contracts
  - current typed type-use coordinate schema, including deterministic
    `type_use.id` and role-specific coordinate relations
- Tests using this fixture:
  - [crates/ploke-db/tests/unit/type_graph_queries/corpus_contracts.rs](../../crates/ploke-db/tests/unit/type_graph_queries/corpus_contracts.rs)
  - shared `TypeShapeCase` matrix coverage in DB/RAG/TUI tests
  - traversal from `memchr_iter` return types to iterator structs such as
    `Memchr`
  - later traversal from iterator self types to `Iterator` and
    `DoubleEndedIterator` impl surfaces

### `corpus_memchr_call_graph_2026-07-15.sqlite`

- Status: active
- File: `tests/backup_dbs/corpus_memchr_call_graph_2026-07-15.sqlite`
- Parsed target: `github:BurntSushi/memchr@24f5daa5257e00e87007c936761600e034827905`
- Checkout slug: `tests/fixture_github_clones/corpus/BurntSushi__memchr`
- Expected DB config:
  - plain backup import
  - no embedding model contract
  - no primary vector index required by call graph query contracts
  - call graph relations projected from the current parser/transform baseline
- Tests using this fixture:
  - real-target call graph matrix rows for arbitrary-expression dynamic
    callees, function-pointer fields, and callable trait object fields
  - generated `unsafe_ifunc!` transmute path and returned-path dynamic frontier
    rows in [fallback.rs](../../crates/ploke-db/tests/unit/call_graph_fixture_queries/real_target_matrix/fallback.rs)
  - memchr `Searcher.call` and `Prefilter.call` rows preserve finite ambiguous
    `DynamicFunction` candidates from cfg-visible shorthand field initializers
  - memchr `Runner::new().fwd(...)` and `Runner::new().rev(...)` method-call
    rows resolve through associated-constructor path-call receiver handling
  - memchr `Runner::run` callable trait-object field path rows are still
    preserved as targetless unsupported rows

### `corpus_memchr_openrouter_embeddings_2026-05-17.sqlite`

- Status: typed type graph
- File: `tests/backup_dbs/corpus_memchr_openrouter_embeddings_2026-05-17.sqlite`
- Parsed target: `github:BurntSushi/memchr@24f5daa5257e00e87007c936761600e034827905`
- Checkout slug: `tests/fixture_github_clones/corpus/BurntSushi__memchr`
- Expected DB config:
  - backup import with embeddings
  - OpenRouter searchable corpus embedding config described above
- Tests using this fixture:
  - shared `TypeShapeCase` RAG and TUI coverage for references and function
    pointer alias targets

### `corpus_generic_array_type_graph_2026-05-17.sqlite`

- Status: typed type graph
- File: `tests/backup_dbs/corpus_generic_array_type_graph_2026-05-17.sqlite`
- Parsed target: `github:fizyk20/generic-array@80bab87431c2e29823dc551a3311324812838a23`
- Checkout slug: `tests/fixture_github_clones/corpus/fizyk20__generic-array`
- Expected DB config:
  - plain backup import
  - no embedding model contract
  - no primary vector index required by the type graph query contracts
  - current typed type-use coordinate schema, including deterministic
    `type_use.id` and role-specific coordinate relations
- Tests using this fixture:
  - [crates/ploke-db/tests/unit/type_graph_queries/corpus_contracts.rs](../../crates/ploke-db/tests/unit/type_graph_queries/corpus_contracts.rs)
  - shared `TypeShapeCase` matrix coverage in DB/RAG/TUI tests
  - traversal from const-generic aliases to `GenericArray`
  - `ArrayBuilder::extend(..., source: impl Iterator<Item = T>)` reaches the
    local generic parameter `T`
  - later traversal through const-generic bounds and associated impls

### `corpus_generic_array_call_graph_2026-07-15.sqlite`

- Status: active
- File: `tests/backup_dbs/corpus_generic_array_call_graph_2026-07-15.sqlite`
- Parsed target: `github:fizyk20/generic-array@80bab87431c2e29823dc551a3311324812838a23`
- Checkout slug: `tests/fixture_github_clones/corpus/fizyk20__generic-array`
- Expected DB config:
  - plain backup import
  - no embedding model contract
  - no primary vector index required by call graph query contracts
  - call graph relations projected from the current parser/transform baseline
- Tests using this fixture:
  - real-target call graph matrix rows for guarded match-arm method-result
    local receiver shapes in `GenericArray` construction paths

### `corpus_generic_array_openrouter_embeddings_2026-05-17.sqlite`

- Status: typed type graph
- File: `tests/backup_dbs/corpus_generic_array_openrouter_embeddings_2026-05-17.sqlite`
- Parsed target: `github:fizyk20/generic-array@80bab87431c2e29823dc551a3311324812838a23`
- Checkout slug: `tests/fixture_github_clones/corpus/fizyk20__generic-array`
- Expected DB config:
  - backup import with embeddings
  - OpenRouter searchable corpus embedding config described above
- Tests using this fixture:
  - shared `TypeShapeCase` RAG and TUI coverage for const-generic aliases,
    raw pointers, trait bounds, trait supers, generic parameter bounds, and
    where-clause owners
  - generic-array `ArrayBuilder::extend` impl-trait parameter coverage is
    DB-only because its terminal target is a generic parameter, not a RAG/TUI
    materialized target selector

### `corpus_chrono_type_graph_2026-05-17.sqlite`

- Status: typed type graph
- File: `tests/backup_dbs/corpus_chrono_type_graph_2026-05-17.sqlite`
- Parsed target: `github:chronotope/chrono@120686c82c5da90377e815edb82c9a80b6b4f2be`
- Checkout slug: `tests/fixture_github_clones/corpus/chronotope__chrono`
- Expected DB config:
  - plain backup import
  - no embedding model contract
  - no primary vector index required by the type graph query contracts
  - current typed type-use coordinate schema, including deterministic
    `type_use.id` and role-specific coordinate relations
- Tests using this fixture:
  - [crates/ploke-db/tests/unit/type_graph_queries/corpus_contracts.rs](../../crates/ploke-db/tests/unit/type_graph_queries/corpus_contracts.rs)
  - shared `TypeShapeCase` matrix coverage in DB/RAG/TUI tests
  - traversal from `MappedLocalTime<T>` aliases to `LocalResult<T>`
  - later traversal from timezone API owners into their generic result model

### `corpus_chrono_call_graph_2026-07-15.sqlite`

- Status: active
- File: `tests/backup_dbs/corpus_chrono_call_graph_2026-07-15.sqlite`
- Parsed target: `github:chronotope/chrono@120686c82c5da90377e815edb82c9a80b6b4f2be`
- Checkout slug: `tests/fixture_github_clones/corpus/chronotope__chrono`
- Expected DB config:
  - plain backup import
  - no embedding model contract
  - no primary vector index required by call graph query contracts
  - call graph relations projected from the current parser/transform baseline
- Tests using this fixture:
  - real-target call graph matrix rows for resolved alias constructors, try
    receivers, and guarded match-arm external slice receiver frontier calls

## 2026-07-07 Chrono Option ok_or Try Receiver Refresh

- Command: `cargo run -p xtask --features call_graph -- recreate-backup-db --fixture corpus_chrono_call_graph`
- Shared snapshot: `~/.config/ploke/db_snapshot_fixtures/corpus_chrono_call_graph_2026-07-07.sqlite`
- Seed artifact: `tests/backup_dbs/corpus_chrono_call_graph_2026-07-07.sqlite`
- Change covered: `chrono/src/format/parsed.rs:836,953`
  `DateTime::from_timestamp*(...).ok_or(OUT_OF_RANGE)?.naive_utc()` now
  resolves the outer `naive_utc` try receiver through local associated
  function proof for `DateTime::from_timestamp* -> Option<Self>`.

## 2026-07-06 Chrono Slice Receiver Frontier Refresh

- Command: `cargo run -p xtask --features call_graph -- recreate-backup-db --fixture corpus_chrono_call_graph`
- Shared snapshot: `~/.config/ploke/db_snapshot_fixtures/corpus_chrono_call_graph_2026-07-06.sqlite`
- Seed artifact: `tests/backup_dbs/corpus_chrono_call_graph_2026-07-06.sqlite`
- Change covered: `chrono/src/format/strftime.rs:635`
  `self.queue.is_empty()` now classifies as an external targetless slice
  receiver frontier using the source-visible field type
  `queue: &'static [Item<'static>]`.

### `corpus_chrono_openrouter_embeddings_2026-05-17.sqlite`

- Status: typed type graph
- File: `tests/backup_dbs/corpus_chrono_openrouter_embeddings_2026-05-17.sqlite`
- Parsed target: `github:chronotope/chrono@120686c82c5da90377e815edb82c9a80b6b4f2be`
- Checkout slug: `tests/fixture_github_clones/corpus/chronotope__chrono`
- Expected DB config:
  - backup import with embeddings
  - OpenRouter searchable corpus embedding config described above
- Tests using this fixture:
  - shared `TypeShapeCase` RAG and TUI coverage for named generic arguments,
    slices, arrays, tuples, associated type bounds, and where-bound type
    contexts

### `corpus_axum_type_graph_2026-05-17.sqlite`

- Status: typed type graph
- File: `tests/backup_dbs/corpus_axum_type_graph_2026-05-17.sqlite`
- Parsed target: `github:tokio-rs/axum@a3446d68bc03d61fb8e7513052bad2825d0c0db1`
- Checkout slug: `tests/fixture_github_clones/corpus/tokio-rs__axum`
- Selected workspace members:
  - `axum`
  - `axum-core`
  - `axum-macros`
- Expected DB config:
  - plain backup import
  - no embedding model contract
  - no primary vector index required by the type graph query contracts
  - current typed type-use coordinate schema, including deterministic
    `type_use.id` and role-specific coordinate relations
- Tests using this fixture:
  - shared `TypeShapeCase` DB matrix coverage for trait object, impl trait,
    nested paren no-target, and nested macro no-target structures
  - `BoxedIntoRoute<S, E>(Box<dyn ErasedIntoRoute<S, E>>)` reaches
    `ErasedIntoRoute`
  - `Map.layer: Box<dyn LayerFn<E, E2>>` reaches `LayerFn`
  - `MakeErasedHandler::clone_box() -> Box<dyn ErasedIntoRoute<S, Infallible>>`
    reaches `ErasedIntoRoute`
  - `zip_longest(...) -> impl Iterator<Item = Item<I::Item>>` reaches the
    local `Item` enum
  - `StripPrefix::layer(...) -> impl Layer<S, Service = Self> + Clone` reaches
    `StripPrefix` through the associated type bound
  - `Option<&(dyn StdError + 'static)>` retains its nested `paren_type` without
    fabricating a terminal target for the paren wrapper
  - `Token![,]` under `Punctuated<syn::Variant, Token![,]>` retains a nested
    macro type without fabricating a terminal target

### `corpus_axum_call_graph_2026-07-16.sqlite`

- Status: active
- File: `tests/backup_dbs/corpus_axum_call_graph_2026-07-16.sqlite`
- Parsed target: `github:tokio-rs/axum@a3446d68bc03d61fb8e7513052bad2825d0c0db1`
- Checkout slug: `tests/fixture_github_clones/corpus/tokio-rs__axum`
- Selected workspace members:
  - `axum`
  - `axum-core`
  - `axum-macros`
- Expected DB config:
  - plain backup import
  - no embedding model contract
  - no primary vector index required by call graph query contracts
  - call graph relations projected from the current parser/transform baseline
- Tests using this fixture:
  - `expand_with` and `expand_attr_with` reach root `expand` through
    owner-centered and target-centered call context
  - `RequestExt::extract` and `RequestPartsExt::extract` reach
    `extract_with_state` through same-impl self-method call resolution
  - `CountingCloneableState::new()` proves the initialized local receiver type
    for `state.clone()` and `state.setup_done()` in axum routing clone tests
  - axum-core `parts.extract_with_state(state)` reaches the local
    `RequestPartsExt for Parts::extract_with_state` impl method through exact
    imported external receiver type proof
  - `Body` conversion impl `Self::empty()` rows reach the inherent
    `Body::empty` associated function through local-exact call resolution
  - `Position::First(item)` reaches the local `Position::First` enum variant
    constructor through local-exact call resolution
  - `Json<T>` trait impl `Self::from_bytes(&bytes)` rows reach the inherent
    `Json::from_bytes` associated function through local-exact call resolution
  - `Handler::call(handler, req, state)` reaches the `Handler::call` trait
    method binding through path-style trait method resolution
  - `ListenerExt::tap_io` records constructor-side `TapIo { tap_fn }`
    parameter-to-field source frontier evidence while `TapIo::accept`
    keeps `(self.tap_fn)(&mut io)` targetless
  - `E::from_request`, `T::from_request`, `E::from_request_parts`, and
    `T::from_request_parts` reach their `FromRequest` / `FromRequestParts`
    trait method bindings through bounded type-parameter associated path
    resolution
  - same-crate axum-core `InnerState::from_ref` and `String::from_ref`
    bounded associated paths reach the `FromRef::from_ref` trait method
    binding; the top-level axum `State` extractor row whose bound imports
    `FromRef` through the parsed workspace dependency root
    `axum_core::extract::FromRef` and the nested middleware local-impl row
    owned by `local_impl_method:from_request_parts` also reach that trait
    method binding through executable where-bound scope resolution
  - `Router` `Default::default` reaches `Router::new` through a local-exact
    `Self::new()` associated-function edge
  - `IntoServiceFuture::new(future)` reaches the generated inherent
    constructor projected from the bounded `opaque_future!` item-position macro
    invocation
  - `routing::post(...)` callsites reach the generated top-level handler
    function projected from the bounded
    `top_level_handler_fn!(post, POST)` item-position macro invocation
  - `*_service(...)` callsites reach generated top-level service functions
    projected from the bounded `top_level_service_fn!` item-position macro
    invocations
  - `QueryRejection::into_response` reaches generated
    `FailedToDeserializeQueryString::into_response` through the
    `Self::FailedToDeserializeQueryString(inner)`
    enum-variant binding projected from the bounded `composite_rejection!`
    item-position macro invocation
  - axum `TestClient::new` reaches the cfg-gated local test helper target for
    168 projected structural rows through nested glob re-export rows,
    inherited parent glob imports, direct `test_helpers::TestClient` imports,
    `test_helpers::* -> pub use test_client::*`, and the axum-core
    `axum::test_helpers::*` workspace dependency glob import at
    `request_parts.rs:193`
  - typed local `Router` receiver `.clone()` rows reach the local
    `impl<S> Clone for Router<S>` method through exact local external-trait impl
    receiver resolution
  - targetless dynamic callees such as `(self.into_route)(...)` and
    `(self.tap_fn)(...)` preserve their self-field callee path while remaining
    unsupported and edge-free
  - `(self.layer)(...)` in `axum/src/boxed.rs:159,163` preserves finite
    ambiguous `DynamicClosure` candidates from the visible
    `MethodRouter::{layer,route_layer}` closure bindings without admitting a
    local traversal edge
  - selected proc-macro entrypoint bodies reach local helper functions through
    `CallBodyOwnerId::Macro` owner edges, including `expand_with` and active
    `expand_attr_with` callers
  - imported external type aliases stay targetless but classify as external,
    including axum-core `Request::new` through `Request = http::Request`
  - initialized local receivers whose initializer path is externally
    classified stay targetless but classify external, including
    axum-core `req.extensions_mut()` after `Request::new(())`
  - selected closure body and dynamic callable field shapes are documented as
    unsupported contracts until nested owner/callable proof improves

### `corpus_axum_openrouter_embeddings_2026-05-17.sqlite`

- Status: typed type graph
- File: `tests/backup_dbs/corpus_axum_openrouter_embeddings_2026-05-17.sqlite`
- Parsed target: `github:tokio-rs/axum@a3446d68bc03d61fb8e7513052bad2825d0c0db1`
- Checkout slug: `tests/fixture_github_clones/corpus/tokio-rs__axum`
- Selected workspace members:
  - `axum`
  - `axum-core`
  - `axum-macros`
- Expected DB config:
  - backup import with embeddings
  - OpenRouter searchable corpus embedding config described above
- Tests using this fixture:
  - shared `TypeShapeCase` RAG and TUI coverage for axum trait-object and
    impl-trait type contexts, including `BoxedIntoRoute`, `Map.layer`,
    `MakeErasedHandler::clone_box`, `zip_longest`, and `StripPrefix::layer`
  - parenthesized no-target coverage is DB-only because the paren wrapper is a
    nested structural node rather than a materializable search result

## `ploke-db_af8e3a20-728d-5967-8523-da8a5ccdae45`

- File: `tests/backup_dbs/ploke-db_af8e3a20-728d-5967-8523-da8a5ccdae45`
- Parsed target(s): `crates/ploke-db`
- Expected DB config:
  - no current test contract
- Tests using this fixture:
  - none found during the 2026-03-20 review
- Notes:
  - treat this as orphaned until a concrete consumer is identified
  - remove or regenerate only after confirming it is not used outside the repo

## Update instructions

- When a fixture changes, update:
  - this document
  - [AGENTS.md](../../AGENTS.md)
  - the shared registry in [crates/test-utils/src/fixture_dbs.rs](../../crates/test-utils/src/fixture_dbs.rs)
- Run `cargo xtask verify-backup-dbs` after schema or fixture changes.
- Use `cargo xtask recreate-backup-db --fixture <id>` instead of ad hoc copying
  whenever the registry already defines a recreation path.
- `tests/backup_dbs/` is ignored by default. Reviewed seed snapshots that
  should travel with a branch must be added explicitly rather than relying on
  normal `git status` output.
- If validation fails on a legacy backup only because `workspace_metadata` is
  missing, use `cargo xtask repair-backup-db-schema --fixture <id>` as the
  explicit schema repair path.
- Prefer adding or updating tests to consume the shared fixture registry instead of hard-coded
  backup paths.
- Prefer `shared_backup_fixture_db` for immutable shared callers and
  `fresh_backup_fixture_db` for isolated immutable callers before introducing a
  new crate-local lazy static.
- Prefer regenerating fixtures from repo code paths instead of copying ad hoc backups from the
  config dir without documentation.
