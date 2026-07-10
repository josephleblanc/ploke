use std::collections::{BTreeMap, BTreeSet};

use uuid::Uuid;

use super::super::*;
use super::common::*;

struct ResolvedTraversalCase {
    label: &'static str,
    target: Uuid,
    expected_call_edges: usize,
    expected_traversal_candidates: usize,
}

struct ResolvedShapeCase {
    label: &'static str,
    target: Uuid,
    expected: Vec<(&'static str, usize)>,
}

#[test]
fn axum_real_target_supported_callers_are_one_hop_traversable() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix:
    //   docs/active/agents/call-graph/
    //   2026-06-28_real-corpus-call-site-oracle-matrices.md
    //
    // This test is the consolidated traversal oracle for the real-corpus rows
    // that are currently expected to resolve. Case-specific modules still pin
    // the source shape, target kind, proof facts, and unsupported gaps. This
    // layer asserts the shared query contract: target-centered edge queries
    // enumerate the exact persisted caller edges, and the context expansion API
    // can still traverse from each caller owner to the callee in one hop even
    // when it collapses multiple same-owner callsites into fewer candidates.
    let cases = [
        ResolvedTraversalCase {
            // axum-macros/src/lib.rs:724,739 call root `expand(...)`.
            // Callee: axum-macros/src/lib.rs root helper `expand`.
            label: "axum-macros root expand helper callers",
            target: function_id_by_name_in_module(&db, &["crate"], "expand")?,
            expected_call_edges: 2,
            expected_traversal_candidates: 2,
        },
        ResolvedTraversalCase {
            // axum-macros/src/typed_path.rs:23 calls
            // `crate::attr_parsing::parse_attrs(...)`; from_ref.rs:30 and
            // from_request/mod.rs:{112,196,471,598,727,892,908,1029,1039}
            // call the imported helper. The :471, :1029, and :1039 rows are
            // owned by nested closure executable owners. Callee:
            // axum-macros/src/attr_parsing.rs:59.
            label: "axum-macros parse_attrs current resolved fanout",
            target: function_id_by_name_in_module(&db, &["crate", "attr_parsing"], "parse_attrs")?,
            expected_call_edges: 11,
            expected_traversal_candidates: 9,
        },
        ResolvedTraversalCase {
            // axum-macros/src/{debug_handler.rs,typed_path.rs,from_ref.rs,
            // from_request/mod.rs} call `crate::run_ui_tests(...)`.
            // Callee: axum-macros/src/lib.rs:797.
            label: "axum-macros run_ui_tests helper callers",
            target: function_id_by_name_in_module(&db, &["crate"], "run_ui_tests")?,
            expected_call_edges: 5,
            expected_traversal_candidates: 5,
        },
        ResolvedTraversalCase {
            // axum-core/src/body.rs:110 and :116 call `Self::empty()`, and
            // axum-core/src/response/into_response.rs response conversion
            // rows, axum-core/src/ext_traits/request.rs helper rows, and
            // direct axum `axum_core::body::Body` imports, local
            // `crate::body::Body` re-export imports, inherited `super::*`
            // imports, and nested closure/local-item owners call `Body::empty()`.
            // Callee: axum-core/src/body.rs:52.
            label: "axum-core Body::empty current resolved subset",
            target: method_id_by_name_and_body_substring(&db, "empty", "Empty::new()")?,
            expected_call_edges: 23,
            expected_traversal_candidates: 23,
        },
        ResolvedTraversalCase {
            // axum/src/json.rs:112 and :128 call `Self::from_bytes(&bytes)`
            // from trait impl bodies for `Json<T>`. Callee:
            // axum/src/json.rs:164.
            label: "axum Json::from_bytes trait-impl Self callers",
            target: method_id_by_name_and_body_substring(
                &db,
                "from_bytes",
                "serde_json::Deserializer::from_slice(bytes)",
            )?,
            expected_call_edges: 2,
            expected_traversal_candidates: 2,
        },
        ResolvedTraversalCase {
            // axum-core/src/body.rs calls same-module `try_downcast(...)`.
            // Callee: axum-core/src/body.rs:26.
            label: "axum-core try_downcast current resolved subset",
            target: function_id_by_name_in_module(&db, &["crate", "body"], "try_downcast")?,
            expected_call_edges: 2,
            expected_traversal_candidates: 2,
        },
        ResolvedTraversalCase {
            // axum/src/routing/mod.rs:205 calls imported
            // `crate::util::try_downcast(...)`. Callee: axum/src/util.rs:99.
            label: "axum try_downcast current resolved subset",
            target: function_id_by_name_in_module(&db, &["crate", "util"], "try_downcast")?,
            expected_call_edges: 1,
            expected_traversal_candidates: 1,
        },
        ResolvedTraversalCase {
            // axum/src/boxed.rs:{23,38,51} call the tuple-struct constructor
            // through `Self(...)`, `BoxedIntoRoute(...)`, and `Self(...)`.
            // Callee binding: axum/src/boxed.rs:12.
            label: "axum BoxedIntoRoute tuple constructor",
            target: struct_id_by_name(&db, "BoxedIntoRoute")?,
            expected_call_edges: 3,
            expected_traversal_candidates: 3,
        },
        ResolvedTraversalCase {
            // axum-macros/src/with_position.rs:92 calls the enum variant
            // constructor `Position::First(item)`. Callee binding:
            // axum-macros/src/with_position.rs:66.
            label: "axum-macros Position::First enum variant constructor",
            target: variant_id_by_enum_and_variant_names(&db, "Position", "First")?,
            expected_call_edges: 1,
            expected_traversal_candidates: 1,
        },
        ResolvedTraversalCase {
            // axum/src/error_handling/mod.rs:65 and
            // axum/src/service_ext.rs:43 call `HandleError::new(...)`.
            // Callee: axum/src/error_handling/mod.rs:80.
            label: "axum HandleError::new constructor callers",
            target: method_id_by_name_and_body_substring(
                &db,
                "new",
                "Self { inner, f, _extractor",
            )?,
            expected_call_edges: 2,
            expected_traversal_candidates: 2,
        },
        ResolvedTraversalCase {
            // axum/src/handler/service.rs:171 calls
            // `Handler::call(handler, req, self.state.clone())`.
            // Callee binding: axum/src/handler/mod.rs:153 trait method.
            label: "axum Handler::call trait method path",
            target: method_id_by_trait_name(&db, "Handler", "call")?,
            expected_call_edges: 1,
            expected_traversal_candidates: 1,
        },
        ResolvedTraversalCase {
            // axum-core/src/ext_traits/request.rs:268 calls
            // `self.extract_with_state(&())`. Callee is the same impl method
            // with body `E::from_request(self, state)`.
            label: "axum-core RequestExt extract self-call",
            target: method_id_by_name_and_body_substring(
                &db,
                "extract_with_state",
                "E::from_request(self, state)",
            )?,
            expected_call_edges: 1,
            expected_traversal_candidates: 1,
        },
        ResolvedTraversalCase {
            // axum-core/src/ext_traits/request_parts.rs:122 calls
            // `self.extract_with_state(&())`. Callee is the same impl method
            // with body `E::from_request_parts(self, state)`.
            // axum-core/src/ext_traits/request_parts.rs:164 also calls
            // `parts.extract_with_state::<State<String>, String>(&state)`
            // after destructuring `Request::new(()).into_parts()`.
            // axum-core/src/ext_traits/request_parts.rs:186 calls
            // `parts.extract_with_state(state)` through the local extension
            // trait impl `RequestPartsExt for Parts`.
            label: "axum-core RequestPartsExt extract self-call",
            target: method_id_by_name_and_body_substring(
                &db,
                "extract_with_state",
                "E::from_request_parts(self, state)",
            )?,
            expected_call_edges: 3,
            expected_traversal_candidates: 3,
        },
        ResolvedTraversalCase {
            // axum-core/src/ext_traits/request.rs:279 calls
            // `E::from_request(self, state)` from `E: FromRequest<S, M>`.
            // axum-core/src/extract/mod.rs:127 calls
            // `T::from_request(req, state)` from `T: FromRequest<S>`.
            // Callee binding: axum-core/src/extract/mod.rs:85 trait method.
            label: "axum-core FromRequest::from_request trait-associated paths",
            target: method_id_by_trait_name(&db, "FromRequest", "from_request")?,
            expected_call_edges: 2,
            expected_traversal_candidates: 2,
        },
        ResolvedTraversalCase {
            // axum-core/src/ext_traits/request.rs:305 and
            // ext_traits/request_parts.rs:133 call
            // `E::from_request_parts(...)` from `E: FromRequestParts<S>`.
            // axum-core/src/extract/mod.rs:115 calls
            // `T::from_request_parts(parts, state)` from
            // `T: FromRequestParts<S>`.
            // axum-core/src/extract/mod.rs:103 calls
            // `Self::from_request_parts(...)` from the nested async block
            // inside the ViaParts blanket impl.
            // Callee binding: axum-core/src/extract/mod.rs:59 trait method.
            label: "axum-core FromRequestParts::from_request_parts trait-associated paths",
            target: method_id_by_trait_name(&db, "FromRequestParts", "from_request_parts")?,
            expected_call_edges: 4,
            expected_traversal_candidates: 4,
        },
        ResolvedTraversalCase {
            // axum-core/src/ext_traits/mod.rs:25 and
            // axum-core/src/ext_traits/mod.rs:45 call
            // `InnerState::from_ref(state)` through
            // `InnerState: FromRef<OuterState>` and `String::from_ref(state)`
            // through `String: FromRef<S>`. axum/src/extract/state.rs:309
            // calls `InnerState::from_ref(state)` through the workspace
            // dependency root `axum_core::extract::FromRef`. The nested local
            // impl method in middleware/from_extractor.rs:328 calls
            // `Secret::from_ref(state)` through `Secret: FromRef<S>`. Callee
            // binding: axum-core/src/extract/from_ref.rs:15 trait method.
            label: "axum-core FromRef::from_ref bounded associated paths",
            target: method_id_by_trait_name(&db, "FromRef", "from_ref")?,
            expected_call_edges: 4,
            expected_traversal_candidates: 4,
        },
        ResolvedTraversalCase {
            // Router::new is defined at axum/src/routing/mod.rs:162. The
            // matrix includes many real `Router::new()` callsites; the
            // regenerated fixture also resolves axum/src/routing/mod.rs:109
            // `Self::new()` from `Default for Router` and
            // axum/src/routing/method_routing.rs:1494 `crate::Router::new()`,
            // plus the axum-core request_parts workspace-import row.
            label: "axum Router::new current resolved fanout",
            target: method_id_by_name_and_body_substring(&db, "new", "default_fallback: true")?,
            expected_call_edges: 310,
            expected_traversal_candidates: 204,
        },
    ];

