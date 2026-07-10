use std::collections::{BTreeMap, BTreeSet};

use cozo::{DataValue, UuidWrapper};
use ploke_db::multi_embedding::db_ext::{ANCESTOR_RULES_NOW, METHOD_NODE_ANCESTOR_RULE};
use ploke_db::{CallStatusKind as DbCallStatusKind, ProofGraphStore};
use serde_json::json;

use super::super::super::super::super::*;
use super::expected::path;

mod remaining;
mod shared_matrix;
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

fn setup_chrono_call_graph_rag() -> Result<(Arc<Database>, RagService), Error> {
    let db = Arc::new(fresh_backup_fixture_db(
        &ploke_test_utils::CORPUS_CHRONO_CALL_GRAPH,
    )?);
    assert!(
        db.has_call_graph_relations()?,
        "corpus_chrono_call_graph must include call graph relations for RAG call-context tests"
    );

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.call_context_degraded(),
        "chrono call graph backup should enable RAG call context"
    );

    Ok((db, rag))
}

fn setup_memchr_call_graph_rag() -> Result<(Arc<Database>, RagService), Error> {
    let db = Arc::new(fresh_backup_fixture_db(
        &ploke_test_utils::CORPUS_MEMCHR_CALL_GRAPH,
    )?);
    assert!(
        db.has_call_graph_relations()?,
        "corpus_memchr_call_graph must include call graph relations for RAG call-context tests"
    );

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.call_context_degraded(),
        "memchr call graph backup should enable RAG call context"
    );

    Ok((db, rag))
}

fn spawn_effect_seed(call_site_id: impl ToString, effect_seed_id: &str) -> serde_json::Value {
    json!({
        "fact_kind": "effect_seed",
        "schema_version": "ploke-proof-facts.v1",
        "effect_seed_id": effect_seed_id,
        "call_site_id": call_site_id.to_string(),
        "effect_class": "async_task_spawn",
        "confidence": "source-oracle",
        "blocker_if_unresolved": false,
        "evidence_use": "proof_only"
    })
}

fn owner_effect_policy(
    owner_id: impl ToString,
    effect_policy_id: &str,
    allowed_effects: &[&str],
) -> serde_json::Value {
    json!({
        "fact_kind": "effect_policy",
        "schema_version": "ploke-proof-facts.v1",
        "effect_policy_id": effect_policy_id,
        "build_domain_id": "bd:axum-call-graph",
        "definition_id": owner_id.to_string(),
        "proof_policy_version": "axum-real-corpus-call-graph-test",
        "review_method": "source-oracle",
        "scope_of_validity": "axum deserialize_error_status_codes call graph fixture",
        "allowed_effects": allowed_effects,
        "invalidation_conditions": "call graph fixture source or proof policy changes",
        "status": "admitted",
        "evidence_use": "proof_only"
    })
}

#[tokio::test]
async fn call_context_exact_reads_axum_body_empty_incoming_callers() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    let target = method_id_by_name_and_body_substring(&db, "empty", "Empty::new()")?;

    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        23,
        "current axum fixture should resolve exactly the twenty-three Body::empty callers: {callers:#?}"
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
    //   axum-core/src/ext_traits/request.rs request helper rows call
    //   `Body::empty()`.
    //   axum/src/{extract/query.rs,extract/raw_form.rs,form.rs,serve/mod.rs}
    //   call `Body::empty()` through direct parsed-workspace imports.
    //   axum routing and middleware tests call `Body::empty()` through local
    //   re-export imports and inherited `super::*` imports, including the
    //   route.rs closure body, routing/tests/merge.rs, and
    //   routing/tests/mod.rs local handler rows.
    // Expected traversal for the current fixture: the RAG exact call-context
    // path preserves the same twenty-three incoming caller-site edges exposed by
    // `Database::callers_for_target`.
    assert_eq!(
        incoming.len(),
        23,
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
        BTreeMap::from([
            (path(&["Body", "empty"]), 21),
            (path(&["Self", "empty"]), 2),
        ]),
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
        11,
        "current axum fixture should resolve the eleven parse_attrs caller sites: {callers:#?}"
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
    //   from_ref.rs:30 and from_request/mod.rs:{112,196,471,598,727,892,908,
    //   1029,1039} call imported `parse_attrs(...)`, including three
    //   closure-owned executable rows.
    // Expected traversal: RAG exact call context preserves the same eleven
    // incoming caller-site edges exposed by `Database::callers_for_target`.
    assert_eq!(
        incoming.len(),
        11,
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
            (path(&["parse_attrs"]), 10),
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
        3,
        "current axum fixture should resolve the three BoxedIntoRoute constructor callers: {callers:#?}"
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

    // Matrix: `BoxedIntoRoute` tuple-struct constructor row.
    // Source chain:
    //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
    //   axum/src/boxed.rs:12 defines `BoxedIntoRoute<S, E>(...)`.
    //   axum/src/boxed.rs:23,38,51 call `Self(...)`,
    //   `BoxedIntoRoute(...)`, and `Self(...)`.
    // Expected traversal: RAG exact call context preserves the same one-hop
    // constructor edges exposed by `Database::callers_for_target`.
    assert_eq!(
        incoming.len(),
        3,
        "RAG exact call context should expose the current BoxedIntoRoute constructor edge: {context:#?}"
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
        "RAG call context should preserve the DB BoxedIntoRoute caller site identities"
    );

    let mut path_counts = BTreeMap::<Vec<String>, usize>::new();
    for call in incoming {
        assert_eq!(call.status, CallStatusKind::Resolved);
        assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(call.targets.len(), 1);
        assert_eq!(call.targets[0].target_id, target);
        assert_eq!(
            call.targets[0].relation,
            CallTargetKind::TupleStructConstructor
        );
        let CallCalleeInfo::Path { path } = &call.callee else {
            panic!("BoxedIntoRoute incoming caller should be a path call: {call:#?}");
        };
        *path_counts.entry(path.clone()).or_default() += 1;
    }
    assert_eq!(
        path_counts,
        BTreeMap::from([(path(&["BoxedIntoRoute"]), 1), (path(&["Self"]), 2)]),
        "RAG call context should preserve explicit and Self constructor path shapes"
    );

    Ok(())
}

#[tokio::test]
async fn call_context_exact_reads_chrono_alias_constructor_callers() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_chrono_call_graph_rag()?;

    let target = variant_id_by_enum_and_variant_names(&db, "LocalResult", "Single")?;
    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        12,
        "current chrono fixture should resolve all MappedLocalTime::Single alias constructor callers: {callers:#?}"
    );

    let context = rag.exact_call_context(target)?;
    let expected_callee = CallCalleeInfo::Path {
        path: path(&["MappedLocalTime", "Single"]),
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

    // Matrix: chrono `MappedLocalTime::Single` alias constructor row.
    // Source chain:
    //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
    //   chrono/src/offset/mod.rs:77 aliases
    //   `MappedLocalTime<T> = LocalResult<T>`.
    //   chrono/src/offset/mod.rs:81-83 defines `LocalResult::Single(T)`.
    //   chrono/src/offset/mod.rs:{143,156,468,502,535},
    //   offset/{fixed.rs:135,138,utc.rs:122,125,local/unix.rs:159}, and
    //   datetime/tests.rs:{75,79} call `MappedLocalTime::Single(...)`.
    // Expected traversal: RAG exact call context preserves every resolved
    // alias constructor caller-site identity exposed by
    // `Database::callers_for_target`.
    assert_eq!(
        incoming.len(),
        12,
        "RAG exact call context should expose all chrono alias constructor edges: {context:#?}"
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
        "RAG call context should preserve the DB chrono alias constructor site identities"
    );

    for call in incoming {
        assert_eq!(call.status, CallStatusKind::Resolved);
        assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(call.targets.len(), 1);
        assert_eq!(call.targets[0].target_id, target);
        assert_eq!(
            call.targets[0].relation,
            CallTargetKind::EnumVariantConstructor
        );
    }

    Ok(())
}

#[tokio::test]
async fn call_context_exact_reads_chrono_option_ok_or_try_receiver_callers() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_chrono_call_graph_rag()?;

    let target = method_id_by_name_and_body_substring(&db, "naive_utc", "self.datetime")?;
    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        2,
        "current chrono fixture should resolve the two parsed.rs DateTime...?.naive_utc callers: {callers:#?}"
    );

    let context = rag.exact_call_context(target)?;
    let expected_callee = CallCalleeInfo::Method {
        name: "naive_utc".to_string(),
        receiver: Some(CallReceiverInfo::TryMethodCallResult {
            method_name: "ok_or".to_string(),
        }),
    };
    let incoming = context
        .iter()
        .filter(|call| {
            call.kind == CallSiteKind::Method
                && call.callee == expected_callee
                && call
                    .targets
                    .iter()
                    .any(|candidate| candidate.target_id == target)
        })
        .collect::<Vec<_>>();

    // Matrix: chrono `DateTime...?.naive_utc()` try receiver row.
    // Source chain:
    //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
    //   chrono/src/datetime/mod.rs:563 defines `DateTime<Tz>::naive_utc`.
    //   chrono/src/datetime/mod.rs:768,803 define the associated
    //   `DateTime::from_timestamp*` constructors returning `Option<Self>`.
    //   chrono/src/format/parsed.rs:836,953 call
    //   `DateTime::from_timestamp*(...).ok_or(OUT_OF_RANGE)?.naive_utc()`.
    // Expected traversal: RAG exact call context preserves both DB-resolved
    // try-receiver caller-site identities and the method target relation.
    assert_eq!(
        incoming.len(),
        2,
        "RAG exact call context should expose both chrono naive_utc try-receiver edges: {context:#?}"
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
        "RAG call context should preserve the DB chrono naive_utc caller site identities"
    );

    for call in incoming {
        assert_eq!(call.status, CallStatusKind::Resolved);
        assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(call.targets.len(), 1);
        assert_eq!(call.targets[0].target_id, target);
        assert_eq!(call.targets[0].relation, CallTargetKind::Method);
    }

    Ok(())
}

