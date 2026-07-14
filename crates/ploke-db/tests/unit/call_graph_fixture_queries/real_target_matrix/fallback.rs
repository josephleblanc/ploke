use ploke_db::CallPathOptions;
use ploke_test_utils::{
    CORPUS_CHRONO_CALL_GRAPH, CORPUS_GENERIC_ARRAY_CALL_GRAPH, CORPUS_MEMCHR_CALL_GRAPH,
};
use uuid::Uuid;

use super::super::*;
use super::common::*;
use super::source_lines::{
    SourceLineFanout, assert_targetless_method_kind_line_fanout,
    assert_targetless_method_line_fanout,
};

#[test]
fn chrono_alias_constructor_rows_reach_local_result_single() -> Result<(), DbError> {
    let db = setup_call_graph_db(&CORPUS_CHRONO_CALL_GRAPH)?;

    // Matrix: `Fallback Source Oracle Matrix` in
    // docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md.
    //
    // Source chain:
    //   chrono/src/offset/mod.rs:77 aliases `MappedLocalTime<T> = LocalResult<T>`.
    //   chrono/src/offset/mod.rs:81-83 defines `LocalResult::Single(T)`.
    //   chrono/src/offset/mod.rs:{143,156,468,502,535} call
    //   `MappedLocalTime::Single(...)` through the alias.
    //   chrono/src/offset/{fixed.rs:135,138,utc.rs:122,125},
    //   chrono/src/offset/local/unix.rs:159, and
    //   chrono/src/datetime/tests.rs:{75,79} are additional
    //   fixture-projected direct constructor rows. The Unix row is visible in
    //   the regenerated fixture because bare `#[cfg(unix)]` is now evaluated
    //   as target-family cfg evidence.
    //
    // Expected traversal: every alias path call now reaches the underlying
    // `LocalResult::Single` enum variant through the existing type-alias
    // `TypeRelation::Ordinary` proof.
    let target = variant_id_by_enum_and_variant_names(&db, "LocalResult", "Single")?;
    assert_no_path_rows(&db, &["LocalResult", "Single"])?;

    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        12,
        "LocalResult::Single should expose all inspected alias constructor callers: {callers:#?}"
    );
    assert_sites_match_callers(
        &db,
        target,
        &callers,
        "chrono LocalResult::Single alias callers",
    )?;
    for caller in &callers {
        assert_eq!(caller.site.kind, CallSiteKind::Path);
        assert_eq!(
            caller.site.path.as_ref(),
            Some(&path(&["MappedLocalTime", "Single"]))
        );
        assert_eq!(caller.status.status, CallStatusKind::Resolved);
        assert_eq!(
            caller.status.resolution,
            Some(CallResolutionKind::LocalExact)
        );
        assert_eq!(
            caller.target.relation,
            CallRelationKind::EnumVariantConstructor
        );
        assert_eq!(caller.target.source_kind, CallSiteKind::Path);
        assert_eq!(caller.target.target_kind, CallTargetKind::Variant);
    }

    struct AliasCase {
        owner: &'static str,
        marker: &'static str,
        file_suffix: &'static str,
        label: &'static str,
        expected_count: usize,
    }

    let cases = [
        AliasCase {
            owner: "map",
            marker: "MappedLocalTime::Single(f(v))",
            file_suffix: "src/offset/mod.rs",
            label: "chrono/src/offset/mod.rs:143 MappedLocalTime::Single",
            expected_count: 1,
        },
        AliasCase {
            owner: "and_then",
            marker: "MappedLocalTime::Single(new)",
            file_suffix: "src/offset/mod.rs",
            label: "chrono/src/offset/mod.rs:156 MappedLocalTime::Single",
            expected_count: 1,
        },
        AliasCase {
            owner: "offset_from_local_date",
            marker: "MappedLocalTime::Single(*self)",
            file_suffix: "src/offset/fixed.rs",
            label: "chrono/src/offset/fixed.rs:135 MappedLocalTime::Single",
            expected_count: 1,
        },
        AliasCase {
            owner: "offset_from_local_datetime",
            marker: "MappedLocalTime::Single(*self)",
            file_suffix: "src/offset/fixed.rs",
            label: "chrono/src/offset/fixed.rs:138 MappedLocalTime::Single",
            expected_count: 1,
        },
        AliasCase {
            owner: "offset_from_local_date",
            marker: "MappedLocalTime::Single(Utc)",
            file_suffix: "src/offset/utc.rs",
            label: "chrono/src/offset/utc.rs:122 MappedLocalTime::Single",
            expected_count: 1,
        },
        AliasCase {
            owner: "offset_from_local_datetime",
            marker: "MappedLocalTime::Single(Utc)",
            file_suffix: "src/offset/utc.rs",
            label: "chrono/src/offset/utc.rs:125 MappedLocalTime::Single",
            expected_count: 1,
        },
        AliasCase {
            owner: "offset",
            marker: "MappedLocalTime::Single(offset)",
            file_suffix: "src/offset/local/unix.rs",
            label: "chrono/src/offset/local/unix.rs:159 MappedLocalTime::Single",
            expected_count: 1,
        },
        AliasCase {
            owner: "offset_from_local_datetime",
            marker: "Unexpected local time {local}",
            file_suffix: "src/datetime/tests.rs",
            label: "chrono/src/datetime/tests.rs:75 and :79 MappedLocalTime::Single",
            expected_count: 2,
        },
        AliasCase {
            owner: "timestamp_opt",
            marker: "MappedLocalTime::Single(self.from_utc_datetime(&dt.naive_utc()))",
            file_suffix: "src/offset/mod.rs",
            label: "chrono/src/offset/mod.rs:468 MappedLocalTime::Single",
            expected_count: 1,
        },
        AliasCase {
            owner: "timestamp_millis_opt",
            marker: "MappedLocalTime::Single(self.from_utc_datetime(&dt.naive_utc()))",
            file_suffix: "src/offset/mod.rs",
            label: "chrono/src/offset/mod.rs:502 MappedLocalTime::Single",
            expected_count: 1,
        },
        AliasCase {
            owner: "timestamp_micros",
            marker: "MappedLocalTime::Single(self.from_utc_datetime(&dt.naive_utc()))",
            file_suffix: "src/offset/mod.rs",
            label: "chrono/src/offset/mod.rs:535 MappedLocalTime::Single",
            expected_count: 1,
        },
    ];

    let mut checked_sites = 0usize;
    for case in cases {
        let owner =
            method_id_by_name_body_and_file_suffix(&db, case.owner, case.marker, case.file_suffix)?;
        let sites = assert_owner_path_resolved_count(
            &db,
            owner,
            &["MappedLocalTime", "Single"],
            target,
            CallRelationKind::EnumVariantConstructor,
            CallTargetKind::Variant,
            case.expected_count,
            case.label,
        )?;
        checked_sites += sites.len();
    }
    assert_eq!(
        checked_sites, 12,
        "source-oracle case table should cover every resolved alias constructor edge"
    );

    Ok(())
}