    for case in cases {
        assert_resolved_callers_traverse_from_owners(&db, case)?;
    }

    Ok(())
}

#[test]
fn axum_real_target_supported_callers_match_oracle_shape_counts() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix:
    //   docs/active/agents/call-graph/
    //   2026-06-28_real-corpus-call-site-oracle-matrices.md
    //
    // This is the source-shape companion to the traversal test above. It keeps
    // the supported rows table-driven at the target level: each resolved target
    // must expose the inspected caller shape fanout, each caller row must map
    // to exactly one persisted call_relation edge, and the target expansion
    // must retain a one-hop path from at least one inspected caller owner to
    // the callee. Per-owner tests in sibling modules pin exact file/line source
    // comments for low-fanout rows; this table protects the consolidated
    // target-centered shape contract.
    let cases = vec![
        ResolvedShapeCase {
            // Callers:
            //   axum-macros/src/lib.rs:724 expand(...)
            //   axum-macros/src/lib.rs:739 expand(...)
            // Callee: axum-macros/src/lib.rs root helper `expand`.
            label: "axum-macros root expand helper callers",
            target: function_id_by_name_in_module(&db, &["crate"], "expand")?,
            expected: vec![("path:expand", 2)],
        },
        ResolvedShapeCase {
            // Callers:
            //   axum-macros/src/typed_path.rs:23 crate::attr_parsing::parse_attrs
            //   axum-macros/src/from_ref.rs:30 parse_attrs
            //   axum-macros/src/from_request/mod.rs:{112,196,471,598,727,892,908,1029,1039}
            // Callee: axum-macros/src/attr_parsing.rs:59 parse_attrs.
            label: "axum-macros parse_attrs current resolved fanout",
            target: function_id_by_name_in_module(&db, &["crate", "attr_parsing"], "parse_attrs")?,
            expected: vec![
                ("path:crate::attr_parsing::parse_attrs", 1),
                ("path:parse_attrs", 10),
            ],
        },
        ResolvedShapeCase {
            // Callers:
            //   axum-macros/src/debug_handler.rs:{885,890}
            //   axum-macros/src/typed_path.rs:443
            //   axum-macros/src/from_ref.rs:104
            //   axum-macros/src/from_request/mod.rs:1050
            // Callee: axum-macros/src/lib.rs:797 run_ui_tests.
            label: "axum-macros run_ui_tests helper callers",
            target: function_id_by_name_in_module(&db, &["crate"], "run_ui_tests")?,
            expected: vec![("path:crate::run_ui_tests", 5)],
        },
        ResolvedShapeCase {
            // Callers:
            //   axum-core/src/body.rs:{110,116} Self::empty()
            //   axum-core/src/response/into_response.rs and
            //   axum-core/src/ext_traits/request.rs Body::empty() rows,
            //   plus direct axum workspace-import, local re-exported
            //   workspace-import, inherited `super::*`, and nested
            //   closure/local-item Body::empty() rows.
            // Callee: axum-core/src/body.rs:52 Body::empty.
            label: "axum-core Body::empty current resolved subset",
            target: method_id_by_name_and_body_substring(&db, "empty", "Empty::new()")?,
            expected: vec![("path:Body::empty", 21), ("path:Self::empty", 2)],
        },
        ResolvedShapeCase {
            // Callers:
            //   axum/src/json.rs:{112,128} Self::from_bytes(&bytes)
            // Callee: axum/src/json.rs:164 Json::from_bytes.
            label: "axum Json::from_bytes trait-impl Self callers",
            target: method_id_by_name_and_body_substring(
                &db,
                "from_bytes",
                "serde_json::Deserializer::from_slice(bytes)",
            )?,
            expected: vec![("path:Self::from_bytes", 2)],
        },
        ResolvedShapeCase {
            // Callers: axum-core/src/body.rs same-module try_downcast(...) rows.
            // Callee: axum-core/src/body.rs:26 try_downcast.
            label: "axum-core try_downcast current resolved subset",
            target: function_id_by_name_in_module(&db, &["crate", "body"], "try_downcast")?,
            expected: vec![("path:try_downcast", 2)],
        },
        ResolvedShapeCase {
            // Caller: axum/src/routing/mod.rs:205 imported try_downcast(...).
            // Callee: axum/src/util.rs:99 try_downcast.
            label: "axum try_downcast current resolved subset",
            target: function_id_by_name_in_module(&db, &["crate", "util"], "try_downcast")?,
            expected: vec![("path:try_downcast", 1)],
        },
        ResolvedShapeCase {
            // Callers: axum/src/boxed.rs:{23,38,51} Self(...),
            // BoxedIntoRoute(...), and Self(...).
            // Callee binding: axum/src/boxed.rs:12 tuple struct.
            label: "axum BoxedIntoRoute tuple constructor",
            target: struct_id_by_name(&db, "BoxedIntoRoute")?,
            expected: vec![("path:BoxedIntoRoute", 1), ("path:Self", 2)],
        },
        ResolvedShapeCase {
            // Caller: axum-macros/src/with_position.rs:92 Position::First(item).
            // Callee binding: axum-macros/src/with_position.rs:66 enum variant.
            label: "axum-macros Position::First enum variant constructor",
            target: variant_id_by_enum_and_variant_names(&db, "Position", "First")?,
            expected: vec![("path:Position::First", 1)],
        },
        ResolvedShapeCase {
            // Callers:
            //   axum/src/error_handling/mod.rs:65 HandleError::new(...)
            //   axum/src/service_ext.rs:43 HandleError::new(self, f)
            // Callee: axum/src/error_handling/mod.rs:80 HandleError::new.
            label: "axum HandleError::new constructor callers",
            target: method_id_by_name_and_body_substring(
                &db,
                "new",
                "Self { inner, f, _extractor",
            )?,
            expected: vec![("path:HandleError::new", 2)],
        },
        ResolvedShapeCase {
            // Caller: axum/src/handler/service.rs:171 Handler::call(...).
            // Callee binding: axum/src/handler/mod.rs:153 trait method.
            label: "axum Handler::call trait method path",
            target: method_id_by_trait_name(&db, "Handler", "call")?,
            expected: vec![("path:Handler::call", 1)],
        },
        ResolvedShapeCase {
            // Caller: axum-core/src/ext_traits/request.rs:268
            // self.extract_with_state(&()).
            // Callee: same impl method body containing E::from_request(...).
            label: "axum-core RequestExt extract self-call",
            target: method_id_by_name_and_body_substring(
                &db,
                "extract_with_state",
                "E::from_request(self, state)",
            )?,
            expected: vec![("method:self.extract_with_state", 1)],
        },
        ResolvedShapeCase {
            // Caller: axum-core/src/ext_traits/request_parts.rs:122
            // self.extract_with_state(&()).
            // Caller: axum-core/src/ext_traits/request_parts.rs:164
            // parts.extract_with_state::<State<String>, String>(&state)
            // where `parts` comes from `Request::new(()).into_parts()`.
            // Caller: axum-core/src/ext_traits/request_parts.rs:186
            // parts.extract_with_state(state).
            // Callee: same impl method body containing E::from_request_parts(...).
            label: "axum-core RequestPartsExt extract self-call",
            target: method_id_by_name_and_body_substring(
                &db,
                "extract_with_state",
                "E::from_request_parts(self, state)",
            )?,
            expected: vec![
                ("method:self.extract_with_state", 1),
                (
                    "method:LocalBinding { name: \"parts\" }.extract_with_state",
                    1,
                ),
                (
                    "method:TupleMethodReturn { name: \"parts\", method_name: \"into_parts\", method_span: (4640, 4669), index: 0 }.extract_with_state",
                    1,
                ),
            ],
        },
        ResolvedShapeCase {
            // Callers:
            //   axum-core/src/ext_traits/request.rs:279 E::from_request(...)
            //   axum-core/src/extract/mod.rs:127 T::from_request(...)
            // Callee binding: axum-core/src/extract/mod.rs:85 trait method.
            label: "axum-core FromRequest::from_request trait-associated paths",
            target: method_id_by_trait_name(&db, "FromRequest", "from_request")?,
            expected: vec![("path:E::from_request", 1), ("path:T::from_request", 1)],
        },
        ResolvedShapeCase {
            // Callers:
            //   axum-core/src/ext_traits/request.rs:305 E::from_request_parts(...)
            //   axum-core/src/ext_traits/request_parts.rs:133 E::from_request_parts(...)
            //   axum-core/src/extract/mod.rs:115 T::from_request_parts(...)
            //   axum-core/src/extract/mod.rs:103 async-block Self::from_request_parts(...)
            // Callee binding: axum-core/src/extract/mod.rs:59 trait method.
            label: "axum-core FromRequestParts::from_request_parts trait-associated paths",
            target: method_id_by_trait_name(&db, "FromRequestParts", "from_request_parts")?,
            expected: vec![
                ("path:E::from_request_parts", 2),
                ("path:T::from_request_parts", 1),
                ("path:Self::from_request_parts", 1),
            ],
        },
        ResolvedShapeCase {
            // Callers:
            //   axum-core/src/ext_traits/mod.rs:25 InnerState::from_ref(state)
            //   axum-core/src/ext_traits/mod.rs:45 String::from_ref(state)
            //   axum/src/extract/state.rs:309 InnerState::from_ref(state)
            //   axum/src/middleware/from_extractor.rs:328 Secret::from_ref(state)
            // Callee binding: axum-core/src/extract/from_ref.rs:15 trait method.
            label: "axum-core FromRef::from_ref bounded associated paths",
            target: method_id_by_trait_name(&db, "FromRef", "from_ref")?,
            expected: vec![
                ("path:InnerState::from_ref", 2),
                ("path:String::from_ref", 1),
                ("path:Secret::from_ref", 1),
            ],
        },
        ResolvedShapeCase {
            // Callee: axum/src/routing/mod.rs:162 Router::new.
            // Source oracle includes many Router::new() callsites. The current
            // fixture resolves 308 literal Router::new rows, plus
            // axum/src/routing/mod.rs:109 Self::new() from Default and
            // axum/src/routing/method_routing.rs:1494 crate::Router::new().
            label: "axum Router::new current resolved fanout",
            target: method_id_by_name_and_body_substring(&db, "new", "default_fallback: true")?,
            expected: vec![
                ("path:Router::new", 308),
                ("path:Self::new", 1),
                ("path:crate::Router::new", 1),
            ],
        },
    ];

    for case in cases {
        assert_resolved_shape_counts(&db, case)?;
    }

    Ok(())
}