#[tokio::test]
async fn call_context_collection_reads_chrono_strftime_queue_slice_frontier() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_chrono_call_graph_rag()?;

    let owner =
        method_id_by_name_and_body_substring(&db, "parse_next_item", "self.queue.is_empty()")?;
    let context = rag.exact_call_context(owner)?;
    let callee = CallCalleeInfo::Method {
        name: "is_empty".to_string(),
        receiver: Some(CallReceiverInfo::SelfField {
            path: vec!["queue".to_string()],
        }),
    };
    let matching = context
        .iter()
        .filter(|call| call.kind == CallSiteKind::Method && call.callee == callee)
        .collect::<Vec<_>>();

    // Matrix: chrono guarded match-arm method guard.
    // Source chain:
    //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
    //   chrono/src/format/strftime.rs:198 defines
    //   `StrftimeItems::queue: &'static [Item<'static>]`.
    //   chrono/src/format/strftime.rs:635 calls `self.queue.is_empty()`
    //   in a guarded match arm.
    // Expected traversal: RAG preserves the DB external frontier row for the
    // slice receiver and does not fabricate a local callee edge.
    assert_eq!(
        matching.len(),
        1,
        "StrftimeItems::parse_next_item should expose one queue.is_empty frontier row: {context:#?}"
    );
    assert_eq!(matching[0].owner_id, owner);
    assert_eq!(matching[0].status, CallStatusKind::External);
    assert_eq!(matching[0].resolution, None);
    assert!(
        matching[0].targets.is_empty(),
        "chrono queue.is_empty slice frontier should remain targetless: {matching:#?}"
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
async fn call_guard_exact_classifies_axum_request_extract_path_policy() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Security analysis / Architecture review:
    //   "Are authorization checks always called before protected state
    //   mutations?"
    //   "Do any call chains bypass the intended abstraction layer?"
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
    let options = CallPathOptions {
        max_depth: 2,
        max_paths: 16,
    };

    let guarded = rag
        .exact_call_guard_report_between(start, target, intermediate, options)?
        .expect("call context enabled");
    assert_eq!(guarded.source.id, start);
    assert_eq!(guarded.target.id, target);
    assert_eq!(guarded.guard.id, intermediate);
    assert!(
        guarded.guarded,
        "RAG guard report should classify the known intermediate as covering every path: {guarded:#?}"
    );
    assert!(
        guarded.violations.is_empty(),
        "RAG guard report should not invent violations for the known intermediate: {guarded:#?}"
    );
    let path = guarded
        .paths
        .iter()
        .find(|path| path.start_id == start && path.end_id == target && path.depth == 2)
        .unwrap_or_else(|| panic!("expected guarded two-hop path: {guarded:#?}"));
    assert_call_path_node(
        path,
        intermediate,
        "::extract_with_state",
        "axum-core/src/ext_traits/request.rs",
        "RAG guarded path",
    );

    // Real unrelated node for the missing-guard case:
    //   axum-macros/src/from_request/mod.rs:230 defines
    //   `parse_single_generic_type_on_struct`, which is not on the
    //   RequestExt::extract -> FromRequest::from_request path.
    let unrelated = function_id_by_name_in_module(
        &db,
        &["crate", "from_request"],
        "parse_single_generic_type_on_struct",
    )?;
    let unguarded = rag
        .exact_call_guard_report_between(start, target, unrelated, options)?
        .expect("call context enabled");
    assert!(
        !unguarded.guarded,
        "RAG guard report should fail closed when the required guard is absent: {unguarded:#?}"
    );
    assert_eq!(
        unguarded.violations.len(),
        unguarded.paths.len(),
        "RAG guard report should preserve every unguarded resolved path as a violation: {unguarded:#?}"
    );

    Ok(())
}

