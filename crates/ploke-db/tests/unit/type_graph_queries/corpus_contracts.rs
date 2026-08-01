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
//! Source-parse variants are mostly ignored because they parse and transform a
//! full GitHub fixture crate. `memchr_iter_return_reaches_iterator_struct` is a
//! small non-ignored source-parse counterweight; backup variants are ordinary
//! executable contracts over committed typed graph fixture databases.
//!
//! Coverage table:
//!
//! | Capability bucket | Real-corpus status | Example contract |
//! | --- | --- | --- |
//! | Function parameter roots | Passing | `matches_req(req: &VersionReq, ver: &Version)` reaches both model structs. |
//! | Function return roots | Passing | `memchr_iter(...) -> Memchr<'h>` reaches `Memchr`. |
//! | Field roots | Passing | `VersionReq.comparators: Vec<Comparator>` reaches nested `Comparator`. |
//! | Type alias target roots | Passing | `MappedLocalTime<T> = LocalResult<T>` reaches `LocalResult`; `ConstGenericArray` reaches `GenericArray`. |
//! | Method parameter roots | Passing | `VersionReq::matches(&self, version: &Version)` reaches `Version`. |
//! | Method return roots | Passing | `Parsed::to_datetime(...) -> ParseResult<DateTime<FixedOffset>>` reaches nested return targets. |
//! | Impl self roots | Passing | `impl GenericSequence<T> for GenericArray<T, N>` reaches `GenericArray`. |
//! | Impl trait roots | Passing | `impl GenericSequence<T> for GenericArray<T, N>` reaches `GenericSequence`. |
//! | Const/static roots | Passing | `MIN_DATE: NaiveDate` and `D_FMT: &[Item<'static>]` reach local targets. |
//! | Reference containment | Passing | `&VersionReq` / `&Version` traverses through the referenced child type. |
//! | Named generic-argument containment | Passing | `Vec<Comparator>` and `GenericArray<T, ConstArrayLength<N>>` expose nested named targets. |
//! | Array containment | Passing | `WeekdaySet::from_array(days: [Weekday; C])` reaches the array element enum. |
//! | Tuple containment | Passing in shared matrix | `WeekdaySet::split_at(...) -> (Self, Self)` reaches `WeekdaySet`; the older ignored Hyper source contract remains a broader future fixture. |
//! | Generic declaration bounds | Passing | `GenericArray<T, N: ArrayLength>` and `Date<Tz: TimeZone>` reach their trait bounds. |
//! | Generic-param-owned bound roots | Passing | `DateTime::Tz` reaches `TimeZone` from the generic parameter owner. |
//! | Where-clause owner bound roots | Passing | `SubsecRound for T where T: Timelike + ...` and `Concat where N: ArrayLength + Add<M>, M: ArrayLength, Sum<N, M>: ArrayLength` pin exact coordinates. |
//! | Where-clause repeated subjects | Passing | `FunctionalSequence::zip` has repeated `Rhs:` predicates that reach distinct local traits. |
//! | Where-clause nested bound terminals | Passing | `Date::format_with_items` reaches local `Item` through `B: Borrow<Item<'a>>`. |
//! | Where-clause composite subjects | Passing | `where GenericArray<U, N>: GenericSequence<...>` reaches `GenericArray` through the subject root. |
//! | Qualified associated type projections | Passing | `<Const<N> as IntoArrayLength>::ArrayLength` reaches `IntoArrayLength`. |
//! | Associated type bounds | Passing | `trait TimeZone { type Offset: Offset; }` reaches the `Offset` trait. |
//! | Trait super roots | Passing in shared matrix | `Concat: GenericSequence<T>` pins `TraitSuperSlot(0)`. |
//! | Slice/raw pointer/never vertices | Passing or no-target in shared matrix | `D_FMT: &[Item]`, memchr raw-pointer `Pointer::distance`, and `from_iter_length_fail() -> !`. |
//! | Function pointer vertices | Passing in shared matrix | `SearcherKindFn = unsafe fn(&Searcher, ...)` reaches `Searcher` through function pointer containment. |
//! | Trait object/impl trait vertices | Passing in shared matrix | Axum pins `Box<dyn ErasedIntoRoute>`, `Map.layer: Box<dyn LayerFn<...>>`, `MakeErasedHandler::clone_box -> Box<dyn ErasedIntoRoute<...>>`, `StripPrefix::layer -> impl Layer<...>`, and `zip_longest(...) -> impl Iterator<Item = Item<...>>`; generic-array pins `ArrayBuilder::extend(source: impl Iterator<Item = T>)`. |
//! | Never/macro/paren no-target vertices | No-target in shared matrix | `from_iter_length_fail() -> !`, axum's `Token![,]` nested macro type, and `&(dyn StdError + 'static)` nested paren assert retained structure without bogus terminal relations. |
//! | Inferred/unknown type vertices | Explicitly absent in current corpus matrix | Valid source-pinned backups currently have no rows for these fallback relations. |
//! | Union/type-alias/generic-param targets | Not covered | Real corpus contracts currently hit struct, enum, and trait targets. |
//! | Import path stress | Not covered | Need renamed import, glob import, re-export chain, `crate::`/`self::`/`super::`, and file-module boundary contracts. |
//! | RAG-facing expansion/ranking | Covered by shared matrix | Low-level reachability is asserted here and matrix rows marked for RAG assert structured `TypeContextInfo`. |
//!
//! These corpus contracts still deliberately stop at DB traversal. The shared
//! matrix is the cross-pipeline carrier: DB matrix tests assert exact roots and
//! terminals, RAG matrix tests assert structured `TypeContextInfo`, and the TUI
//! direct tool test asserts `request_code_context` emits that carrier in
//! `ConciseContext.type_context`.
//!
//! Recursive/nested coverage is example-bounded: each test follows the deepest
//! nesting present in the selected corpus expression, for example
//! `Borrow<Item<'a>>` and `GenericArray<U, N>`. The suite does not attempt to
//! exhaust the unbounded recursive grammar of Rust types.
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

