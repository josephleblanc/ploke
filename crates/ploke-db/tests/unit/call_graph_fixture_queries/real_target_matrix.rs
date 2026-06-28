//! Real-target call graph contracts over the pinned axum corpus fixture.
//!
//! These tests use the immutable `corpus_axum_call_graph` backup, not parser
//! fixture crates. Each row starts from source inspected in the pinned checkout:
//!
//! ```text
//! github:tokio-rs/axum@a3446d68bc03d61fb8e7513052bad2825d0c0db1
//! selected members: axum, axum-core, axum-macros
//! ```
//!
//! Source-oracle references:
//! - `docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-case-matrix.md`
//! - `docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md`
//!
//! Coverage table for this consolidated batch:
//!
//! | Bucket | Source ground truth | DB contract |
//! | --- | --- | --- |
//! | Regular helper callers | `axum-macros/src/lib.rs:{724,739}` call root `expand(...)` | `callers_for_target`, `call_sites_for_target`, and owner traversal resolve both helper callers. |
//! | Import/path function call | `axum-macros/src/typed_path.rs:23` calls `crate::attr_parsing::parse_attrs(...)` | currently unresolved in the corpus fixture; the structural row is asserted as a targetless gap. |
//! | Inherent associated function | `axum/src/json.rs:{112,128}` call `Self::from_bytes(...)` | currently unsupported: the structural rows are visible but do not yet traverse to `Json::from_bytes`. |
//! | Trait associated function | `axum/src/handler/service.rs:171` calls `Handler::call(...)` | currently unresolved: the structural row is visible but does not yet traverse to the trait method. |
//! | Tuple-struct constructor | `axum/src/boxed.rs:38` calls `BoxedIntoRoute(...)` | explicit tuple-struct constructor resolves to the `BoxedIntoRoute` struct in one call edge. |
//! | Inherent constructor | `axum/src/error_handling/mod.rs:65` calls `HandleError::new(...)` | extension methods resolve to the local inherent constructor in one call edge. |
//! | Same-impl self methods | `axum-core/src/ext_traits/{request.rs:268,request_parts.rs:122}` call `self.extract_with_state(&())` | owner traversal resolves both method calls to their same-impl `extract_with_state`. |
//! | Proc-macro body calls | `axum-macros/src/lib.rs:{377,426,665,715}` call `expand_with(...)` | currently unsupported: proc-macro function bodies are not visited for call sites. |
//! | Closure body call | `axum-macros/src/from_ref.rs:23` calls `expand_field(...)` inside a closure | currently unsupported: no call site targets `expand_field`. |
//! | Dynamic callable fields | `axum/src/boxed.rs:{85,159}` call function-pointer / trait-object fields | currently unsupported: visible dynamic call sites remain targetless blockers. |

use std::collections::BTreeMap;

use cozo::DataValue;
use ploke_test_utils::{CORPUS_AXUM_CALL_GRAPH, fresh_backup_fixture_db};
use uuid::Uuid;

use super::*;

#[derive(Clone, Copy)]
struct ExpectedCaller {
    owner_name: &'static str,
    path: &'static [&'static str],
}

#[derive(Clone, Copy)]
struct TraversalExpectation {
    label: &'static str,
    owner: Uuid,
    target: Uuid,
    site_id: Uuid,
}

#[test]
fn axum_proc_macro_body_calls_are_documented_unsupported_gap() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;
    let target = function_id_by_name_in_module(&db, &["crate"], "expand_with")?;

    // Ground truth:
    //   rg -n "\bexpand_with\(" axum-macros/src/lib.rs
    // direct calls at lines 377, 426, 665, and 715.
    //
    // Current model gap: proc-macro item functions are recorded as functions,
    // but their bodies are not visited for structural call-site extraction.
    for macro_name in [
        "derive_from_request",
        "derive_from_request_parts",
        "derive_typed_path",
        "derive_from_ref",
    ] {
        macro_id_by_name(&db, macro_name)?;
    }

    let callers = db.callers_for_target(target)?;
    assert!(
        callers.is_empty(),
        "expand_with should have no resolved callers until proc-macro bodies are visited: {callers:#?}"
    );

    let sites = db.call_sites_for_target(target)?;
    assert!(
        sites.is_empty(),
        "call_sites_for_target should mirror targetless proc-macro-body gap: {sites:#?}"
    );

    Ok(())
}