#[tokio::test]
async fn call_paths_exact_reads_axum_from_request_free_function_two_hop_path() -> Result<(), Error>
{
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    // Matrix:
    //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
    // Source-oracle chain:
    //   axum-macros/src/from_request/mod.rs:145
    //     `from_request::expand` calls `impl_struct_by_extracting_each_field(...)`.
    //   axum-macros/src/from_request/mod.rs:342
    //     `impl_struct_by_extracting_each_field` calls `extract_fields(...)`.
    //   axum-macros/src/from_request/mod.rs:412
    //     defines the `extract_fields` terminal helper.
    //
    // This covers the regular free-function multi-hop bucket for RAG: all
    // path nodes are normal functions in one real target module, and the
    // traversal should preserve the DB's ordered resolved path.
    let start = function_id_by_name_in_module(&db, &["crate", "from_request"], "expand")?;
    let intermediate = function_id_by_name_in_module(
        &db,
        &["crate", "from_request"],
        "impl_struct_by_extracting_each_field",
    )?;
    let target = function_id_by_name_in_module(&db, &["crate", "from_request"], "extract_fields")?;

    let outgoing = rag.exact_call_paths_from_owner(
        start,
        CallPathOptions {
            max_depth: 2,
            max_paths: 128,
        },
    )?;
    let path = outgoing
        .iter()
        .find(|path| path.end_id == target && path.depth == 2)
        .unwrap_or_else(|| {
            panic!(
                "expected RAG free-function path from from_request::expand to extract_fields: {outgoing:#?}"
            )
        });
    assert_eq!(path.start_id, start);
    assert_eq!(path.edges.len(), 2);
    assert_eq!(path.edges[0].caller_id, start);
    assert_eq!(path.edges[0].callee_id, intermediate);
    assert_eq!(path.edges[0].relation, CallTargetKind::Function);
    assert_eq!(path.edges[1].caller_id, intermediate);
    assert_eq!(path.edges[1].callee_id, target);
    assert_eq!(path.edges[1].relation, CallTargetKind::Function);

    let db_paths = db.call_paths_between(
        start,
        target,
        CallPathOptions {
            max_depth: 2,
            max_paths: 128,
        },
    )?;
    let db_path = db_paths
        .iter()
        .find(|path| path.start_id == start && path.end_id == target && path.depth == 2)
        .unwrap_or_else(|| panic!("expected DB free-function path for RAG oracle: {db_paths:#?}"));
    assert_eq!(path.edges[0].span, db_path.edges[0].span);
    assert_eq!(path.edges[1].span, db_path.edges[1].span);
    assert_call_path_node(
        path,
        start,
        "::expand",
        "axum-macros/src/from_request/mod.rs",
        "RAG free-function outgoing path",
    );
    assert_call_path_node(
        path,
        intermediate,
        "::impl_struct_by_extracting_each_field",
        "axum-macros/src/from_request/mod.rs",
        "RAG free-function outgoing path",
    );
    assert_call_path_node(
        path,
        target,
        "::extract_fields",
        "axum-macros/src/from_request/mod.rs",
        "RAG free-function outgoing path",
    );

    let one_hop = rag.exact_call_paths_between(
        start,
        target,
        CallPathOptions {
            max_depth: 1,
            max_paths: 128,
        },
    )?;
    assert!(
        one_hop.is_empty(),
        "RAG free-function reachability should not skip the intermediate helper: {one_hop:#?}"
    );

    let direct = rag.exact_call_paths_between(
        start,
        target,
        CallPathOptions {
            max_depth: 2,
            max_paths: 128,
        },
    )?;
    let direct_path = direct
        .iter()
        .find(|path| path.start_id == start && path.end_id == target && path.depth == 2)
        .unwrap_or_else(|| {
            panic!("expected RAG direct free-function reachability path: {direct:#?}")
        });
    assert_eq!(direct_path.edges, path.edges);

    let incoming = rag.exact_call_paths_to_target(
        target,
        CallPathOptions {
            max_depth: 2,
            max_paths: 128,
        },
    )?;
    let reverse_path = incoming
        .iter()
        .find(|path| path.start_id == start && path.end_id == target && path.depth == 2)
        .unwrap_or_else(|| panic!("expected RAG reverse free-function path lookup: {incoming:#?}"));
    assert_eq!(reverse_path.edges, path.edges);

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
    assert_eq!(
        report.target.module_path,
        path(&["crate", "extract"]),
        "RAG impact target should preserve the DB module path"
    );
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
    let start_caller = report
        .callers
        .iter()
        .find(|caller| caller.id == start)
        .unwrap_or_else(|| panic!("RAG impact callers should include RequestExt::extract"));
    assert_eq!(
        start_caller.module_path,
        path(&["crate", "ext_traits", "request"]),
        "RAG impact caller should preserve the DB module path"
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
                && call.arg_count == Some(2)
                && call
                    .targets
                    .iter()
                    .any(|target_row| target_row.target_id == target)
        }),
        "RAG impact summary should include the E::from_request callsite row: {report:#?}"
    );
    assert!(
        report.callsite_buckets.iter().any(|bucket| {
            bucket.kind == CallSiteKind::Path
                && bucket.relation == CallTargetKind::AssociatedFunction
                && bucket.count == 2
        }),
        "RAG impact summary should expose the direct path/associated-function callsite bucket: {report:#?}"
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
    assert_call_source_module(
        &report.source_modules,
        &["crate", "ext_traits", "request"],
        "RAG impact source modules",
    );
    assert_call_source_module(
        &report.source_modules,
        &["crate", "extract"],
        "RAG impact source modules",
    );

    // Source oracle:
    //   axum-macros/src/lib.rs:377,426,665,715 call `expand_with(...)` from
    //   public proc-macro entrypoints.
    let proc_macro_target = function_id_by_name_in_module(&db, &["crate"], "expand_with")?;
    let proc_macro_callers = [
        (
            macro_id_by_name(&db, "derive_from_request")?,
            "derive_from_request",
        ),
        (
            macro_id_by_name(&db, "derive_from_request_parts")?,
            "derive_from_request_parts",
        ),
        (
            macro_id_by_name(&db, "derive_typed_path")?,
            "derive_typed_path",
        ),
        (macro_id_by_name(&db, "derive_from_ref")?, "derive_from_ref"),
    ];
    let proc_macro_report = rag
        .exact_call_impact_for_target(
            proc_macro_target,
            CallPathOptions {
                max_depth: 2,
                max_paths: 16,
            },
        )?
        .expect("call context enabled");
    assert_eq!(proc_macro_report.target.name, "expand_with");
    assert_eq!(proc_macro_report.paths.len(), proc_macro_callers.len());
    assert_eq!(proc_macro_report.callers.len(), proc_macro_callers.len());
    assert_eq!(
        proc_macro_report.direct_callers.len(),
        proc_macro_callers.len()
    );
    assert_eq!(
        proc_macro_report.direct_call_sites.len(),
        proc_macro_callers.len()
    );
    assert_eq!(
        proc_macro_report.public_callers.len(),
        proc_macro_callers.len()
    );
    for (id, name) in proc_macro_callers {
        assert_call_node(
            &proc_macro_report.callers,
            id,
            name,
            "axum-macros/src/lib.rs",
            "RAG proc-macro impact callers",
        );
        assert_call_node(
            &proc_macro_report.public_callers,
            id,
            name,
            "axum-macros/src/lib.rs",
            "RAG proc-macro public callers",
        );
        assert!(
            proc_macro_report
                .paths
                .iter()
                .any(|path| path.start_id == id
                    && path.end_id == proc_macro_target
                    && path.depth == 1),
            "RAG proc-macro impact paths should include one-hop path from {name}: {proc_macro_report:#?}"
        );
    }
    for call in &proc_macro_report.direct_call_sites {
        assert_eq!(call.kind, CallSiteKind::Path);
        assert_eq!(call.status, CallStatusKind::Resolved);
        assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(call.arg_count, Some(2));
        assert!(
            matches!(&call.callee, CallCalleeInfo::Path { path: call_path } if call_path == &path(&["expand_with"])),
            "RAG proc-macro direct call should preserve expand_with path callee: {call:#?}"
        );
        assert!(
            call.targets.iter().any(|target| {
                target.target_id == proc_macro_target && target.relation == CallTargetKind::Function
            }),
            "RAG proc-macro direct call should target expand_with: {call:#?}"
        );
    }
    assert!(
        proc_macro_report.callsite_buckets.iter().any(|bucket| {
            bucket.kind == CallSiteKind::Path
                && bucket.relation == CallTargetKind::Function
                && bucket.count == 4
        }),
        "RAG proc-macro impact summary should expose path/function bucket: {proc_macro_report:#?}"
    );
    assert_call_source_file(
        &proc_macro_report.source_files,
        "axum-macros/src/lib.rs",
        "RAG proc-macro impact source files",
    );

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Impact analysis / API understanding:
    //   "Which public APIs eventually call this helper?"
    //   "How is this library function used in real target code?"
    //
    // Source oracle:
    //   axum/src/routing/method_routing.rs:799 defines
    //     `MethodRouter::new`.
    //   axum/src/routing/method_routing.rs:375,434,472,514 call
    //     `MethodRouter::new()` from public top-level routing functions.
    let method_router_new = method_id_by_file(
        &db,
        "new",
        "let fallback = Route::new",
        "axum/src/routing/method_routing.rs",
    )?;
    let on_service =
        function_id_by_name_in_module(&db, &["crate", "routing", "method_routing"], "on_service")?;
    let any_service =
        function_id_by_name_in_module(&db, &["crate", "routing", "method_routing"], "any_service")?;
    let on = function_id_by_name_in_module(&db, &["crate", "routing", "method_routing"], "on")?;
    let any = function_id_by_name_in_module(&db, &["crate", "routing", "method_routing"], "any")?;

    let public_report = rag
        .exact_call_impact_for_target(
            method_router_new,
            CallPathOptions {
                max_depth: 1,
                max_paths: 64,
            },
        )?
        .expect("call context enabled");
    assert_call_node(
        &public_report.public_callers,
        on_service,
        "on_service",
        "axum/src/routing/method_routing.rs",
        "RAG MethodRouter::new public impact callers",
    );
    assert_call_node(
        &public_report.public_callers,
        any_service,
        "any_service",
        "axum/src/routing/method_routing.rs",
        "RAG MethodRouter::new public impact callers",
    );
    assert_call_node(
        &public_report.public_callers,
        on,
        "on",
        "axum/src/routing/method_routing.rs",
        "RAG MethodRouter::new public impact callers",
    );
    assert_call_node(
        &public_report.public_callers,
        any,
        "any",
        "axum/src/routing/method_routing.rs",
        "RAG MethodRouter::new public impact callers",
    );
    assert!(
        public_report
            .public_callers
            .iter()
            .all(|caller| caller.is_public),
        "RAG public_callers should only include stored-public nodes: {public_report:#?}"
    );
    assert!(
        public_report.direct_call_sites.iter().any(|call| {
            call.owner_id == on_service
                && matches!(
                    &call.callee,
                    CallCalleeInfo::Path { path: call_path }
                        if call_path == &path(&["MethodRouter", "new"])
                )
                && call.arg_count == Some(0)
        }),
        "RAG MethodRouter::new impact should preserve zero-argument public caller sites: {public_report:#?}"
    );
    assert_call_source_file(
        &public_report.source_files,
        "axum/src/routing/method_routing.rs",
        "RAG MethodRouter::new impact source files",
    );

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Build or deployment optimization / API understanding:
    //   "Which components are affected by a change to this API?"
    //   "How is this library function used in real target code?"
    //
    // Source oracle:
    //   axum-core/src/body.rs:52 defines `Body::empty`.
    //   axum-core, axum, closure-owned, and local-item rows call
    //   `Body::empty()` or `Self::empty()` through direct imports,
    //   re-exports, and inherited glob imports.
    let body_empty = method_id_by_name_and_body_substring(&db, "empty", "Empty::new()")?;
    let body_report = rag
        .exact_call_impact_for_target(
            body_empty,
            CallPathOptions {
                max_depth: 1,
                max_paths: 64,
            },
        )?
        .expect("call context enabled");
    assert_eq!(body_report.target.id, body_empty);
    assert_eq!(body_report.target.kind, "Method");
    assert_eq!(body_report.target.name, "empty");
    assert_eq!(
        body_report.paths.len(),
        23,
        "RAG Body::empty component impact should preserve all current direct paths: {body_report:#?}"
    );
    assert_eq!(
        body_report.direct_call_sites.len(),
        23,
        "RAG Body::empty component impact should preserve every direct callsite: {body_report:#?}"
    );
    assert!(
        !body_report.test_callers.is_empty() && !body_report.non_test_callers.is_empty(),
        "RAG Body::empty component impact should preserve test/non-test caller buckets: {body_report:#?}"
    );
    assert_eq!(
        body_report.test_callers.len() + body_report.non_test_callers.len(),
        body_report.callers.len(),
        "RAG Body::empty test/non-test buckets should partition eventual callers: {body_report:#?}"
    );
    assert!(
        body_report.callsite_buckets.iter().any(|bucket| {
            bucket.kind == CallSiteKind::Path
                && bucket.relation == CallTargetKind::AssociatedFunction
                && bucket.count == 23
        }),
        "RAG Body::empty API summary should preserve the path/associated-function bucket: {body_report:#?}"
    );
    let path_counts = body_report.direct_call_sites.iter().fold(
        BTreeMap::<Vec<String>, usize>::new(),
        |mut counts, call| {
            let CallCalleeInfo::Path { path: call_path } = &call.callee else {
                panic!("RAG Body::empty impact callsite should be path-shaped: {call:#?}");
            };
            *counts.entry(call_path.clone()).or_default() += 1;
            counts
        },
    );
    assert_eq!(
        path_counts,
        BTreeMap::from([
            (path(&["Body", "empty"]), 21),
            (path(&["Self", "empty"]), 2),
        ]),
        "RAG Body::empty component impact should distinguish Body::empty and Self::empty rows"
    );
    for suffix in [
        "axum-core/src/body.rs",
        "axum-core/src/ext_traits/request.rs",
        "axum/src/middleware/from_fn.rs",
        "axum/src/routing/tests/mod.rs",
    ] {
        assert_call_source_file(
            &body_report.source_files,
            suffix,
            "RAG Body::empty component impact source files",
        );
    }
    assert_call_source_crate(
        &body_report.source_crates,
        "axum-core",
        "RAG Body::empty component impact source crates",
    );
    assert_call_source_crate(
        &body_report.source_crates,
        "axum",
        "RAG Body::empty component impact source crates",
    );
    for module in [
        &["crate", "body"][..],
        &["crate", "ext_traits", "request"][..],
        &["crate", "middleware", "from_fn"][..],
        &["crate", "routing", "tests"][..],
    ] {
        assert_call_source_module(
            &body_report.source_modules,
            module,
            "RAG Body::empty component impact source modules",
        );
    }

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
                max_paths: 512,
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
async fn call_impact_exact_reports_private_target_without_incoming_callers() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Dead code detection:
    //   "Which private helpers have no incoming callers?"
    //   "Is this function reachable from any binary, test, macro entrypoint,
    //   or exported API?"
    //
    // Source oracle:
    //   axum/src/error_handling/mod.rs:257 defines `#[test] fn traits()`.
    //   No checked-in axum source row calls `traits(...)`; generated test
    //   harness entrypoints are outside the persisted source call graph.
    let target = function_id_by_name_in_module(&db, &["crate", "error_handling"], "traits")?;
    let parse_attrs =
        function_id_by_name_in_module(&db, &["crate", "attr_parsing"], "parse_attrs")?;
    let uncalled = rag
        .exact_private_uncalled_nodes()?
        .expect("call context enabled");
    assert!(
        uncalled.iter().any(|node| node.id == target),
        "RAG private uncalled-node helper should list error_handling::traits: {uncalled:#?}"
    );
    assert!(
        uncalled.iter().all(|node| node.id != parse_attrs),
        "RAG private uncalled-node helper should exclude called parse_attrs helper: {uncalled:#?}"
    );

    let report = rag
        .exact_call_impact_for_target(
            target,
            CallPathOptions {
                max_depth: 3,
                max_paths: 16,
            },
        )?
        .expect("call context enabled");
    assert_eq!(report.target.id, target);
    assert_eq!(report.target.name, "traits");
    assert_eq!(report.target.kind, "Function");
    assert_eq!(
        report.target.module_path,
        path(&["crate", "error_handling"])
    );
    assert!(!report.target.is_public);
    assert!(
        report
            .target
            .file_path
            .as_ref()
            .ends_with("axum/src/error_handling/mod.rs")
    );
    assert!(report.paths.is_empty(), "{report:#?}");
    assert!(report.callers.is_empty(), "{report:#?}");
    assert!(report.direct_callers.is_empty(), "{report:#?}");
    assert!(report.direct_call_sites.is_empty(), "{report:#?}");
    assert!(report.callsite_buckets.is_empty(), "{report:#?}");
    assert!(report.public_callers.is_empty(), "{report:#?}");
    assert!(report.test_callers.is_empty(), "{report:#?}");
    assert!(report.non_test_callers.is_empty(), "{report:#?}");
    assert_call_source_file(
        &report.source_files,
        "axum/src/error_handling/mod.rs",
        "RAG zero-caller impact source files",
    );

    let domain_id = "bd:corpus-axum-call-graph";
    let mut records = ploke_test_utils::axum_call_graph_domain_records(domain_id);
    records.push(ploke_test_utils::axum_entrypoint_record(domain_id, target));
    records.push(ploke_test_utils::axum_entrypoint_effect_policy_record(
        domain_id,
        target,
        &["ffi_boundary"],
    ));
    db.upsert_proof_fact_values(&records)?;
    let target_id = target.to_string();
    let proof_rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !proof_rag.proof_context_degraded(),
        "entrypoint summary admission should enable proof context for this focused check"
    );
    let proof = proof_rag.exact_proof_context(target)?;
    assert!(
        proof.iter().any(|row| {
            row.kind == "entrypoint_summary"
                && row.definition_id.as_deref() == Some(target_id.as_str())
                && row.target_kind.as_deref() == Some("test")
                && row.target_name.as_deref() == Some("generated-test-harness")
                && row.summary_class.as_deref() == Some("analyzed_source")
                && row.status.as_deref() == Some("admitted")
        }),
        "RAG exact proof context should expose the generated test-harness summary without adding call impact edges: {proof:#?}"
    );
    let domains = proof_rag
        .exact_call_build_domains_for_node(target)?
        .expect("call context enabled");
    assert_eq!(domains.len(), 1, "{domains:#?}");
    let domain = &domains[0];
    assert_eq!(domain.build_domain_id, domain_id);
    assert_eq!(domain.target_kind.as_deref(), Some("library"));
    assert_eq!(domain.target_name.as_deref(), Some("axum"));
    assert_eq!(domain.target_root.as_deref(), Some("axum/src/lib.rs"));
    assert!(
        domain.blocker_reasons.is_empty(),
        "RAG build-domain summary should preserve admitted cfg/rustc evidence: {domains:#?}"
    );
    let entrypoints = proof_rag
        .exact_call_test_entrypoints_for_node(target)?
        .expect("call context enabled");
    assert_eq!(
        entrypoints.len(),
        1,
        "RAG should expose one generated-test entrypoint summary: {entrypoints:#?}"
    );
    let entrypoint = &entrypoints[0];
    assert_eq!(
        entrypoint.entrypoint_summary_id,
        "entrypoint-summary:axum-error-handling-traits-test"
    );
    assert_eq!(entrypoint.build_domain_id.as_deref(), Some(domain_id));
    assert_eq!(
        entrypoint.definition_id.as_deref(),
        Some(target_id.as_str())
    );
    assert_eq!(entrypoint.target_kind.as_deref(), Some("test"));
    assert_eq!(
        entrypoint.target_name.as_deref(),
        Some("generated-test-harness")
    );
    assert_eq!(entrypoint.summary_class.as_deref(), Some("analyzed_source"));
    assert_eq!(
        entrypoint.required_containment.as_deref(),
        Some("rust-test-harness")
    );
    assert_eq!(entrypoint.status.as_deref(), Some("admitted"));
    assert_eq!(entrypoint.allowed_effects, vec!["ffi_boundary".to_string()]);
    assert!(
        entrypoint.blocker_reasons.is_empty(),
        "RAG admitted entrypoint summary should remain unblocked: {entrypoints:#?}"
    );

    Ok(())
}