fn assert_resolved_callers_traverse_from_owners(
    db: &Database,
    case: ResolvedTraversalCase,
) -> Result<(), DbError> {
    let callers = db.callers_for_target(case.target)?;
    assert_eq!(
        callers.len(),
        case.expected_call_edges,
        "{} should expose exactly {} resolved caller edge(s): {callers:#?}",
        case.label,
        case.expected_call_edges
    );
    assert_sites_match_callers(db, case.target, &callers, case.label)?;

    let mut sites_by_owner = BTreeMap::<Uuid, BTreeSet<Uuid>>::new();
    for caller in &callers {
        assert_eq!(
            caller.status.status,
            CallStatusKind::Resolved,
            "{} should only use resolved caller rows: {caller:#?}",
            case.label
        );
        assert_eq!(
            caller.status.resolution,
            Some(CallResolutionKind::LocalExact),
            "{} should only use local-exact caller rows: {caller:#?}",
            case.label
        );
        assert_eq!(
            caller.target.target_id, case.target,
            "{} returned a caller for the wrong target: {caller:#?}",
            case.label
        );
        assert_raw_relation_matches_caller(db, caller, case.label)?;
        sites_by_owner
            .entry(caller.site.owner_id)
            .or_default()
            .insert(caller.site.id);
    }

    let incoming = db.expand_call_context(
        CallContextSeed::Target(case.target),
        CallContextOptions {
            include_outgoing_targets: false,
            max_candidates: 2048,
            ..CallContextOptions::default()
        },
    )?;
    let incoming_count = incoming
        .iter()
        .filter(|candidate| {
            candidate.target_id == case.target
                && candidate.relation == ploke_db::CallContextRelation::IncomingCaller
                && candidate.distance == 1
        })
        .count();
    assert_eq!(
        incoming_count, case.expected_traversal_candidates,
        "{} should expose exactly {} one-hop incoming traversal candidate(s): {incoming:#?}",
        case.label, case.expected_traversal_candidates
    );
    let caller_sites = sites_by_owner
        .values()
        .flat_map(|sites| sites.iter().copied())
        .collect::<BTreeSet<_>>();
    for candidate in incoming.iter().filter(|candidate| {
        candidate.target_id == case.target
            && candidate.relation == ploke_db::CallContextRelation::IncomingCaller
            && candidate.distance == 1
    }) {
        assert!(
            sites_by_owner
                .get(&candidate.node_id)
                .is_some_and(|sites| sites.contains(&candidate.call_site_id)),
            "{} target traversal candidate should point back to an inspected caller/site pair: {candidate:#?}; callers: {sites_by_owner:#?}",
            case.label
        );
        assert!(
            caller_sites.contains(&candidate.call_site_id),
            "{} target traversal should not invent call-site ids outside callers_for_target: {candidate:#?}",
            case.label
        );
    }

    for (owner, site_ids) in sites_by_owner {
        let outgoing = db.expand_call_context(
            CallContextSeed::Owner(owner),
            CallContextOptions {
                include_incoming_callers: false,
                max_candidates: 2048,
                ..CallContextOptions::default()
            },
        )?;

        assert!(
            outgoing.iter().any(|candidate| {
                candidate.node_id == case.target
                    && candidate.target_id == case.target
                    && candidate.relation == ploke_db::CallContextRelation::OutgoingTarget
                    && candidate.distance == 1
            }),
            "{} should traverse owner {owner} to target {} in one hop for caller site(s) {site_ids:#?}: {outgoing:#?}",
            case.label,
            case.target
        );
        let outgoing_target_matches = outgoing
            .iter()
            .filter(|candidate| {
                candidate.node_id == case.target
                    && candidate.target_id == case.target
                    && candidate.relation == ploke_db::CallContextRelation::OutgoingTarget
                    && candidate.distance == 1
            })
            .collect::<Vec<_>>();
        assert_eq!(
            outgoing_target_matches.len(),
            1,
            "{} should expose one de-duplicated owner-to-target traversal candidate for owner {owner}: {outgoing:#?}",
            case.label
        );
        assert!(
            site_ids.contains(&outgoing_target_matches[0].call_site_id),
            "{} owner traversal should use one of the inspected caller sites for owner {owner}: {outgoing_target_matches:#?}; expected sites: {site_ids:#?}",
            case.label
        );
    }

    Ok(())
}