#[test]
fn chrono_try_receiver_method_rows_resolve_option_ok_or_oracles() -> Result<(), DbError> {
    let db = setup_call_graph_db(&CORPUS_CHRONO_CALL_GRAPH)?;

    // Matrix: `Fallback Source Oracle Matrix`.
    //
    // Source chain:
    //   chrono/src/datetime/mod.rs:563 defines `DateTime<Tz>::naive_utc`.
    //   chrono/src/format/parsed.rs:836 calls
    //   `DateTime::from_timestamp_secs(ts).ok_or(...)?.naive_utc()`.
    //   chrono/src/format/parsed.rs:953 calls
    //   `DateTime::from_timestamp(...).ok_or(...)?.naive_utc()`.
    //   chrono also exposes two cfg(test, feature = "clock") initialized
    //   `Local::now()` receiver rows to the same target.
    //
    // Expected traversal: the receiver is a `?` applied after `Option::ok_or`
    // on a local `DateTime::from_timestamp*` associated function returning
    // `Option<Self>`, so the outer method call reaches `DateTime::naive_utc`.
    let target = method_id_by_name_body_and_file_suffix(
        &db,
        "naive_utc",
        "self.datetime",
        "src/datetime/mod.rs",
    )?;

    struct TryCase {
        owner: &'static str,
        marker: &'static str,
        label: &'static str,
    }

    let cases = [
        TryCase {
            owner: "to_naive_datetime_with_offset",
            marker: "DateTime::from_timestamp_secs(ts).ok_or(OUT_OF_RANGE)?.naive_utc()",
            label: "chrono/src/format/parsed.rs:836 DateTime...?.naive_utc",
        },
        TryCase {
            owner: "to_datetime_with_timezone",
            marker: "DateTime::from_timestamp(timestamp, nanosecond).ok_or(OUT_OF_RANGE)?.naive_utc()",
            label: "chrono/src/format/parsed.rs:953 DateTime...?.naive_utc",
        },
    ];

    let receiver = CallReceiver::TryMethodCallResult {
        method_name: "ok_or".to_string(),
    };
    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        4,
        "DateTime::naive_utc should expose the two parsed.rs try-receiver callers plus two cfg(test) Local::now initialized-local callers: {callers:#?}"
    );
    assert_sites_match_callers(
        &db,
        target,
        &callers,
        "chrono DateTime::naive_utc try callers",
    )?;
    assert_eq!(
        callers
            .iter()
            .filter(|caller| caller.site.receiver.as_ref() == Some(&receiver))
            .count(),
        2,
        "DateTime::naive_utc should retain exactly the two parsed.rs ok_or try-receiver rows: {callers:#?}"
    );
    assert_eq!(
        callers
            .iter()
            .filter(|caller| {
                caller.site.receiver.as_ref()
                    == Some(&CallReceiver::InitializedLocalBinding {
                        name: "now".to_string(),
                        init_path: path(&["Local", "now"]),
                    })
            })
            .count(),
        2,
        "DateTime::naive_utc should retain exactly the two cfg(test) Local::now initialized-local rows: {callers:#?}"
    );

    for case in cases {
        let owner = method_id_by_name_body_and_file_suffix(
            &db,
            case.owner,
            case.marker,
            "src/format/parsed.rs",
        )?;
        let context = db.call_context_for_owner(owner)?;
        let rows = context
            .iter()
            .filter(|row| {
                row.site.kind == CallSiteKind::Method
                    && row.site.method.as_deref() == Some("naive_utc")
                    && row.site.receiver.as_ref() == Some(&receiver)
            })
            .collect::<Vec<_>>();
        assert_eq!(
            rows.len(),
            1,
            "{} should expose exactly one resolved try-receiver row: {context:#?}",
            case.label
        );
        let row = rows[0];
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Method,
            CallSiteKind::Method,
            CallTargetKind::Method,
        );
        assert_eq!(
            relations_for_site(&db, row.site.id)?.rows.len(),
            1,
            "{} should preserve exactly one raw call_relation edge",
            case.label
        );
        assert_one_edge_traversal(
            &db,
            TraversalExpectation {
                label: case.label,
                owner,
                target,
                site_id: row.site.id,
                expected_edge_count: 1,
            },
        )?;
    }

    Ok(())
}