#[test]
fn axum_macros_expand_helpers_reach_root_expand() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;
    let target = function_id_by_name_in_module(&db, &["crate"], "expand")?;

    // Ground truth:
    //   axum-macros/src/lib.rs:724 expand(syn::parse(input).and_then(f))
    //   axum-macros/src/lib.rs:739 expand(expand_result)
    let cases = [
        ExpectedCaller {
            owner_name: "expand_with",
            path: &["expand"],
        },
        ExpectedCaller {
            owner_name: "expand_attr_with",
            path: &["expand"],
        },
    ];

    for case in cases {
        let owner = function_id_by_name_in_module(&db, &["crate"], case.owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        let row = row_by_path(&context, case.path);
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Function,
            CallSiteKind::Path,
            CallTargetKind::Function,
        );
        assert_one_edge_traversal(
            &db,
            TraversalExpectation {
                label: case.owner_name,
                owner,
                target,
                site_id: row.site.id,
            },
        )?;
    }

    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        cases.len(),
        "root expand should have exactly the inspected helper callers: {callers:#?}"
    );
    for case in cases {
        let owner = function_id_by_name_in_module(&db, &["crate"], case.owner_name)?;
        let caller = caller_by_owner_kind_path(&callers, owner, CallSiteKind::Path, case.path);
        assert_eq!(caller.status.status, CallStatusKind::Resolved);
    }

    Ok(())
}

#[test]
fn axum_real_target_explicit_crate_path_parse_attrs_is_documented_gap() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: `parse_attrs` path/import row.
    // Source chain:
    //   axum-macros/src/attr_parsing.rs:59 defines `parse_attrs`.
    //   axum-macros/src/typed_path.rs:23 calls
    //   `crate::attr_parsing::parse_attrs(&input.attrs, "typed_path")`.
    // Current model gap: the callsite is structurally present, but this corpus
    // fixture does not yet resolve the explicit crate path to the helper.
    let owner = function_id_by_name_in_module(&db, &["crate", "typed_path"], "expand")?;
    let target = function_id_by_name_in_module(&db, &["crate", "attr_parsing"], "parse_attrs")?;
    let context = db.call_context_for_owner(owner)?;
    let row = row_by_path(&context, &["crate", "attr_parsing", "parse_attrs"]);

    assert_eq!(row.status.status, CallStatusKind::Unresolved);
    assert_eq!(row.status.resolution, None);
    assert!(
        row.targets.is_empty(),
        "unresolved parse_attrs row should not expose traversal targets: {row:#?}"
    );
    assert!(
        db.callers_for_target(target)?.iter().all(|caller| {
            caller.site.owner_id != owner
                || caller.site.path.as_ref()
                    != Some(&path(&["crate", "attr_parsing", "parse_attrs"]))
        }),
        "the typed_path::expand parse_attrs row should remain targetless until explicit crate-path resolution handles this corpus row"
    );

    Ok(())
}

#[test]
fn axum_real_target_json_from_bytes_self_paths_are_documented_gap() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;
    let target = method_id_by_name_and_body_substring(
        &db,
        "from_bytes",
        "serde_json::Deserializer::from_slice(bytes)",
    )?;

    // Matrix: `Json::from_bytes` inherent method row.
    // Source chain:
    //   axum/src/json.rs:164 defines `Json::from_bytes`.
    //   axum/src/json.rs:112 and :128 call `Self::from_bytes(&bytes)`.
    // Current model gap: associated-function `Self::...` resolution is not
    // available in the axum fixture, so each structural row remains targetless.
    let owners =
        method_ids_by_name_and_body_substring(&db, "from_request", "Self::from_bytes(&bytes)")?;
    assert_eq!(
        owners.len(),
        2,
        "axum/src/json.rs should expose two from_request owners that call Self::from_bytes"
    );

    for owner in owners {
        let context = db.call_context_for_owner(owner)?;
        let row = row_by_kind_path(&context, CallSiteKind::Path, &["Self", "from_bytes"]);
        assert_eq!(row.status.status, CallStatusKind::Unsupported);
        assert_eq!(row.status.resolution, None);
        assert!(
            row.targets.is_empty(),
            "unsupported Self::from_bytes row should remain targetless: {row:#?}"
        );
    }

    assert!(
        db.call_sites_for_target(target)?.is_empty(),
        "Json::from_bytes should remain targetless until Self::associated-function resolution lands"
    );

    Ok(())
}

