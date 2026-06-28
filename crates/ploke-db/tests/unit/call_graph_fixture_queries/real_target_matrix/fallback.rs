use ploke_test_utils::{
    CORPUS_CHRONO_CALL_GRAPH, CORPUS_GENERIC_ARRAY_CALL_GRAPH, CORPUS_MEMCHR_CALL_GRAPH,
};

use super::super::*;
use super::common::*;
use super::source_lines::{
    SourceLineFanout, assert_targetless_dynamic_line_fanout_by_method,
    assert_targetless_method_line_fanout, assert_targetless_path_line_fanout,
};

#[test]
fn chrono_alias_constructor_rows_are_targetless_fallback_oracles() -> Result<(), DbError> {
    let db = setup_call_graph_db(&CORPUS_CHRONO_CALL_GRAPH)?;

    // Matrix: `Fallback Source Oracle Matrix` in
    // docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md.
    //
    // Source chain:
    //   chrono/src/offset/mod.rs:77 aliases `MappedLocalTime<T> = LocalResult<T>`.
    //   chrono/src/offset/mod.rs:81-83 defines `LocalResult::Single(T)`.
    //   chrono/src/offset/mod.rs:{143,156,468,502,535} call
    //   `MappedLocalTime::Single(...)` through the alias.
    //   chrono/src/offset/{fixed.rs:135,138,utc.rs:122,125} and
    //   chrono/src/datetime/tests.rs:{75,79} are additional fixture-projected
    //   direct constructor rows.
    //
    // Current model gap: alias constructor binding is not resolved to
    // `LocalResult::Single`, so the fixture must preserve targetless rows
    // rather than fabricate enum-variant traversal edges.
    assert_targetless_path_rows(
        &db,
        &["MappedLocalTime", "Single"],
        CallStatusKind::Unresolved,
        11,
    )?;
    assert_targetless_path_line_fanout(
        &db,
        &CORPUS_CHRONO_CALL_GRAPH,
        &["MappedLocalTime", "Single"],
        CallStatusKind::Unresolved,
        &[
            SourceLineFanout {
                file_suffix: "src/datetime/tests.rs",
                lines: &[75, 79],
            },
            SourceLineFanout {
                file_suffix: "src/offset/fixed.rs",
                lines: &[135, 138],
            },
            SourceLineFanout {
                file_suffix: "src/offset/mod.rs",
                lines: &[143, 156, 468, 502, 535],
            },
            SourceLineFanout {
                file_suffix: "src/offset/utc.rs",
                lines: &[122, 125],
            },
        ],
    )?;
    let target = variant_id_by_enum_and_variant_names(&db, "LocalResult", "Single")?;
    assert_no_incoming_traversal_to_target(
        &db,
        target,
        "chrono/src/offset/mod.rs alias constructor rows",
    )?;
    assert_no_path_rows(&db, &["LocalResult", "Single"])?;

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

    for case in cases {
        let owner =
            method_id_by_name_body_and_file_suffix(&db, case.owner, case.marker, case.file_suffix)?;
        assert_owner_path_targetless_count(
            &db,
            owner,
            &["MappedLocalTime", "Single"],
            CallStatusKind::Unresolved,
            case.expected_count,
            case.label,
        )?;
    }

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
fn chrono_guarded_match_arm_method_guard_is_targetless_fallback_oracle() -> Result<(), DbError> {
    let db = setup_call_graph_db(&CORPUS_CHRONO_CALL_GRAPH)?;

    // Matrix: `Fallback Source Oracle Matrix`.
    //
    // Source chain:
    //   chrono/src/format/strftime.rs:193-198 defines `StrftimeItems::queue`.
    //   chrono/src/format/strftime.rs:635 guards a match arm with
    //   `self.queue.is_empty()`.
    //
    // Current model gap: field receiver classification is structural only here;
    // the external slice method must remain targetless.
    assert_targetless_method_rows(
        &db,
        "is_empty",
        "SelfField",
        Some(&["queue"]),
        CallStatusKind::Unsupported,
        1,
    )?;
    assert_targetless_method_line_fanout(
        &db,
        &CORPUS_CHRONO_CALL_GRAPH,
        "is_empty",
        "SelfField",
        Some(&["queue"]),
        CallStatusKind::Unsupported,
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
        CallStatusKind::Unsupported,
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
fn generic_array_guarded_match_arm_method_guard_is_absent_fallback_gap() -> Result<(), DbError> {
    let db = setup_call_graph_db(&CORPUS_GENERIC_ARRAY_CALL_GRAPH)?;

    // Matrix: `Fallback Source Oracle Matrix`.
    //
    // Source chain:
    //   generic-array/src/lib.rs:1239 calls `iter.size_hint()`, with guarded
    //   tuple arms at lines 1241 and 1243.
    //   generic-array/src/lib.rs:1276 repeats the same guard shape, with arms
    //   at lines 1278 and 1280.
    //
    // Current model gap: these guarded match-arm receiver calls are absent from
    // the call-site projection in this fixture.
    assert_no_method_rows(&db, "size_hint")
}

fn assert_owner_dynamic_targetless(
    db: &Database,
    owner: uuid::Uuid,
    expected_arg_count: u32,
    label: &str,
) -> Result<uuid::Uuid, DbError> {
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