#[test]
fn chrono_guarded_match_arm_slice_method_guard_is_external_frontier() -> Result<(), DbError> {
    let db = setup_call_graph_db(&CORPUS_CHRONO_CALL_GRAPH)?;

    // Matrix: `Fallback Source Oracle Matrix`.
    //
    // Source chain:
    //   chrono/src/format/strftime.rs:193-198 defines `StrftimeItems::queue`.
    //   chrono/src/format/strftime.rs:635 guards a match arm with
    //   `self.queue.is_empty()`.
    //
    // Expected traversal: the named self-field type is source-visible as
    // `&'static [Item<'static>]`, so `is_empty` is classified as an external
    // slice frontier. No local traversal edge is fabricated.
    assert_targetless_method_rows(
        &db,
        "is_empty",
        "SelfField",
        Some(&["queue"]),
        CallStatusKind::External,
        1,
    )?;
    assert_targetless_method_line_fanout(
        &db,
        &CORPUS_CHRONO_CALL_GRAPH,
        "is_empty",
        "SelfField",
        Some(&["queue"]),
        CallStatusKind::External,
        &[SourceLineFanout {
            file_suffix: "src/format/strftime.rs",
            lines: &[635],
        }],
    )?;

    let owner = method_id_by_name_body_and_file_suffix(
        &db,
        "parse_next_item",
        "self.queue.is_empty()",
        "src/format/strftime.rs",
    )?;
    assert_owner_method_targetless(
        &db,
        owner,
        "is_empty",
        &CallReceiver::SelfField {
            path: vec!["queue".to_string()],
        },
        CallStatusKind::External,
        "chrono/src/format/strftime.rs:635 self.queue.is_empty",
    )?;

    Ok(())
}