#[test]
fn axum_real_target_handler_service_trait_call_is_documented_gap() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: `Handler::call` trait dispatch row.
    // Source chain:
    //   axum/src/handler/mod.rs:153 declares `Handler::call`.
    //   axum/src/handler/service.rs:171 calls `Handler::call(handler, req, state)`.
    // Current model gap: the structural path row is present, but full trait
    // associated-function resolution is not yet modeled for this corpus row.
    let owner = method_id_by_name_and_body_substring(
        &db,
        "call",
        "Handler::call(handler, req, self.state.clone())",
    )?;
    let target = method_id_by_trait_name(&db, "Handler", "call")?;
    let context = db.call_context_for_owner(owner)?;
    let row = row_by_path(&context, &["Handler", "call"]);

    assert_eq!(row.status.status, CallStatusKind::Unresolved);
    assert_eq!(row.status.resolution, None);
    assert!(
        row.targets.is_empty(),
        "unresolved Handler::call row should not expose traversal targets: {row:#?}"
    );
    assert!(
        db.call_sites_for_target(target)?.is_empty(),
        "Handler::call should remain targetless until trait associated-function resolution lands"
    );

    Ok(())
}

#[test]
fn axum_real_target_boxed_into_route_explicit_constructor_reaches_struct() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: `BoxedIntoRoute` tuple-struct constructor row.
    // Source chain:
    //   axum/src/boxed.rs:12 defines `BoxedIntoRoute<S, E>(...)`.
    //   axum/src/boxed.rs:38 calls `BoxedIntoRoute(Box::new(...))`.
    // Expected traversal: `BoxedIntoRoute::map` -> struct constructor, one call edge.
    let owner = method_id_by_name_and_body_substring(&db, "map", "BoxedIntoRoute(Box::new")?;
    let target = struct_id_by_name(&db, "BoxedIntoRoute")?;
    let context = db.call_context_for_owner(owner)?;
    let row = row_by_path(&context, &["BoxedIntoRoute"]);

    assert_resolved_target(
        row,
        target,
        CallRelationKind::TupleStructConstructor,
        CallSiteKind::Path,
        CallTargetKind::Struct,
    );
    assert_one_edge_traversal(
        &db,
        TraversalExpectation {
            label: "BoxedIntoRoute::map -> BoxedIntoRoute",
            owner,
            target,
            site_id: row.site.id,
        },
    )
}

#[test]
fn axum_real_target_handle_error_extension_reaches_constructor() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: `HandleError::new` inherent constructor row.
    // Source chain:
    //   axum/src/error_handling/mod.rs:80 defines `HandleError::new`.
    //   axum/src/error_handling/mod.rs:65 calls `HandleError::new(self, f)`.
    // Expected traversal: `HandleErrorExt::handle_error` -> constructor, one call edge.
    let owner =
        method_id_by_name_and_body_substring(&db, "handle_error", "HandleError::new(self, f)")?;
    let target = method_id_by_name_and_body_substring(&db, "new", "Self { inner, f, _extractor")?;
    let context = db.call_context_for_owner(owner)?;
    let row = row_by_path(&context, &["HandleError", "new"]);

    assert_resolved_target(
        row,
        target,
        CallRelationKind::AssociatedFunction,
        CallSiteKind::Path,
        CallTargetKind::Method,
    );
    assert_one_edge_traversal(
        &db,
        TraversalExpectation {
            label: "handle_error -> HandleError::new",
            owner,
            target,
            site_id: row.site.id,
        },
    )
}