fn assert_resolved_shape_counts(db: &Database, case: ResolvedShapeCase) -> Result<(), DbError> {
    let callers = db.callers_for_target(case.target)?;
    assert_sites_match_callers(db, case.target, &callers, case.label)?;

    let mut actual = BTreeMap::<String, usize>::new();
    let mut caller_owners = BTreeSet::<Uuid>::new();
    for caller in &callers {
        assert_eq!(
            caller.status.status,
            CallStatusKind::Resolved,
            "{} should only use resolved caller rows: {caller:#?}",
            case.label
        );
        assert_eq!(
            caller.status.resolution,
            Some(CallResolutionKind::LocalExact),
            "{} should only use local-exact caller rows: {caller:#?}",
            case.label
        );
        assert_eq!(
            caller.target.target_id, case.target,
            "{} returned a caller for the wrong target: {caller:#?}",
            case.label
        );
        assert_raw_relation_matches_caller(db, caller, case.label)?;
        *actual.entry(call_shape_key(caller)).or_default() += 1;
        caller_owners.insert(caller.site.owner_id);
    }

    let expected = case
        .expected
        .into_iter()
        .map(|(shape, count)| (shape.to_string(), count))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        actual, expected,
        "{} should match the oracle matrix caller shape fanout",
        case.label
    );

    let incoming = db.expand_call_context(
        CallContextSeed::Target(case.target),
        CallContextOptions {
            include_outgoing_targets: false,
            max_candidates: 2048,
            ..CallContextOptions::default()
        },
    )?;
    let incoming_owners = incoming
        .iter()
        .filter(|candidate| {
            candidate.target_id == case.target
                && candidate.relation == ploke_db::CallContextRelation::IncomingCaller
                && candidate.distance == 1
        })
        .map(|candidate| candidate.node_id)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        incoming_owners, caller_owners,
        "{} should preserve one-hop traversal from each inspected caller owner to the callee",
        case.label
    );

    Ok(())
}

