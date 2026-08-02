use ploke_db::{CallPathOptions, LocalBindingRelationKind};
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

const fn fanout(file_suffix: &'static str, lines: &'static [u32]) -> SourceLineFanout {
    SourceLineFanout { file_suffix, lines }
}

#[derive(Clone, Copy)]
struct CallableCase {
    site: Uuid,
    method: &'static str,
    assignment: &'static str,
    field: &'static str,
    line: u32,
    label: &'static str,
}

struct CallableSet {
    owner: Uuid,
    cases: [CallableCase; 2],
}

fn memchr_callable_set(db: &Database) -> Result<CallableSet, DbError> {
    let owner = method_id_by_name_body_and_file_suffix(
        db,
        "run",
        "fwd(t.haystack.as_bytes(), t.needle.as_bytes())",
        "src/tests/substring/mod.rs",
    )?;
    let context = db.call_context_for_owner(owner)?;
    let fwd = row_by_path(&context, &["fwd"]).site.id;
    let rev = row_by_path(&context, &["rev"]).site.id;
    #[rustfmt::skip]
    let cases = [
        CallableCase { site: fwd, method: "fwd", assignment: "self.fwd = Some(Box::new(search));", field: "fwd", line: 137, label: "memchr/src/tests/substring/mod.rs:94 boxed fwd dyn FnMut local binding" },
        CallableCase { site: rev, method: "rev", assignment: "self.rev = Some(Box::new(search));", field: "rev", line: 153, label: "memchr/src/tests/substring/mod.rs:110 boxed rev dyn FnMut local binding" },
    ];
    Ok(CallableSet { owner, cases })
}

fn memchr_setter(db: &Database, case: &CallableCase) -> Result<Uuid, DbError> {
    method_id_by_name_body_and_file_suffix(
        db,
        case.method,
        case.assignment,
        "src/tests/substring/mod.rs",
    )
}

fn assert_field_shape(flow: &ploke_db::SelfFieldAssignmentFlow, case: &CallableCase, setter: Uuid) {
    assert_eq!(flow.site.kind, CallSiteKind::Path);
    assert_eq!(flow.site.path.as_ref(), Some(&path(&[case.field])));
    assert_eq!(flow.status.status, CallStatusKind::Unsupported);
    assert!(flow.status.resolution.is_none());
    assert_eq!(flow.setter_id, setter);
    assert_eq!(flow.parameter_binding.name, "search");
}

fn assert_edge_free(db: &Database, site: Uuid, label: &str) -> Result<(), DbError> {
    assert!(
        relations_for_site(db, site)?.rows.is_empty(),
        "{label} must remain free of raw call_relation targets"
    );
    Ok(())
}

fn chrono_slice_site(db: &Database) -> Result<(Uuid, Uuid), DbError> {
    let owner = method_id_by_name_body_and_file_suffix(
        db,
        "parse_next_item",
        "self.queue.is_empty()",
        "src/format/strftime.rs",
    )?;
    let site = assert_owner_method_targetless(
        db,
        owner,
        "is_empty",
        &CallReceiver::SelfField {
            path: vec!["queue".to_string()],
        },
        CallStatusKind::External,
        "chrono/src/format/strftime.rs:635 self.queue.is_empty",
    )?;
    Ok((owner, site))
}