#[test]
fn axum_core_extract_self_methods_reach_same_impl_methods() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Ground truth:
    //   axum-core/src/ext_traits/request.rs:268 self.extract_with_state(&())
    //   axum-core/src/ext_traits/request_parts.rs:122 self.extract_with_state(&())
    let owners =
        method_ids_by_name_and_body_substring(&db, "extract", "self.extract_with_state(&())")?;
    assert_eq!(
        owners.len(),
        2,
        "axum should expose both RequestExt and RequestPartsExt extract methods"
    );

    let request_target = method_id_by_name_and_body_substring(
        &db,
        "extract_with_state",
        "E::from_request(self, state)",
    )?;
    let parts_target = method_id_by_name_and_body_substring(
        &db,
        "extract_with_state",
        "E::from_request_parts(self, state)",
    )?;
    let expected_targets = [request_target, parts_target];

    for owner in owners {
        let context = db.call_context_for_owner(owner)?;
        let row = row_by_method_receiver(&context, "extract_with_state", &CallReceiver::SelfValue);
        assert_eq!(row.status.status, CallStatusKind::Resolved);
        assert_eq!(row.status.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(row.targets.len(), 1);
        assert!(
            expected_targets.contains(&row.targets[0].target_id),
            "self.extract_with_state should resolve to one of the same-impl extract_with_state methods: {row:#?}"
        );
        assert_eq!(row.targets[0].relation, CallRelationKind::Method);
        assert_eq!(row.targets[0].source_kind, CallSiteKind::Method);
        assert_eq!(row.targets[0].target_kind, CallTargetKind::Method);
        assert_one_edge_traversal(
            &db,
            TraversalExpectation {
                label: "extract -> extract_with_state",
                owner,
                target: row.targets[0].target_id,
                site_id: row.site.id,
            },
        )?;
    }

    for target in expected_targets {
        let callers = db.callers_for_target(target)?;
        assert_eq!(
            callers.len(),
            1,
            "each extract_with_state impl should have exactly one inspected self-method caller"
        );
        assert_eq!(callers[0].status.status, CallStatusKind::Resolved);
        assert_eq!(callers[0].target.target_id, target);
    }

    Ok(())
}

#[test]
fn axum_closure_body_call_is_documented_unsupported_gap() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Ground truth:
    //   axum-macros/src/from_ref.rs:23
    //   .map(|(idx, field)| expand_field(&item.ident, idx, field))
    //
    // Current model gap: closure bodies do not yet get independent call-body
    // ownership, so the `expand_field(...)` call is not projected.
    let owner = function_id_by_name_in_module(&db, &["crate", "from_ref"], "expand")?;
    let target = function_id_by_name_in_module(&db, &["crate", "from_ref"], "expand_field")?;

    let context = db.call_context_for_owner(owner)?;
    let unsupported_path = path(&["expand_field"]);
    assert!(
        context
            .iter()
            .all(|row| row.site.path.as_ref() != Some(&unsupported_path)),
        "closure-body expand_field call should remain absent until closure ownership is modeled: {context:#?}"
    );

    let sites = db.call_sites_for_target(target)?;
    assert!(
        sites.is_empty(),
        "expand_field should have no resolved call sites until closure body calls are projected: {sites:#?}"
    );

    Ok(())
}

#[derive(Clone, Copy)]
struct DynamicGap {
    method_name: &'static str,
    body_marker: &'static str,
    source_line: u32,
}

