use ploke_db::{Database, DbError, TypeRelationKind};
use ploke_test_utils::{
    CORPUS_CHRONO_TYPE_GRAPH, CORPUS_GENERIC_ARRAY_TYPE_GRAPH, CORPUS_MEMCHR_TYPE_GRAPH,
    CORPUS_SEMVER_TYPE_GRAPH,
};

use super::common::{
    assert_owner_reaches_target, enum_id_by_name, field_id_by_owner_index,
    function_id_by_name_in_file_suffix, function_id_by_name_in_module, setup_typed_backup_db,
    setup_typed_corpus_db, struct_id_by_name, struct_id_by_name_in_file_suffix,
    struct_id_by_name_in_module, type_alias_row_by_name,
};

// These tests are graphRAG contracts over real corpus crates. They are ignored
// by default because each test parses and transforms a full GitHub fixture
// crate, but the assertions are ordinary success assertions. They are not
// placeholders.
//
// The meta-design target is: whenever we would currently use `rg` to find
// names, signatures, and nearby impls, the DB should eventually answer the
// same question structurally by traversing:
//
//   code owner -> root type use -> structural type containment
//   -> terminal named/trait type use -> resolved definition
//   -> other owners that use the same exact/root/terminal type.
//
// Examples of the grep-to-graph replacement we want:
//
// - `rg "VersionReq|Version|matches_req" semver/src`
//   should become traversal from `matches_req` to the `VersionReq` and
//   `Version` definitions, then to other parse/matches/serde code using them.
//
// - `rg "Vec<Comparator>|Comparator|Op" semver/src`
//   should become traversal from `VersionReq.comparators` through the `Vec`
//   argument to `Comparator`, then outward to comparator parsing/evaluation.
//
// - `rg "ConstGenericArray|GenericArray|ArrayLength" generic-array/src`
//   should become traversal through alias expansion and generic bounds, so
//   fixed-size array APIs cluster with `GenericArray<T, N>` impls.
//
// - `rg "memchr_iter|Memchr|DoubleEndedIterator" memchr/src`
//   should become traversal from iterator-returning constructors to iterator
//   self types and then to their trait impls.
//
// - `rg "channel|Sender|Receiver|TrySendError|RetryPromise" hyper/src`
//   should become traversal through tuple returns, type aliases, nested
//   `Result`, and async callback/error types.
//
// - `rg "MappedLocalTime|LocalResult|TimeZone" chrono/src`
//   should become alias and generic traversal from public API names into the
//   real enum/generic timezone model.

#[test]
#[ignore = "corpus graphRAG contract: parses and transforms dtolnay__semver"]
fn semver_matches_req_reaches_both_public_model_types() -> Result<(), DbError> {
    let db = setup_typed_corpus_db("dtolnay__semver")?;
    assert_semver_matches_req_reaches_both_public_model_types(&db)
}

#[test]
fn semver_backup_matches_req_reaches_both_public_model_types() -> Result<(), DbError> {
    let db = setup_typed_backup_db(&CORPUS_SEMVER_TYPE_GRAPH)?;
    assert_semver_matches_req_reaches_both_public_model_types(&db)
}

fn assert_semver_matches_req_reaches_both_public_model_types(db: &Database) -> Result<(), DbError> {
    let owner_id = function_id_by_name_in_module(&db, &["crate", "eval"], "matches_req")?;
    let version_req_id = struct_id_by_name_in_module(&db, &["crate"], "VersionReq")?;
    let version_id = struct_id_by_name(&db, "Version")?;

    // Current search habit:
    //   rg -n "matches_req|VersionReq|Version" semver/src
    //
    // Desired graphRAG traversal:
    //   matches_req(req: &VersionReq, ver: &Version)
    //   -> reference child VersionReq / Version
    //   -> struct definitions
    assert_owner_reaches_target(
        &db,
        owner_id,
        version_req_id,
        TypeRelationKind::Ordinary,
        1,
        "matches_req should reach VersionReq through its first referenced parameter",
    )?;
    assert_owner_reaches_target(
        &db,
        owner_id,
        version_id,
        TypeRelationKind::Ordinary,
        1,
        "matches_req should reach Version through its second referenced parameter",
    )?;
    Ok(())
}