#[test]
fn memchr_function_pointer_field_calls_preserve_ambiguous_candidates() -> Result<(), DbError> {
    let db = setup_call_graph_db(&CORPUS_MEMCHR_CALL_GRAPH)?;

    // Matrix: `Fallback Source Oracle Matrix`.
    //
    // Source chain:
    //   memchr/src/memmem/searcher.rs:33-35 defines `Searcher.call`.
    //   memchr/src/memmem/searcher.rs:222 calls
    //   `(self.call)(self, prestate, haystack, needle)`.
    //   memchr/src/memmem/searcher.rs:604-605 defines `Prefilter.call`.
    //   memchr/src/memmem/searcher.rs:718 calls `(self.call)(self, haystack)`.
    //
    // Current model: both function-pointer field calls are structural dynamic
    // rows owned by methods named `find`. Local initializer proof can bound the
    // cfg-visible helper function candidates, so the rows are ambiguous rather
    // than targetless. Missing cfg-gated architecture helpers are not invented
    // as candidates for this fixture.
    let searcher_candidates = function_ids_by_names(
        &db,
        &[
            "searcher_kind_empty",
            "searcher_kind_one_byte",
            "searcher_kind_two_way",
            "searcher_kind_two_way_with_prefilter",
            "searcher_kind_sse2",
            "searcher_kind_avx2",
        ],
    )?;
    let prefilter_candidates = function_ids_by_names(
        &db,
        &[
            "prefilter_kind_fallback",
            "prefilter_kind_sse2",
            "prefilter_kind_avx2",
        ],
    )?;

    let searcher_find = method_id_by_name_body_and_file_suffix(
        &db,
        "find",
        "(self.call)(self, prestate, haystack, needle)",
        "src/memmem/searcher.rs",
    )?;
    let searcher_site = assert_owner_dynamic_function_candidates(
        &db,
        searcher_find,
        &searcher_candidates,
        4,
        "memchr/src/memmem/searcher.rs:222 Searcher.call",
    )?;

    let prefilter_find = method_id_by_name_body_and_file_suffix(
        &db,
        "find",
        "(self.call)(self, haystack)",
        "src/memmem/searcher.rs",
    )?;
    let prefilter_site = assert_owner_dynamic_function_candidates(
        &db,
        prefilter_find,
        &prefilter_candidates,
        2,
        "memchr/src/memmem/searcher.rs:718 Prefilter.call",
    )?;

    for (owner, site, expected, label) in [
        (
            searcher_find,
            searcher_site,
            searcher_candidates.as_slice(),
            "memchr/src/memmem/searcher.rs:222 Searcher.call",
        ),
        (
            prefilter_find,
            prefilter_site,
            prefilter_candidates.as_slice(),
            "memchr/src/memmem/searcher.rs:718 Prefilter.call",
        ),
    ] {
        let site = site.to_string();
        let expected_names = candidate_strings(expected);
        let facts = db.call_proof_facts_for_owner(owner, "bd:corpus-memchr-call-graph")?;
        let site_facts = facts
            .into_iter()
            .filter(|fact| {
                fact.get("call_site_id").and_then(serde_json::Value::as_str) == Some(site.as_str())
            })
            .collect::<Vec<_>>();
        assert_candidate_proof(&site_facts, &site, &expected_names, label);

        let projected =
            db.project_call_proof_facts_for_owner(owner, "bd:corpus-memchr-call-graph")?;
        assert!(
            projected >= 2,
            "{label} should project at least call_site and call_resolution proof facts"
        );
        assert_candidate_blocker(&db, &site, label)?;
    }

    Ok(())
}

#[test]
fn memchr_arbitrary_expression_dynamic_callee_projects_generated_rows() -> Result<(), DbError> {
    let db = setup_call_graph_db(&CORPUS_MEMCHR_CALL_GRAPH)?;

    // Matrix: `Fallback Source Oracle Matrix`.
    //
    // Source chain:
    //   memchr/src/arch/x86_64/memchr.rs:72-74 defines `Fn`, `RealFn`, and `FN`.
    //   memchr/src/arch/x86_64/memchr.rs:153 invokes
    //   `core::mem::transmute::<Fn, RealFn>(fun)(...)`.
    //   macro invocations at lines 180, 203, 227, 252, 278, 305, and 326
    //   instantiate that arbitrary-expression dynamic callee.
    //
    // Expected traversal: the bounded source-oracle projection records the
    // generated inner path call and outer arbitrary-expression dynamic call,
    // but both remain targetless. The inner path is a `core` external frontier;
    // the outer returned-path dynamic call inherits that external frontier and
    // does not fabricate a concrete function-pointer target.
    assert_targetless_path_rows(
        &db,
        &["core", "mem", "transmute"],
        CallStatusKind::External,
        7,
    )?;
    assert_memchr_ifunc_owner(&db, "memchr_raw", "unsafe_ifunc!", 3, "memchr_raw")?;
    assert_memchr_ifunc_owner(&db, "memrchr_raw", "unsafe_ifunc!", 3, "memrchr_raw")?;
    assert_memchr_ifunc_owner(&db, "memchr2_raw", "unsafe_ifunc!", 4, "memchr2_raw")?;
    assert_memchr_ifunc_owner(&db, "memrchr2_raw", "unsafe_ifunc!", 4, "memrchr2_raw")?;
    assert_memchr_ifunc_owner(&db, "memchr3_raw", "unsafe_ifunc!", 5, "memchr3_raw")?;
    assert_memchr_ifunc_owner(&db, "memrchr3_raw", "unsafe_ifunc!", 5, "memrchr3_raw")?;
    assert_memchr_ifunc_owner(&db, "count_raw", "unsafe_ifunc!", 3, "count_raw")?;

    Ok(())
}