#[tokio::test]
async fn call_context_exact_reads_axum_router_clone_typed_local_callers() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    // Source oracle:
    //   axum/src/routing/mod.rs:90 defines `impl<S> Clone for Router<S>`.
    //   axum/src/serve/mod.rs projects ten `router.clone()` rows from typed
    //     `let router: Router = Router::new()` bindings across
    //     `if_it_compiles_it_works` and the local-address tests.
    //   axum/src/routing/tests/mod.rs:804 calls `app.clone()` from an
    //     explicitly typed `Router` local.
    //   axum/src/boxed.rs:134 and axum/src/routing/mod.rs:673 call
    //     `self.router.clone()` from wrapper clone impls.
    let target = method_id_by_file(
        &db,
        "clone",
        "inner: Arc::clone(&self.inner)",
        "axum/src/routing/mod.rs",
    )?;

    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        13,
        "current axum fixture should resolve the typed-local Router::clone caller sites: {callers:#?}"
    );

    let context = rag.exact_call_context(target)?;
    let incoming = context
        .iter()
        .filter(|call| {
            call.kind == CallSiteKind::Method
                && call
                    .targets
                    .iter()
                    .any(|candidate| candidate.target_id == target)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        incoming.len(),
        13,
        "RAG exact call context should expose all current Router::clone incoming edges: {context:#?}"
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
        "RAG call context should preserve the DB Router::clone caller site identities"
    );

    let mut receiver_counts = BTreeMap::<String, usize>::new();
    for call in incoming {
        assert_eq!(call.status, CallStatusKind::Resolved);
        assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(call.targets.len(), 1);
        assert_eq!(call.targets[0].target_id, target);
        assert_eq!(call.targets[0].relation, CallTargetKind::Method);
        let CallCalleeInfo::Method {
            name: method_name,
            receiver,
        } = &call.callee
        else {
            panic!("Router::clone incoming caller should be a method call: {call:#?}");
        };
        assert_eq!(method_name, "clone");
        match receiver {
            Some(CallReceiverInfo::TypedLocalBinding {
                name: receiver_name,
                type_path,
            }) => {
                assert_eq!(type_path, &path(&["Router"]));
                *receiver_counts
                    .entry(format!("typed:{receiver_name}"))
                    .or_default() += 1;
            }
            Some(CallReceiverInfo::SelfField { path: field_path }) => {
                assert_eq!(field_path, &path(&["router"]));
                *receiver_counts
                    .entry("self_field:router".to_string())
                    .or_default() += 1;
            }
            _ => {
                panic!("Router::clone caller should preserve a supported receiver: {call:#?}");
            }
        };
    }
    assert_eq!(
        receiver_counts,
        BTreeMap::from([
            ("self_field:router".to_string(), 2),
            ("typed:app".to_string(), 1),
            ("typed:router".to_string(), 10),
        ]),
        "RAG call context should preserve typed-local and self-field receiver buckets"
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
    assert_eq!(
        report.owner.module_path,
        path(&["crate", "ext_traits", "request"]),
        "RAG reach owner should preserve the DB module path"
    );
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
    assert_eq!(
        direct_site.arg_count,
        Some(1),
        "RAG reach should preserve the one explicit argument to extract_with_state: {direct_site:#?}"
    );
    assert!(
        report.boundary_call_sites.is_empty(),
        "RAG RequestExt::extract reach should not mark the same-module direct call as a module-boundary row: {report:#?}"
    );
    assert_eq!(
        report.boundary_edges.len(),
        1,
        "RAG RequestExt::extract reach should expose the transitive cross-module FromRequest edge: {report:#?}"
    );
    let boundary_edge = report.boundary_edges[0].clone();
    assert_eq!(boundary_edge.caller_id, intermediate);
    assert_eq!(boundary_edge.callee_id, target);
    assert_eq!(boundary_edge.source_kind, CallSiteKind::Path);
    assert_eq!(boundary_edge.relation, CallTargetKind::AssociatedFunction);
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
    assert_call_source_module(
        &report.source_modules,
        &["crate", "ext_traits", "request"],
        "RAG reach source modules",
    );
    assert_call_source_module(
        &report.source_modules,
        &["crate", "extract"],
        "RAG reach source modules",
    );
    assert_call_source_crate(
        &report.source_crates,
        "axum-core",
        "RAG reach source crates",
    );

    let listener_owners = listener_accept_owner_ids(&db)?;
    assert_eq!(
        listener_owners.len(),
        2,
        "RAG fixture should expose TcpListener and UnixListener accept owners"
    );
    let mut cfg_reports = 0;
    for owner in listener_owners {
        let listener = rag
            .exact_call_reach_for_owner(
                owner,
                CallPathOptions {
                    max_depth: 1,
                    max_paths: 16,
                },
            )?
            .expect("call context enabled");
        if listener.source_cfgs.iter().any(|cfg| cfg == "unix") {
            cfg_reports += 1;
            assert!(
                listener.external_frontier_calls.iter().any(|call| {
                    matches!(
                        &call.callee,
                        CallCalleeInfo::Path { path: call_path }
                            if call_path == &path(&["Self", "accept"])
                    )
                }),
                "RAG listener cfg summary should be tied to the external Self::accept frontier: {listener:#?}"
            );
        }
    }
    assert_eq!(
        cfg_reports, 1,
        "RAG reach summaries should preserve the unix cfg for exactly one listener owner"
    );

    let boundary = rag
        .exact_call_reach_for_owner(
            intermediate,
            CallPathOptions {
                max_depth: 1,
                max_paths: 16,
            },
        )?
        .expect("call context enabled");
    assert_eq!(
        boundary.boundary_call_sites.len(),
        1,
        "RAG RequestExt::extract_with_state should expose its direct cross-module FromRequest callsite: {boundary:#?}"
    );
    let boundary_call = &boundary.boundary_call_sites[0];
    assert_eq!(boundary_call.owner_id, intermediate);
    assert_eq!(
        boundary_call.arg_count,
        Some(2),
        "RAG boundary row should preserve the two explicit E::from_request source arguments: {boundary_call:#?}"
    );
    assert!(
        matches!(
            &boundary_call.callee,
            CallCalleeInfo::Path { path: call_path }
                if call_path == &path(&["E", "from_request"])
        ),
        "RAG module-boundary row should preserve the E::from_request path call: {boundary:#?}"
    );
    assert!(
        boundary_call
            .targets
            .iter()
            .any(|target_row| target_row.target_id == target),
        "RAG module-boundary row should target FromRequest::from_request: {boundary:#?}"
    );

    // Source oracle:
    //   axum/src/json.rs:164 defines `Json::from_bytes`.
    //   axum/src/json.rs:184 calls
    //     `serde_json::Deserializer::from_slice(bytes)`.
    //   axum/src/lib.rs:488-489 gates the file module with
    //     `#[cfg(feature = "json")] mod json;`.
    // Current contract: dependency-root path calls are visible as external
    // frontier rows but do not become local traversal edges. RAG reach
    // summaries must still carry the feature gate for build/deployment
    // questions over feature-specific call paths.
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
    assert!(
        json_report
            .source_cfgs
            .iter()
            .any(|cfg| cfg == r#"feature = "json""#),
        "RAG Json::from_bytes reach summary should preserve the json feature cfg: {json_report:#?}"
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

    // Source oracle:
    //   axum/src/handler/service.rs:155 binds
    //   `type Future = super::future::IntoServiceFuture<H::Future>`.
    //   axum/src/handler/service.rs:174 calls
    //   `super::future::IntoServiceFuture::new(future)`.
    // Current contract: generated constructor calls remain visible as
    // unresolved frontier rows until macro-expanded inherent items are modeled.
    let service_owner =
        method_id_by_name_and_body_substring(&db, "call", "IntoServiceFuture::new(future)")?;
    let service_report = rag
        .exact_call_reach_for_owner(
            service_owner,
            CallPathOptions {
                max_depth: 2,
                max_paths: 128,
            },
        )?
        .expect("call context enabled");
    let unresolved = service_report
        .unresolved_frontier_calls
        .iter()
        .find(|call| {
            matches!(
                &call.callee,
                CallCalleeInfo::Path { path: call_path }
                    if call_path == &path(&["super", "future", "IntoServiceFuture", "new"])
            )
        })
        .unwrap_or_else(|| {
            panic!(
                "RAG reach should include IntoServiceFuture::new in unresolved frontier rows: {service_report:#?}"
            )
        });
    assert_eq!(unresolved.owner_id, service_owner);
    assert_eq!(unresolved.status, CallStatusKind::Unresolved);
    assert!(
        unresolved.targets.is_empty(),
        "unresolved generated constructor row should remain targetless: {unresolved:#?}"
    );
    assert!(
        service_report.ambiguous_frontier_calls.is_empty(),
        "this axum owner should not report ambiguous frontier rows: {service_report:#?}"
    );

    Ok(())
}

#[tokio::test]
async fn module_boundary_edges_exact_reads_axum_request_extract_summary() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Architecture review:
    //   "Which modules call across a boundary that should be one-way?"
    //
    // Source-oracle chain:
    //   axum-core/src/ext_traits/request.rs:268
    //     `RequestExt::extract` calls `self.extract_with_state(&())`.
    //   axum-core/src/ext_traits/request.rs:279
    //     `RequestExt::extract_with_state` calls `E::from_request(self, state)`.
    //   axum-core/src/extract/mod.rs:85
    //     defines the `FromRequest::from_request` trait method binding.
    // Expected contract: RAG exposes the enriched module-boundary edge row,
    // including the exact edge plus source-labeled caller, callee, and callsite
    // metadata, so architecture-review tools do not have to manually rejoin
    // `call_reach.boundary_edges` to node and site tables.
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

    let rows = rag
        .exact_module_boundary_edges_from_owner(
            start,
            CallPathOptions {
                max_depth: 2,
                max_paths: 16,
            },
        )?
        .expect("call context enabled");
    assert_eq!(
        rows.len(),
        1,
        "RAG module-boundary rows should expose the transitive FromRequest boundary edge: {rows:#?}"
    );

    let row = &rows[0];
    assert_eq!(row.edge.caller_id, intermediate);
    assert_eq!(row.edge.callee_id, target);
    assert_eq!(row.edge.source_kind, CallSiteKind::Path);
    assert_eq!(row.edge.relation, CallTargetKind::AssociatedFunction);
    assert_eq!(row.caller.id, intermediate);
    assert_eq!(row.caller.name, "extract_with_state");
    assert_eq!(
        row.caller.module_path,
        path(&["crate", "ext_traits", "request"])
    );
    assert!(
        row.caller
            .file_path
            .as_ref()
            .ends_with("axum-core/src/ext_traits/request.rs"),
        "boundary caller should preserve source file metadata: {row:#?}"
    );
    assert_eq!(row.callee.id, target);
    assert_eq!(row.callee.name, "from_request");
    assert_eq!(row.callee.module_path, path(&["crate", "extract"]));
    assert!(
        row.callee
            .file_path
            .as_ref()
            .ends_with("axum-core/src/extract/mod.rs"),
        "boundary callee should preserve source file metadata: {row:#?}"
    );
    assert_eq!(row.site.owner_id, intermediate);
    assert_eq!(row.site.kind, CallSiteKind::Path);
    assert_eq!(row.site.status, CallStatusKind::Resolved);
    assert_eq!(row.site.arg_count, Some(2));
    assert!(
        matches!(
            &row.site.callee,
            CallCalleeInfo::Path { path: call_path }
                if call_path == &path(&["E", "from_request"])
        ),
        "boundary site should preserve the E::from_request path call: {row:#?}"
    );
    assert!(
        row.site.targets.iter().any(|target_row| {
            target_row.target_id == target
                && target_row.relation == CallTargetKind::AssociatedFunction
        }),
        "boundary site should include the FromRequest::from_request target: {row:#?}"
    );

    Ok(())
}

#[tokio::test]
async fn call_effects_exact_reads_axum_task_spawn_seed() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Security/performance:
    //   "Can this entrypoint reach a sensitive sink?"
    //   "Which call chain reaches a task-spawn point?"
    //
    // Source-oracle chain:
    //   axum/src/form.rs:262
    //     `deserialize_error_status_codes` calls `TestClient::new(app)`.
    //   axum/src/test_helpers/test_client.rs:36
    //     `TestClient::new` calls `spawn_service(svc)`.
    //   axum/src/test_helpers/test_client.rs:23
    //     `spawn_service` calls `tokio::spawn(...)`.
    // Expected contract: RAG exposes the DB reachable-effect query for the
    // task-spawn sink, while the external `tokio::spawn` frontier remains
    // targetless and does not become a local call graph edge.
    let start = function_id_by_name_in_module(
        &db,
        &["crate", "form", "tests"],
        "deserialize_error_status_codes",
    )?;
    let spawn_owner = function_id_by_name_in_module(
        &db,
        &["crate", "test_helpers", "test_client"],
        "spawn_service",
    )?;
    let spawn_context = db.call_context_for_owner(spawn_owner)?;
    let spawn_row = spawn_context
        .iter()
        .find(|row| {
            row.site
                .path
                .as_ref()
                .is_some_and(|call_path| call_path == &path(&["tokio", "spawn"]))
        })
        .unwrap_or_else(|| {
            panic!("spawn_service should expose the tokio::spawn frontier: {spawn_context:#?}")
        });
    assert_eq!(spawn_row.status.status, DbCallStatusKind::External);
    assert!(
        spawn_row.targets.is_empty(),
        "tokio::spawn should stay targetless in DB call context: {spawn_row:#?}"
    );

    let mut relation_params = BTreeMap::new();
    relation_params.insert(
        "site_id".to_string(),
        DataValue::Uuid(UuidWrapper(spawn_row.site.id)),
    );
    let raw_relations = db.raw_query_params(
        r#"?[target_id] :=
            *call_relation { source_id: $site_id, target_id @ 'NOW' }"#,
        relation_params,
    )?;
    assert!(
        raw_relations.rows.is_empty(),
        "tokio::spawn should remain an external frontier, not a local edge: {raw_relations:#?}"
    );

    db.upsert_proof_fact_values(&[spawn_effect_seed(
        spawn_row.site.id,
        "effect:axum-rag-test-client-task-spawn",
    )])?;

    let effects = rag
        .exact_call_effects_reachable_from_owner(
            start,
            CallPathOptions {
                max_depth: 3,
                max_paths: 16,
            },
        )?
        .expect("call context enabled");
    let effect = effects
        .iter()
        .find(|effect| effect.effect_seed_id == "effect:axum-rag-test-client-task-spawn")
        .unwrap_or_else(|| {
            panic!("RAG should expose the axum tokio::spawn reachable effect: {effects:#?}")
        });
    assert_eq!(effect.effect_class, "async_task_spawn");
    assert_eq!(effect.confidence.as_deref(), Some("source-oracle"));
    assert_eq!(effect.blocker_if_unresolved, Some(false));
    assert!(
        effect.blocker_reasons.is_empty(),
        "non-blocking effect seed should not add proof blockers: {effect:#?}"
    );
    assert_eq!(effect.call_site.site_id, spawn_row.site.id);
    assert_eq!(effect.call_site.owner_id, spawn_owner);
    assert_eq!(effect.call_site.status, CallStatusKind::External);
    let effect_path = effect
        .paths_to_owner
        .iter()
        .find(|path| path.start_id == start && path.end_id == spawn_owner && path.depth == 2)
        .unwrap_or_else(|| {
            panic!("RAG should expose the resolved path to the task-spawn owner: {effect:#?}")
        });
    assert_eq!(effect_path.edges.len(), 2);
    assert_eq!(effect_path.edges[0].caller_id, start);
    assert_eq!(effect_path.edges[1].callee_id, spawn_owner);
    assert!(
        matches!(
            &effect.call_site.callee,
            CallCalleeInfo::Path { path: call_path }
                if call_path == &path(&["tokio", "spawn"])
        ),
        "RAG effect payload should preserve the original tokio::spawn callee path: {effect:#?}"
    );
    assert!(
        effect.call_site.targets.is_empty(),
        "reachable effect annotations must not fabricate RAG target rows: {effect:#?}"
    );

    Ok(())
}

#[tokio::test]
async fn call_effects_exact_reads_admitted_external_summary_effect() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Security/performance:
    //   "Which reviewed external frontiers are reachable from this owner?"
    //   "Which trusted boundary effects are covered by an admitted summary?"
    //
    // Source oracle:
    //   axum/src/response/sse.rs:445 defines `EventDataWriter::write_buf`.
    //   axum/src/response/sse.rs:449 calls
    //     `std::mem::replace(&mut self.data_written, true)`.
    // Expected contract: RAG exposes the DB-derived summary effect after the
    // admitted `std::mem::replace` summary is linked, while the callsite stays
    // external, targetless, and edge-free.
    let owner = method_id_by_name_and_body_substring(&db, "write_buf", "std::mem::replace")?;
    let context = db.call_context_for_owner(owner)?;
    let replace = context
        .iter()
        .find(|row| {
            row.site
                .path
                .as_ref()
                .is_some_and(|call_path| call_path == &path(&["std", "mem", "replace"]))
        })
        .unwrap_or_else(|| {
            panic!("EventDataWriter::write_buf should expose std::mem::replace: {context:#?}")
        });
    assert_eq!(replace.status.status, DbCallStatusKind::External);
    assert!(
        replace.targets.is_empty(),
        "std::mem::replace should stay targetless before summary admission: {replace:#?}"
    );
    let projected = db.project_call_proof_facts_for_owner(owner, "bd:corpus-axum-call-graph")?;
    assert!(
        projected >= 2,
        "EventDataWriter::write_buf should project call_site and call_resolution proof rows: {projected}"
    );

    db.upsert_proof_fact_values(&ploke_test_utils::axum_std_mem_replace_summary_records(
        replace.site.id,
    ))?;

    let summary_id = ploke_test_utils::AXUM_STD_MEM_REPLACE_SUMMARY_ID;
    let effect_id = format!("summary-effect:{summary_id}:external_summary_boundary");
    let db_effects = db.call_effects_reachable_from_owner(
        owner,
        CallPathOptions {
            max_depth: 1,
            max_paths: 16,
        },
    )?;
    assert!(
        db_effects
            .iter()
            .any(|effect| effect.effect_seed_id == effect_id),
        "DB should expose the admitted std::mem::replace summary effect before RAG mapping: {db_effects:#?}"
    );

    let effects = rag
        .exact_call_effects_reachable_from_owner(
            owner,
            CallPathOptions {
                max_depth: 1,
                max_paths: 16,
            },
        )?
        .expect("call context enabled");
    let effect = effects
        .iter()
        .find(|effect| effect.effect_seed_id == effect_id)
        .unwrap_or_else(|| {
            panic!("RAG should expose the admitted std::mem::replace summary effect: {effects:#?}")
        });
    assert_eq!(effect.effect_class, "external_summary_boundary");
    assert_eq!(effect.confidence.as_deref(), Some("source-oracle-review"));
    assert_eq!(effect.blocker_if_unresolved, Some(false));
    assert_eq!(effect.call_site.site_id, replace.site.id);
    assert_eq!(effect.call_site.owner_id, owner);
    assert_eq!(effect.call_site.status, CallStatusKind::External);
    assert!(
        effect.paths_to_owner.is_empty(),
        "direct external summary effect should not need an intermediate path: {effect:#?}"
    );
    assert!(
        effect.blocker_reasons.is_empty(),
        "admitted summary should discharge the missing-summary blocker in RAG: {effect:#?}"
    );
    assert!(
        matches!(
            &effect.call_site.callee,
            CallCalleeInfo::Path { path: call_path }
                if call_path == &path(&["std", "mem", "replace"])
        ),
        "RAG effect payload should preserve the original std::mem::replace path: {effect:#?}"
    );
    assert!(
        effect.call_site.targets.is_empty(),
        "summary-derived effects must not fabricate RAG target rows: {effect:#?}"
    );

    Ok(())
}