use cozo::DataValue;
use ploke_db::{Database, DbError, TypeRelationKind, TypeUseCoordinate, TypeUseRole, to_uuid};
use ploke_test_utils::{
    CORPUS_CHRONO_TYPE_GRAPH, CORPUS_GENERIC_ARRAY_TYPE_GRAPH, CORPUS_MEMCHR_TYPE_GRAPH,
    CORPUS_SEMVER_TYPE_GRAPH,
};

use super::common::{
    TypePathDepth, assert_owner_reaches_target, assert_type_use_reaches_target,
    const_id_by_name_in_file_suffix, enum_id_by_name, exactly_one_uuid, field_id_by_owner_index,
    function_id_by_name_in_file_suffix, function_id_by_name_in_module,
    impl_id_by_trait_and_self_type_names, method_id_by_impl_self_type_name,
    method_id_by_impl_trait_and_self_type_names, setup_typed_backup_db, setup_typed_corpus_db,
    static_id_by_name_in_file_suffix, struct_id_by_name, struct_id_by_name_in_file_suffix,
    struct_id_by_name_in_module, trait_id_by_name_in_module, type_alias_row_by_name,
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

mod where_clause_bounds {
    use super::*;

    #[test]
    fn chrono_backup_subsec_round_impl_where_bound_reaches_timelike_trait() -> Result<(), DbError> {
        let db = setup_typed_backup_db(&CORPUS_CHRONO_TYPE_GRAPH)?;
        let impl_id = impl_id_by_trait_name_in_file_suffix(&db, "src/round.rs", "SubsecRound")?;
        let timelike_id = trait_id_by_name_in_module(&db, &["crate", "traits"], "Timelike")?;

        // Source: `impl<T> SubsecRound for T where
        // T: Timelike + Add<TimeDelta, Output = T> + Sub<TimeDelta, Output = T>`.
        //
        // We assert the local trait target only. `Add` and `Sub` are external
        // std/core traits and do not have local DB target nodes in this corpus.
        assert_type_use_reaches_target(
            &db,
            impl_id,
            TypeUseRole::WherePredicateBound,
            TypeUseCoordinate::WhereBoundSlot {
                predicate_index: 0,
                bound_index: 0,
            },
            timelike_id,
            TypeRelationKind::Trait,
            TypePathDepth::Exact(0),
            "SubsecRound blanket impl should reach Timelike through its first where bound",
        )
    }

    #[test]
    fn generic_array_backup_concat_impl_where_bounds_reach_array_length_trait()
    -> Result<(), DbError> {
        let db = setup_typed_backup_db(&CORPUS_GENERIC_ARRAY_TYPE_GRAPH)?;
        let impl_id = impl_id_by_trait_name_in_file_suffix(&db, "src/sequence.rs", "Concat")?;
        let array_length_id = trait_id_by_name_in_module(&db, &["crate"], "ArrayLength")?;

        // Source: `unsafe impl<T, N, M> Concat<T, M> for GenericArray<T, N>
        // where N: ArrayLength + Add<M>, M: ArrayLength, Sum<N, M>: ArrayLength`.
        //
        // This pins both a multi-bound predicate (`N`) and later predicates on
        // `M` and `Sum<N, M>`. `Add` is external, so the local `ArrayLength`
        // bound is the strict target for each asserted slot.
        for (predicate_index, bound_index) in [(0, 0), (1, 0), (2, 0)] {
            assert_type_use_reaches_target(
                &db,
                impl_id,
                TypeUseRole::WherePredicateBound,
                TypeUseCoordinate::WhereBoundSlot {
                    predicate_index,
                    bound_index,
                },
                array_length_id,
                TypeRelationKind::Trait,
                TypePathDepth::Exact(0),
                &format!(
                    "Concat impl should reach ArrayLength from where-bound coordinate ({predicate_index}, {bound_index})"
                ),
            )?;
        }

        Ok(())
    }

    #[test]
    fn chrono_backup_format_with_items_where_bound_reaches_nested_item_enum() -> Result<(), DbError>
    {
        let db = setup_typed_backup_db(&CORPUS_CHRONO_TYPE_GRAPH)?;
        let method_id = method_id_by_impl_self_type_name(&db, "Date", "format_with_items")?;
        let item_id = enum_id_by_name(&db, "Item")?;

        // Source: `B: Borrow<Item<'a>>`.
        //
        // `Borrow` is external, but the bound's generic argument is the local
        // `format::Item` enum. The where-bound root should still expose that
        // nested local terminal through containment.
        assert_type_use_reaches_target(
            &db,
            method_id,
            TypeUseRole::WherePredicateBound,
            TypeUseCoordinate::WhereBoundSlot {
                predicate_index: 1,
                bound_index: 0,
            },
            item_id,
            TypeRelationKind::Ordinary,
            TypePathDepth::Min(1),
            "Date::format_with_items should reach Item through B: Borrow<Item<'a>>",
        )
    }

    #[test]
    fn generic_array_backup_zip_repeated_rhs_where_bounds_reach_both_local_traits()
    -> Result<(), DbError> {
        let db = setup_typed_backup_db(&CORPUS_GENERIC_ARRAY_TYPE_GRAPH)?;
        let method_id = method_id_by_impl_trait_and_self_type_names(
            &db,
            "FunctionalSequence",
            "GenericArray",
            "zip",
        )?;
        let mapped_sequence_id =
            trait_id_by_name_in_module(&db, &["crate", "functional"], "MappedGenericSequence")?;
        let generic_sequence_id =
            trait_id_by_name_in_module(&db, &["crate", "sequence"], "GenericSequence")?;

        // Source:
        // `Rhs: MappedGenericSequence<B, U, Mapped = MappedSequence<Self, T, U>>`
        // `Rhs: GenericSequence<B, Length = Self::Length>`.
        //
        // These repeated-subject predicates should be independently reachable
        // as owner where-bound slots.
        assert_type_use_reaches_target(
            &db,
            method_id,
            TypeUseRole::WherePredicateBound,
            TypeUseCoordinate::WhereBoundSlot {
                predicate_index: 1,
                bound_index: 0,
            },
            mapped_sequence_id,
            TypeRelationKind::Trait,
            TypePathDepth::Exact(0),
            "FunctionalSequence::zip should reach MappedGenericSequence from the first Rhs where predicate",
        )?;
        assert_type_use_reaches_target(
            &db,
            method_id,
            TypeUseRole::WherePredicateBound,
            TypeUseCoordinate::WhereBoundSlot {
                predicate_index: 2,
                bound_index: 0,
            },
            generic_sequence_id,
            TypeRelationKind::Trait,
            TypePathDepth::Exact(0),
            "FunctionalSequence::zip should reach GenericSequence from the second Rhs where predicate",
        )
    }

    #[test]
    fn generic_array_backup_mapped_sequence_impl_composite_where_subject_reaches_generic_array()
    -> Result<(), DbError> {
        let db = setup_typed_backup_db(&CORPUS_GENERIC_ARRAY_TYPE_GRAPH)?;
        let impl_id =
            impl_id_by_trait_name_in_file_suffix(&db, "src/lib.rs", "MappedGenericSequence")?;
        let generic_array_id = struct_id_by_name(&db, "GenericArray")?;

        // Source: `impl<T, U, N: ArrayLength> MappedGenericSequence<T, U>
        // for GenericArray<T, N> where GenericArray<U, N>:
        // GenericSequence<U, Length = N>`.
        //
        // The bound trait is local too, but this assertion specifically pins
        // the composite where subject root so graph traversal can reach the
        // local `GenericArray` definition before following the bound.
        assert_type_use_reaches_target(
            &db,
            impl_id,
            TypeUseRole::WherePredicateSubject,
            TypeUseCoordinate::WhereSubjectSlot { predicate_index: 0 },
            generic_array_id,
            TypeRelationKind::Ordinary,
            TypePathDepth::Exact(0),
            "MappedGenericSequence impl should reach GenericArray through its composite where subject",
        )
    }
}

mod qualified_projections {
    use super::*;

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
        assert_owner_reaches_target(
            &db,
            alias_owner_id,
            into_array_length_id,
            TypeRelationKind::Trait,
            1,
            "ConstArrayLength should reach IntoArrayLength through its qualified associated type projection",
        )
    }
}

