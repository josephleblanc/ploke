use std::collections::{BTreeMap, BTreeSet};

use cozo::DataValue;
use ploke_db::multi_embedding::db_ext::{ANCESTOR_RULES_NOW, METHOD_NODE_ANCESTOR_RULE};

use super::super::super::super::super::*;
use super::expected::path;

mod remaining;
mod targetless;

fn setup_axum_call_graph_rag() -> Result<(Arc<Database>, RagService), Error> {
    let db = Arc::new(fresh_backup_fixture_db(
        &ploke_test_utils::CORPUS_AXUM_CALL_GRAPH,
    )?);
    assert!(
        db.has_call_graph_relations()?,
        "corpus_axum_call_graph must include call graph relations for RAG call-context tests"
    );

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.call_context_degraded(),
        "axum call graph backup should enable RAG call context"
    );

    Ok((db, rag))
}

#[tokio::test]
async fn call_context_exact_reads_axum_body_empty_incoming_callers() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    let target = method_id_by_name_and_body_substring(&db, "empty", "Empty::new()")?;

    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        4,
        "current axum fixture should resolve exactly the four axum-core Body::empty callers: {callers:#?}"
    );

    let context = rag.exact_call_context(target)?;
    let incoming = context
        .iter()
        .filter(|call| {
            call.kind == CallSiteKind::Path
                && call
                    .targets
                    .iter()
                    .any(|candidate| candidate.target_id == target)
        })
        .collect::<Vec<_>>();

    // Matrix: `Body::empty` re-exported constructor row.
    // Source chain:
    //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
    //   axum-core/src/body.rs:52 defines `Body::empty`.
    //   axum-core/src/body.rs:110 and :116 call `Self::empty()`.
    //   axum-core/src/response/into_response.rs response conversion rows call
    //   `Body::empty()`.
    // Expected traversal for the current fixture: the RAG exact call-context
    // path preserves the same four incoming caller-site edges exposed by
    // `Database::callers_for_target`. Broader axum re-export fanout remains a
    // separate import/re-export completeness gap tracked by the matrix.
    assert_eq!(
        incoming.len(),
        4,
        "RAG exact call context should expose all current Body::empty incoming edges: {context:#?}"
    );

    let expected_site_ids = callers
        .iter()
        .map(|caller| caller.site.id)
        .collect::<BTreeSet<_>>();
    let incoming_site_ids = incoming
        .iter()
        .map(|call| call.site_id)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        incoming_site_ids, expected_site_ids,
        "RAG call context should preserve the DB caller site identities"
    );

    let mut path_counts = BTreeMap::<Vec<String>, usize>::new();
    for call in incoming {
        assert_eq!(call.status, CallStatusKind::Resolved);
        assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(call.targets.len(), 1);
        assert_eq!(call.targets[0].target_id, target);
        assert_eq!(call.targets[0].relation, CallTargetKind::AssociatedFunction);
        let CallCalleeInfo::Path { path } = &call.callee else {
            panic!("Body::empty incoming caller should be a path call: {call:#?}");
        };
        *path_counts.entry(path.clone()).or_default() += 1;
    }
    assert_eq!(
        path_counts,
        BTreeMap::from([(path(&["Body", "empty"]), 2), (path(&["Self", "empty"]), 2),]),
        "RAG call context should preserve literal Body::empty and trait-impl Self::empty path shapes"
    );

    Ok(())
}

#[tokio::test]
async fn call_context_exact_reads_axum_parse_attrs_incoming_callers() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    let target = function_id_by_name_in_module(&db, &["crate", "attr_parsing"], "parse_attrs")?;

    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        8,
        "current axum fixture should resolve the eight parse_attrs caller sites: {callers:#?}"
    );

    let context = rag.exact_call_context(target)?;
    let incoming = context
        .iter()
        .filter(|call| {
            call.kind == CallSiteKind::Path
                && call
                    .targets
                    .iter()
                    .any(|candidate| candidate.target_id == target)
        })
        .collect::<Vec<_>>();

    // Matrix: `parse_attrs` path/import row.
    // Source chain:
    //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
    //   axum-macros/src/attr_parsing.rs:59 defines `parse_attrs`.
    //   axum-macros/src/typed_path.rs:23 calls
    //   `crate::attr_parsing::parse_attrs(...)`.
    //   from_ref.rs:30 and from_request/mod.rs:{112,196,592,715,880,896}
    //   call imported `parse_attrs(...)`.
    // Expected traversal: RAG exact call context preserves the same eight
    // incoming caller-site edges exposed by `Database::callers_for_target`.
    assert_eq!(
        incoming.len(),
        8,
        "RAG exact call context should expose all current parse_attrs incoming edges: {context:#?}"
    );

    let expected_site_ids = callers
        .iter()
        .map(|caller| caller.site.id)
        .collect::<BTreeSet<_>>();
    let incoming_site_ids = incoming
        .iter()
        .map(|call| call.site_id)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        incoming_site_ids, expected_site_ids,
        "RAG call context should preserve the DB parse_attrs caller site identities"
    );

    let mut path_counts = BTreeMap::<Vec<String>, usize>::new();
    for call in incoming {
        assert_eq!(call.status, CallStatusKind::Resolved);
        assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(call.targets.len(), 1);
        assert_eq!(call.targets[0].target_id, target);
        assert_eq!(call.targets[0].relation, CallTargetKind::Function);
        let CallCalleeInfo::Path { path } = &call.callee else {
            panic!("parse_attrs incoming caller should be a path call: {call:#?}");
        };
        *path_counts.entry(path.clone()).or_default() += 1;
    }
    assert_eq!(
        path_counts,
        BTreeMap::from([
            (path(&["crate", "attr_parsing", "parse_attrs"]), 1),
            (path(&["parse_attrs"]), 7),
        ]),
        "RAG call context should preserve explicit and imported parse_attrs path shapes"
    );

    Ok(())
}

