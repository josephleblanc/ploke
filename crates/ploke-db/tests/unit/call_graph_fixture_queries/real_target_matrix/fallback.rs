use ploke_test_utils::{
    CORPUS_CHRONO_CALL_GRAPH, CORPUS_GENERIC_ARRAY_CALL_GRAPH, CORPUS_MEMCHR_CALL_GRAPH,
};
use uuid::Uuid;

use super::super::*;
use super::common::*;
use super::source_lines::{
    SourceLineFanout, assert_targetless_dynamic_line_fanout_by_method,
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
fn chrono_try_receiver_method_rows_are_targetless_fallback_oracles() -> Result<(), DbError> {
    let db = setup_call_graph_db(&CORPUS_CHRONO_CALL_GRAPH)?;

    // Matrix: `Fallback Source Oracle Matrix`.
    //
    // Source chain:
    //   chrono/src/datetime/mod.rs:563 defines `DateTime<Tz>::naive_utc`.
    //   chrono/src/format/parsed.rs:836 calls
    //   `DateTime::from_timestamp_secs(ts).ok_or(...)?.naive_utc()`.
    //   chrono/src/format/parsed.rs:953 calls
    //   `DateTime::from_timestamp(...).ok_or(...)?.naive_utc()`.
    //
    // Current model gap: the preceding constructor paths resolve separately,
    // but the `?` receiver is represented as a targetless `TryResult` method
    // call with no traversal edge to `DateTime::naive_utc`.
    assert_targetless_method_rows(
        &db,
        "naive_utc",
        "TryResult",
        None,
        CallStatusKind::Unsupported,
        2,
    )?;
    assert_targetless_method_line_fanout(
        &db,
        &CORPUS_CHRONO_CALL_GRAPH,
        "naive_utc",
        "TryResult",
        None,
        CallStatusKind::Unsupported,
        &[SourceLineFanout {
            file_suffix: "src/format/parsed.rs",
            lines: &[836, 953],
        }],
    )?;
    let target = method_id_by_name_body_and_file_suffix(
        &db,
        "naive_utc",
        "self.datetime",
        "src/datetime/mod.rs",
    )?;
    assert_no_incoming_traversal_to_target(
        &db,
        target,
        "chrono/src/format/parsed.rs try receiver naive_utc rows",
    )?;

    struct TryCase {
        owner: &'static str,
        marker: &'static str,
        line: u32,
    }

    let cases = [
        TryCase {
            owner: "to_naive_datetime_with_offset",
            marker: "DateTime::from_timestamp_secs(ts).ok_or(OUT_OF_RANGE)?.naive_utc()",
            line: 836,
        },
        TryCase {
            owner: "to_datetime_with_timezone",
            marker: "DateTime::from_timestamp(timestamp, nanosecond).ok_or(OUT_OF_RANGE)?.naive_utc()",
            line: 953,
        },
    ];

    for case in cases {
        let owner = method_id_by_name_body_and_file_suffix(
            &db,
            case.owner,
            case.marker,
            "src/format/parsed.rs",
        )?;
        assert_owner_method_targetless(
            &db,
            owner,
            "naive_utc",
            &CallReceiver::TryResult,
            CallStatusKind::Unsupported,
            &format!(
                "chrono/src/format/parsed.rs:{} DateTime...?.naive_utc",
                case.line
            ),
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
fn memchr_function_pointer_field_calls_are_dynamic_targetless_oracles() -> Result<(), DbError> {
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
    // rows owned by methods named `find`, with no local target edge.
    assert_targetless_dynamic_rows_by_method_name(&db, "find", &[4, 2])?;
    assert_targetless_dynamic_line_fanout_by_method(
        &db,
        &CORPUS_MEMCHR_CALL_GRAPH,
        "find",
        CallStatusKind::Unsupported,
        &[SourceLineFanout {
            file_suffix: "src/memmem/searcher.rs",
            lines: &[222, 718],
        }],
    )?;

    let searcher_find = method_id_by_name_body_and_file_suffix(
        &db,
        "find",
        "(self.call)(self, prestate, haystack, needle)",
        "src/memmem/searcher.rs",
    )?;
    assert_owner_dynamic_targetless(
        &db,
        searcher_find,
        4,
        "memchr/src/memmem/searcher.rs:222 Searcher.call",
    )?;

    let prefilter_find = method_id_by_name_body_and_file_suffix(
        &db,
        "find",
        "(self.call)(self, haystack)",
        "src/memmem/searcher.rs",
    )?;
    assert_owner_dynamic_targetless(
        &db,
        prefilter_find,
        2,
        "memchr/src/memmem/searcher.rs:718 Prefilter.call",
    )?;

    Ok(())
}

#[test]
fn memchr_arbitrary_expression_dynamic_callee_is_absent_fallback_gap() -> Result<(), DbError> {
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
    // Current model gap: the transmute path and outer arbitrary-expression call
    // are not projected, so the fixture must not invent a callee.
    assert_no_path_rows(&db, &["core", "mem", "transmute"])?;
    assert_no_dynamic_rows_by_function_names(&db, &["find_raw", "rfind_raw", "count_raw"])
}

#[test]
fn memchr_callable_trait_object_field_calls_are_absent_fallback_gap() -> Result<(), DbError> {
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
    // Current model gap: callable trait-object local bindings are not projected
    // as dynamic call rows under `Runner::run`.
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
    )
}

#[test]
fn generic_array_guarded_match_arm_method_guard_is_targetless_fallback_oracle()
-> Result<(), DbError> {
    let db = setup_call_graph_db(&CORPUS_GENERIC_ARRAY_CALL_GRAPH)?;

    // Matrix: `Fallback Source Oracle Matrix`.
    //
    // Source chain:
    //   generic-array/src/lib.rs:1239 calls `iter.size_hint()`, with guarded
    //   tuple arms at lines 1241 and 1243.
    //   generic-array/src/lib.rs:1276 repeats the same guard shape, with arms
    //   at lines 1278 and 1280.
    //
    // Current model gap: the guarded local-binding receiver rows are visible
    // but remain targetless because receiver binding tracking does not yet
    // resolve `iter` back to the iterator type.
    assert_targetless_method_rows_by_name(&db, "size_hint", CallStatusKind::Unsupported, 2)
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

fn assert_owner_dynamic_targetless(
    db: &Database,
    owner: Uuid,
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
    assert_eq!(
        row.site.arg_count,
        Some(expected_arg_count),
        "{label} should preserve the dynamic call argument count"
    );
    assert_targetless_status(row, CallStatusKind::Unsupported);
    assert!(
        relations_for_site(db, row.site.id)?.rows.is_empty(),
        "{label} should not have raw call_relation targets"
    );
    assert_no_traversal_candidates_for_site(db, owner, row.site.id, label)?;

    Ok(row.site.id)
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