#[test]
#[ignore = "corpus graphRAG contract: parses and transforms dtolnay__semver"]
fn semver_version_req_comparators_field_reaches_comparator() -> Result<(), DbError> {
    let db = setup_typed_corpus_db("dtolnay__semver")?;
    assert_semver_version_req_comparators_field_reaches_comparator(&db)
}

#[test]
fn semver_backup_version_req_comparators_field_reaches_comparator() -> Result<(), DbError> {
    let db = setup_typed_backup_db(&CORPUS_SEMVER_TYPE_GRAPH)?;
    assert_semver_version_req_comparators_field_reaches_comparator(&db)
}

fn assert_semver_version_req_comparators_field_reaches_comparator(
    db: &Database,
) -> Result<(), DbError> {
    let version_req_id = struct_id_by_name_in_module(&db, &["crate"], "VersionReq")?;
    // Source: `pub comparators: Vec<Comparator>`. The current parsed field row
    // carries a synthetic display name, so the contract identifies this single
    // source field by its struct slot.
    let comparators_field_id = field_id_by_owner_index(&db, version_req_id, 0)?;
    let comparator_id = struct_id_by_name(&db, "Comparator")?;

    // Current search habit:
    //   rg -n "comparators|Vec<Comparator>|struct Comparator" semver/src
    //
    // Desired graphRAG traversal:
    //   VersionReq.comparators: Vec<Comparator>
    //   -> named generic argument Comparator
    //   -> Comparator definition and its parser/evaluator users.
    assert_owner_reaches_target(
        &db,
        comparators_field_id,
        comparator_id,
        TypeRelationKind::Ordinary,
        1,
        "VersionReq.comparators should reach Comparator through Vec's type argument",
    )
}

#[test]
#[ignore = "corpus graphRAG contract: parses and transforms fizyk20__generic-array"]
fn generic_array_const_generic_alias_reaches_generic_array() -> Result<(), DbError> {
    let db = setup_typed_corpus_db("fizyk20__generic-array")?;
    assert_generic_array_const_generic_alias_reaches_generic_array(&db)
}

#[test]
fn generic_array_backup_const_generic_alias_reaches_generic_array() -> Result<(), DbError> {
    let db = setup_typed_backup_db(&CORPUS_GENERIC_ARRAY_TYPE_GRAPH)?;
    assert_generic_array_const_generic_alias_reaches_generic_array(&db)
}

fn assert_generic_array_const_generic_alias_reaches_generic_array(
    db: &Database,
) -> Result<(), DbError> {
    let (alias_owner_id, _root_type_id) = type_alias_row_by_name(&db, "ConstGenericArray")?;
    let generic_array_id = struct_id_by_name(&db, "GenericArray")?;

    // Current search habit:
    //   rg -n "ConstGenericArray|GenericArray|ConstArrayLength" generic-array/src
    //
    // Desired graphRAG traversal:
    //   ConstGenericArray<T, const N>
    //   -> GenericArray<T, ConstArrayLength<N>>
    //   -> GenericArray definition and its `N: ArrayLength` impl surface.
    assert_owner_reaches_target(
        &db,
        alias_owner_id,
        generic_array_id,
        TypeRelationKind::Ordinary,
        0,
        "ConstGenericArray alias should reach the GenericArray definition",
    )
}

#[test]
#[ignore = "corpus graphRAG contract: parses and transforms BurntSushi__memchr"]
fn memchr_iter_return_reaches_iterator_struct() -> Result<(), DbError> {
    let db = setup_typed_corpus_db("BurntSushi__memchr")?;
    assert_memchr_iter_return_reaches_iterator_struct(&db)
}