#[tokio::test]
async fn call_context_exact_reads_axum_json_from_bytes_incoming_callers() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    let target = method_id_by_name_and_body_substring(
        &db,
        "from_bytes",
        "serde_json::Deserializer::from_slice(bytes)",
    )?;

    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        2,
        "current axum fixture should resolve the two Json::from_bytes caller sites: {callers:#?}"
    );

    let context = rag.exact_call_context(target)?;
    let incoming = context
        .iter()
        .filter(|call| {
            call.kind == CallSiteKind::Path
                && call
                    .targets
                    .iter()
                    .any(|candidate| candidate.target_id == target)
        })
        .collect::<Vec<_>>();

    // Matrix: `Json::from_bytes` inherent method row.
    // Source chain:
    //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
    //   axum/src/json.rs:164 defines `Json::from_bytes`.
    //   axum/src/json.rs:112 and :128 call `Self::from_bytes(&bytes)`.
    // Expected traversal: RAG exact call context preserves both one-hop
    // `Self::from_bytes` associated-function edges exposed by
    // `Database::callers_for_target`.
    assert_eq!(
        incoming.len(),
        2,
        "RAG exact call context should expose both Json::from_bytes incoming edges: {context:#?}"
    );

    let expected_site_ids = callers
        .iter()
        .map(|caller| caller.site.id)
        .collect::<BTreeSet<_>>();
    let incoming_site_ids = incoming
        .iter()
        .map(|call| call.site_id)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        incoming_site_ids, expected_site_ids,
        "RAG call context should preserve the DB Json::from_bytes caller site identities"
    );

    let mut path_counts = BTreeMap::<Vec<String>, usize>::new();
    for call in incoming {
        assert_eq!(call.status, CallStatusKind::Resolved);
        assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(call.targets.len(), 1);
        assert_eq!(call.targets[0].target_id, target);
        assert_eq!(call.targets[0].relation, CallTargetKind::AssociatedFunction);
        let CallCalleeInfo::Path { path } = &call.callee else {
            panic!("Json::from_bytes incoming caller should be a path call: {call:#?}");
        };
        *path_counts.entry(path.clone()).or_default() += 1;
    }
    assert_eq!(
        path_counts,
        BTreeMap::from([(path(&["Self", "from_bytes"]), 2)]),
        "RAG call context should preserve the trait-impl Self::from_bytes path shape"
    );

    Ok(())
}

#[tokio::test]
async fn call_context_exact_reads_axum_boxed_into_route_constructor_callers() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    let target = struct_id_by_name(&db, "BoxedIntoRoute")?;

    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        1,
        "current axum fixture should resolve the explicit BoxedIntoRoute constructor caller: {callers:#?}"
    );

    let context = rag.exact_call_context(target)?;
    let expected_callee = CallCalleeInfo::Path {
        path: path(&["BoxedIntoRoute"]),
    };
    let incoming = context
        .iter()
        .filter(|call| {
            call.kind == CallSiteKind::Path
                && call.callee == expected_callee
                && call
                    .targets
                    .iter()
                    .any(|candidate| candidate.target_id == target)
        })
        .collect::<Vec<_>>();

    // Matrix: `BoxedIntoRoute` tuple-struct constructor row.
    // Source chain:
    //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
    //   axum/src/boxed.rs:12 defines `BoxedIntoRoute<S, E>(...)`.
    //   axum/src/boxed.rs:38 calls `BoxedIntoRoute(Box::new(...))`.
    // Expected traversal: RAG exact call context preserves the same one-hop
    // constructor edge exposed by `Database::callers_for_target`; the
    // `Self(...)` rows at boxed.rs:23 and :51 remain unsupported targetless
    // gaps and are not incoming callers for this struct target.
    assert_eq!(
        incoming.len(),
        1,
        "RAG exact call context should expose the current BoxedIntoRoute constructor edge: {context:#?}"
    );

    let call = incoming[0];
    assert_eq!(call.owner_id, callers[0].site.owner_id);
    assert_eq!(call.site_id, callers[0].site.id);
    assert_eq!(call.status, CallStatusKind::Resolved);
    assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(call.targets.len(), 1);
    assert_eq!(call.targets[0].target_id, target);
    assert_eq!(
        call.targets[0].relation,
        CallTargetKind::TupleStructConstructor
    );

    Ok(())
}

#[tokio::test]
async fn call_context_exact_reads_axum_handler_call_trait_method_caller() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    let target = method_id_by_trait_name(&db, "Handler", "call")?;
    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        1,
        "current axum fixture should resolve the Handler::call trait-method caller: {callers:#?}"
    );

    let context = rag.exact_call_context(target)?;
    let expected_callee = CallCalleeInfo::Path {
        path: path(&["Handler", "call"]),
    };
    let incoming = context
        .iter()
        .filter(|call| {
            call.kind == CallSiteKind::Path
                && call.callee == expected_callee
                && call
                    .targets
                    .iter()
                    .any(|candidate| candidate.target_id == target)
        })
        .collect::<Vec<_>>();

    // Matrix: `Handler::call` trait method path row.
    // Source chain:
    //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
    //   axum/src/handler/mod.rs:153 declares trait method `Handler::call`.
    //   axum/src/handler/service.rs:148 binds `H: Handler<T, S>`.
    //   axum/src/handler/service.rs:171 calls
    //   `Handler::call(handler, req, self.state.clone())`.
    // Expected traversal: RAG exact call context preserves the same one-hop
    // trait-method binding edge exposed by `Database::callers_for_target`.
    // Concrete runtime impl dispatch remains type-parameter dependent.
    assert_eq!(
        incoming.len(),
        1,
        "RAG exact call context should expose the current Handler::call trait-method edge: {context:#?}"
    );

    let call = incoming[0];
    assert_eq!(call.owner_id, callers[0].site.owner_id);
    assert_eq!(call.site_id, callers[0].site.id);
    assert_eq!(call.status, CallStatusKind::Resolved);
    assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(call.targets.len(), 1);
    assert_eq!(call.targets[0].target_id, target);
    assert_eq!(call.targets[0].relation, CallTargetKind::AssociatedFunction);

    Ok(())
}

