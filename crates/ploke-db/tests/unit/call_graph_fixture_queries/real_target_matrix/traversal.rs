use std::collections::BTreeMap;

use uuid::Uuid;

use super::super::*;
use super::common::*;

struct ResolvedTraversalCase {
    label: &'static str,
    target: Uuid,
    expected_call_edges: usize,
    expected_traversal_candidates: usize,
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
            // from_request/mod.rs:{112,196,592,715,880,896} call the imported
            // helper. Callee: axum-macros/src/attr_parsing.rs:59.
            label: "axum-macros parse_attrs current resolved fanout",
            target: function_id_by_name_in_module(&db, &["crate", "attr_parsing"], "parse_attrs")?,
            expected_call_edges: 8,
            expected_traversal_candidates: 6,
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
            // rows call `Body::empty()`. Callee: axum-core/src/body.rs:52.
            label: "axum-core Body::empty current resolved subset",
            target: method_id_by_name_and_body_substring(&db, "empty", "Empty::new()")?,
            expected_call_edges: 4,
            expected_traversal_candidates: 4,
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
            // axum/src/boxed.rs:38 calls the tuple-struct constructor
            // `BoxedIntoRoute(...)`. Callee binding: axum/src/boxed.rs:12.
            label: "axum BoxedIntoRoute tuple constructor",
            target: struct_id_by_name(&db, "BoxedIntoRoute")?,
            expected_call_edges: 1,
            expected_traversal_candidates: 1,
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
            label: "axum-core RequestPartsExt extract self-call",
            target: method_id_by_name_and_body_substring(
                &db,
                "extract_with_state",
                "E::from_request_parts(self, state)",
            )?,
            expected_call_edges: 1,
            expected_traversal_candidates: 1,
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
            // Callee binding: axum-core/src/extract/mod.rs:59 trait method.
            label: "axum-core FromRequestParts::from_request_parts trait-associated paths",
            target: method_id_by_trait_name(&db, "FromRequestParts", "from_request_parts")?,
            expected_call_edges: 3,
            expected_traversal_candidates: 3,
        },
        ResolvedTraversalCase {
            // axum-core/src/ext_traits/mod.rs:25 and
            // axum-core/src/ext_traits/mod.rs:45 call
            // `InnerState::from_ref(state)` through
            // `InnerState: FromRef<OuterState>` and `String::from_ref(state)`
            // through `String: FromRef<S>`. Callee binding:
            // axum-core/src/extract/from_ref.rs:15 trait method. The
            // axum/src dependency-root `axum_core::extract::FromRef` rows are
            // asserted separately as unsupported.
            label: "axum-core FromRef::from_ref same-crate bounded associated paths",
            target: method_id_by_trait_name(&db, "FromRef", "from_ref")?,
            expected_call_edges: 2,
            expected_traversal_candidates: 2,
        },
        ResolvedTraversalCase {
            // Router::new is defined at axum/src/routing/mod.rs:162. The
            // matrix includes many real `Router::new()` callsites; the
            // regenerated fixture also resolves axum/src/routing/mod.rs:109
            // `Self::new()` from `Default for Router` and
            // axum/src/routing/method_routing.rs:1494 `crate::Router::new()`.
            label: "axum Router::new current resolved fanout",
            target: method_id_by_name_and_body_substring(&db, "new", "default_fallback: true")?,
            expected_call_edges: 144,
            expected_traversal_candidates: 123,
        },
    ];

    for case in cases {
        assert_resolved_callers_traverse_from_owners(&db, case)?;
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

    let mut sites_by_owner = BTreeMap::<Uuid, Vec<Uuid>>::new();
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
        sites_by_owner
            .entry(caller.site.owner_id)
            .or_default()
            .push(caller.site.id);
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
    }

    Ok(())
}