#[test]
fn memchr_callable_trait_object_field_calls_are_visible_targetless_path_rows() -> Result<(), DbError>
{
    let db = setup_call_graph_db(&CORPUS_MEMCHR_CALL_GRAPH)?;

    // Matrix: `Fallback Source Oracle Matrix`.
    //
    // Source chain:
    //   memchr/src/tests/substring/mod.rs:67-71 defines boxed `fwd`/`rev`
    //   `dyn FnMut` fields.
    //   memchr/src/tests/substring/mod.rs:94 calls
    //   `fwd(t.haystack.as_bytes(), t.needle.as_bytes())`.
    //   memchr/src/tests/substring/mod.rs:110 calls
    //   `rev(t.haystack.as_bytes(), t.needle.as_bytes())`.
    //   setters at lines 137 and 153 bind the closures into those fields.
    //
    // Current model gap: callable trait-object local bindings are visible as
    // targetless path rows, but boxed `dyn FnMut` dispatch is not resolved and
    // must not fabricate call edges to the setter closures.
    assert_no_dynamic_rows_by_method_name(&db, "run")?;
    let owner = method_id_by_name_body_and_file_suffix(
        &db,
        "run",
        "fwd(t.haystack.as_bytes(), t.needle.as_bytes())",
        "src/tests/substring/mod.rs",
    )?;
    assert_no_owner_dynamic_rows(
        &db,
        owner,
        "memchr/src/tests/substring/mod.rs:94 and :110 boxed dyn FnMut calls",
    )?;
    let fwd_site = assert_owner_path_targetless(
        &db,
        owner,
        &["fwd"],
        CallStatusKind::Unsupported,
        "memchr/src/tests/substring/mod.rs:94 boxed fwd dyn FnMut local binding",
    )?;
    let rev_site = assert_owner_path_targetless(
        &db,
        owner,
        &["rev"],
        CallStatusKind::Unsupported,
        "memchr/src/tests/substring/mod.rs:110 boxed rev dyn FnMut local binding",
    )?;
    db.upsert_proof_fact_values(&[
        ploke_test_utils::memchr_callable_trait_object_runtime_dispatch_blocker(fwd_site),
        ploke_test_utils::memchr_callable_trait_object_runtime_dispatch_blocker(rev_site),
    ])?;

    let needs = db.runtime_dispatch_needs_for_owner(
        owner,
        CallPathOptions {
            max_depth: 1,
            max_paths: 16,
        },
    )?;
    for (site, label) in [
        (
            fwd_site,
            "memchr/src/tests/substring/mod.rs:94 boxed fwd dyn FnMut local binding",
        ),
        (
            rev_site,
            "memchr/src/tests/substring/mod.rs:110 boxed rev dyn FnMut local binding",
        ),
    ] {
        let need = needs
            .iter()
            .find(|need| need.call_site.site.id == site)
            .unwrap_or_else(|| {
                panic!(
                    "{label} should be listed as an owner-scoped runtime-dispatch need: {needs:#?}"
                )
            });
        assert!(
            need.paths_to_owner.is_empty(),
            "{label} is a direct boxed dyn FnMut frontier and should not need an intermediate path: {need:#?}"
        );
        assert!(
            need.blocker_reasons
                .iter()
                .any(|reason| reason == "dynamic_dispatch_unbounded"),
            "{label} should retain the dynamic dispatch blocker: {need:#?}"
        );
        assert!(
            relations_for_site(&db, site)?.rows.is_empty(),
            "{label} runtime-dispatch proof queue must not fabricate a boxed dyn FnMut edge"
        );
    }

    db.upsert_proof_fact_values(&[
        ploke_test_utils::memchr_callable_trait_object_runtime_dispatch_summary(fwd_site),
        ploke_test_utils::memchr_callable_trait_object_runtime_dispatch_summary(rev_site),
    ])?;
    let after = db.runtime_dispatch_needs_for_owner(
        owner,
        CallPathOptions {
            max_depth: 1,
            max_paths: 16,
        },
    )?;
    for (site, label) in [
        (
            fwd_site,
            "memchr/src/tests/substring/mod.rs:94 boxed fwd dyn FnMut local binding",
        ),
        (
            rev_site,
            "memchr/src/tests/substring/mod.rs:110 boxed rev dyn FnMut local binding",
        ),
    ] {
        assert!(
            after.iter().all(|need| need.call_site.site.id != site),
            "{label} admitted runtime-dispatch summary should discharge the proof-authoring need: {after:#?}"
        );
        assert!(
            relations_for_site(&db, site)?.rows.is_empty(),
            "{label} admitted runtime-dispatch summary must not fabricate a boxed dyn FnMut edge"
        );
    }

    let projected = db.project_call_proof_facts_for_owner(owner, "bd:corpus-memchr-call-graph")?;
    assert!(
        projected >= 4,
        "memchr boxed dyn FnMut owner should project call_site and call_resolution rows for fwd/rev"
    );
    let blockers = db.proof_blockers()?;
    for (site, label) in [
        (
            fwd_site,
            "memchr/src/tests/substring/mod.rs:94 boxed fwd dyn FnMut local binding",
        ),
        (
            rev_site,
            "memchr/src/tests/substring/mod.rs:110 boxed rev dyn FnMut local binding",
        ),
    ] {
        assert_blocked_resolution_proof(
            &db,
            site,
            "type_resolution_missing",
            "src/tests/substring/mod.rs",
            label,
        )?;
        let site = site.to_string();
        assert!(
            blockers.iter().any(|proof| {
                proof.call_site_id.as_deref() == Some(site.as_str())
                    && proof.reason == "dynamic_dispatch_unbounded"
                    && proof.status == "blocked"
            }),
            "{label} should expose an explicit dynamic dispatch proof blocker: {blockers:#?}"
        );
    }
    let proof_rows = db.proof_graphrag_context("boxed dyn FnMut dispatch")?;
    for (site, label) in [
        (
            fwd_site,
            "memchr/src/tests/substring/mod.rs:94 boxed fwd dyn FnMut local binding",
        ),
        (
            rev_site,
            "memchr/src/tests/substring/mod.rs:110 boxed rev dyn FnMut local binding",
        ),
    ] {
        let site = site.to_string();
        assert!(
            proof_rows.iter().any(|proof| {
                proof.kind == "proof_blocker"
                    && proof.call_site_id.as_deref() == Some(site.as_str())
                    && proof.blocker_reason.as_deref() == Some("dynamic_dispatch_unbounded")
                    && proof.status.as_deref() == Some("blocked")
            }),
            "{label} should be retrievable as explicit boxed dyn FnMut dispatch proof context: {proof_rows:#?}"
        );
    }

    Ok(())
}