#[tokio::test]
async fn call_paths_exact_reads_axum_request_extract_two_hop_trait_path() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    // Matrix:
    //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
    // Source-oracle chain:
    //   axum-core/src/ext_traits/request.rs:268
    //     `RequestExt::extract` calls `self.extract_with_state(&())`.
    //   axum-core/src/ext_traits/request.rs:279
    //     `RequestExt::extract_with_state` calls `E::from_request(self, state)`.
    //   axum-core/src/extract/mod.rs:85
    //     defines the `FromRequest::from_request` trait method binding.
    //
    // RAG should expose the DB's ordered resolved path so downstream prompt
    // assembly and tools can answer multi-hop call-chain questions without
    // reimplementing traversal over one-hop context rows.
    let start = method_id_by_file(
        &db,
        "extract",
        "self.extract_with_state(&())",
        "axum-core/src/ext_traits/request.rs",
    )?;
    let intermediate = method_id_by_file(
        &db,
        "extract_with_state",
        "E::from_request(self, state)",
        "axum-core/src/ext_traits/request.rs",
    )?;
    let target = method_id_by_trait_name(&db, "FromRequest", "from_request")?;

    let outgoing = rag.exact_call_paths_from_owner(
        start,
        CallPathOptions {
            max_depth: 2,
            max_paths: 16,
        },
    )?;
    let path = outgoing
        .iter()
        .find(|path| path.end_id == target && path.depth == 2)
        .unwrap_or_else(|| {
            panic!(
                "expected RAG two-hop path from RequestExt::extract to FromRequest::from_request: {outgoing:#?}"
            )
        });
    assert_eq!(path.start_id, start);
    assert_eq!(path.edges.len(), 2);
    assert_eq!(path.edges[0].caller_id, start);
    assert_eq!(path.edges[0].callee_id, intermediate);
    assert_eq!(path.edges[0].relation, CallTargetKind::Method);
    assert_eq!(path.edges[1].caller_id, intermediate);
    assert_eq!(path.edges[1].callee_id, target);
    assert_eq!(path.edges[1].relation, CallTargetKind::AssociatedFunction);
    let db_paths = db.call_paths_between(
        start,
        target,
        CallPathOptions {
            max_depth: 2,
            max_paths: 16,
        },
    )?;
    let db_path = db_paths
        .iter()
        .find(|path| path.start_id == start && path.end_id == target && path.depth == 2)
        .unwrap_or_else(|| panic!("expected DB two-hop path for RAG span oracle: {db_paths:#?}"));
    assert_eq!(
        path.edges[0].span, db_path.edges[0].span,
        "RAG first path edge should preserve the DB callsite span"
    );
    assert_eq!(
        path.edges[1].span, db_path.edges[1].span,
        "RAG second path edge should preserve the DB callsite span"
    );
    assert_call_path_node(
        path,
        start,
        "::extract",
        "axum-core/src/ext_traits/request.rs",
        "RAG outgoing two-hop path",
    );
    assert_call_path_node(
        path,
        intermediate,
        "::extract_with_state",
        "axum-core/src/ext_traits/request.rs",
        "RAG outgoing two-hop path",
    );
    assert_call_path_node(
        path,
        target,
        "::from_request",
        "axum-core/src/extract/mod.rs",
        "RAG outgoing two-hop path",
    );

    let incoming = rag.exact_call_paths_to_target(
        target,
        CallPathOptions {
            max_depth: 2,
            max_paths: 16,
        },
    )?;
    let reverse_path = incoming
        .iter()
        .find(|path| path.start_id == start && path.end_id == target && path.depth == 2)
        .unwrap_or_else(|| {
            panic!("expected RAG reverse path lookup to find the same two-hop chain: {incoming:#?}")
        });
    assert_eq!(reverse_path.edges, path.edges);
    assert_call_path_node(
        reverse_path,
        start,
        "::extract",
        "axum-core/src/ext_traits/request.rs",
        "RAG incoming two-hop path",
    );
    assert_call_path_node(
        reverse_path,
        intermediate,
        "::extract_with_state",
        "axum-core/src/ext_traits/request.rs",
        "RAG incoming two-hop path",
    );
    assert_call_path_node(
        reverse_path,
        target,
        "::from_request",
        "axum-core/src/extract/mod.rs",
        "RAG incoming two-hop path",
    );

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Security analysis / Performance work / Debugging:
    //   "Can this entrypoint reach this sink/helper/error-producing function?"
    //   "What ordered call path connects the two known symbols?"
    let one_hop = rag.exact_call_paths_between(
        start,
        target,
        CallPathOptions {
            max_depth: 1,
            max_paths: 16,
        },
    )?;
    assert!(
        one_hop.is_empty(),
        "RAG direct reachability should not skip the intermediate method: {one_hop:#?}"
    );

    let direct = rag.exact_call_paths_between(
        start,
        target,
        CallPathOptions {
            max_depth: 2,
            max_paths: 16,
        },
    )?;
    let direct_path = direct
        .iter()
        .find(|path| path.start_id == start && path.end_id == target && path.depth == 2)
        .unwrap_or_else(|| {
            panic!("expected RAG direct reachability to return the two-hop chain: {direct:#?}")
        });
    assert_eq!(direct_path.edges, path.edges);
    assert_call_path_node(
        direct_path,
        start,
        "::extract",
        "axum-core/src/ext_traits/request.rs",
        "RAG direct two-hop path",
    );
    assert_call_path_node(
        direct_path,
        intermediate,
        "::extract_with_state",
        "axum-core/src/ext_traits/request.rs",
        "RAG direct two-hop path",
    );
    assert_call_path_node(
        direct_path,
        target,
        "::from_request",
        "axum-core/src/extract/mod.rs",
        "RAG direct two-hop path",
    );

    Ok(())
}

