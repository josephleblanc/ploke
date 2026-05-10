//! GraphRAG contracts over real corpus crates.
//!
//! These tests are stricter than parser-fixture unit tests: each case starts
//! from a real crate shape we currently search for with `rg` and asserts the DB
//! can answer the same question through typed graph traversal:
//!
//! ```text
//! code owner -> root type use -> structural type containment
//! -> terminal named/trait type use -> resolved definition
//! -> other owners that use the same exact/root/terminal type
//! ```
//!
//! Source-parse variants are ignored by default because they parse and transform
//! a full GitHub fixture crate. Backup variants are ordinary executable
//! contracts over committed typed graph fixture databases.
//!
//! Coverage table:
//!
//! | Capability bucket | Real-corpus status | Example contract |
//! | --- | --- | --- |
//! | Function parameter roots | Passing | `matches_req(req: &VersionReq, ver: &Version)` reaches both model structs. |
//! | Function return roots | Passing | `memchr_iter(...) -> Memchr<'h>` reaches `Memchr`. |
//! | Field roots | Passing | `VersionReq.comparators: Vec<Comparator>` reaches nested `Comparator`. |
//! | Type alias target roots | Passing | `MappedLocalTime<T> = LocalResult<T>` reaches `LocalResult`; `ConstGenericArray` reaches `GenericArray`. |
//! | Method parameter roots | New coverage | `VersionReq::matches(&self, version: &Version)` should reach `Version`. |
//! | Method return roots | New coverage | `Parsed::to_datetime(...) -> ParseResult<DateTime<FixedOffset>>` should reach nested return targets. |
//! | Impl self roots | New coverage | `impl GenericSequence<T> for GenericArray<T, N>` should reach `GenericArray`. |
//! | Impl trait roots | New coverage | `impl GenericSequence<T> for GenericArray<T, N>` should reach `GenericSequence`. |
//! | Const/static roots | New coverage | `MIN_DATE: NaiveDate` and `D_FMT: &[Item<'static>]` should reach local targets. |
//! | Reference containment | Passing | `&VersionReq` / `&Version` traverses through the referenced child type. |
//! | Named generic-argument containment | Passing | `Vec<Comparator>` and `GenericArray<T, ConstArrayLength<N>>` expose nested named targets. |
//! | Array containment | New coverage | `WeekdaySet::from_array(days: [Weekday; C])` should reach the array element enum. |
//! | Tuple containment | Source-only ignored | `watch::channel(...) -> (Sender, Receiver)` has an ignored Hyper source contract, but no backup fixture contract yet. |
//! | Generic declaration bounds | Passing | `GenericArray<T, N: ArrayLength>` and `Date<Tz: TimeZone>` reach their trait bounds. |
//! | Generic-param-owned bound roots | Passing | `DateTime::Tz` reaches `TimeZone` from the generic parameter owner. |
//! | Qualified associated type projections | Red | `<Const<N> as IntoArrayLength>::ArrayLength` does not yet expose `IntoArrayLength`. |
//! | Associated type bounds | Red | `trait TimeZone { type Offset: Offset; }` does not yet expose `Offset`. |
//! | Trait super roots | Not covered | Need real corpus `trait Child: Parent` backup contract. |
//! | Slice/raw pointer/function pointer/paren/never/inferred/macro/unknown type vertices | Not covered | These structural node families are parser/DB-fixture covered only, if at all. |
//! | Union/type-alias/generic-param targets | Not covered | Real corpus contracts currently hit struct, enum, and red trait targets. |
//! | Import path stress | Not covered | Need renamed import, glob import, re-export chain, `crate::`/`self::`/`super::`, and file-module boundary contracts. |
//! | RAG-facing expansion/ranking | Mostly wishlist | Low-level reachability is asserted here; `expand_type_context` corpus behavior is covered separately as wishlist/API contracts. |
//!
//! Examples of the grep-to-graph replacement we want:
//!
//! - `rg "VersionReq|Version|matches_req" semver/src`
//!   should become traversal from `matches_req` to the `VersionReq` and
//!   `Version` definitions, then to other parse/matches/serde code using them.
//! - `rg "Vec<Comparator>|Comparator|Op" semver/src`
//!   should become traversal from `VersionReq.comparators` through the `Vec`
//!   argument to `Comparator`, then outward to comparator parsing/evaluation.
//! - `rg "ConstGenericArray|GenericArray|ArrayLength" generic-array/src`
//!   should become traversal through alias expansion and generic bounds, so
//!   fixed-size array APIs cluster with `GenericArray<T, N>` impls.
//! - `rg "memchr_iter|Memchr|DoubleEndedIterator" memchr/src`
//!   should become traversal from iterator-returning constructors to iterator
//!   self types and then to their trait impls.
//! - `rg "channel|Sender|Receiver|TrySendError|RetryPromise" hyper/src`
//!   should become traversal through tuple returns, type aliases, nested
//!   `Result`, and async callback/error types.
//! - `rg "MappedLocalTime|LocalResult|TimeZone" chrono/src`
//!   should become alias and generic traversal from public API names into the
//!   real enum/generic timezone model.