#[test]
fn chrono_alias_constructor_rows_reach_local_result_single() -> Result<(), DbError> {
    let db = setup_call_graph_db(&CORPUS_CHRONO_CALL_GRAPH)?;

    // All 12 MappedLocalTime::Single alias calls resolve to LocalResult::Single.
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
        file: &'static str,
        label: &'static str,
        count: usize,
    }

    #[rustfmt::skip]
    let cases = [
        AliasCase { owner: "map", marker: "MappedLocalTime::Single(f(v))", file: "src/offset/mod.rs", label: "chrono/src/offset/mod.rs:143 MappedLocalTime::Single", count: 1 },
        AliasCase { owner: "and_then", marker: "MappedLocalTime::Single(new)", file: "src/offset/mod.rs", label: "chrono/src/offset/mod.rs:156 MappedLocalTime::Single", count: 1 },
        AliasCase { owner: "offset_from_local_date", marker: "MappedLocalTime::Single(*self)", file: "src/offset/fixed.rs", label: "chrono/src/offset/fixed.rs:135 MappedLocalTime::Single", count: 1 },
        AliasCase { owner: "offset_from_local_datetime", marker: "MappedLocalTime::Single(*self)", file: "src/offset/fixed.rs", label: "chrono/src/offset/fixed.rs:138 MappedLocalTime::Single", count: 1 },
        AliasCase { owner: "offset_from_local_date", marker: "MappedLocalTime::Single(Utc)", file: "src/offset/utc.rs", label: "chrono/src/offset/utc.rs:122 MappedLocalTime::Single", count: 1 },
        AliasCase { owner: "offset_from_local_datetime", marker: "MappedLocalTime::Single(Utc)", file: "src/offset/utc.rs", label: "chrono/src/offset/utc.rs:125 MappedLocalTime::Single", count: 1 },
        AliasCase { owner: "offset", marker: "MappedLocalTime::Single(offset)", file: "src/offset/local/unix.rs", label: "chrono/src/offset/local/unix.rs:159 MappedLocalTime::Single", count: 1 },
        AliasCase { owner: "offset_from_local_datetime", marker: "Unexpected local time {local}", file: "src/datetime/tests.rs", label: "chrono/src/datetime/tests.rs:75 and :79 MappedLocalTime::Single", count: 2 },
        AliasCase { owner: "timestamp_opt", marker: "MappedLocalTime::Single(self.from_utc_datetime(&dt.naive_utc()))", file: "src/offset/mod.rs", label: "chrono/src/offset/mod.rs:468 MappedLocalTime::Single", count: 1 },
        AliasCase { owner: "timestamp_millis_opt", marker: "MappedLocalTime::Single(self.from_utc_datetime(&dt.naive_utc()))", file: "src/offset/mod.rs", label: "chrono/src/offset/mod.rs:502 MappedLocalTime::Single", count: 1 },
        AliasCase { owner: "timestamp_micros", marker: "MappedLocalTime::Single(self.from_utc_datetime(&dt.naive_utc()))", file: "src/offset/mod.rs", label: "chrono/src/offset/mod.rs:535 MappedLocalTime::Single", count: 1 },
    ];

    let mut checked_sites = 0usize;
    for case in cases {
        let owner =
            method_id_by_name_body_and_file_suffix(&db, case.owner, case.marker, case.file)?;
        let sites = assert_owner_path_resolved_count(
            &db,
            owner,
            &["MappedLocalTime", "Single"],
            target,
            CallRelationKind::EnumVariantConstructor,
            CallTargetKind::Variant,
            case.count,
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

    // Preserve all seven exact DateTime::naive_utc receiver shapes and edges.
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

    #[rustfmt::skip]
    let cases = [
        TryCase { owner: "to_naive_datetime_with_offset", marker: "DateTime::from_timestamp_secs(ts).ok_or(OUT_OF_RANGE)?.naive_utc()", label: "chrono/src/format/parsed.rs:836 DateTime...?.naive_utc" },
        TryCase { owner: "to_datetime_with_timezone", marker: "DateTime::from_timestamp(timestamp, nanosecond).ok_or(OUT_OF_RANGE)?.naive_utc()", label: "chrono/src/format/parsed.rs:953 DateTime...?.naive_utc" },
    ];

    let receiver = CallReceiver::TryMethodCallResult {
        method_name: "ok_or".to_string(),
    };
    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        7,
        "DateTime::naive_utc should expose the current resolved chrono caller fanout: {callers:#?}"
    );
    assert_sites_match_callers(
        &db,
        target,
        &callers,
        "chrono DateTime::naive_utc try callers",
    )?;
    #[rustfmt::skip]
    let receiver_counts = [
        (receiver.clone(), 2, "parsed.rs:836/:953 ok_or try receivers"),
        (CallReceiver::InitializedLocalBinding { name: "now".to_string(), init_path: path(&["Local", "now"]) }, 2, "cfg(test) Local::now initialized receivers"),
        (CallReceiver::PathCallResult { path: path(&["DateTime", "from_timestamp_nanos"]) }, 1, "offset/mod.rs:520 from_timestamp_nanos result receiver"),
        (CallReceiver::SelfValue, 2, "offset/mod.rs:468/:502 Some(dt) self receivers"),
    ];
    for (shape, count, label) in receiver_counts {
        assert_eq!(
            callers
                .iter()
                .filter(|caller| caller.site.receiver.as_ref() == Some(&shape))
                .count(),
            count,
            "DateTime::naive_utc should retain exactly {count} {label}: {callers:#?}"
        );
    }

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
fn chrono_parse_internal_function_pointer_match_tuple_binding_exposes_ambiguous_candidates()
-> Result<(), DbError> {
    let db = setup_call_graph_db(&CORPUS_CHRONO_CALL_GRAPH)?;

    // parse.rs:421 retains the exact 21 runtime-selected setter candidates without reach.
    let owner = function_id_by_name(&db, "parse_internal")?;
    let target_label = "chrono/src/format/parse.rs:421 set(parsed, v)";
    let context = db.call_context_for_owner(owner)?;
    let rows = context
        .iter()
        .filter(|row| row.site.kind == CallSiteKind::Path && row.site.path == Some(path(&["set"])))
        .collect::<Vec<_>>();
    assert_eq!(
        rows.len(),
        1,
        "{target_label} should expose one ambiguous path-call row: {context:#?}"
    );
    let row = rows[0];
    assert_eq!(row.status.status, CallStatusKind::Ambiguous);
    assert_eq!(row.status.resolution, None);
    assert_eq!(row.targets.len(), 21, "{target_label} candidates: {row:#?}");
    assert_eq!(
        relations_for_site(&db, row.site.id)?.rows.len(),
        21,
        "{target_label} should persist the finite reviewed candidate set"
    );

    let free_targets = [
        function_id_by_name(&db, "set_weekday_with_num_days_from_sunday")?,
        function_id_by_name(&db, "set_weekday_with_number_from_monday")?,
    ];
    for target in free_targets {
        assert!(
            row.targets.iter().any(|candidate| {
                candidate.target_id == target
                    && candidate.relation == CallRelationKind::Function
                    && candidate.source_kind == CallSiteKind::Path
                    && candidate.target_kind == CallTargetKind::Function
            }),
            "{target_label} should include the free setter function candidate {target}: {row:#?}"
        );
    }

    #[rustfmt::skip]
    let methods = [
        "set_year", "set_year_div_100", "set_year_mod_100", "set_isoyear", "set_isoyear_div_100",
        "set_isoyear_mod_100", "set_quarter", "set_month", "set_day", "set_week_from_sun",
        "set_week_from_mon", "set_isoweek", "set_ordinal", "set_hour", "set_hour12", "set_minute",
        "set_second", "set_nanosecond", "set_timestamp",
    ];
    for method in methods {
        let target = method_id_by_impl_self_type_name(&db, "Parsed", method)?;
        assert!(
            row.targets.iter().any(|candidate| {
                candidate.target_id == target
                    && candidate.relation == CallRelationKind::AssociatedFunction
                    && candidate.source_kind == CallSiteKind::Path
                    && candidate.target_kind == CallTargetKind::Method
            }),
            "{target_label} should include Parsed::{method} as an associated-function candidate: {row:#?}"
        );
    }

    let paths = db.call_paths_from_owner(
        owner,
        CallPathOptions {
            max_depth: 1,
            max_paths: 64,
        },
    )?;
    assert!(
        paths.iter().all(|path| path
            .edges
            .iter()
            .all(|edge| edge.call_site_id != row.site.id)),
        "{target_label} must remain outside resolved-only traversal paths: {paths:#?}"
    );

    let bindings = db.local_bindings_for_owner(owner)?;
    assert!(
        bindings.iter().any(|binding| binding.name == "set"
            && binding.kind == "LetBinding"
            && binding.source_kind == "Typed"),
        "chrono/src/format/parse.rs:380 should preserve the typed tuple binding for `set`: {bindings:#?}"
    );

    let projected = db.project_call_proof_facts_for_owner(owner, "bd:corpus-chrono-call-graph")?;
    assert!(
        projected >= 2,
        "chrono parse_internal should project node-scoped proof rows for `set`: {projected}"
    );

    Ok(())
}

#[test]
fn chrono_guarded_match_arm_slice_method_guard_is_external_frontier() -> Result<(), DbError> {
    let db = setup_call_graph_db(&CORPUS_CHRONO_CALL_GRAPH)?;

    // strftime.rs:635 remains an exact external, edge-free slice frontier.
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
        &[fanout("src/format/strftime.rs", &[635])],
    )?;
    chrono_slice_site(&db)?;

    Ok(())
}