#[tokio::test]
async fn call_context_expansion_reads_axum_two_hop_path_candidates() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    // Matrix:
    //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
    // Source-oracle chain:
    //   axum-core/src/ext_traits/request.rs:268
    //     `RequestExt::extract` calls `self.extract_with_state(&())`.
    //   axum-core/src/ext_traits/request.rs:279
    //     `RequestExt::extract_with_state` calls `E::from_request(self, state)`.
    //   axum-core/src/extract/mod.rs:85
    //     defines the `FromRequest::from_request` trait method binding.
    //
    // RAG expansion should use the DB's bounded path expansion so downstream
    // context retrieval can answer multi-hop call-chain questions without
    // requiring the terminal callee to be a direct retrieval hit.
    let start = method_id_by_file(
        &db,
        "extract",
        "self.extract_with_state(&())",
        "axum-core/src/ext_traits/request.rs",
    )?;
    let intermediate = method_id_by_file(
        &db,
        "extract_with_state",
        "E::from_request(self, state)",
        "axum-core/src/ext_traits/request.rs",
    )?;
    let target = method_id_by_trait_name(&db, "FromRequest", "from_request")?;

    let (expanded, expansion_info) = rag.expand_hits_with_call_context_info(&[(start, 1.0)])?;
    let expanded_ids = expanded.iter().map(|(id, _)| *id).collect::<Vec<_>>();
    assert!(
        expanded_ids.contains(&start)
            && expanded_ids.contains(&intermediate)
            && expanded_ids.contains(&target),
        "RAG call expansion should retain the seed, direct callee, and two-hop terminal target: expanded={expanded:#?}; expansion_info={expansion_info:#?}"
    );

    let target_info = expansion_info.get(&target).unwrap_or_else(|| {
        panic!("two-hop terminal target should carry CallExpansionInfo: {expansion_info:#?}")
    });
    assert_eq!(target_info.seed_id, start);
    assert_eq!(
        target_info.relation,
        ploke_core::rag_types::CallExpansionKind::OutgoingTarget
    );
    assert_eq!(target_info.target_id, target);
    assert_eq!(target_info.distance, 2);

    let target_score = expanded
        .iter()
        .find(|(id, _)| *id == target)
        .map(|(_, score)| *score)
        .expect("two-hop terminal target should be scored");
    assert_eq!(target_score, 0.25);

    Ok(())
}

#[tokio::test]
async fn call_impact_exact_reads_axum_usage_question_summary() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Impact analysis / Refactoring support:
    //   "Which callers eventually reach this function?"
    //   "Which callers need migration before this helper can be split or removed?"
    //
    // Source-oracle chain:
    //   axum-core/src/ext_traits/request.rs:268
    //     `RequestExt::extract` calls `self.extract_with_state(&())`.
    //   axum-core/src/ext_traits/request.rs:279
    //     `RequestExt::extract_with_state` calls `E::from_request(self, state)`.
    //   axum-core/src/extract/mod.rs:85
    //     defines the `FromRequest::from_request` trait method binding.
    let start = method_id_by_file(
        &db,
        "extract",
        "self.extract_with_state(&())",
        "axum-core/src/ext_traits/request.rs",
    )?;
    let intermediate = method_id_by_file(
        &db,
        "extract_with_state",
        "E::from_request(self, state)",
        "axum-core/src/ext_traits/request.rs",
    )?;
    let target = method_id_by_trait_name(&db, "FromRequest", "from_request")?;

    let report = rag
        .exact_call_impact_for_target(
            target,
            CallPathOptions {
                max_depth: 2,
                max_paths: 16,
            },
        )?
        .expect("call context enabled");
    assert_eq!(report.target.id, target);
    assert_eq!(report.target.kind, "Method");
    assert_eq!(report.target.name, "from_request");
    assert!(
        report
            .target
            .file_path
            .as_ref()
            .ends_with("axum-core/src/extract/mod.rs")
    );
    assert!(
        report
            .paths
            .iter()
            .any(|path| path.start_id == start && path.end_id == target && path.depth == 2),
        "RAG impact summary should preserve the two-hop incoming path: {report:#?}"
    );
    assert_call_node(
        &report.callers,
        start,
        "extract",
        "axum-core/src/ext_traits/request.rs",
        "RAG impact eventual callers",
    );
    assert_call_node(
        &report.callers,
        intermediate,
        "extract_with_state",
        "axum-core/src/ext_traits/request.rs",
        "RAG impact eventual callers",
    );
    assert_call_node(
        &report.direct_callers,
        intermediate,
        "extract_with_state",
        "axum-core/src/ext_traits/request.rs",
        "RAG impact direct callers",
    );
    assert_eq!(
        report.direct_call_sites.len(),
        2,
        "RAG impact summary should preserve both direct target-centered callsite rows: {report:#?}"
    );
    assert!(
        report.direct_call_sites.iter().any(|call| {
            call.owner_id == intermediate
                && matches!(
                    &call.callee,
                    CallCalleeInfo::Path { path: call_path }
                        if call_path == &path(&["E", "from_request"])
                )
                && call
                    .targets
                    .iter()
                    .any(|target_row| target_row.target_id == target)
        }),
        "RAG impact summary should include the E::from_request callsite row: {report:#?}"
    );
    assert!(
        report.public_callers.is_empty(),
        "RAG impact summary should preserve the DB's direct stored-public predicate: {report:#?}"
    );
    assert_call_source_file(
        &report.source_files,
        "axum-core/src/ext_traits/request.rs",
        "RAG impact source files",
    );
    assert_call_source_file(
        &report.source_files,
        "axum-core/src/extract/mod.rs",
        "RAG impact source files",
    );

    // Source oracle:
    //   axum-macros/src/lib.rs:377,426,665,715 call `expand_with(...)` from
    //   public proc-macro entrypoints. Proc-macro bodies are not visited yet,
    //   so the exact impact surface must remain fail-closed.
    let unsupported_target = function_id_by_name_in_module(&db, &["crate"], "expand_with")?;
    let unsupported = rag
        .exact_call_impact_for_target(
            unsupported_target,
            CallPathOptions {
                max_depth: 2,
                max_paths: 16,
            },
        )?
        .expect("call context enabled");
    assert_eq!(unsupported.target.name, "expand_with");
    assert!(
        unsupported.paths.is_empty()
            && unsupported.callers.is_empty()
            && unsupported.direct_callers.is_empty()
            && unsupported.direct_call_sites.is_empty()
            && unsupported.public_callers.is_empty(),
        "RAG impact summary must not fabricate unsupported proc-macro public callers: {unsupported:#?}"
    );
    assert_call_source_file(
        &unsupported.source_files,
        "axum-macros/src/lib.rs",
        "RAG unsupported impact source files",
    );

    Ok(())
}