fn call_shape_key(caller: &ploke_db::CallCallerRow) -> String {
    match caller.site.kind {
        CallSiteKind::Path => format!(
            "path:{}",
            caller
                .site
                .path
                .as_ref()
                .expect("resolved path caller should carry path")
                .join("::")
        ),
        CallSiteKind::Method => {
            let method = caller
                .site
                .method
                .as_ref()
                .expect("resolved method caller should carry method name");
            let receiver = match caller.site.receiver.as_ref() {
                Some(CallReceiver::SelfValue) => "self".to_string(),
                Some(other) => format!("{other:?}"),
                None => "<none>".to_string(),
            };
            format!("method:{receiver}.{method}")
        }
        other => format!("{other:?}"),
    }
}

fn assert_raw_relation_matches_caller(
    db: &Database,
    caller: &ploke_db::CallCallerRow,
    label: &str,
) -> Result<(), DbError> {
    let raw = relations_for_site(db, caller.site.id)?;
    let mut matching = 0usize;
    for row in &raw.rows {
        if to_uuid(&row[0])? == caller.target.target_id
            && data_str(&row[1], "relation_kind") == format!("{:?}", caller.target.relation)
            && data_str(&row[2], "source_kind") == format!("{:?}", caller.target.source_kind)
            && data_str(&row[3], "target_kind") == format!("{:?}", caller.target.target_kind)
        {
            matching += 1;
        }
    }
    assert_eq!(
        matching, 1,
        "{label} callers_for_target row should correspond to exactly one persisted call_relation edge: caller={caller:#?}; raw={raw:#?}"
    );
    Ok(())
}