#[tokio::test]
async fn call_effect_policy_violations_exact_reads_axum_task_spawn_sink() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Security/performance:
    //   "Can this entrypoint reach a sensitive sink that is outside the
    //   caller's reviewed effect policy?"
    //
    // Source-oracle chain:
    //   axum/src/form.rs:262
    //     `deserialize_error_status_codes` calls `TestClient::new(app)`.
    //   axum/src/test_helpers/test_client.rs:36
    //     `TestClient::new` calls `spawn_service(svc)`.
    //   axum/src/test_helpers/test_client.rs:23
    //     `spawn_service` calls `tokio::spawn(...)`.
    // Expected contract: RAG exposes DB policy violations over existing
    // reachable effect seeds only, preserving the external targetless
    // `tokio::spawn` payload and the resolved path to the sink owner.
    let start = function_id_by_name_in_module(
        &db,
        &["crate", "form", "tests"],
        "deserialize_error_status_codes",
    )?;
    let spawn_owner = function_id_by_name_in_module(
        &db,
        &["crate", "test_helpers", "test_client"],
        "spawn_service",
    )?;
    let spawn_context = db.call_context_for_owner(spawn_owner)?;
    let spawn_row = spawn_context
        .iter()
        .find(|row| {
            row.site
                .path
                .as_ref()
                .is_some_and(|call_path| call_path == &path(&["tokio", "spawn"]))
        })
        .unwrap_or_else(|| {
            panic!("spawn_service should expose the tokio::spawn frontier: {spawn_context:#?}")
        });
    assert_eq!(spawn_row.status.status, DbCallStatusKind::External);
    assert!(
        spawn_row.targets.is_empty(),
        "tokio::spawn should stay targetless in DB call context: {spawn_row:#?}"
    );

    db.upsert_proof_fact_values(&[spawn_effect_seed(
        spawn_row.site.id,
        "effect:axum-rag-test-client-task-spawn-policy",
    )])?;

    let violations = rag
        .exact_call_effect_policy_violations_for_owner(
            start,
            CallPathOptions {
                max_depth: 3,
                max_paths: 16,
            },
            &["ffi_boundary"],
        )?
        .expect("call context enabled");
    let violation = violations
        .iter()
        .find(|violation| {
            violation.effect.effect_seed_id == "effect:axum-rag-test-client-task-spawn-policy"
        })
        .unwrap_or_else(|| {
            panic!("RAG should expose the axum async_task_spawn policy violation: {violations:#?}")
        });
    assert_eq!(violation.allowed_effects, vec!["ffi_boundary".to_string()]);
    assert_eq!(violation.effect.effect_class, "async_task_spawn");
    assert_eq!(violation.effect.call_site.site_id, spawn_row.site.id);
    assert_eq!(violation.effect.call_site.owner_id, spawn_owner);
    assert_eq!(violation.effect.call_site.status, CallStatusKind::External);
    assert!(
        violation
            .effect
            .paths_to_owner
            .iter()
            .any(|path| path.start_id == start && path.end_id == spawn_owner && path.depth == 2),
        "RAG policy violation should preserve the resolved path to spawn_service: {violation:#?}"
    );
    assert!(
        violation.effect.call_site.targets.is_empty(),
        "policy violations must not fabricate RAG target rows: {violation:#?}"
    );

    let allowed = rag
        .exact_call_effect_policy_violations_for_owner(
            start,
            CallPathOptions {
                max_depth: 3,
                max_paths: 16,
            },
            &["async_task_spawn"],
        )?
        .expect("call context enabled");
    assert!(
        allowed.is_empty(),
        "allowing async_task_spawn should clear the RAG policy violation: {allowed:#?}"
    );

    Ok(())
}