#[tokio::test]
async fn call_impact_exact_buckets_axum_callers_by_test_source() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Dead code detection / Test planning:
    //   "Is this function only used by tests, or is it reachable from
    //   production entrypoints?"
    //   "Which tests should cover a change to this function?"
    //
    // Source oracle:
    //   axum/src/routing/mod.rs:162 defines `Router::new`.
    //   axum/src/routing/mod.rs:109 calls `Self::new()` from
    //   `Default for Router`.
    //   axum/src/serve/mod.rs:756 calls `Router::new()` from the
    //   `serve::tests::if_it_compiles_it_works` test helper.
    let target = method_id_by_name_and_body_substring(&db, "new", "default_fallback: true")?;
    let non_test_owner =
        method_id_by_file(&db, "default", "Self::new()", "axum/src/routing/mod.rs")?;
    let test_owner = function_id_by_name_in_module(
        &db,
        &["crate", "serve", "tests"],
        "if_it_compiles_it_works",
    )?;

    let report = rag
        .exact_call_impact_for_target(
            target,
            CallPathOptions {
                max_depth: 1,
                max_paths: 256,
            },
        )?
        .expect("call context enabled");
    assert!(
        !report.test_callers.is_empty() && !report.non_test_callers.is_empty(),
        "RAG Router::new impact should expose both test and non-test caller buckets: {report:#?}"
    );
    assert_eq!(
        report.test_callers.len() + report.non_test_callers.len(),
        report.callers.len(),
        "RAG test/non-test impact buckets should partition eventual callers: {report:#?}"
    );
    assert_call_node(
        &report.test_callers,
        test_owner,
        "if_it_compiles_it_works",
        "axum/src/serve/mod.rs",
        "RAG Router::new test impact callers",
    );
    assert_call_node(
        &report.non_test_callers,
        non_test_owner,
        "default",
        "axum/src/routing/mod.rs",
        "RAG Router::new non-test impact callers",
    );

    Ok(())
}