#[test]
fn chrono_guarded_match_arm_slice_method_guard_reach_is_bounded() -> Result<(), DbError> {
    let db = setup_call_graph_db(&CORPUS_CHRONO_CALL_GRAPH)?;

    // Bounded owner reach must retain, but never traverse, the slice frontier.
    let (owner, site) = chrono_slice_site(&db)?;

    let report = db.call_reach_for_owner(
        owner,
        CallPathOptions {
            max_depth: 2,
            max_paths: 64,
        },
    )?;
    let frontier = report
        .external_frontier_calls
        .iter()
        .find(|row| row.site.id == site)
        .unwrap_or_else(|| {
            panic!("reach should preserve the guarded slice `is_empty` frontier: {report:#?}")
        });
    assert_external_targetless(frontier);
    assert_eq!(frontier.site.owner_id, owner);
    assert_edge_free(&db, site, "slice method frontier after owner reach")?;

    Ok(())
}

#[test]
fn chrono_guarded_match_arm_external_summary_need_is_bounded() -> Result<(), DbError> {
    let db = setup_call_graph_db(&CORPUS_CHRONO_CALL_GRAPH)?;

    // Bounded proof-needs retain the missing-summary blocker without adding an edge.
    let (owner, site) = chrono_slice_site(&db)?;
    let projected = db.project_call_proof_facts_for_owner(owner, "bd:corpus-chrono-call-graph")?;
    assert!(
        projected >= 2,
        "chrono parse_next_item should project node-scoped call proof rows: {projected}"
    );

    let needs = db.external_summary_needs_for_owner(
        owner,
        CallPathOptions {
            max_depth: 2,
            max_paths: 64,
        },
    )?;
    let need = needs
        .iter()
        .find(|need| need.call_site.site.id == site)
        .unwrap_or_else(|| {
            panic!("external-summary needs should preserve queue.is_empty: {needs:#?}")
        });
    assert_external_targetless(&need.call_site);
    assert_eq!(need.call_site.site.owner_id, owner);
    assert!(
        need.blocker_reasons
            .iter()
            .any(|reason| reason == "external_dependency_summary_missing"),
        "need should preserve the missing-summary blocker: {need:#?}"
    );
    assert_edge_free(&db, site, "slice method frontier after proof-needs query")?;

    Ok(())
}