#[tokio::test]
async fn stored_effect_policy_exact_reads_axum_task_spawn_sink() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Security/performance:
    //   "Can this entrypoint reach a sensitive sink that is outside its
    //   admitted stored effect policy?"
    //
    // Source-oracle chain:
    //   axum/src/form.rs:262
    //     `deserialize_error_status_codes` calls `TestClient::new(app)`.
    //   axum/src/test_helpers/test_client.rs:36
    //     `TestClient::new` calls `spawn_service(svc)`.
    //   axum/src/test_helpers/test_client.rs:23
    //     `spawn_service` calls `tokio::spawn(...)`.
    // Expected contract: RAG reads the admitted owner policy from proof facts
    // through the DB helper and reports the disallowed `async_task_spawn`
    // frontier while preserving the resolved path to the sink owner.
    let start = function_id_by_name_in_module(
        &db,
        &["crate", "form", "tests"],
        "deserialize_error_status_codes",
    )?;
    let spawn_owner = function_id_by_name_in_module(
        &db,
        &["crate", "test_helpers", "test_client"],
        "spawn_service",
    )?;
    let spawn_context = db.call_context_for_owner(spawn_owner)?;
    let spawn_row = spawn_context
        .iter()
        .find(|row| {
            row.site
                .path
                .as_ref()
                .is_some_and(|call_path| call_path == &path(&["tokio", "spawn"]))
        })
        .unwrap_or_else(|| {
            panic!("spawn_service should expose the tokio::spawn frontier: {spawn_context:#?}")
        });
    assert_eq!(spawn_row.status.status, DbCallStatusKind::External);
    assert!(
        spawn_row.targets.is_empty(),
        "tokio::spawn should stay targetless in DB call context: {spawn_row:#?}"
    );

    db.upsert_proof_fact_values(&[
        spawn_effect_seed(
            spawn_row.site.id,
            "effect:axum-rag-test-client-task-spawn-stored-policy",
        ),
        owner_effect_policy(
            start,
            "effect-policy:axum-rag-test-client:stored-policy",
            &["ffi_boundary"],
        ),
    ])?;

    let violations = rag
        .exact_call_effect_policy_violations_for_stored_owner_policy(
            start,
            CallPathOptions {
                max_depth: 3,
                max_paths: 16,
            },
        )?
        .expect("call context enabled");
    let violation = violations
        .iter()
        .find(|violation| {
            violation.effect.effect_seed_id
                == "effect:axum-rag-test-client-task-spawn-stored-policy"
        })
        .unwrap_or_else(|| {
            panic!(
                "RAG should expose the stored-policy async_task_spawn violation: {violations:#?}"
            )
        });
    assert_eq!(violation.allowed_effects, vec!["ffi_boundary".to_string()]);
    assert_eq!(violation.effect.effect_class, "async_task_spawn");
    assert_eq!(violation.effect.call_site.site_id, spawn_row.site.id);
    assert_eq!(violation.effect.call_site.owner_id, spawn_owner);
    assert_eq!(violation.effect.call_site.status, CallStatusKind::External);
    assert!(
        violation
            .effect
            .paths_to_owner
            .iter()
            .any(|path| path.start_id == start && path.end_id == spawn_owner && path.depth == 2),
        "RAG stored-policy violation should preserve the resolved path to spawn_service: {violation:#?}"
    );
    assert!(
        violation.effect.call_site.targets.is_empty(),
        "stored policy violations must not fabricate RAG target rows: {violation:#?}"
    );

    Ok(())
}