#[tokio::test]
async fn call_reach_exact_reads_axum_usage_question_summary() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Navigation / Security analysis / Performance work:
    //   "What functions does this request handler call directly?"
    //   "From this owner function, which local callees can I traverse to in
    //   the persisted graph?"
    //   "What call chains reach a known sink/helper?"
    //
    // Source-oracle chain:
    //   axum-core/src/ext_traits/request.rs:268
    //     `RequestExt::extract` calls `self.extract_with_state(&())`.
    //   axum-core/src/ext_traits/request.rs:279
    //     `RequestExt::extract_with_state` calls `E::from_request(self, state)`.
    //   axum-core/src/extract/mod.rs:85
    //     defines the `FromRequest::from_request` trait method binding.
    let start = method_id_by_file(
        &db,
        "extract",
        "self.extract_with_state(&())",
        "axum-core/src/ext_traits/request.rs",
    )?;
    let intermediate = method_id_by_file(
        &db,
        "extract_with_state",
        "E::from_request(self, state)",
        "axum-core/src/ext_traits/request.rs",
    )?;
    let target = method_id_by_trait_name(&db, "FromRequest", "from_request")?;

    let report = rag
        .exact_call_reach_for_owner(
            start,
            CallPathOptions {
                max_depth: 2,
                max_paths: 16,
            },
        )?
        .expect("call context enabled");
    assert_eq!(report.owner.id, start);
    assert_eq!(report.owner.kind, "Method");
    assert_eq!(report.owner.name, "extract");
    assert!(
        report
            .owner
            .file_path
            .as_ref()
            .ends_with("axum-core/src/ext_traits/request.rs")
    );
    assert!(
        report
            .paths
            .iter()
            .any(|path| path.start_id == start && path.end_id == target && path.depth == 2),
        "RAG reach summary should preserve the two-hop outgoing path: {report:#?}"
    );
    assert_call_node(
        &report.callees,
        intermediate,
        "extract_with_state",
        "axum-core/src/ext_traits/request.rs",
        "RAG reach eventual callees",
    );
    assert_call_node(
        &report.callees,
        target,
        "from_request",
        "axum-core/src/extract/mod.rs",
        "RAG reach eventual callees",
    );
    assert_call_node(
        &report.direct_callees,
        intermediate,
        "extract_with_state",
        "axum-core/src/ext_traits/request.rs",
        "RAG reach direct callees",
    );
    assert_eq!(
        report.direct_call_sites.len(),
        1,
        "RAG reach summary should preserve the exact resolved direct callsite row: {report:#?}"
    );
    let direct_site = report
        .direct_call_sites
        .iter()
        .find(|call| {
            call.owner_id == start
                && call.kind == CallSiteKind::Method
                && matches!(
                    &call.callee,
                    CallCalleeInfo::Method { name, .. } if name == "extract_with_state"
                )
                && call
                    .targets
                    .iter()
                    .any(|target_row| target_row.target_id == intermediate)
        })
        .unwrap_or_else(|| {
            panic!(
                "RAG reach summary should include the extract_with_state callsite row: {report:#?}"
            )
        });
    assert_eq!(direct_site.status, CallStatusKind::Resolved);
    assert_call_node(
        &report.public_callees,
        target,
        "from_request",
        "axum-core/src/extract/mod.rs",
        "RAG reach public callees",
    );
    assert_call_source_file(
        &report.source_files,
        "axum-core/src/ext_traits/request.rs",
        "RAG reach source files",
    );
    assert_call_source_file(
        &report.source_files,
        "axum-core/src/extract/mod.rs",
        "RAG reach source files",
    );

    // Source oracle:
    //   axum/src/json.rs:164 defines `Json::from_bytes`.
    //   axum/src/json.rs:184 calls
    //     `serde_json::Deserializer::from_slice(bytes)`.
    // Current contract: dependency-root path calls are visible as external
    // frontier rows but do not become local traversal edges.
    let json_owner = method_id_by_file(
        &db,
        "from_bytes",
        "serde_json::Deserializer::from_slice(bytes)",
        "axum/src/json.rs",
    )?;
    let json_report = rag
        .exact_call_reach_for_owner(
            json_owner,
            CallPathOptions {
                max_depth: 2,
                max_paths: 16,
            },
        )?
        .expect("call context enabled");
    assert!(
        json_report.paths.is_empty() && json_report.callees.is_empty(),
        "RAG reach should not fabricate local edges for external dependency calls: {json_report:#?}"
    );
    let frontier = json_report
        .frontier_calls
        .iter()
        .find(|call| {
            matches!(
                &call.callee,
                CallCalleeInfo::Path { path: call_path }
                    if call_path == &path(&["serde_json", "Deserializer", "from_slice"])
            )
        })
        .unwrap_or_else(|| {
            panic!(
                "RAG reach should include serde_json frontier row for Json::from_bytes: {json_report:#?}"
            )
        });
    assert_eq!(frontier.owner_id, json_owner);
    assert_eq!(frontier.status, CallStatusKind::External);
    assert!(
        frontier.targets.is_empty(),
        "external frontier call should remain targetless: {frontier:#?}"
    );
    let external_frontier = json_report
        .external_frontier_calls
        .iter()
        .find(|call| {
            matches!(
                &call.callee,
                CallCalleeInfo::Path { path: call_path }
                    if call_path == &path(&["serde_json", "Deserializer", "from_slice"])
            )
        })
        .unwrap_or_else(|| {
            panic!(
                "RAG reach should include serde_json in external frontier rows: {json_report:#?}"
            )
        });
    assert_eq!(external_frontier.owner_id, json_owner);
    assert_eq!(external_frontier.status, CallStatusKind::External);
    assert!(
        external_frontier.targets.is_empty(),
        "external-only frontier call should remain targetless: {external_frontier:#?}"
    );
    assert_call_source_file(
        &json_report.source_files,
        "axum/src/json.rs",
        "RAG external-frontier reach source files",
    );

    Ok(())
}