#[test]
fn generic_array_guarded_match_arm_method_guard_is_external_frontier() -> Result<(), DbError> {
    let db = setup_call_graph_db(&CORPUS_GENERIC_ARRAY_CALL_GRAPH)?;

    // Matrix: `Fallback Source Oracle Matrix`.
    //
    // Source chain:
    //   generic-array/src/lib.rs:1239 calls `iter.size_hint()`, with guarded
    //   tuple arms at lines 1241 and 1243.
    //   generic-array/src/lib.rs:1276 repeats the same guard shape, with arms
    //   at lines 1278 and 1280.
    //
    // Expected contract: the guarded receiver rows are visible and targetless,
    // but no longer unsupported. The local binding proof ties `iter` to the
    // `iter.into_iter()` initializer, and the owner where-clause proves the
    // receiver is an external `IntoIterator` result.
    let sites = generic_array_size_hint_sites(&db)?;
    let site_ids = sites.iter().map(|(_, site)| *site).collect::<Vec<_>>();
    assert_targetless_method_kind_line_fanout(
        &db,
        &CORPUS_GENERIC_ARRAY_CALL_GRAPH,
        "size_hint",
        "MethodResultLocalBinding",
        CallStatusKind::External,
        &[SourceLineFanout {
            file_suffix: "src/lib.rs",
            lines: &[1239, 1276],
        }],
    )?;

    for (owner_id, _) in &sites {
        db.project_call_proof_facts_for_owner(*owner_id, "bd:corpus-generic-array-call-graph")?;
    }

    let proof_rows = db.proof_graphrag_context("external_dependency_summary_missing")?;
    for site_id in site_ids {
        let site = site_id.to_string();
        assert!(
            proof_rows.iter().any(|proof| {
                proof.kind == "call_resolution"
                    && proof.call_site_id.as_deref() == Some(site.as_str())
                    && proof.resolution_state.as_deref() == Some("blocked")
                    && proof.blocker_reason.as_deref()
                        == Some("external_dependency_summary_missing")
            }),
            "generic-array guarded size_hint site {site} should expose the external frontier through proof context: {proof_rows:#?}"
        );
    }

    Ok(())
}