#[test]
fn memchr_backup_iter_return_reaches_iterator_struct() -> Result<(), DbError> {
    let db = setup_typed_backup_db(&CORPUS_MEMCHR_TYPE_GRAPH)?;
    assert_memchr_iter_return_reaches_iterator_struct(&db)
}

fn assert_memchr_iter_return_reaches_iterator_struct(db: &Database) -> Result<(), DbError> {
    let owner_id = function_id_by_name_in_module(&db, &["crate", "memchr"], "memchr_iter")?;
    let memchr_id = struct_id_by_name(&db, "Memchr")?;

    // Current search habit:
    //   rg -n "memchr_iter|struct Memchr|DoubleEndedIterator" memchr/src
    //
    // Desired graphRAG traversal:
    //   memchr_iter(...) -> Memchr<'h>
    //   -> Memchr definition
    //   -> Iterator / DoubleEndedIterator impls for the same self type.
    assert_owner_reaches_target(
        &db,
        owner_id,
        memchr_id,
        TypeRelationKind::Ordinary,
        0,
        "memchr_iter should reach its Memchr iterator return type",
    )
}

#[test]
#[ignore = "corpus graphRAG contract: parses and transforms hyperium__hyper"]
fn hyper_watch_channel_tuple_return_reaches_sender_and_receiver() -> Result<(), DbError> {
    let db = setup_typed_corpus_db("hyperium__hyper")?;
    let file_suffix = "src/common/watch.rs";
    let owner_id = function_id_by_name_in_file_suffix(&db, file_suffix, "channel")?;
    let sender_id = struct_id_by_name_in_file_suffix(&db, file_suffix, "Sender")?;
    let receiver_id = struct_id_by_name_in_file_suffix(&db, file_suffix, "Receiver")?;

    // Current search habit:
    //   rg -n "channel|Sender|Receiver|Shared" hyper/src/common/watch.rs
    //
    // Desired graphRAG traversal:
    //   watch::channel(initial: Value) -> (Sender, Receiver)
    //   -> tuple elements Sender / Receiver
    //   -> impls and shared state flow for those watch endpoints.
    for (target_id, label) in [(sender_id, "Sender"), (receiver_id, "Receiver")] {
        assert_owner_reaches_target(
            &db,
            owner_id,
            target_id,
            TypeRelationKind::Ordinary,
            1,
            &format!("hyper channel should reach tuple return element {label}"),
        )?;
    }
    Ok(())
}

#[test]
#[ignore = "corpus graphRAG contract: parses and transforms chronotope__chrono"]
fn chrono_mapped_local_time_alias_reaches_local_result() -> Result<(), DbError> {
    let db = setup_typed_corpus_db("chronotope__chrono")?;
    assert_chrono_mapped_local_time_alias_reaches_local_result(&db)
}

#[test]
fn chrono_backup_mapped_local_time_alias_reaches_local_result() -> Result<(), DbError> {
    let db = setup_typed_backup_db(&CORPUS_CHRONO_TYPE_GRAPH)?;
    assert_chrono_mapped_local_time_alias_reaches_local_result(&db)
}

fn assert_chrono_mapped_local_time_alias_reaches_local_result(
    db: &Database,
) -> Result<(), DbError> {
    let (alias_owner_id, _root_type_id) = type_alias_row_by_name(&db, "MappedLocalTime")?;
    let local_result_id = enum_id_by_name(&db, "LocalResult")?;

    // Current search habit:
    //   rg -n "MappedLocalTime|LocalResult|TimeZone" chrono/src
    //
    // Desired graphRAG traversal:
    //   MappedLocalTime<T> = LocalResult<T>
    //   -> LocalResult enum
    //   -> timezone mapping APIs that return or transform local-time results.
    assert_owner_reaches_target(
        &db,
        alias_owner_id,
        local_result_id,
        TypeRelationKind::Ordinary,
        0,
        "MappedLocalTime alias should reach the LocalResult enum definition",
    )
}