mod associated_type_bounds {
    use super::*;

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
        assert_owner_reaches_target(
            &db,
            timezone_id,
            offset_id,
            TypeRelationKind::Trait,
            0,
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

fn impl_id_by_trait_name_in_file_suffix(
    db: &Database,
    file_suffix: &str,
    trait_name: &str,
) -> Result<uuid::Uuid, DbError> {
    let rows = db.raw_query(&format!(
        r#"?[impl_id, file_path] :=
            *impl {{ id: impl_id @ 'NOW' }},
            *type_use {{
                owner_id: impl_id,
                root_type_id: trait_type_id,
                role: "ImplTrait" @ 'NOW'
            }},
            *type_relation {{
                source_id: trait_type_id,
                target_id: trait_target_id,
                relation_kind: "Trait" @ 'NOW'
            }},
            *trait {{ id: trait_target_id, name: "{trait_name}" @ 'NOW' }},
            *syntax_edge {{
                source_id: module_id,
                target_id: impl_id,
                relation_kind: "Contains" @ 'NOW'
            }},
            *file_mod {{ owner_id: module_id, file_path @ 'NOW' }}"#
    ))?;
    exactly_one_id_matching_file_suffix(rows.rows, "impl", trait_name, file_suffix)
}

fn exactly_one_id_matching_file_suffix(
    rows: Vec<Vec<DataValue>>,
    item_kind: &str,
    item_name: &str,
    file_suffix: &str,
) -> Result<uuid::Uuid, DbError> {
    let matching: Vec<_> = rows
        .iter()
        .filter_map(|row| {
            let file_path = match &row[1] {
                DataValue::Str(path) => path.as_str(),
                _ => return None,
            };
            file_path.ends_with(file_suffix).then(|| row[0].clone())
        })
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "expected exactly one {item_kind} for {item_name} in file suffix {file_suffix}; rows: {rows:#?}"
    );
    to_uuid(&matching[0])
}