fn generic_array_size_hint_sites(db: &Database) -> Result<Vec<(Uuid, Uuid)>, DbError> {
    let mut params = std::collections::BTreeMap::new();
    params.insert("method".to_string(), cozo::DataValue::from("size_hint"));
    params.insert("status".to_string(), cozo::DataValue::from("External"));

    let rows = db.raw_query_params(
        r#"?[site_id, owner_id, resolution_kind] :=
            *call_site {
                id: site_id,
                owner_id,
                call_kind: "Method",
                method_name: $method @ 'NOW'
            },
            *call_resolution_status {
                source_id: site_id,
                source_kind: "Method",
                status_kind: $status,
                resolution_kind @ 'NOW'
            }"#,
        params,
    )?;
    assert_eq!(
        rows.rows.len(),
        2,
        "expected two external generic-array size_hint rows: {:#?}",
        rows.rows
    );

    let mut sites = Vec::new();
    let mut blocked = Vec::new();
    for row in &rows.rows {
        assert_eq!(row[2], cozo::DataValue::Null);
        let site_id = to_uuid(&row[0])?;
        let owner = to_uuid(&row[1])?;
        assert!(
            relations_for_site(db, site_id)?.rows.is_empty(),
            "generic-array size_hint row should not have call_relation targets"
        );
        sites.push((owner, site_id));
        blocked.push((owner, site_id));
    }
    assert_no_traversal_candidates_for_sites(db, &blocked, "generic-array size_hint guard rows")?;
    Ok(sites)
}