#[test]
fn axum_dynamic_callable_fields_are_visible_unsupported_blockers() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Ground truth:
    //   axum/src/boxed.rs:85  (self.into_route)(self.handler, state)
    //   axum/src/boxed.rs:159 (self.layer)(self.inner.into_route(state))
    let cases = [
        DynamicGap {
            method_name: "into_route",
            body_marker: "(self.into_route)(self.handler, state)",
            source_line: 85,
        },
        DynamicGap {
            method_name: "into_route",
            body_marker: "(self.layer)(self.inner.into_route(state))",
            source_line: 159,
        },
    ];

    for case in cases {
        let owner = method_id_by_name_and_body_substring(&db, case.method_name, case.body_marker)?;
        let context = db.call_context_for_owner(owner)?;
        let dynamic_rows = context
            .iter()
            .filter(|row| row.site.kind == CallSiteKind::Dynamic)
            .collect::<Vec<_>>();

        assert_eq!(
            dynamic_rows.len(),
            1,
            "axum/src/boxed.rs:{} should project one dynamic callable field call: {context:#?}",
            case.source_line
        );
        assert_eq!(dynamic_rows[0].status.status, CallStatusKind::Unsupported);
        assert!(
            dynamic_rows[0].targets.is_empty(),
            "unsupported dynamic callable field call should remain targetless: {dynamic_rows:#?}"
        );
    }

    Ok(())
}

fn setup_axum_call_graph_db() -> Result<Database, DbError> {
    let db = fresh_backup_fixture_db(&CORPUS_AXUM_CALL_GRAPH)
        .map_err(|err| DbError::QueryExecution(err.to_string()))?;
    assert!(
        db.has_call_graph_relations()?,
        "corpus_axum_call_graph must be regenerated with populated call graph relations"
    );
    Ok(db)
}

fn assert_one_edge_traversal(db: &Database, expected: TraversalExpectation) -> Result<(), DbError> {
    let outgoing = db.expand_call_context(
        CallContextSeed::Owner(expected.owner),
        CallContextOptions {
            include_incoming_callers: false,
            max_candidates: 512,
            ..CallContextOptions::default()
        },
    )?;
    assert_outgoing_candidate(&outgoing, expected.target, expected.site_id, expected.label);

    let incoming = db.expand_call_context(
        CallContextSeed::Target(expected.target),
        CallContextOptions {
            include_outgoing_targets: false,
            max_candidates: 512,
            ..CallContextOptions::default()
        },
    )?;
    assert_incoming_candidate(
        &incoming,
        expected.owner,
        expected.site_id,
        expected.target,
        expected.label,
    );

    Ok(())
}

fn method_id_by_name_and_body_substring(
    db: &Database,
    name: &str,
    body_marker: &str,
) -> Result<Uuid, DbError> {
    let matching = method_ids_by_name_and_body_substring(db, name, body_marker)?;
    assert_eq!(
        matching.len(),
        1,
        "expected exactly one method named {name:?} whose body contains {body_marker:?}"
    );
    Ok(matching[0])
}

fn macro_id_by_name(db: &Database, name: &str) -> Result<Uuid, DbError> {
    let mut params = BTreeMap::new();
    params.insert("name".to_string(), DataValue::from(name));

    let rows = db.raw_query_params(
        r#"?[id] :=
            *macro { id, name: $name @ 'NOW' }"#,
        params,
    )?;
    assert_eq!(
        rows.rows.len(),
        1,
        "expected exactly one macro node named {name:?}; rows: {:#?}",
        rows.rows
    );
    to_uuid(&rows.rows[0][0])
}

fn method_ids_by_name_and_body_substring(
    db: &Database,
    name: &str,
    body_marker: &str,
) -> Result<Vec<Uuid>, DbError> {
    let mut params = BTreeMap::new();
    params.insert("name".to_string(), DataValue::from(name));

    let rows = db.raw_query_params(
        r#"?[id, body] :=
            *method { id, name: $name, body @ 'NOW' }"#,
        params,
    )?;
    let normalized_marker = body_key(body_marker);
    let matching = rows
        .rows
        .iter()
        .filter_map(|row| {
            let body = match &row[1] {
                DataValue::Str(body) => body.as_str(),
                _ => return None,
            };
            body_key(body)
                .contains(&normalized_marker)
                .then(|| row[0].clone())
        })
        .collect::<Vec<_>>();

    matching.iter().map(to_uuid).collect()
}

fn body_key(value: &str) -> String {
    value.chars().filter(|ch| !ch.is_whitespace()).collect()
}