#[tokio::test]
async fn external_summary_needs_exact_reads_axum_request_builder_queue() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;
    let domain_id = "bd:corpus-axum-call-graph";

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Security/performance:
    //   "Which reachable external dependency frontiers still need an audited
    //   summary before their effects can be trusted?"
    //
    // Source oracle:
    //   axum/src/middleware/from_fn.rs:411 calls `Request::builder()`.
    // Expected contract: RAG exposes the DB owner-scoped external-summary
    // queue while the proof blocker is active, and the row disappears after
    // an admitted summary is linked without adding a local call edge.
    let owner =
        function_id_by_name_in_module(&db, &["crate", "middleware", "from_fn", "tests"], "basic")?;
    let context = db.call_context_for_owner(owner)?;
    let builder = context
        .iter()
        .find(|row| {
            row.site
                .path
                .as_ref()
                .is_some_and(|call_path| call_path == &path(&["Request", "builder"]))
        })
        .unwrap_or_else(|| {
            panic!("from_fn::tests::basic should expose Request::builder: {context:#?}")
        });
    assert_eq!(builder.status.status, DbCallStatusKind::External);
    assert!(
        builder.targets.is_empty(),
        "Request::builder should stay targetless before summary admission: {builder:#?}"
    );

    let projected = db.project_call_proof_facts_for_owner(owner, domain_id)?;
    assert!(
        projected >= 2,
        "owner proof projection should include call_site and call_resolution rows: {projected}"
    );
    let options = CallPathOptions {
        max_depth: 3,
        max_paths: 64,
    };
    let needs = rag
        .exact_external_summary_needs_for_owner(owner, options)?
        .expect("call context enabled");
    let need = needs
        .iter()
        .find(|need| need.call_site.site_id == builder.site.id)
        .unwrap_or_else(|| {
            panic!("RAG should expose Request::builder as an external-summary need: {needs:#?}")
        });
    assert_eq!(need.call_site.owner_id, owner);
    assert_eq!(need.call_site.status, CallStatusKind::External);
    assert!(
        need.paths_to_owner.is_empty(),
        "direct owner frontier should not carry an intermediate path: {need:#?}"
    );
    assert!(
        need.blocker_reasons
            .iter()
            .any(|reason| reason == "external_dependency_summary_missing"),
        "RAG need should preserve the active missing-summary blocker: {need:#?}"
    );
    assert!(
        matches!(
            &need.call_site.callee,
            CallCalleeInfo::Path { path: call_path } if call_path == &path(&["Request", "builder"])
        ),
        "RAG need should preserve the original Request::builder path: {need:#?}"
    );

    db.upsert_proof_fact_values(&ploke_test_utils::axum_request_builder_summary_records(
        builder.site.id,
    ))?;
    let after = rag
        .exact_external_summary_needs_for_owner(owner, options)?
        .expect("call context enabled");
    assert!(
        after
            .iter()
            .all(|need| need.call_site.site_id != builder.site.id),
        "admitted Request::builder summary should discharge this RAG need: {after:#?}"
    );

    Ok(())
}

#[tokio::test]
async fn external_summary_needs_exact_reads_axum_std_mem_replace_queue() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;
    let domain_id = "bd:corpus-axum-call-graph";

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Security/performance:
    //   "Which reachable external dependency frontiers still need an audited
    //   summary before their effects can be trusted?"
    //
    // Source oracle:
    //   axum/src/response/sse.rs:449 calls
    //   `std::mem::replace(&mut self.data_written, true)`.
    // Expected contract: RAG exposes the owner-scoped external-summary queue
    // for the std-root frontier while the proof blocker is active, and the row
    // disappears after the admitted summary is linked without adding a local
    // traversal edge.
    let owner = method_id_by_name_and_body_substring(&db, "write_buf", "std::mem::replace")?;
    let context = db.call_context_for_owner(owner)?;
    let replace = context
        .iter()
        .find(|row| {
            row.site
                .path
                .as_ref()
                .is_some_and(|call_path| call_path == &path(&["std", "mem", "replace"]))
        })
        .unwrap_or_else(|| {
            panic!("EventDataWriter::write_buf should expose std::mem::replace: {context:#?}")
        });
    assert_eq!(replace.status.status, DbCallStatusKind::External);
    assert!(
        replace.targets.is_empty(),
        "std::mem::replace should stay targetless before summary admission: {replace:#?}"
    );

    let projected = db.project_call_proof_facts_for_owner(owner, domain_id)?;
    assert!(
        projected >= 2,
        "owner proof projection should include call_site and call_resolution rows: {projected}"
    );
    let options = CallPathOptions {
        max_depth: 3,
        max_paths: 64,
    };
    let needs = rag
        .exact_external_summary_needs_for_owner(owner, options)?
        .expect("call context enabled");
    let need = needs
        .iter()
        .find(|need| need.call_site.site_id == replace.site.id)
        .unwrap_or_else(|| {
            panic!("RAG should expose std::mem::replace as an external-summary need: {needs:#?}")
        });
    assert_eq!(need.call_site.owner_id, owner);
    assert_eq!(need.call_site.status, CallStatusKind::External);
    assert!(
        need.paths_to_owner.is_empty(),
        "direct owner frontier should not carry an intermediate path: {need:#?}"
    );
    assert!(
        need.blocker_reasons
            .iter()
            .any(|reason| reason == "external_dependency_summary_missing"),
        "RAG need should preserve the active missing-summary blocker: {need:#?}"
    );
    assert!(
        matches!(
            &need.call_site.callee,
            CallCalleeInfo::Path { path: call_path }
                if call_path == &path(&["std", "mem", "replace"])
        ),
        "RAG need should preserve the original std::mem::replace path: {need:#?}"
    );

    db.upsert_proof_fact_values(&ploke_test_utils::axum_std_mem_replace_summary_records(
        replace.site.id,
    ))?;
    let after = rag
        .exact_external_summary_needs_for_owner(owner, options)?
        .expect("call context enabled");
    assert!(
        after
            .iter()
            .all(|need| need.call_site.site_id != replace.site.id),
        "admitted std::mem::replace summary should discharge this RAG need: {after:#?}"
    );
    let context_after = db.call_context_for_owner(owner)?;
    let replace_after = context_after
        .iter()
        .find(|row| row.site.id == replace.site.id)
        .unwrap_or_else(|| {
            panic!("std::mem::replace frontier row should remain visible: {context_after:#?}")
        });
    assert!(
        replace_after.targets.is_empty(),
        "summary admission must not fabricate a local std::mem::replace edge: {replace_after:#?}"
    );

    Ok(())
}