#[test]
fn memchr_function_pointer_field_calls_preserve_ambiguous_candidates() -> Result<(), DbError> {
    let db = setup_call_graph_db(&CORPUS_MEMCHR_CALL_GRAPH)?;

    // Searcher/Prefilter function fields retain exact finite ambiguous candidate sets.
    #[rustfmt::skip]
    let searcher_candidates = function_ids_by_names(&db, &[
        "searcher_kind_empty", "searcher_kind_one_byte", "searcher_kind_two_way",
        "searcher_kind_two_way_with_prefilter", "searcher_kind_sse2", "searcher_kind_avx2",
    ])?;
    #[rustfmt::skip]
    let prefilter_candidates = function_ids_by_names(
        &db, &["prefilter_kind_fallback", "prefilter_kind_sse2", "prefilter_kind_avx2"],
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
        assert_self_field_callable_candidate_proof(&site_facts, &site, &expected_names, label);

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

    // x86_64/memchr.rs:153 expanded at :180/:203/:227/:252/:278/:305/:326 stays edge-free.
    assert_targetless_path_rows(
        &db,
        &["core", "mem", "transmute"],
        CallStatusKind::External,
        7,
    )?;
    #[rustfmt::skip]
    let cases = [
        ("memchr_raw", 3), ("memrchr_raw", 3), ("memchr2_raw", 4), ("memrchr2_raw", 4),
        ("memchr3_raw", 5), ("memrchr3_raw", 5), ("count_raw", 3),
    ];
    for (owner, args) in cases {
        assert_memchr_ifunc_owner(&db, owner, "unsafe_ifunc!", args, owner)?;
    }

    Ok(())
}

#[test]
fn memchr_callable_trait_object_field_calls_are_visible_targetless_path_rows() -> Result<(), DbError>
{
    let db = setup_call_graph_db(&CORPUS_MEMCHR_CALL_GRAPH)?;

    // substring/mod.rs:67-71/:94/:110/:137/:153 retain boxed fwd/rev setter proof, never edges.
    assert_no_dynamic_rows_by_method_name(&db, "run")?;
    let CallableSet { owner, cases } = memchr_callable_set(&db)?;
    assert_no_owner_dynamic_rows(
        &db,
        owner,
        "memchr/src/tests/substring/mod.rs:94 and :110 boxed dyn FnMut calls",
    )?;
    for case in &cases {
        let site = assert_owner_path_targetless(
            &db,
            owner,
            &[case.method],
            CallStatusKind::Unsupported,
            case.label,
        )?;
        assert_eq!(
            site, case.site,
            "{} should retain its exact site",
            case.label
        );
    }

    let flows = db.self_field_assignment_flows_for_owner(owner)?;
    assert_eq!(
        flows.len(),
        3,
        "Runner::run should expose substring fwd/rev plus packedpair fwd setter flows: {flows:#?}"
    );
    for case in &cases {
        let setter = memchr_setter(&db, case)?;
        let bindings = db.local_bindings_for_owner(setter)?;
        let search = bindings
            .iter()
            .find(|binding| {
                binding.kind == "ParameterBinding"
                    && binding.name == "search"
                    && binding.source_kind == "Parameter"
            })
            .unwrap_or_else(|| {
                panic!(
                    "memchr/src/tests/substring/mod.rs:{} should persist the setter search parameter: {bindings:#?}",
                    case.line
                )
            });
        let field = bindings
            .iter()
            .find(|binding| {
                binding.kind == "FieldAssignment"
                    && binding.name == format!("self.{}", case.field)
                    && binding.source_kind == "SelfFieldAssignment"
                    && binding.source_path.as_ref() == Some(&path(&[case.field]))
                    && binding.callee_kind.as_deref() == Some("Path")
                    && binding.callee_path.as_ref() == Some(&path(&["search"]))
            })
            .unwrap_or_else(|| {
                panic!(
                    "memchr/src/tests/substring/mod.rs:{} should persist the self.{} setter assignment source: {bindings:#?}",
                    case.line, case.field
                )
            });
        let edges = db.local_binding_edges_for_owner(setter)?;
        assert!(
            edges.iter().any(|edge| edge.relation
                == LocalBindingRelationKind::OwnerContainsBinding
                && edge.source_id == setter
                && edge.target_id == field.id
                && edge.target_kind == "LocalBinding"),
            "{} should expose owner-to-field-assignment evidence: {edges:#?}",
            case.label
        );
        assert!(
            edges.iter().any(|edge| edge.relation
                == LocalBindingRelationKind::BindingSourceParameter
                && edge.source_id == field.id
                && edge.target_id == search.id
                && edge.source_kind == "LocalBinding"
                && edge.target_kind == "LocalBinding"),
            "{} should link its field assignment to the search parameter: {edges:#?}",
            case.label
        );
        let flow = flows
            .iter()
            .find(|flow| flow.site.id == case.site && flow.setter_id == setter)
            .unwrap_or_else(|| {
                panic!(
                    "{} should expose a self-field assignment source flow: {flows:#?}",
                    case.label
                )
            });
        assert_field_shape(flow, case, setter);
        assert_eq!(flow.owner_type, "Runner");
        assert_eq!(flow.assignment_binding.owner_id, setter);
        assert_eq!(flow.assignment_binding.kind, "FieldAssignment");
        assert_eq!(flow.assignment_binding.source_kind, "SelfFieldAssignment");
        assert_eq!(
            flow.assignment_binding.source_path.as_ref(),
            Some(&path(&[case.field]))
        );
        assert_eq!(flow.assignment_binding.callee_kind.as_deref(), Some("Path"));
        assert_eq!(
            flow.assignment_binding.callee_path.as_ref(),
            Some(&path(&["search"]))
        );
        assert_eq!(flow.parameter_binding.owner_id, setter);
        assert_eq!(flow.parameter_binding.kind, "ParameterBinding");
        assert_eq!(flow.parameter_binding.source_kind, "Parameter");
        assert_eq!(
            flow.source_edge.relation,
            LocalBindingRelationKind::BindingSourceParameter
        );
        assert_eq!(flow.source_edge.source_id, flow.assignment_binding.id);
        assert_eq!(flow.source_edge.target_id, flow.parameter_binding.id);
    }

    db.upsert_proof_fact_values(&cases.map(|case| {
        ploke_test_utils::memchr_callable_trait_object_runtime_dispatch_blocker(case.site)
    }))?;

    let needs = db.runtime_dispatch_needs_for_owner(
        owner,
        CallPathOptions {
            max_depth: 1,
            max_paths: 16,
        },
    )?;
    for case in &cases {
        let need = needs
            .iter()
            .find(|need| need.call_site.site.id == case.site)
            .unwrap_or_else(|| {
                panic!(
                    "{} should be listed in runtime-dispatch needs: {needs:#?}",
                    case.label
                )
            });
        assert!(
            need.paths_to_owner.is_empty(),
            "{} should be a direct frontier: {need:#?}",
            case.label
        );
        assert!(
            need.blocker_reasons
                .iter()
                .any(|reason| reason == "dynamic_dispatch_unbounded"),
            "{} should retain the dynamic dispatch blocker: {need:#?}",
            case.label
        );
        assert_edge_free(&db, case.site, case.label)?;
    }

    db.upsert_proof_fact_values(&cases.map(|case| {
        ploke_test_utils::memchr_callable_trait_object_runtime_dispatch_summary(case.site)
    }))?;
    let after = db.runtime_dispatch_needs_for_owner(
        owner,
        CallPathOptions {
            max_depth: 1,
            max_paths: 16,
        },
    )?;
    for case in &cases {
        assert!(
            after.iter().all(|need| need.call_site.site.id != case.site),
            "{} summary should discharge the proof need: {after:#?}",
            case.label
        );
        assert_edge_free(&db, case.site, case.label)?;
    }

    let projected = db.project_call_proof_facts_for_owner(owner, "bd:corpus-memchr-call-graph")?;
    assert!(
        projected >= 4,
        "memchr boxed dyn FnMut owner should project call_site and call_resolution rows for fwd/rev"
    );
    let blockers = db.proof_blockers()?;
    let proof_rows = db.proof_graphrag_context("boxed dyn FnMut dispatch")?;
    for case in &cases {
        assert_blocked_resolution_proof(
            &db,
            case.site,
            "type_resolution_missing",
            "src/tests/substring/mod.rs",
            case.label,
        )?;
        let site = case.site.to_string();
        assert!(
            blockers.iter().any(|proof| {
                proof.call_site_id.as_deref() == Some(site.as_str())
                    && proof.reason == "dynamic_dispatch_unbounded"
                    && proof.status == "blocked"
            }),
            "{} should expose an explicit dispatch blocker: {blockers:#?}",
            case.label
        );
        assert!(
            proof_rows.iter().any(|proof| {
                proof.kind == "proof_blocker"
                    && proof.call_site_id.as_deref() == Some(site.as_str())
                    && proof.blocker_reason.as_deref() == Some("dynamic_dispatch_unbounded")
                    && proof.status.as_deref() == Some("blocked")
            }),
            "{} should be retrievable as dispatch proof context: {proof_rows:#?}",
            case.label
        );
    }

    Ok(())
}

#[test]
fn memchr_callable_trait_object_setter_arguments_remain_proof_only() -> Result<(), DbError> {
    let db = setup_call_graph_db(&CORPUS_MEMCHR_CALL_GRAPH)?;

    // substring/mod.rs:94/:110 and setters :137/:153 retain argument proof, never edges.
    let CallableSet { owner, cases } = memchr_callable_set(&db)?;

    let flows = db.self_field_assignment_argument_flows_for_owner(owner)?;
    assert!(
        flows.len() >= 2,
        "Runner::run should expose setter argument proof for at least the substring fwd/rev fields: {flows:#?}"
    );

    for case in &cases {
        let setter = memchr_setter(&db, case)?;
        let flow = flows
            .iter()
            .find(|flow| {
                flow.field_flow.site.id == case.site && flow.field_flow.setter_id == setter
            })
            .unwrap_or_else(|| {
                panic!(
                    "{} should expose caller argument proof: {flows:#?}",
                    case.label
                )
            });
        assert_field_shape(&flow.field_flow, case, setter);
        assert_eq!(flow.setter_call.site.method.as_deref(), Some(case.method));
        assert_resolved_target(
            &flow.setter_call,
            setter,
            CallRelationKind::Method,
            CallSiteKind::Method,
            CallTargetKind::Method,
        );
        assert_eq!(
            flow.argument_edge.relation,
            LocalBindingRelationKind::ArgumentSuppliesParameter
        );
        assert_eq!(flow.argument_edge.source_id, flow.setter_call.site.id);
        assert_eq!(
            flow.argument_edge.target_id,
            flow.field_flow.parameter_binding.id
        );
        assert_edge_free(&db, case.site, case.label)?;
    }

    Ok(())
}

#[test]
fn generic_array_guarded_match_arm_method_guard_is_external_frontier() -> Result<(), DbError> {
    let db = setup_call_graph_db(&CORPUS_GENERIC_ARRAY_CALL_GRAPH)?;

    // Both guarded iter.size_hint rows remain exact external proof frontiers.
    let sites = generic_array_size_hint_sites(&db)?;
    let site_ids = sites.iter().map(|(_, site)| *site).collect::<Vec<_>>();
    assert_targetless_method_kind_line_fanout(
        &db,
        &CORPUS_GENERIC_ARRAY_CALL_GRAPH,
        "size_hint",
        "MethodResultLocalBinding",
        CallStatusKind::External,
        &[fanout("src/lib.rs", &[1239, 1276])],
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