fn assert_owner_path_resolved_count(
    db: &Database,
    owner: Uuid,
    path_parts: &[&str],
    target: Uuid,
    relation: CallRelationKind,
    target_kind: CallTargetKind,
    expected_count: usize,
    label: &str,
) -> Result<Vec<Uuid>, DbError> {
    let context = db.call_context_for_owner(owner)?;
    let rows = context
        .iter()
        .filter(|row| {
            row.site.kind == CallSiteKind::Path && row.site.path.as_ref() == Some(&path(path_parts))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        rows.len(),
        expected_count,
        "{label} should expose exactly {expected_count} resolved path row(s): {context:#?}"
    );

    let mut sites = Vec::new();
    for row in rows {
        assert_resolved_target(row, target, relation, CallSiteKind::Path, target_kind);
        assert_eq!(
            relations_for_site(db, row.site.id)?.rows.len(),
            1,
            "{label} should preserve exactly one raw call_relation edge per alias constructor site"
        );
        sites.push(row.site.id);
    }

    let outgoing = db.expand_call_context(
        CallContextSeed::Owner(owner),
        CallContextOptions {
            include_incoming_callers: false,
            max_candidates: 512,
            ..CallContextOptions::default()
        },
    )?;
    assert!(
        outgoing.iter().any(|candidate| {
            candidate.node_id == target
                && candidate.relation == ploke_db::CallContextRelation::OutgoingTarget
                && candidate.target_id == target
                && candidate.distance == 1
                && sites.contains(&candidate.call_site_id)
        }),
        "{label} should expose owner-to-target reachability for at least one matching alias constructor site: {outgoing:#?}"
    );

    let incoming = db.expand_call_context(
        CallContextSeed::Target(target),
        CallContextOptions {
            include_outgoing_targets: false,
            max_candidates: 512,
            ..CallContextOptions::default()
        },
    )?;
    assert!(
        incoming.iter().any(|candidate| {
            candidate.node_id == owner
                && candidate.relation == ploke_db::CallContextRelation::IncomingCaller
                && candidate.target_id == target
                && candidate.distance == 1
                && sites.contains(&candidate.call_site_id)
        }),
        "{label} should expose target-to-owner reachability for at least one matching alias constructor site: {incoming:#?}"
    );

    Ok(sites)
}

fn function_ids_by_names(db: &Database, names: &[&str]) -> Result<Vec<Uuid>, DbError> {
    let mut ids = names
        .iter()
        .map(|name| function_id_by_name(db, name))
        .collect::<Result<Vec<_>, _>>()?;
    ids.sort_unstable();
    Ok(ids)
}

fn assert_owner_dynamic_function_candidates(
    db: &Database,
    owner: Uuid,
    expected: &[Uuid],
    expected_arg_count: u32,
    label: &str,
) -> Result<Uuid, DbError> {
    let context = db.call_context_for_owner(owner)?;
    let rows = context
        .iter()
        .filter(|row| row.site.kind == CallSiteKind::Dynamic)
        .collect::<Vec<_>>();
    assert_eq!(
        rows.len(),
        1,
        "{label} should project exactly one dynamic row: {context:#?}"
    );
    let row = rows[0];
    assert_dynamic_path_function_candidates_with_args(
        row,
        owner,
        &["self", "call"],
        expected_arg_count,
        expected,
        label,
    );
    assert_eq!(
        relations_for_site(db, row.site.id)?.rows.len(),
        expected.len(),
        "{label} should store one DynamicFunction relation for each candidate"
    );

    Ok(row.site.id)
}

fn assert_memchr_ifunc_owner(
    db: &Database,
    owner_name: &str,
    body_marker: &str,
    expected_arg_count: u32,
    label: &str,
) -> Result<Uuid, DbError> {
    let owner =
        function_id_by_name_in_module(db, &["crate", "arch", "x86_64", "memchr"], owner_name)?;
    let context = db.call_context_for_owner(owner)?;
    assert!(
        context.iter().any(|row| {
            row.site.kind == CallSiteKind::Macro
                && row.site.macro_name.as_deref() == Some("unsafe_ifunc")
        }),
        "{label} should keep the source macro invocation visible: {context:#?}"
    );

    let path_row = context
        .iter()
        .find(|row| {
            row.site.kind == CallSiteKind::Path
                && row.site.path.as_deref()
                    == Some(&["core", "mem", "transmute"].map(str::to_string))
        })
        .unwrap_or_else(|| {
            panic!("{label} should include generated transmute path row: {context:#?}")
        });
    assert_eq!(
        path_row.site.arg_count,
        Some(1),
        "{label} generated transmute path row should preserve the `fun` argument"
    );
    assert_eq!(
        path_row.site.generic_arg_count,
        Some(2),
        "{label} generated transmute path row should preserve `<Fn, RealFn>`"
    );
    assert_eq!(
        path_row.site.unsafe_block, true,
        "{label} generated transmute path row should preserve macro unsafe block"
    );
    assert_targetless_status(path_row, CallStatusKind::External);
    assert!(
        relations_for_site(db, path_row.site.id)?.rows.is_empty(),
        "{label} generated transmute path row should not have call_relation targets"
    );

    let dynamic_row = context
        .iter()
        .find(|row| {
            row.site.kind == CallSiteKind::Dynamic
                && row.site.path.as_deref() == Some(&["core", "mem", "transmute"].map(str::to_string))
                && row.site.arg_count == Some(expected_arg_count)
        })
        .unwrap_or_else(|| {
            panic!(
                "{label} should include generated returned-path dynamic row with {expected_arg_count} args: {context:#?}"
            )
        });
    assert_eq!(
        dynamic_row.site.unsafe_block, true,
        "{label} generated dynamic row should preserve macro unsafe block"
    );
    assert_targetless_status(dynamic_row, CallStatusKind::External);
    assert!(
        relations_for_site(db, dynamic_row.site.id)?.rows.is_empty(),
        "{label} generated dynamic row should not have call_relation targets"
    );
    assert_no_traversal_candidates_for_site(db, owner, dynamic_row.site.id, label)?;
    assert!(
        context
            .iter()
            .any(|row| row.site.kind == CallSiteKind::Macro
                && row.site.macro_name.as_deref() == Some(body_marker.trim_end_matches('!'))),
        "{label} macro row should match body marker {body_marker:?}: {context:#?}"
    );

    Ok(dynamic_row.site.id)
}

fn assert_blocked_resolution_proof(
    db: &Database,
    site_id: Uuid,
    blocker_reason: &str,
    source_suffix: &str,
    label: &str,
) -> Result<(), DbError> {
    let site = site_id.to_string();
    let rows = db.proof_graphrag_context(blocker_reason)?;
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_resolution"
                && row.call_site_id.as_deref() == Some(site.as_str())
                && row.resolution_state.as_deref() == Some("blocked")
                && row.blocker_reason.as_deref() == Some(blocker_reason)
        }),
        "{label} should project a blocked {blocker_reason} call_resolution proof row: {rows:#?}"
    );

    let provenance = db
        .proof_source_provenance(&site)?
        .unwrap_or_else(|| panic!("{label} should project source provenance for {site}"));
    assert!(
        provenance.source_file.ends_with(source_suffix),
        "{label} proof source provenance should point at {source_suffix}: {provenance:#?}"
    );

    Ok(())
}

fn assert_no_owner_dynamic_rows(
    db: &Database,
    owner: uuid::Uuid,
    label: &str,
) -> Result<(), DbError> {
    let context = db.call_context_for_owner(owner)?;
    assert!(
        context
            .iter()
            .all(|row| row.site.kind != CallSiteKind::Dynamic),
        "{label} should not project dynamic call rows before callable trait-object receiver support: {context:#?}"
    );

    Ok(())
}