#[tokio::test]
async fn external_summary_needs_exact_reads_axum_feature_gated_json_queue() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;
    let domain_id = "bd:corpus-axum-call-graph";

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Security/performance:
    //   "Which feature-gated external dependency frontiers still need an
    //   audited summary before their effects can be trusted?"
    //
    // Source oracle:
    //   axum/src/json.rs:164 defines `Json::from_bytes`.
    //   axum/src/json.rs:184 calls
    //     `serde_json::Deserializer::from_slice(bytes)`.
    //   axum/src/lib.rs:488-489 gates the file module with
    //     `#[cfg(feature = "json")] mod json;`.
    // Expected contract: RAG exposes the owner-scoped external-summary queue
    // for the feature-gated serde_json frontier while the proof blocker is
    // active, and the row disappears after the admitted summary is linked
    // without adding a local traversal edge.
    let owner = method_id_by_file(
        &db,
        "from_bytes",
        "serde_json::Deserializer::from_slice(bytes)",
        "axum/src/json.rs",
    )?;
    let context = db.call_context_for_owner(owner)?;
    let from_slice = context
        .iter()
        .find(|row| {
            row.site.path.as_ref().is_some_and(|call_path| {
                call_path == &path(&["serde_json", "Deserializer", "from_slice"])
            })
        })
        .unwrap_or_else(|| {
            panic!(
                "Json::from_bytes should expose serde_json::Deserializer::from_slice: {context:#?}"
            )
        });
    assert_eq!(from_slice.status.status, DbCallStatusKind::External);
    assert!(
        from_slice.targets.is_empty(),
        "serde_json::Deserializer::from_slice should stay targetless before summary admission: {from_slice:#?}"
    );

    let projected = db.project_call_proof_facts_for_owner(owner, domain_id)?;
    assert!(
        projected >= 2,
        "owner proof projection should include call_site and call_resolution rows: {projected}"
    );
    let options = CallPathOptions {
        max_depth: 1,
        max_paths: 16,
    };
    let needs = rag
        .exact_external_summary_needs_for_owner(owner, options)?
        .expect("call context enabled");
    let need = needs
        .iter()
        .find(|need| need.call_site.site_id == from_slice.site.id)
        .unwrap_or_else(|| {
            panic!(
                "RAG should expose serde_json::Deserializer::from_slice as an external-summary need: {needs:#?}"
            )
        });
    assert_eq!(need.call_site.owner_id, owner);
    assert_eq!(need.call_site.status, CallStatusKind::External);
    assert!(
        need.paths_to_owner.is_empty(),
        "direct owner frontier should not carry an intermediate path: {need:#?}"
    );
    assert!(
        need.blocker_reasons
            .iter()
            .any(|reason| reason == "external_dependency_summary_missing"),
        "RAG need should preserve the active missing-summary blocker: {need:#?}"
    );
    assert!(
        matches!(
            &need.call_site.callee,
            CallCalleeInfo::Path { path: call_path }
                if call_path == &path(&["serde_json", "Deserializer", "from_slice"])
        ),
        "RAG need should preserve the original serde_json::Deserializer::from_slice path: {need:#?}"
    );

    db.upsert_proof_fact_values(
        &ploke_test_utils::axum_serde_json_from_slice_summary_records(from_slice.site.id),
    )?;
    let after = rag
        .exact_external_summary_needs_for_owner(owner, options)?
        .expect("call context enabled");
    assert!(
        after
            .iter()
            .all(|need| need.call_site.site_id != from_slice.site.id),
        "admitted serde_json::Deserializer::from_slice summary should discharge this RAG need: {after:#?}"
    );
    let context_after = db.call_context_for_owner(owner)?;
    let from_slice_after = context_after
        .iter()
        .find(|row| row.site.id == from_slice.site.id)
        .unwrap_or_else(|| {
            panic!(
                "serde_json::Deserializer::from_slice frontier row should remain visible: {context_after:#?}"
            )
        });
    assert!(
        from_slice_after.targets.is_empty(),
        "summary admission must not fabricate a local serde_json::Deserializer::from_slice edge: {from_slice_after:#?}"
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

fn assert_call_source_crate(crates: &[String], expected: &str, label: &str) {
    assert!(
        crates.iter().any(|name| name == expected),
        "{label} should include crate {expected:?}: {crates:#?}"
    );
}

fn assert_call_source_module(modules: &[Vec<String>], expected: &[&str], label: &str) {
    let expected = path(expected);
    assert!(
        modules.iter().any(|module| module == &expected),
        "{label} should include module {expected:?}: {modules:#?}"
    );
}

#[tokio::test]
async fn call_context_collection_preserves_axum_turbofish_generic_counts() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    let owner = function_id_by_name_in_module(
        &db,
        &["crate", "attr_parsing"],
        "parse_parenthesized_attribute",
    )?;

    let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
    let context = call_context.get(&owner).unwrap_or_else(|| {
        panic!("parse_parenthesized_attribute should receive outgoing call context")
    });
    let type_name = context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Path
                && matches!(
                    &call.callee,
                    CallCalleeInfo::Path { path: call_path }
                        if call_path == &path(&["std", "any", "type_name"])
                )
        })
        .unwrap_or_else(|| {
            panic!("RAG context should include std::any::type_name::<K> row: {context:#?}")
        });

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // API understanding:
    //   "What argument shapes do existing callers pass?"
    // Build/deployment optimization:
    //   "Are there feature-gated or platform-specific call paths that should
    //   be checked separately?"
    //
    // Source oracle:
    //   axum-macros/src/attr_parsing.rs:22 calls
    //   `std::any::type_name::<K>()`.
    //
    // Current contract: dependency-root calls stay external and targetless,
    // but RAG payloads preserve the turbofish arity needed for API-shape
    // answers.
    assert_eq!(type_name.owner_id, owner);
    assert_eq!(type_name.status, CallStatusKind::External);
    assert_eq!(type_name.generic_arg_count, Some(1));
    assert!(
        type_name.targets.is_empty(),
        "external type_name::<K> call should remain targetless: {type_name:#?}"
    );

    Ok(())
}

#[tokio::test]
async fn call_context_collection_preserves_axum_turbofish_method_receiver_shape()
-> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    let owner = function_id_by_name_in_module(
        &db,
        &["crate", "ext_traits", "request_parts", "tests"],
        "extract_with_state",
    )?;
    let target =
        method_id_by_name_and_body_substring(&db, "extract_with_state", "E::from_request_parts")?;

    let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
    let context = call_context.get(&owner).unwrap_or_else(|| {
        panic!("request_parts::tests::extract_with_state should receive outgoing call context")
    });
    let expected_callee = CallCalleeInfo::Method {
        name: "extract_with_state".to_string(),
        receiver: Some(CallReceiverInfo::TupleMethodReturn {
            name: "parts".to_string(),
            method_name: "into_parts".to_string(),
            method_span: (4640, 4669),
            index: 0,
        }),
    };
    let turbofish = context
        .iter()
        .filter(|call| call.kind == CallSiteKind::Method && call.callee == expected_callee)
        .collect::<Vec<_>>();

    // Matrix:
    //   docs/active/agents/call-graph/
    //   2026-06-28_real-corpus-call-site-oracle-matrices.md
    //
    // Source chain:
    //   axum-core/src/ext_traits/request_parts.rs:159 binds `parts` from
    //   `Request::new(()).into_parts()`.
    //   axum-core/src/ext_traits/request_parts.rs:164 calls
    //   `parts.extract_with_state::<State<String>, String>(&state)`.
    //
    // Current contract: the row is visible to RAG, preserves the two explicit
    // method generic arguments and tuple-method-return receiver proof, and
    // resolves to the local `RequestPartsExt for Parts::extract_with_state`
    // method through the external-return summary for `Request::into_parts`.
    assert_eq!(
        turbofish.len(),
        1,
        "RAG call context should expose exactly the request_parts.rs:164 turbofish row: {context:#?}"
    );

    let call = turbofish[0];
    assert_eq!(call.owner_id, owner);
    assert_eq!(call.status, CallStatusKind::Resolved);
    assert_eq!(call.generic_arg_count, Some(2));
    assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(call.targets.len(), 1);
    assert_eq!(call.targets[0].target_id, target);
    let reach = rag
        .exact_call_reach_for_owner(
            owner,
            CallPathOptions {
                max_depth: 2,
                max_paths: 128,
            },
        )?
        .expect("call context enabled");
    assert!(
        reach
            .unsupported_frontier_calls
            .iter()
            .all(|frontier| frontier.site_id != call.site_id),
        "RAG reach should not keep the resolved turbofish row in the unsupported frontier: {reach:#?}"
    );
    let path = reach
        .paths
        .iter()
        .find(|path| {
            path.edges
                .iter()
                .any(|edge| edge.call_site_id == call.site_id && edge.callee_id == target)
        })
        .unwrap_or_else(|| {
            panic!("RAG reach should expose the resolved turbofish edge: {reach:#?}")
        });
    assert!(
        path.edges
            .iter()
            .any(|edge| edge.call_site_id == call.site_id && edge.callee_id == target),
        "RAG reach should include the request_parts.rs:164 edge to RequestPartsExt::extract_with_state: {path:#?}"
    );

    Ok(())
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
        receiver: Some(CallReceiverInfo::AwaitMethodCallResult {
            method_name: "acquire_owned".to_string(),
        }),
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
    // `AwaitMethodCallResult(acquire_owned).unwrap` row as unsupported and
    // targetless. It must expose zero traversal targets rather than guessing
    // the concrete awaited result type or a local `unwrap` callee.
    assert_eq!(
        await_unwrap.len(),
        1,
        "RAG call context should expose exactly the listener AwaitMethodCallResult unwrap row: {context:#?}"
    );

    let call = await_unwrap[0];
    assert_eq!(call.owner_id, owner);
    assert_eq!(call.status, CallStatusKind::Unsupported);
    assert_eq!(call.resolution, None);
    assert!(
        call.targets.is_empty(),
        "AwaitMethodCallResult unwrap should remain targetless in RAG call context: {call:#?}"
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
                "RAG reach should expose AwaitMethodCallResult unwrap in unsupported frontier rows: {reach:#?}"
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

fn listener_accept_owner_ids(db: &Database) -> Result<Vec<Uuid>, Error> {
    let mut params = BTreeMap::new();
    params.insert("name".to_string(), DataValue::from("accept"));
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
    file_owner_for_module[mod_id, file_id],
    *file_mod{{ owner_id: file_id, file_path @ 'NOW' }}
"#
    );
    let rows = db.raw_query_params(&script, params)?;
    rows.rows
        .iter()
        .filter_map(|row| {
            let DataValue::Str(body) = &row[1] else {
                return None;
            };
            let DataValue::Str(file_path) = &row[2] else {
                return None;
            };
            (body_key(body).contains(&body_key("Self::accept(self).await"))
                && file_path.ends_with("axum/src/serve/listener.rs"))
            .then(|| row[0].clone())
        })
        .map(|value| to_uuid(&value).map_err(Error::from))
        .collect()
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

fn macro_id_by_name(db: &Database, name: &str) -> Result<Uuid, Error> {
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

fn variant_id_by_enum_and_variant_names(
    db: &Database,
    enum_name: &str,
    variant_name: &str,
) -> Result<Uuid, Error> {
    let mut params = BTreeMap::new();
    params.insert("enum_name".to_string(), DataValue::from(enum_name));
    params.insert("variant_name".to_string(), DataValue::from(variant_name));

    let rows = db.raw_query_params(
        r#"?[id] :=
            *enum { id: enum_id, name: $enum_name @ 'NOW' },
            *variant { id, name: $variant_name, owner_id: enum_id @ 'NOW' }"#,
        params,
    )?;
    assert_eq!(
        rows.rows.len(),
        1,
        "expected exactly one enum variant {enum_name}::{variant_name}; rows: {:#?}",
        rows.rows
    );

    to_uuid(&rows.rows[0][0]).map_err(Error::from)
}

fn body_key(value: &str) -> String {
    value.chars().filter(|ch| !ch.is_whitespace()).collect()
}
