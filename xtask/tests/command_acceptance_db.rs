//! Command-level acceptance tests for `db` subcommands (PRIMARY_TASK_SPEC A.4).
//!
//! Tests are named `acceptance_db_*`. Gap-signal tests assert `todo!()` panics until
//! `hnsw-build` / `hnsw-rebuild` / `bm25-rebuild` are implemented; replace those with
//! behavior-gated tests when commands are real.

use ploke_test_utils::FIXTURE_NODES_CANONICAL;

use xtask::commands::db::{
    Bm25Rebuild, DbOutput, EmbeddingStatus, HnswBuild, HnswRebuild, ListRelations,
};
use xtask::expect_command_ok;
use xtask::test_harness::CommandTestHarness;

/// **Command:** `db list-relations`  
/// **Fixture:** isolated copy of `FIXTURE_NODES_CANONICAL`.  
/// **Expect:** non-empty relation list; names are non-empty strings.
#[test]
fn acceptance_db_list_relations_success() {
    let iso = CommandTestHarness::isolated_fixture_copy(&FIXTURE_NODES_CANONICAL)
        .expect("isolated fixture copy");

    let cmd = ListRelations {
        db: Some(iso.db_path.clone()),
        no_hnsw: false,
        counts: false,
    };

    let harness = CommandTestHarness::new().expect("CommandTestHarness");
    let output = expect_command_ok(
        harness.executor().execute(cmd),
        "db list-relations must succeed on fixture DB",
    );

    let DbOutput::RelationsList { relations } = output else {
        panic!("expected RelationsList, got {output:?}");
    };
    assert!(
        !relations.is_empty(),
        "fixture DB should expose at least one relation"
    );
    for r in &relations {
        assert!(!r.name.is_empty(), "relation name must be non-empty: {r:?}");
    }
}

/// **Command:** `db list-relations --counts`  
/// **Expect:** each relation has `row_count` when `--counts` is set.
#[test]
fn acceptance_db_list_relations_with_counts() {
    let iso = CommandTestHarness::isolated_fixture_copy(&FIXTURE_NODES_CANONICAL)
        .expect("isolated fixture copy");

    let cmd = ListRelations {
        db: Some(iso.db_path.clone()),
        no_hnsw: false,
        counts: true,
    };

    let harness = CommandTestHarness::new().expect("CommandTestHarness");
    let output = expect_command_ok(
        harness.executor().execute(cmd),
        "db list-relations --counts must succeed",
    );

    let DbOutput::RelationsList { relations } = output else {
        panic!("expected RelationsList, got {output:?}");
    };
    assert!(!relations.is_empty());
    // `count_relation_rows` uses `.ok()` per relation; some system relations may not support the generic count query.
    let populated: Vec<_> = relations
        .iter()
        .filter(|r| r.row_count.is_some())
        .collect();
    assert!(
        !populated.is_empty(),
        "expected at least one relation with row_count when --counts; got {:?}",
        relations
            .iter()
            .map(|r| (&r.name, r.row_count))
            .collect::<Vec<_>>()
    );
}

/// **Command:** `db embedding-status`  
/// **Fixture:** isolated `FIXTURE_NODES_CANONICAL`.  
/// **Expect:** `total_nodes` matches function row count semantics; counts are consistent with current command implementation.
#[test]
fn acceptance_db_embedding_status_success() {
    let iso = CommandTestHarness::isolated_fixture_copy(&FIXTURE_NODES_CANONICAL)
        .expect("isolated fixture copy");

    let cmd = EmbeddingStatus {
        db: Some(iso.db_path.clone()),
        set: None,
        detailed: false,
    };

    let harness = CommandTestHarness::new().expect("CommandTestHarness");
    let output = expect_command_ok(
        harness.executor().execute(cmd),
        "db embedding-status must succeed on fixture DB",
    );

    let DbOutput::EmbeddingStatus {
        total_nodes,
        embedded,
        pending,
        ..
    } = output
    else {
        panic!("expected EmbeddingStatus variant, got {output:?}");
    };

    assert!(total_nodes > 0, "fixture should have function rows: {total_nodes}");
    assert!(
        embedded <= total_nodes,
        "embedded={embedded} should not exceed total_nodes={total_nodes}"
    );
    // Invariant from commands/db.rs: embedded = total_nodes.saturating_sub(pending)
    assert_eq!(embedded, total_nodes.saturating_sub(pending));
}

/// **Gap-signal:** `db hnsw-build` still `todo!()` — passes while unimplemented; replace with Ok + DB checks when done.
#[test]
fn acceptance_db_hnsw_build_panics_until_implemented() {
    let harness = CommandTestHarness::new().expect("CommandTestHarness");
    let cmd = HnswBuild {
        db: None,
        embedding_set: None,
        dimensions: None,
    };
    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = harness.executor().execute(cmd);
    }))
    .is_err();
    assert!(
        panicked,
        "db hnsw-build should panic until implementation replaces todo!()"
    );
}

/// **Gap-signal:** `db hnsw-rebuild` still `todo!()`.
#[test]
fn acceptance_db_hnsw_rebuild_panics_until_implemented() {
    let harness = CommandTestHarness::new().expect("CommandTestHarness");
    let cmd = HnswRebuild { db: None, force: false };
    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = harness.executor().execute(cmd);
    }))
    .is_err();
    assert!(
        panicked,
        "db hnsw-rebuild should panic until implementation replaces todo!()"
    );
}

/// **Gap-signal:** `db bm25-rebuild` still `todo!()`.
#[test]
fn acceptance_db_bm25_rebuild_panics_until_implemented() {
    let harness = CommandTestHarness::new().expect("CommandTestHarness");
    let cmd = Bm25Rebuild {
        db: None,
        batch_size: 1000,
    };
    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = harness.executor().execute(cmd);
    }))
    .is_err();
    assert!(
        panicked,
        "db bm25-rebuild should panic until implementation replaces todo!()"
    );
}