#[tokio::test]
async fn get_context_attaches_axum_request_extract_two_hop_call_paths() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(fresh_backup_fixture_db(
        &ploke_test_utils::CORPUS_AXUM_CALL_GRAPH,
    )?);
    assert!(
        db.has_call_graph_relations()?,
        "corpus_axum_call_graph must include call graph relations for assembled call-path tests"
    );

    // Matrix:
    //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
    // Source-oracle chain:
    //   axum-core/src/ext_traits/request.rs:268
    //     `RequestExt::extract` calls `self.extract_with_state(&())`.
    //   axum-core/src/ext_traits/request.rs:279
    //     `RequestExt::extract_with_state` calls `E::from_request(self, state)`.
    //   axum-core/src/extract/mod.rs:85
    //     defines the `FromRequest::from_request` trait method binding.
    //
    // This test exercises the public `get_context` path, proving RAG answers can
    // carry bounded multi-hop path context for a real target crate rather than
    // requiring a second exact edge-tool lookup.
    let start = method_id_by_file(
        &db,
        "extract",
        "self.extract_with_state(&())",
        "axum-core/src/ext_traits/request.rs",
    )?;
    let intermediate = method_id_by_file(
        &db,
        "extract_with_state",
        "E::from_request(self, state)",
        "axum-core/src/ext_traits/request.rs",
    )?;
    let target = method_id_by_trait_name(&db, "FromRequest", "from_request")?;
    let top_k = 64;

    let mut cfg = crate::RagConfig::default();
    cfg.type_context.enabled = false;
    cfg.proof_context.enabled = false;
    cfg.call_context.max_owner_hits = top_k;
    cfg.call_context.path_depth = 2;
    cfg.call_context.path_limit = 16;
    let rag = RagService::new_full(
        Arc::clone(&db),
        runtime_for(&db, EmbeddingProcessor::new_mock()),
        IoManagerHandle::new(),
        cfg,
    )?;
    rag.bm25_rebuild().await?;

    let query = "self.extract_with_state";
    let sparse_hits = rag
        .search_bm25_strict(query, top_k, LOADED_WORKSPACE_SCOPE)
        .await?;
    assert!(
        sparse_hits.iter().any(|(id, _)| *id == start),
        "source-level axum query should retrieve RequestExt::extract as a RAG seed: {sparse_hits:#?}"
    );

    let assembled = rag
        .get_context(
            query,
            top_k,
            &TokenBudget {
                max_total: 131072,
                per_file_max: 131072,
                per_part_max: 8192,
            },
            &RetrievalStrategy::Sparse { strict: Some(true) },
            LOADED_WORKSPACE_SCOPE,
        )
        .await?;
    let part = assembled
        .parts
        .iter()
        .find(|part| part.id == start)
        .unwrap_or_else(|| {
            panic!("assembled context should include RequestExt::extract: {assembled:#?}")
        });
    let path = part
        .call_paths_from_owner
        .iter()
        .find(|path| path.end_id == target && path.depth == 2)
        .unwrap_or_else(|| {
            panic!(
                "RequestExt::extract context part should carry the two-hop path to FromRequest::from_request: {part:#?}"
            )
        });
    assert_eq!(path.edges.len(), 2);
    assert_eq!(path.edges[0].caller_id, start);
    assert_eq!(path.edges[0].callee_id, intermediate);
    assert_eq!(path.edges[1].caller_id, intermediate);
    assert_eq!(path.edges[1].callee_id, target);
    assert_call_path_node(
        path,
        start,
        "::extract",
        "axum-core/src/ext_traits/request.rs",
        "RAG assembled outgoing two-hop path",
    );
    assert_call_path_node(
        path,
        intermediate,
        "::extract_with_state",
        "axum-core/src/ext_traits/request.rs",
        "RAG assembled outgoing two-hop path",
    );
    assert_call_path_node(
        path,
        target,
        "::from_request",
        "axum-core/src/extract/mod.rs",
        "RAG assembled outgoing two-hop path",
    );

    Ok(())
}

fn assert_call_path_node(
    path: &ploke_core::rag_types::CallPathInfo,
    id: Uuid,
    canon_suffix: &str,
    file_suffix: &str,
    label: &str,
) {
    assert!(
        path.nodes.iter().any(|node| {
            node.id == id
                && node.canon_path.as_ref().ends_with(canon_suffix)
                && node.file_path.as_ref().ends_with(file_suffix)
        }),
        "{label} should include call path node {id} ending with {canon_suffix:?} in {file_suffix:?}: {path:#?}"
    );
}

fn assert_call_node(
    nodes: &[ploke_core::rag_types::CallNodeInfo],
    id: Uuid,
    name: &str,
    file_suffix: &str,
    label: &str,
) {
    assert!(
        nodes.iter().any(|node| {
            node.id == id && node.name == name && node.file_path.as_ref().ends_with(file_suffix)
        }),
        "{label} should include call node {id} named {name:?} in {file_suffix:?}: {nodes:#?}"
    );
}

fn assert_call_source_file(
    files: &[ploke_core::rag_types::NodeFilepath],
    suffix: &str,
    label: &str,
) {
    assert!(
        files.iter().any(|file| file.as_ref().ends_with(suffix)),
        "{label} should include file ending with {suffix:?}: {files:#?}"
    );
}

#[tokio::test]
async fn call_context_collection_reads_axum_await_result_receiver_gap() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    let owner = method_id_by_name_and_body_substring(
        &db,
        "accept",
        "self.sem.clone().acquire_owned().await.unwrap()",
    )?;

    let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
    let context = call_context
        .get(&owner)
        .unwrap_or_else(|| panic!("ConnLimiter::accept should receive outgoing call context"));
    let expected_callee = CallCalleeInfo::Method {
        name: "unwrap".to_string(),
        receiver: Some(CallReceiverInfo::AwaitResult),
    };
    let await_unwrap = context
        .iter()
        .filter(|call| call.kind == CallSiteKind::Method && call.callee == expected_callee)
        .collect::<Vec<_>>();

    // Matrix: awaited-result receiver row.
    // Source chain:
    //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
    //   axum/src/serve/listener.rs:142 owns `ConnLimiter<T>::accept`.
    //   axum/src/serve/listener.rs:143 calls
    //   `self.sem.clone().acquire_owned().await.unwrap()`.
    // Current contract: RAG owner-seeded call context preserves the DB-pinned
    // `AwaitResult.unwrap` row as unsupported and targetless. It must expose
    // zero traversal targets rather than guessing the concrete awaited result
    // type or a local `unwrap` callee.
    assert_eq!(
        await_unwrap.len(),
        1,
        "RAG call context should expose exactly the listener AwaitResult unwrap row: {context:#?}"
    );

    let call = await_unwrap[0];
    assert_eq!(call.owner_id, owner);
    assert_eq!(call.status, CallStatusKind::Unsupported);
    assert_eq!(call.resolution, None);
    assert!(
        call.targets.is_empty(),
        "AwaitResult unwrap should remain targetless in RAG call context: {call:#?}"
    );
    let reach = rag
        .exact_call_reach_for_owner(
            owner,
            CallPathOptions {
                max_depth: 2,
                max_paths: 128,
            },
        )?
        .expect("call context enabled");
    let unsupported = reach
        .unsupported_frontier_calls
        .iter()
        .find(|frontier| frontier.site_id == call.site_id)
        .unwrap_or_else(|| {
            panic!(
                "RAG reach should expose AwaitResult unwrap in unsupported frontier rows: {reach:#?}"
            )
        });
    assert_eq!(unsupported.owner_id, owner);
    assert_eq!(unsupported.status, CallStatusKind::Unsupported);
    assert!(
        unsupported.targets.is_empty(),
        "RAG unsupported frontier call should remain targetless: {unsupported:#?}"
    );

    Ok(())
}

