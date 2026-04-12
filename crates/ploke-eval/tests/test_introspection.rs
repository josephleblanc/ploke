// Test script for introspection API
// Run with: cargo test -p ploke-eval --test test_introspection -- --nocapture

use ploke_eval::record::read_compressed_record;
use std::path::PathBuf;

#[tokio::test]
async fn test_introspection_api_on_latest_run() {
    // Path to the run record
    let record_path =
        PathBuf::from("/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/record.json.gz");
    let db_path =
        PathBuf::from("/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/final-snapshot.db");

    // 1. Load the RunRecord
    let record = read_compressed_record(&record_path).expect("Failed to read record");

    // 2. Verify SetupPhase is populated
    let setup = record
        .phases
        .setup
        .as_ref()
        .expect("SetupPhase should be populated");
    assert!(
        !setup.indexed_crates.is_empty(),
        "SetupPhase should record indexed crates"
    );

    // 3. Get the first turn
    let turn = record
        .phases
        .agent_turns
        .first()
        .expect("Should have at least one turn");
    assert!(
        turn.db_timestamp_micros > 0,
        "Turn should have a valid DB timestamp"
    );

    // 4. Open the DB from backup
    let db = ploke_db::Database::create_new_backup_default(&db_path)
        .await
        .expect("Failed to open DB from backup");

    // 5. Test turn.db_state().lookup()
    let db_state = turn.db_state();
    assert_eq!(
        db_state.timestamp_micros(),
        turn.db_timestamp_micros,
        "DbState should use the turn timestamp"
    );

    let globset = db_state
        .lookup(&db, "GlobSet")
        .expect("lookup('GlobSet') should execute")
        .expect("lookup('GlobSet') should find the struct");
    assert_eq!(globset.name, "GlobSet");

    let new_fn = db_state
        .lookup(&db, "new")
        .expect("lookup('new') should execute")
        .expect("lookup('new') should find a function or method");
    assert_eq!(new_fn.name, "new");

    let result = record
        .replay_query(
            turn.turn_number,
            &db,
            "?[count(id)] := *function{id @ 'NOW'}",
        )
        .expect("replay_query should execute");
    assert!(
        !result.rows.is_empty(),
        "replay_query should return at least one row"
    );
    let count = result.rows[0][0]
        .get_int()
        .expect("replay_query count should be an integer");
    assert!(count > 0, "Function count should be positive");
}
