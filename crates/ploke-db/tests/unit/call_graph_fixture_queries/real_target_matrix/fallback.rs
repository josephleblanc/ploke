use ploke_test_utils::{
    CORPUS_CHRONO_CALL_GRAPH, CORPUS_GENERIC_ARRAY_CALL_GRAPH, CORPUS_MEMCHR_CALL_GRAPH,
};

use super::super::*;
use super::common::*;

#[test]
fn chrono_alias_constructor_rows_are_targetless_fallback_oracles() -> Result<(), DbError> {
    let db = setup_call_graph_db(&CORPUS_CHRONO_CALL_GRAPH)?;

    // Matrix: `Fallback Source Oracle Matrix` in
    // docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md.
    //
    // Source chain:
    //   chrono/src/offset/mod.rs:77 aliases `MappedLocalTime<T> = LocalResult<T>`.
    //   chrono/src/offset/mod.rs:81-83 defines `LocalResult::Single(T)`.
    //   chrono/src/offset/mod.rs:{143,156,178,193,216,238,260,468,502,535}
    //   calls `MappedLocalTime::Single(...)` through the alias.
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
    assert_no_path_rows(&db, &["LocalResult", "Single"])
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
    )
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
    )
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
    assert_targetless_dynamic_rows_by_method_name(&db, "find", &[4, 2])
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
    assert_no_dynamic_rows_by_method_name(&db, "run")
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