use ploke_db::{Database, DbError, TypeRelationKind};
use ploke_test_utils::{
    CORPUS_CHRONO_TYPE_GRAPH, CORPUS_GENERIC_ARRAY_TYPE_GRAPH, CORPUS_MEMCHR_TYPE_GRAPH,
    CORPUS_SEMVER_TYPE_GRAPH,
};

use super::common::{
    assert_owner_reaches_target, const_id_by_name_in_file_suffix, enum_id_by_name,
    exactly_one_uuid, field_id_by_owner_index, function_id_by_name_in_file_suffix,
    function_id_by_name_in_module, impl_id_by_trait_and_self_type_names,
    method_id_by_impl_self_type_name, method_id_by_impl_trait_and_self_type_names,
    setup_typed_backup_db, setup_typed_corpus_db, static_id_by_name_in_file_suffix,
    struct_id_by_name, struct_id_by_name_in_file_suffix, struct_id_by_name_in_module,
    trait_id_by_name_in_module, type_alias_row_by_name,
};

mod ordinary_reachability {
    use super::*;

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

    fn assert_semver_matches_req_reaches_both_public_model_types(
        db: &Database,
    ) -> Result<(), DbError> {
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
}

mod structural_containment {
    use super::*;

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
    fn chrono_backup_weekday_set_from_array_param_reaches_weekday() -> Result<(), DbError> {
        let db = setup_typed_backup_db(&CORPUS_CHRONO_TYPE_GRAPH)?;
        let method_id = method_id_by_impl_self_type_name(&db, "WeekdaySet", "from_array")?;
        let weekday_id = enum_id_by_name(&db, "Weekday")?;

        // Source: `pub const fn from_array<const C: usize>(days: [Weekday; C]) -> Self`.
        //
        // Current search habit:
        //   rg -n "from_array|\\[Weekday; C\\]|enum Weekday" chrono/src
        //
        // Desired graphRAG traversal:
        //   WeekdaySet::from_array(days: [Weekday; C])
        //   -> array element Weekday
        //   -> Weekday enum definition and weekday set APIs.
        assert_owner_reaches_target(
            &db,
            method_id,
            weekday_id,
            TypeRelationKind::Ordinary,
            1,
            "WeekdaySet::from_array should reach Weekday through its array parameter element",
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
}

mod method_roots {
    use super::*;

    #[test]
    fn semver_backup_version_req_matches_method_param_reaches_version() -> Result<(), DbError> {
        let db = setup_typed_backup_db(&CORPUS_SEMVER_TYPE_GRAPH)?;
        let method_id = method_id_by_impl_self_type_name(&db, "VersionReq", "matches")?;
        let version_id = struct_id_by_name(&db, "Version")?;

        // Source: `pub fn matches(&self, version: &Version) -> bool`.
        //
        // Current search habit:
        //   rg -n "VersionReq|fn matches|&Version" semver/src
        //
        // Desired graphRAG traversal:
        //   VersionReq::matches(version: &Version)
        //   -> reference child Version
        //   -> Version definition and related parsing/evaluation code.
        assert_owner_reaches_target(
            &db,
            method_id,
            version_id,
            TypeRelationKind::Ordinary,
            1,
            "VersionReq::matches should reach Version through its method parameter",
        )
    }

    #[test]
    fn chrono_backup_parsed_to_datetime_return_reaches_datetime_and_offset() -> Result<(), DbError>
    {
        let db = setup_typed_backup_db(&CORPUS_CHRONO_TYPE_GRAPH)?;
        let method_id = method_id_by_impl_self_type_name(&db, "Parsed", "to_datetime")?;
        let datetime_id = struct_id_by_name(&db, "DateTime")?;
        let fixed_offset_id = struct_id_by_name(&db, "FixedOffset")?;

        // Source: `pub fn to_datetime(&self) -> ParseResult<DateTime<FixedOffset>>`.
        //
        // Current search habit:
        //   rg -n "to_datetime|DateTime<FixedOffset>|struct FixedOffset" chrono/src
        //
        // Desired graphRAG traversal:
        //   Parsed::to_datetime()
        //   -> return alias/container
        //   -> DateTime
        //   -> nested FixedOffset timezone argument.
        assert_owner_reaches_target(
            &db,
            method_id,
            datetime_id,
            TypeRelationKind::Ordinary,
            1,
            "Parsed::to_datetime should reach DateTime through its method return",
        )?;
        assert_owner_reaches_target(
            &db,
            method_id,
            fixed_offset_id,
            TypeRelationKind::Ordinary,
            2,
            "Parsed::to_datetime should reach FixedOffset through nested return type arguments",
        )
    }

    #[test]
    fn generic_array_backup_generate_method_return_reaches_generic_array() -> Result<(), DbError> {
        let db = setup_typed_backup_db(&CORPUS_GENERIC_ARRAY_TYPE_GRAPH)?;
        let method_id = method_id_by_impl_trait_and_self_type_names(
            &db,
            "GenericSequence",
            "GenericArray",
            "generate",
        )?;
        let generic_array_id = struct_id_by_name(&db, "GenericArray")?;

        // Source: `fn generate<F>(...) -> GenericArray<T, N>` in
        // `unsafe impl GenericSequence<T> for GenericArray<T, N>`.
        //
        // Current search habit:
        //   rg -n "GenericSequence|fn generate|GenericArray<T, N>" generic-array/src
        //
        // Desired graphRAG traversal:
        //   GenericSequence::generate implementation
        //   -> concrete return type GenericArray<T, N>
        //   -> GenericArray definition and impl surface.
        assert_owner_reaches_target(
            &db,
            method_id,
            generic_array_id,
            TypeRelationKind::Ordinary,
            0,
            "GenericSequence::generate impl method should reach its GenericArray return type",
        )
    }
}

mod impl_roots {
    use super::*;

    #[test]
    fn generic_array_backup_generic_sequence_impl_reaches_self_and_trait() -> Result<(), DbError> {
        let db = setup_typed_backup_db(&CORPUS_GENERIC_ARRAY_TYPE_GRAPH)?;
        let impl_id = impl_id_by_trait_and_self_type_names(&db, "GenericSequence", "GenericArray")?;
        let generic_array_id = struct_id_by_name(&db, "GenericArray")?;
        let generic_sequence_id =
            trait_id_by_name_in_module(&db, &["crate", "sequence"], "GenericSequence")?;

        // Source: `unsafe impl<T, N: ArrayLength> GenericSequence<T> for GenericArray<T, N>`.
        //
        // Current search habit:
        //   rg -n "GenericSequence<T> for GenericArray" generic-array/src
        //
        // Desired graphRAG traversal:
        //   impl block
        //   -> ImplSelf GenericArray<T, N>
        //   -> ImplTrait GenericSequence<T>
        //   -> both local definitions.
        assert_owner_reaches_target(
            &db,
            impl_id,
            generic_array_id,
            TypeRelationKind::Ordinary,
            0,
            "GenericSequence impl should reach its GenericArray self type",
        )?;
        assert_owner_reaches_target(
            &db,
            impl_id,
            generic_sequence_id,
            TypeRelationKind::Trait,
            0,
            "GenericSequence impl should reach its local trait target",
        )
    }
}

mod alias_expansion {
    use super::*;

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
}

mod const_static_roots {
    use super::*;

    #[test]
    fn chrono_backup_min_naive_date_const_reaches_naive_date() -> Result<(), DbError> {
        let db = setup_typed_backup_db(&CORPUS_CHRONO_TYPE_GRAPH)?;
        let const_id = const_id_by_name_in_file_suffix(&db, "src/naive/date/mod.rs", "MIN_DATE")?;
        let naive_date_id = struct_id_by_name(&db, "NaiveDate")?;

        // Source: `pub const MIN_DATE: NaiveDate = NaiveDate::MIN`.
        //
        // Current search habit:
        //   rg -n "MIN_DATE|struct NaiveDate" chrono/src/naive/date
        //
        // Desired graphRAG traversal:
        //   MIN_DATE const
        //   -> NaiveDate type annotation
        //   -> NaiveDate definition and constructors.
        assert_owner_reaches_target(
            &db,
            const_id,
            naive_date_id,
            TypeRelationKind::Ordinary,
            0,
            "MIN_DATE should reach its NaiveDate const type",
        )
    }

    #[test]
    fn chrono_backup_strftime_static_format_reaches_item() -> Result<(), DbError> {
        let db = setup_typed_backup_db(&CORPUS_CHRONO_TYPE_GRAPH)?;
        let static_id = static_id_by_name_in_file_suffix(&db, "src/format/strftime.rs", "D_FMT")?;
        let item_id = enum_id_by_name(&db, "Item")?;

        // Source: `static D_FMT: &[Item<'static>] = ...`.
        //
        // Current search habit:
        //   rg -n "D_FMT|Item<'static>|enum Item" chrono/src/format
        //
        // Desired graphRAG traversal:
        //   D_FMT static
        //   -> reference/slice child
        //   -> Item enum definition.
        assert_owner_reaches_target(
            &db,
            static_id,
            item_id,
            TypeRelationKind::Ordinary,
            2,
            "D_FMT should reach Item through its referenced slice element type",
        )
    }
}

mod generic_bounds {
    use super::*;

    // Generic declaration bounds were originally part of the KL-008 red bucket.
    // Regenerated typed corpus backups now contain the GenericBound and
    // GenericParamBound roots emitted by fresh ingestion, so these are ordinary
    // executable contracts rather than known failures.

    #[test]
    fn generic_array_backup_generic_array_bound_reaches_array_length_trait() -> Result<(), DbError>
    {
        let db = setup_typed_backup_db(&CORPUS_GENERIC_ARRAY_TYPE_GRAPH)?;
        let generic_array_id = struct_id_by_name(&db, "GenericArray")?;
        let array_length_id = trait_id_by_name_in_module(&db, &["crate"], "ArrayLength")?;

        // Source: `pub struct GenericArray<T, N: ArrayLength>`.
        //
        // Current search habit:
        //   rg -n "struct GenericArray|N: ArrayLength|trait ArrayLength" generic-array/src
        //
        // Desired graphRAG traversal:
        //   GenericArray declaration
        //   -> generic parameter bound N: ArrayLength
        //   -> ArrayLength trait definition.
        assert_owner_reaches_target(
            &db,
            generic_array_id,
            array_length_id,
            TypeRelationKind::Trait,
            0,
            "GenericArray should reach ArrayLength through its N: ArrayLength declaration bound",
        )
    }

    #[test]
    fn chrono_backup_date_timezone_bound_reaches_timezone_trait() -> Result<(), DbError> {
        let db = setup_typed_backup_db(&CORPUS_CHRONO_TYPE_GRAPH)?;
        let date_id = struct_id_by_name(&db, "Date")?;
        let timezone_id = trait_id_by_name_in_module(&db, &["crate", "offset"], "TimeZone")?;

        // Source: `pub struct Date<Tz: TimeZone>`.
        //
        // Current search habit:
        //   rg -n "struct Date|Tz: TimeZone|trait TimeZone" chrono/src
        //
        // Desired graphRAG traversal:
        //   Date declaration
        //   -> generic parameter bound Tz: TimeZone
        //   -> TimeZone trait definition.
        assert_owner_reaches_target(
            &db,
            date_id,
            timezone_id,
            TypeRelationKind::Trait,
            0,
            "Date should reach TimeZone through its Tz: TimeZone declaration bound",
        )
    }

    #[test]
    fn chrono_backup_datetime_timezone_param_bound_source_reaches_timezone_trait()
    -> Result<(), DbError> {
        let db = setup_typed_backup_db(&CORPUS_CHRONO_TYPE_GRAPH)?;
        let datetime_id = struct_id_by_name(&db, "DateTime")?;
        let timezone_param_id = generic_type_param_id_by_owner_name(&db, datetime_id, "Tz")?;
        let timezone_id = trait_id_by_name_in_module(&db, &["crate", "offset"], "TimeZone")?;

        // Source: `pub struct DateTime<Tz: TimeZone>`.
        //
        // This variant anchors the same declaration-bound expectation at the
        // generic parameter node itself, not just at the containing struct.
        // That gives the graph both the GraphRAG-friendly containing owner and
        // the precise declaration-site owner.
        assert_owner_reaches_target(
            &db,
            timezone_param_id,
            timezone_id,
            TypeRelationKind::Trait,
            0,
            "DateTime::Tz should reach TimeZone through its generic parameter bound",
        )
    }
}

mod constraint_surfaces_red {
    use super::*;

    // Known limitation: KL-008 documents the constraint surfaces covered by
    // these strict red contracts. Keep these tests failing until the typed graph
    // emits real paths for projections and associated bounds.
    // See docs/design/known_limitations/KL-008-typed-type-graph-constraint-surfaces.md.

    #[test]
    fn generic_array_backup_const_array_length_projection_reaches_into_array_length_trait()
    -> Result<(), DbError> {
        let db = setup_typed_backup_db(&CORPUS_GENERIC_ARRAY_TYPE_GRAPH)?;
        let (alias_owner_id, _root_type_id) = type_alias_row_by_name(&db, "ConstArrayLength")?;
        let into_array_length_id = trait_id_by_name_in_module(&db, &["crate"], "IntoArrayLength")?;

        // Source: `pub type ConstArrayLength<const N: usize> =
        // <Const<N> as IntoArrayLength>::ArrayLength`.
        //
        // Current search habit:
        //   rg -n "ConstArrayLength|IntoArrayLength|ArrayLength" generic-array/src
        //
        // Desired graphRAG traversal:
        //   ConstArrayLength alias target projection
        //   -> `as IntoArrayLength` trait qualifier
        //   -> IntoArrayLength trait definition.
        //
        // This is expected to expose the current associated-type/projection gap.
        assert_owner_reaches_target(
            &db,
            alias_owner_id,
            into_array_length_id,
            TypeRelationKind::Trait,
            1,
            "ConstArrayLength should reach IntoArrayLength through its qualified associated type projection",
        )
    }

    #[test]
    fn chrono_backup_timezone_associated_offset_bound_reaches_offset_trait() -> Result<(), DbError>
    {
        let db = setup_typed_backup_db(&CORPUS_CHRONO_TYPE_GRAPH)?;
        let timezone_id = trait_id_by_name_in_module(&db, &["crate", "offset"], "TimeZone")?;
        let offset_id = trait_id_by_name_in_module(&db, &["crate", "offset"], "Offset")?;

        // Source: `pub trait TimeZone { type Offset: Offset; }`.
        //
        // Current search habit:
        //   rg -n "trait TimeZone|type Offset: Offset|trait Offset" chrono/src
        //
        // Desired graphRAG traversal:
        //   TimeZone trait declaration
        //   -> associated type bound type Offset: Offset
        //   -> Offset trait definition.
        //
        // This should fail until associated trait items and their bounds are
        // represented as type-use owners.
        assert_owner_reaches_target(
            &db,
            timezone_id,
            offset_id,
            TypeRelationKind::Trait,
            1,
            "TimeZone should reach Offset through its associated type bound",
        )
    }
}

fn generic_type_param_id_by_owner_name(
    db: &Database,
    owner_id: uuid::Uuid,
    name: &str,
) -> Result<uuid::Uuid, DbError> {
    exactly_one_uuid(
        db,
        &format!(
            r#"?[id] :=
                *generic_type {{
                    id,
                    owner_id: to_uuid("{owner_id}"),
                    name: "{name}" @ 'NOW'
                }}"#
        ),
        0,
    )
}