fn method_id_by_name_and_body_substring(
    db: &Database,
    name: &str,
    body_marker: &str,
) -> Result<Uuid, Error> {
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
    assert_eq!(
        matching.len(),
        1,
        "expected exactly one method named {name:?} whose body contains {body_marker:?}; rows: {:#?}",
        rows.rows
    );

    to_uuid(&matching[0]).map_err(Error::from)
}

fn method_id_by_file(
    db: &Database,
    name: &str,
    body_marker: &str,
    file: &str,
) -> Result<Uuid, Error> {
    let mut params = BTreeMap::new();
    params.insert("name".to_string(), DataValue::from(name));

    let script = format!(
        r#"
ancestor[desc, desc] := *module{{ id: desc @ 'NOW' }}
{ANCESTOR_RULES_NOW}
{METHOD_NODE_ANCESTOR_RULE}

module_has_file[mid] := *file_mod{{ owner_id: mid @ 'NOW' }}
file_owner_for_module[mod_id, file_id] := module_has_file[mod_id], file_id = mod_id
file_owner_for_module[mod_id, file_id] := ancestor[mod_id, parent], module_has_file[parent], file_id = parent

?[id, body, file_path] :=
    *method {{ id, name: $name, body @ 'NOW' }},
    ancestor[id, mod_id],
    *module{{ id: mod_id @ 'NOW' }},
    file_owner_for_module[mod_id, file_id],
    *file_mod{{ owner_id: file_id, file_path @ 'NOW' }}
"#
    );
    let rows = db.raw_query_params(&script, params)?;
    let marker = body_key(body_marker);
    let matching = rows
        .rows
        .iter()
        .filter(|row| {
            let DataValue::Str(body) = &row[1] else {
                return false;
            };
            let DataValue::Str(file_path) = &row[2] else {
                return false;
            };
            body_key(body).contains(&marker) && file_path.ends_with(file)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "expected exactly one method named {name:?} in {file:?} whose body contains {body_marker:?}; rows: {:#?}",
        rows.rows
    );

    to_uuid(&matching[0][0]).map_err(Error::from)
}

fn function_id_by_name_in_module(
    db: &Database,
    module_path: &[&str],
    name: &str,
) -> Result<Uuid, Error> {
    let mut params = BTreeMap::new();
    params.insert("name".to_string(), DataValue::from(name));
    params.insert(
        "path".to_string(),
        DataValue::List(
            module_path
                .iter()
                .map(|part| DataValue::from(*part))
                .collect(),
        ),
    );

    let rows = db.raw_query_params(
        r#"?[id] :=
            *function { id, name: $name, module_id @ 'NOW' },
            *module { id: module_id, path: $path @ 'NOW' }"#,
        params,
    )?;
    assert_eq!(
        rows.rows.len(),
        1,
        "expected exactly one function named {name:?} in module {module_path:?}; rows: {:#?}",
        rows.rows
    );

    to_uuid(&rows.rows[0][0]).map_err(Error::from)
}

fn method_id_by_trait_name(
    db: &Database,
    trait_name: &str,
    method_name: &str,
) -> Result<Uuid, Error> {
    let mut params = BTreeMap::new();
    params.insert("trait_name".to_string(), DataValue::from(trait_name));
    params.insert("method_name".to_string(), DataValue::from(method_name));

    let rows = db.raw_query_params(
        r#"?[id] :=
            *trait { id: trait_id, name: $trait_name @ 'NOW' },
            *method { id, name: $method_name, owner_id: trait_id @ 'NOW' }"#,
        params,
    )?;
    assert_eq!(
        rows.rows.len(),
        1,
        "expected exactly one trait method {trait_name}::{method_name}; rows: {:#?}",
        rows.rows
    );

    to_uuid(&rows.rows[0][0]).map_err(Error::from)
}

fn struct_id_by_name(db: &Database, name: &str) -> Result<Uuid, Error> {
    let mut params = BTreeMap::new();
    params.insert("name".to_string(), DataValue::from(name));

    let rows = db.raw_query_params(
        r#"?[id] :=
            *struct { id, name: $name @ 'NOW' }"#,
        params,
    )?;
    assert_eq!(
        rows.rows.len(),
        1,
        "expected exactly one struct named {name:?}; rows: {:#?}",
        rows.rows
    );

    to_uuid(&rows.rows[0][0]).map_err(Error::from)
}

fn body_key(value: &str) -> String {
    value.chars().filter(|ch| !ch.is_whitespace()).collect()
}
