use std::collections::{BTreeMap, BTreeSet};

use cozo::{DataValue, UuidWrapper};
use ploke_core::rag_types::CallReachEffectInfo;
use ploke_db::multi_embedding::db_ext::{ANCESTOR_RULES_NOW, METHOD_NODE_ANCESTOR_RULE};
use ploke_db::{
    CallReceiver, CallRelationKind as DbCallRelationKind,
    CallResolutionKind as DbCallResolutionKind, CallSiteKind as DbCallSiteKind,
    CallStatusKind as DbCallStatusKind, CallTargetKind as DbCallTargetKind,
    CrateBoundaryPolicyRule, ModuleBoundaryPolicyRule, ProofGraphStore,
};
use serde_json::json;

use super::super::super::super::super::*;
use super::expected::path;

mod ifunc;
mod local_bindings;
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

fn effect_seed(
    call_site_id: impl ToString,
    effect_seed_id: &str,
    effect_class: &str,
) -> serde_json::Value {
    json!({
        "fact_kind": "effect_seed",
        "schema_version": "ploke-proof-facts.v1",
        "effect_seed_id": effect_seed_id,
        "call_site_id": call_site_id.to_string(),
        "effect_class": effect_class,
        "confidence": "source-oracle",
        "blocker_if_unresolved": false,
        "evidence_use": "proof_only"
    })
}

fn spawn_effect_seed(call_site_id: impl ToString, effect_seed_id: &str) -> serde_json::Value {
    effect_seed(call_site_id, effect_seed_id, "async_task_spawn")
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

type ExpectedPaths<'a> = &'a [(&'a [&'a str], usize)];
type IncomingCase<'a> = (Uuid, CallTargetKind, usize, ExpectedPaths<'a>, &'a str);

// Source-oracle ledger:
// - incoming rows: Body::{empty,new}, parse_attrs, Json::from_bytes,
//   BoxedIntoRoute, Handler::call, MappedLocalTime::Single, and naive_utc;
// - outgoing rows: generated rejection methods, generated MethodRouter
//   chaining, both turbofish shapes, and the awaited-result frontier.
// Detailed source locations remain in the shared real-corpus oracle matrix:
// docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md.
#[tokio::test]
async fn call_context_exact_reads_axum_incoming_matrix() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;
    let cases: &[IncomingCase<'_>] = &[
        (
            method_id_by_name_and_body_substring(&db, "empty", "Empty::new()")?,
            CallTargetKind::AssociatedFunction,
            23,
            &[(&["Body", "empty"], 21), (&["Self", "empty"], 2)],
            "Body::empty",
        ),
        (
            function_id_by_name_in_module(&db, &["crate", "attr_parsing"], "parse_attrs")?,
            CallTargetKind::Function,
            11,
            &[
                (&["crate", "attr_parsing", "parse_attrs"], 1),
                (&["parse_attrs"], 10),
            ],
            "parse_attrs",
        ),
        (
            method_id_by_name_and_body_substring(
                &db,
                "from_bytes",
                "serde_json::Deserializer::from_slice(bytes)",
            )?,
            CallTargetKind::AssociatedFunction,
            2,
            &[(&["Self", "from_bytes"], 2)],
            "Json::from_bytes",
        ),
        (
            struct_id_by_name(&db, "BoxedIntoRoute")?,
            CallTargetKind::TupleStructConstructor,
            3,
            &[(&["BoxedIntoRoute"], 1), (&["Self"], 2)],
            "BoxedIntoRoute",
        ),
        (
            method_id_by_trait_name(&db, "Handler", "call")?,
            CallTargetKind::AssociatedFunction,
            1,
            &[(&["Handler", "call"], 1)],
            "Handler::call",
        ),
    ];
    for (target, relation, count, paths, label) in cases {
        assert_path_incoming(&db, &rag, *target, relation, *count, paths, label)?;
    }

    let target = method_id_by_file(&db, "new", "try_downcast(body)", "axum-core/src/body.rs")?;
    let generated = axum_body_from_impl_generated_callers(&db, target)?;
    let callers = db.callers_for_target(target)?;
    assert!(
        callers.len() >= generated.len(),
        "Body::new callers should include the generated subset: {callers:#?}"
    );
    let context = rag.exact_call_context(target)?;
    let incoming = path_target_calls(&context, target);
    assert_incoming_ids(
        &incoming,
        callers
            .iter()
            .map(|caller| (caller.site.owner_id, caller.site.id))
            .collect(),
        "Body::new",
    );
    assert_eq!(generated.len(), 7, "generated Body::from caller count");
    for expected in generated {
        let call = incoming
            .iter()
            .copied()
            .find(|call| call.owner_id == expected.owner && call.site_id == expected.site)
            .unwrap_or_else(|| {
                panic!("missing generated Body::from caller {expected:#?}: {context:#?}")
            });
        assert_resolved_target(
            call,
            target,
            &CallTargetKind::AssociatedFunction,
            "Body::from",
        );
        assert_eq!(
            call.callee,
            CallCalleeInfo::Path {
                path: path(&["Self", "new"]),
            },
            "generated Body::from callee"
        );
    }

    Ok(())
}

#[tokio::test]
async fn call_context_exact_reads_chrono_incoming_matrix() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_chrono_call_graph_rag()?;

    let target = variant_id_by_enum_and_variant_names(&db, "LocalResult", "Single")?;
    assert_path_incoming(
        &db,
        &rag,
        target,
        &CallTargetKind::EnumVariantConstructor,
        12,
        &[(&["MappedLocalTime", "Single"], 12)],
        "MappedLocalTime::Single",
    )?;

    let target = method_id_by_name_and_body_substring(&db, "naive_utc", "self.datetime")?;
    let callers = db.callers_for_target(target)?;
    assert_eq!(callers.len(), 7, "naive_utc DB callers: {callers:#?}");
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
    assert_eq!(incoming.len(), 7, "naive_utc incoming rows: {context:#?}");
    assert_incoming_ids(
        &incoming,
        callers
            .iter()
            .map(|caller| (caller.site.owner_id, caller.site.id))
            .collect(),
        "naive_utc",
    );

    let mut receiver_counts = BTreeMap::<CallReceiverInfo, usize>::new();
    for call in incoming {
        assert_resolved_target(call, target, &CallTargetKind::Method, "naive_utc");
        let CallCalleeInfo::Method {
            name: method_name,
            receiver: Some(receiver),
        } = &call.callee
        else {
            panic!("naive_utc incoming caller should be a method call: {call:#?}");
        };
        assert_eq!(method_name, "naive_utc");
        *receiver_counts.entry(receiver.clone()).or_default() += 1;
    }
    assert_eq!(
        receiver_counts,
        BTreeMap::from([
            (
                CallReceiverInfo::TryMethodCallResult {
                    method_name: "ok_or".to_string(),
                },
                2,
            ),
            (
                CallReceiverInfo::InitializedLocalBinding {
                    name: "now".to_string(),
                    init_path: path(&["Local", "now"]),
                },
                2,
            ),
            (
                CallReceiverInfo::PathCallResult {
                    path: path(&["DateTime", "from_timestamp_nanos"]),
                },
                1,
            ),
            (CallReceiverInfo::SelfValue, 2),
        ]),
        "RAG call context should preserve all DateTime::naive_utc receiver shapes"
    );

    Ok(())
}

fn assert_path_incoming(
    db: &Database,
    rag: &RagService,
    target: Uuid,
    relation: &CallTargetKind,
    count: usize,
    expected: ExpectedPaths<'_>,
    label: &str,
) -> Result<(), Error> {
    let callers = db.callers_for_target(target)?;
    assert_eq!(callers.len(), count, "{label} DB callers: {callers:#?}");
    let context = rag.exact_call_context(target)?;
    let incoming = path_target_calls(&context, target);
    assert_eq!(incoming.len(), count, "{label} RAG callers: {context:#?}");
    assert_incoming_ids(
        &incoming,
        callers
            .iter()
            .map(|caller| (caller.site.owner_id, caller.site.id))
            .collect(),
        label,
    );

    let mut actual = BTreeMap::<Vec<String>, usize>::new();
    for call in incoming {
        assert_resolved_target(call, target, relation, label);
        let CallCalleeInfo::Path { path } = &call.callee else {
            panic!("{label} incoming caller should be a path: {call:#?}");
        };
        *actual.entry(path.clone()).or_default() += 1;
    }
    let expected = expected
        .iter()
        .map(|(parts, count)| (path(parts), *count))
        .collect();
    assert_eq!(actual, expected, "{label} callee path counts");
    Ok(())
}

fn path_target_calls(context: &[CallContextInfo], target: Uuid) -> Vec<&CallContextInfo> {
    context
        .iter()
        .filter(|call| {
            call.kind == CallSiteKind::Path
                && call
                    .targets
                    .iter()
                    .any(|candidate| candidate.target_id == target)
        })
        .collect()
}

fn assert_incoming_ids(calls: &[&CallContextInfo], expected: BTreeSet<(Uuid, Uuid)>, label: &str) {
    let actual = calls
        .iter()
        .map(|call| (call.owner_id, call.site_id))
        .collect::<BTreeSet<_>>();
    assert_eq!(actual, expected, "{label} caller owner/site identities");
}

fn assert_resolved_target(
    call: &CallContextInfo,
    target: Uuid,
    relation: &CallTargetKind,
    label: &str,
) {
    assert_eq!(call.status, CallStatusKind::Resolved, "{label} status");
    assert_eq!(
        call.resolution,
        Some(CallResolutionKind::LocalExact),
        "{label} resolution"
    );
    assert_eq!(call.targets.len(), 1, "{label} target count");
    assert_eq!(call.targets[0].target_id, target, "{label} target");
    assert_eq!(&call.targets[0].relation, relation, "{label} relation");
}

#[derive(Clone, Copy)]
enum ContextQuery {
    Exact,
    Collected,
}

struct ExpectedTarget {
    id: Uuid,
    relation: Option<CallTargetKind>,
}

struct OutgoingCase {
    label: String,
    owner: Uuid,
    site: Option<Uuid>,
    query: ContextQuery,
    select_target: bool,
    kind: CallSiteKind,
    callee: CallCalleeInfo,
    status: CallStatusKind,
    resolution: Option<Option<CallResolutionKind>>,
    target: Option<ExpectedTarget>,
    arg_count: Option<u32>,
    generic_count: Option<u32>,
    match_count: Option<usize>,
}

fn resolved_method_case(
    label: String,
    owner: Uuid,
    site: Option<Uuid>,
    name: &str,
    receiver: CallReceiverInfo,
    target: Uuid,
) -> OutgoingCase {
    OutgoingCase {
        label,
        owner,
        site,
        query: ContextQuery::Collected,
        select_target: false,
        kind: CallSiteKind::Method,
        callee: CallCalleeInfo::Method {
            name: name.to_string(),
            receiver: Some(receiver),
        },
        status: CallStatusKind::Resolved,
        resolution: Some(Some(CallResolutionKind::LocalExact)),
        target: Some(ExpectedTarget {
            id: target,
            relation: Some(CallTargetKind::Method),
        }),
        arg_count: None,
        generic_count: None,
        match_count: None,
    }
}

#[tokio::test]
async fn call_context_reads_axum_outgoing_matrix() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    let owner = method_id_by_name_and_body_substring(
        &db,
        "into_response",
        "rejection_type=MissingExtension",
    )?;
    for row in missing_extension_self_methods(&db, owner)? {
        assert_outgoing_case(
            &rag,
            &resolved_method_case(
                format!("generated MissingExtension self.{}", row.method),
                owner,
                Some(row.site),
                row.method,
                CallReceiverInfo::SelfValue,
                row.target,
            ),
        )?;
    }

    let owner = method_id_by_name_and_body_substring(
        &db,
        "into_response",
        "Self::FailedToDeserializeQueryString(inner)=>inner.into_response()",
    )?;
    let row = query_rejection_delegate_method(&db, owner)?;
    assert_outgoing_case(
        &rag,
        &resolved_method_case(
            "generated QueryRejection delegate".to_string(),
            owner,
            Some(row.site),
            row.method,
            CallReceiverInfo::EnumVariantBinding {
                name: "inner".to_string(),
                enum_path: path(&["Self"]),
                variant_name: "FailedToDeserializeQueryString".to_string(),
                field_index: 0,
            },
            row.target,
        ),
    )?;

    for (owner, target, method) in [
        (
            method_id_by_file(
                &db,
                "post",
                "self.on(MethodFilter::",
                "axum/src/routing/method_routing.rs",
            )?,
            method_id_by_file(
                &db,
                "on",
                "self.on_endpoint(filter, &MethodEndpoint::BoxedHandler",
                "axum/src/routing/method_routing.rs",
            )?,
            "on",
        ),
        (
            method_id_by_file(
                &db,
                "post_service",
                "self.on_service(MethodFilter::",
                "axum/src/routing/method_routing.rs",
            )?,
            method_id_by_file(
                &db,
                "on_service",
                "self.on_endpoint(filter, &MethodEndpoint::Route",
                "axum/src/routing/method_routing.rs",
            )?,
            "on_service",
        ),
    ] {
        let mut case = resolved_method_case(
            format!("generated chained {method} edge"),
            owner,
            None,
            method,
            CallReceiverInfo::SelfValue,
            target,
        );
        case.query = ContextQuery::Exact;
        case.select_target = true;
        case.arg_count = Some(2);
        assert_outgoing_case(&rag, &case)?;
    }

    let owner = function_id_by_name_in_module(
        &db,
        &["crate", "attr_parsing"],
        "parse_parenthesized_attribute",
    )?;
    assert_outgoing_case(
        &rag,
        &OutgoingCase {
            label: "std::any::type_name::<K>".to_string(),
            owner,
            site: None,
            query: ContextQuery::Collected,
            select_target: false,
            kind: CallSiteKind::Path,
            callee: CallCalleeInfo::Path {
                path: path(&["std", "any", "type_name"]),
            },
            status: CallStatusKind::External,
            resolution: None,
            target: None,
            arg_count: None,
            generic_count: Some(1),
            match_count: None,
        },
    )?;

    let owner = function_id_by_name_in_module(
        &db,
        &["crate", "ext_traits", "request_parts", "tests"],
        "extract_with_state",
    )?;
    let target =
        method_id_by_name_and_body_substring(&db, "extract_with_state", "E::from_request_parts")?;
    let site = assert_outgoing_case(
        &rag,
        &OutgoingCase {
            label: "request_parts extract_with_state turbofish".to_string(),
            owner,
            site: None,
            query: ContextQuery::Collected,
            select_target: false,
            kind: CallSiteKind::Method,
            callee: CallCalleeInfo::Method {
                name: "extract_with_state".to_string(),
                receiver: Some(CallReceiverInfo::TupleMethodReturn {
                    name: "parts".to_string(),
                    method_name: "into_parts".to_string(),
                    method_span: (4640, 4669),
                    index: 0,
                }),
            },
            status: CallStatusKind::Resolved,
            resolution: Some(Some(CallResolutionKind::LocalExact)),
            target: Some(ExpectedTarget {
                id: target,
                relation: None,
            }),
            arg_count: None,
            generic_count: Some(2),
            match_count: Some(1),
        },
    )?;
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
            .all(|frontier| frontier.site_id != site),
        "resolved turbofish row must leave the unsupported frontier: {reach:#?}"
    );
    let path = reach
        .paths
        .iter()
        .find(|path| {
            path.edges
                .iter()
                .any(|edge| edge.call_site_id == site && edge.callee_id == target)
        })
        .unwrap_or_else(|| panic!("resolved turbofish edge missing: {reach:#?}"));
    assert!(
        path.edges
            .iter()
            .any(|edge| edge.call_site_id == site && edge.callee_id == target),
        "resolved turbofish path must retain the exact edge: {path:#?}"
    );

    let owner = method_id_by_name_and_body_substring(
        &db,
        "accept",
        "self.sem.clone().acquire_owned().await.unwrap()",
    )?;
    let site = assert_outgoing_case(
        &rag,
        &OutgoingCase {
            label: "ConnLimiter::accept awaited unwrap".to_string(),
            owner,
            site: None,
            query: ContextQuery::Collected,
            select_target: false,
            kind: CallSiteKind::Method,
            callee: CallCalleeInfo::Method {
                name: "unwrap".to_string(),
                receiver: Some(CallReceiverInfo::AwaitMethodCallResult {
                    method_name: "acquire_owned".to_string(),
                }),
            },
            status: CallStatusKind::External,
            resolution: Some(None),
            target: None,
            arg_count: None,
            generic_count: None,
            match_count: Some(1),
        },
    )?;
    let reach = rag
        .exact_call_reach_for_owner(
            owner,
            CallPathOptions {
                max_depth: 2,
                max_paths: 128,
            },
        )?
        .expect("call context enabled");
    let frontier = reach
        .external_frontier_calls
        .iter()
        .find(|frontier| frontier.site_id == site)
        .unwrap_or_else(|| panic!("awaited unwrap external frontier missing: {reach:#?}"));
    assert_eq!(frontier.owner_id, owner);
    assert_eq!(frontier.status, CallStatusKind::External);
    assert!(
        frontier.targets.is_empty(),
        "awaited unwrap frontier must remain targetless: {frontier:#?}"
    );

    Ok(())
}

fn assert_outgoing_case(rag: &RagService, case: &OutgoingCase) -> Result<Uuid, Error> {
    let context = match case.query {
        ContextQuery::Exact => rag.exact_call_context(case.owner)?,
        ContextQuery::Collected => {
            let contexts = rag.collect_call_context(&[(case.owner, 1.0)])?;
            contexts
                .get(&case.owner)
                .unwrap_or_else(|| panic!("{} should receive outgoing context", case.label))
                .clone()
        }
    };
    let matching = context
        .iter()
        .filter(|call| {
            call.kind == case.kind
                && call.callee == case.callee
                && case.site.is_none_or(|site| call.site_id == site)
                && (!case.select_target
                    || case.target.as_ref().is_some_and(|target| {
                        call.targets
                            .iter()
                            .any(|candidate| candidate.target_id == target.id)
                    }))
        })
        .collect::<Vec<_>>();
    if let Some(count) = case.match_count {
        assert_eq!(
            matching.len(),
            count,
            "{} matching row count: {context:#?}",
            case.label
        );
    }
    let call = matching
        .first()
        .copied()
        .unwrap_or_else(|| panic!("{} missing from outgoing context: {context:#?}", case.label));
    assert_eq!(call.owner_id, case.owner, "{} owner", case.label);
    assert_eq!(call.status, case.status, "{} status", case.label);
    if let Some(resolution) = &case.resolution {
        assert_eq!(&call.resolution, resolution, "{} resolution", case.label);
    }
    match &case.target {
        Some(target) => {
            assert_eq!(call.targets.len(), 1, "{} target count", case.label);
            assert_eq!(
                call.targets[0].target_id, target.id,
                "{} target",
                case.label
            );
            if let Some(relation) = &target.relation {
                assert_eq!(
                    &call.targets[0].relation, relation,
                    "{} relation",
                    case.label
                );
            }
        }
        None => assert!(
            call.targets.is_empty(),
            "{} must remain targetless: {call:#?}",
            case.label
        ),
    }
    if let Some(count) = case.arg_count {
        assert_eq!(call.arg_count, Some(count), "{} argument count", case.label);
    }
    if let Some(count) = case.generic_count {
        assert_eq!(
            call.generic_arg_count,
            Some(count),
            "{} generic argument count",
            case.label
        );
    }
    Ok(call.site_id)
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
    let call = matching[0];
    assert_eq!(call.owner_id, owner);
    assert_eq!(call.status, CallStatusKind::External);
    assert_eq!(call.resolution, None);
    assert!(
        call.targets.is_empty(),
        "chrono queue.is_empty slice frontier should remain targetless: {matching:#?}"
    );
    let reach = rag
        .exact_call_reach_for_owner(
            owner,
            CallPathOptions {
                max_depth: 2,
                max_paths: 64,
            },
        )?
        .expect("call context enabled");
    let external = reach
        .external_frontier_calls
        .iter()
        .find(|frontier| frontier.site_id == call.site_id)
        .unwrap_or_else(|| {
            panic!("RAG reach should preserve the guarded slice `is_empty` frontier: {reach:#?}")
        });
    assert_eq!(external.owner_id, owner);
    assert_eq!(external.status, CallStatusKind::External);
    assert!(
        external.targets.is_empty(),
        "RAG reach should keep chrono queue.is_empty targetless: {external:#?}"
    );
    assert!(
        reach.paths.iter().all(|path| path
            .edges
            .iter()
            .all(|edge| edge.call_site_id != call.site_id)),
        "RAG reach must not fabricate a traversal edge for the external slice method: {reach:#?}"
    );
    let projected = db.project_call_proof_facts_for_node(owner, "bd:corpus-chrono-call-graph")?;
    assert!(
        projected >= 2,
        "chrono shared-matrix owner should project node-scoped call proof rows: {projected}"
    );
    let needs = rag
        .exact_external_summary_needs_for_owner(
            owner,
            CallPathOptions {
                max_depth: 2,
                max_paths: 64,
            },
        )?
        .expect("call context enabled");
    let need = needs
        .iter()
        .find(|need| need.call_site.site_id == call.site_id)
        .unwrap_or_else(|| {
            panic!(
                "RAG external-summary needs should preserve the guarded slice `is_empty` frontier: {needs:#?}"
            )
        });
    assert_eq!(need.call_site.owner_id, owner);
    assert_eq!(need.call_site.status, CallStatusKind::External);
    assert!(
        need.call_site.targets.is_empty(),
        "RAG external-summary need should keep chrono queue.is_empty targetless: {need:#?}"
    );
    assert!(
        need.blocker_reasons
            .iter()
            .any(|reason| reason == "external_dependency_summary_missing"),
        "RAG external-summary need should preserve the missing-summary blocker: {need:#?}"
    );

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
            // FromRequest::from_request has generated handler arity callers,
            // so target-centered traversal needs the same fan-in budget as
            // the DB oracle to include the inspected RequestExt source path.
            max_paths: 128,
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
                max_paths: 128,
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
        34,
        "RAG impact summary should preserve all direct target-centered callsite rows, including generated handler and tuple extractor rows: {report:#?}"
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
                && bucket.count == 34
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
async fn call_test_selection_exact_keeps_generated_harness_proof_only() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Test planning / build optimization:
    //   "Which source tests and generated harness entrypoints should be
    //   considered for a change to this code item?"
    //
    // Source-test oracle:
    //   axum/src/routing/mod.rs:162 defines `Router::new`.
    //   axum/src/serve/mod.rs:756 calls `Router::new()` from
    //   `serve::tests::if_it_compiles_it_works`.
    //
    // Generated-entrypoint oracle:
    //   axum/src/error_handling/mod.rs:257 defines private `#[test] fn traits()`.
    //   The Rust test harness is generated outside stored source, so it remains
    //   an admitted `entrypoint_summary` proof row, not a source call edge.
    let router_new = method_id_by_name_and_body_substring(&db, "new", "default_fallback: true")?;
    let source_test = function_id_by_name_in_module(
        &db,
        &["crate", "serve", "tests"],
        "if_it_compiles_it_works",
    )?;

    let router_selection = rag
        .exact_call_test_selection_for_target(
            router_new,
            CallPathOptions {
                max_depth: 1,
                max_paths: 512,
            },
        )?
        .expect("call context enabled");
    assert_eq!(router_selection.target.id, router_new);
    assert_call_node(
        &router_selection.source_test_callers,
        source_test,
        "if_it_compiles_it_works",
        "axum/src/serve/mod.rs",
        "RAG Router::new source test selection callers",
    );
    assert!(
        router_selection
            .source_test_paths
            .iter()
            .any(|path| path.start_id == source_test
                && path.end_id == router_new
                && path.depth == 1),
        "RAG Router::new test selection should preserve the resolved source-test path: {router_selection:#?}"
    );
    assert!(
        router_selection.generated_entrypoints.is_empty(),
        "RAG source-test selection should not invent generated entrypoint metadata: {router_selection:#?}"
    );
    assert!(
        router_selection.build_domains.is_empty(),
        "RAG Router::new has no admitted build-domain proof in this test: {router_selection:#?}"
    );

    let traits = function_id_by_name_in_module(&db, &["crate", "error_handling"], "traits")?;
    let before = rag
        .exact_call_test_selection_for_target(
            traits,
            CallPathOptions {
                max_depth: 3,
                max_paths: 16,
            },
        )?
        .expect("call context enabled");
    assert!(before.source_test_callers.is_empty(), "{before:#?}");
    assert!(before.source_test_paths.is_empty(), "{before:#?}");
    assert!(before.generated_entrypoints.is_empty(), "{before:#?}");
    assert!(before.build_domains.is_empty(), "{before:#?}");

    let domain_id = "bd:corpus-axum-call-graph";
    let mut records = ploke_test_utils::axum_call_graph_domain_records(domain_id);
    records.push(ploke_test_utils::axum_entrypoint_record(domain_id, traits));
    records.push(ploke_test_utils::axum_entrypoint_effect_policy_record(
        domain_id,
        traits,
        &["ffi_boundary"],
    ));
    db.upsert_proof_fact_values(&records)?;
    let proof_rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !proof_rag.call_context_degraded(),
        "entrypoint summary admission should keep RAG call context enabled"
    );

    let after = proof_rag
        .exact_call_test_selection_for_target(
            traits,
            CallPathOptions {
                max_depth: 3,
                max_paths: 16,
            },
        )?
        .expect("call context enabled");
    let traits_id = traits.to_string();
    assert_eq!(after.target.id, traits);
    assert!(after.source_test_callers.is_empty(), "{after:#?}");
    assert!(after.source_test_paths.is_empty(), "{after:#?}");
    assert_eq!(
        after.generated_entrypoints.len(),
        1,
        "RAG generated test-harness proof should be selected without source call edges: {after:#?}"
    );
    let entrypoint = &after.generated_entrypoints[0];
    assert_eq!(
        entrypoint.entrypoint_summary_id,
        "entrypoint-summary:axum-error-handling-traits-test"
    );
    assert_eq!(entrypoint.build_domain_id.as_deref(), Some(domain_id));
    assert_eq!(
        entrypoint.definition_id.as_deref(),
        Some(traits_id.as_str())
    );
    assert_eq!(entrypoint.target_kind.as_deref(), Some("test"));
    assert_eq!(
        entrypoint.target_name.as_deref(),
        Some("generated-test-harness")
    );
    assert_eq!(entrypoint.status.as_deref(), Some("admitted"));
    assert_eq!(entrypoint.allowed_effects, vec!["ffi_boundary".to_string()]);
    assert_eq!(after.build_domains.len(), 1, "{after:#?}");
    assert_eq!(after.build_domains[0].build_domain_id, domain_id);
    assert!(
        after.build_domains[0].blocker_reasons.is_empty(),
        "RAG admitted generated harness build domain should be unblocked: {after:#?}"
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
    //   axum/src/routing/tests/merge.rs resolves thirteen clone calls from
    //     locals initialized by Router method results.
    let target = method_id_by_file(
        &db,
        "clone",
        "inner: Arc::clone(&self.inner)",
        "axum/src/routing/mod.rs",
    )?;

    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        26,
        "current axum fixture should resolve all Router::clone caller sites: {callers:#?}"
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
        26,
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
        assert_resolved_target(call, target, &CallTargetKind::Method, "Router::clone");
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
            Some(CallReceiverInfo::MethodResultLocalBinding { .. }) => {
                *receiver_counts
                    .entry("method_result".to_string())
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
            ("method_result".to_string(), 13),
            ("self_field:router".to_string(), 2),
            ("typed:app".to_string(), 1),
            ("typed:router".to_string(), 10),
        ]),
        "RAG call context should preserve all Router::clone receiver buckets"
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
    db.upsert_proof_fact_values(&[effect_seed(
        external_frontier.site_id,
        "effect:axum-json-parse-surface-measure",
        "surface_measure",
    )])?;
    let effects = rag
        .exact_call_effects_reachable_from_owner(
            json_owner,
            CallPathOptions {
                max_depth: 1,
                max_paths: 16,
            },
        )?
        .expect("call context enabled");
    let effect = effects
        .iter()
        .find(|effect| effect.effect_seed_id == "effect:axum-json-parse-surface-measure")
        .unwrap_or_else(|| {
            panic!("RAG should expose the serde_json parse surface-measure effect: {effects:#?}")
        });
    assert_eq!(effect.effect_class, "surface_measure");
    assert_eq!(effect.confidence.as_deref(), Some("source-oracle"));
    assert_eq!(effect.blocker_if_unresolved, Some(false));
    assert_eq!(effect.call_site.site_id, external_frontier.site_id);
    assert_eq!(effect.call_site.owner_id, json_owner);
    assert_eq!(effect.call_site.status, CallStatusKind::External);
    assert!(
        effect.paths_to_owner.is_empty(),
        "direct serde_json surface-measure effect should not need an intermediate path: {effect:#?}"
    );
    assert!(
        effect.call_site.targets.is_empty(),
        "RAG surface-measure effect must not fabricate local target rows: {effect:#?}"
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
    // Current contract: bounded `opaque_future!` generated constructors are
    // modeled as local associated-function edges, not unresolved frontiers.
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
    let direct = service_report
        .direct_call_sites
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
                "RAG reach should include IntoServiceFuture::new in direct callsite rows: {service_report:#?}"
            )
        });
    assert_eq!(direct.owner_id, service_owner);
    assert_eq!(direct.status, CallStatusKind::Resolved);
    assert_eq!(direct.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(
        direct.targets.len(),
        1,
        "resolved generated constructor row should expose one RAG target: {direct:#?}"
    );
    assert_eq!(
        direct.targets[0].relation,
        CallTargetKind::AssociatedFunction
    );
    assert!(
        service_report
            .unresolved_frontier_calls
            .iter()
            .all(|call| call.site_id != direct.site_id),
        "resolved generated constructor should not remain in unresolved frontier rows: {service_report:#?}"
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
async fn crate_boundary_edges_exact_reads_axum_body_empty_component_crossing() -> Result<(), Error>
{
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Architecture and component review:
    //   "Are lower-level crates depending on higher-level application code?"
    //   "Which crates or binaries need rebuilding after this internal function changes?"
    //
    // Source oracle:
    //   tests/fixture_github_clones/corpus/axum/axum/src/middleware/from_fn.rs:394
    //     defines `tests::basic`.
    //   tests/fixture_github_clones/corpus/axum/axum/src/middleware/from_fn.rs:411
    //     calls `Body::empty()`.
    //   tests/fixture_github_clones/corpus/axum/axum-core/src/body.rs:52
    //     defines `Body::empty`.
    // Expected contract: RAG exposes the enriched crate-boundary edge row,
    // including exact edge plus source-labeled caller, callee, crate, and
    // callsite metadata.
    let owner =
        function_id_by_name_in_module(&db, &["crate", "middleware", "from_fn", "tests"], "basic")?;
    let target = method_id_by_name_and_body_substring(&db, "empty", "Empty::new()")?;

    let rows = rag
        .exact_crate_boundary_edges_from_owner(
            owner,
            CallPathOptions {
                max_depth: 1,
                max_paths: 64,
            },
        )?
        .expect("call context enabled");
    assert!(
        rows.iter().all(|row| row.caller_crate != row.callee_crate),
        "RAG crate-boundary rows should only contain cross-crate edges: {rows:#?}"
    );
    let row = rows
        .iter()
        .find(|row| row.edge.caller_id == owner && row.edge.callee_id == target)
        .unwrap_or_else(|| {
            panic!("RAG crate-boundary rows should expose from_fn::tests::basic -> Body::empty: {rows:#?}")
        });
    assert_eq!(row.caller_crate, "axum");
    assert_eq!(row.callee_crate, "axum-core");
    assert_eq!(row.edge.source_kind, CallSiteKind::Path);
    assert_eq!(row.edge.relation, CallTargetKind::AssociatedFunction);
    assert_eq!(row.caller.id, owner);
    assert_eq!(row.caller.name, "basic");
    assert_eq!(
        row.caller.module_path,
        path(&["crate", "middleware", "from_fn"])
    );
    assert_eq!(row.callee.id, target);
    assert_eq!(row.callee.name, "empty");
    assert_eq!(row.callee.module_path, path(&["crate", "body"]));
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Path);
    assert_eq!(row.site.status, CallStatusKind::Resolved);
    assert_eq!(row.site.arg_count, Some(0));
    assert!(
        matches!(
            &row.site.callee,
            CallCalleeInfo::Path { path: call_path }
                if call_path == &path(&["Body", "empty"])
        ),
        "crate-boundary site should preserve the Body::empty path call: {row:#?}"
    );
    assert!(
        row.site.targets.iter().any(|target_row| {
            target_row.target_id == target
                && target_row.relation == CallTargetKind::AssociatedFunction
        }),
        "crate-boundary site should include the Body::empty target: {row:#?}"
    );

    Ok(())
}

#[tokio::test]
async fn crate_boundary_policy_exact_flags_axum_body_empty_component_crossing() -> Result<(), Error>
{
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Architecture review:
    //   "Which crate-boundary calls violate the intended dependency direction?"
    //
    // Source oracle:
    //   axum/src/middleware/from_fn.rs:411 in the `axum` crate calls
    //     `Body::empty()`.
    //   axum-core/src/body.rs:52 defines `Body::empty`.
    // Expected contract: RAG can answer caller-supplied dependency policy
    // questions over resolved crate-boundary edges without adding local edges
    // for targetless dependency frontiers.
    let owner =
        function_id_by_name_in_module(&db, &["crate", "middleware", "from_fn", "tests"], "basic")?;
    let target = method_id_by_name_and_body_substring(&db, "empty", "Empty::new()")?;

    let rows = rag
        .exact_crate_boundary_policy_violations_from_owner(
            owner,
            CallPathOptions {
                max_depth: 1,
                max_paths: 64,
            },
            &[CrateBoundaryPolicyRule {
                rule_id: "axum-must-not-call-axum-core".to_string(),
                caller_crate: "axum".to_string(),
                callee_crate: "axum-core".to_string(),
            }],
        )?
        .expect("call context enabled");
    assert_eq!(
        rows.len(),
        1,
        "crate policy helper should flag exactly the Body::empty crossing: {rows:#?}"
    );
    let row = &rows[0];
    assert_eq!(row.rule_id, "axum-must-not-call-axum-core");
    assert_eq!(row.edge.edge.caller_id, owner);
    assert_eq!(row.edge.edge.callee_id, target);
    assert_eq!(row.edge.caller_crate, "axum");
    assert_eq!(row.edge.callee_crate, "axum-core");
    assert!(
        matches!(
            &row.edge.site.callee,
            CallCalleeInfo::Path { path: call_path }
                if call_path == &path(&["Body", "empty"])
        ),
        "crate policy violation should preserve the source callsite path: {row:#?}"
    );

    let reverse_rows = rag
        .exact_crate_boundary_policy_violations_from_owner(
            owner,
            CallPathOptions {
                max_depth: 1,
                max_paths: 64,
            },
            &[CrateBoundaryPolicyRule {
                rule_id: "axum-core-must-not-call-axum".to_string(),
                caller_crate: "axum-core".to_string(),
                callee_crate: "axum".to_string(),
            }],
        )?
        .expect("call context enabled");
    assert!(
        reverse_rows.is_empty(),
        "reverse crate policy should not match this axum owner: {reverse_rows:#?}"
    );

    Ok(())
}

#[tokio::test]
async fn module_boundary_policy_exact_flags_axum_request_extract_boundary() -> Result<(), Error> {
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
    // Expected contract: RAG can answer caller-supplied architecture policy
    // questions over resolved module-boundary edges without rejoining the DB
    // tables manually.
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
        .exact_module_boundary_policy_violations_from_owner(
            start,
            CallPathOptions {
                max_depth: 2,
                max_paths: 16,
            },
            &[ModuleBoundaryPolicyRule {
                rule_id: "ext-traits-must-not-call-extract".to_string(),
                caller_module_prefix: path(&["crate", "ext_traits"]),
                callee_module_prefix: path(&["crate", "extract"]),
            }],
        )?
        .expect("call context enabled");
    assert_eq!(
        rows.len(),
        1,
        "policy helper should flag the transitive FromRequest boundary edge: {rows:#?}"
    );

    let row = &rows[0];
    assert_eq!(row.rule_id, "ext-traits-must-not-call-extract");
    assert_eq!(row.edge.edge.caller_id, intermediate);
    assert_eq!(row.edge.edge.callee_id, target);
    assert_eq!(
        row.edge.caller.module_path,
        path(&["crate", "ext_traits", "request"])
    );
    assert_eq!(row.edge.callee.module_path, path(&["crate", "extract"]));
    assert!(
        matches!(
            &row.edge.site.callee,
            CallCalleeInfo::Path { path: call_path }
                if call_path == &path(&["E", "from_request"])
        ),
        "policy violation should preserve the source callsite path: {row:#?}"
    );

    let reverse_rows = rag
        .exact_module_boundary_policy_violations_from_owner(
            start,
            CallPathOptions {
                max_depth: 2,
                max_paths: 16,
            },
            &[ModuleBoundaryPolicyRule {
                rule_id: "extract-must-not-call-ext-traits".to_string(),
                caller_module_prefix: path(&["crate", "extract"]),
                callee_module_prefix: path(&["crate", "ext_traits"]),
            }],
        )?
        .expect("call context enabled");
    assert!(
        reverse_rows.is_empty(),
        "reverse policy should not match this axum call chain: {reverse_rows:#?}"
    );

    Ok(())
}

#[derive(Clone, Copy)]
enum OracleOwner {
    Function {
        module: &'static [&'static str],
        name: &'static str,
    },
    Method {
        name: &'static str,
        body: &'static str,
    },
    MethodFile {
        name: &'static str,
        body: &'static str,
        file: &'static str,
    },
}

#[derive(Clone, Copy)]
enum OracleReceiver {
    SelfField {
        path: &'static [&'static str],
    },
    MethodResultField {
        method: &'static str,
        field: &'static [&'static str],
    },
}

#[derive(Clone, Copy)]
enum OracleSite {
    Path(&'static [&'static str]),
    Method {
        name: &'static str,
        receiver: OracleReceiver,
    },
}

#[derive(Clone, Copy)]
enum OracleSummary {
    RequestBuilder,
    StdReplace,
    JsonFromSlice,
    BodySizeHint,
    RouteOneshot,
}

fn oracle_owner_id(db: &Database, owner: OracleOwner) -> Result<Uuid, Error> {
    match owner {
        OracleOwner::Function { module, name } => function_id_by_name_in_module(db, module, name),
        OracleOwner::Method { name, body } => method_id_by_name_and_body_substring(db, name, body),
        OracleOwner::MethodFile { name, body, file } => method_id_by_file(db, name, body, file),
    }
}

fn db_receiver_matches(actual: Option<&CallReceiver>, expected: OracleReceiver) -> bool {
    match (actual, expected) {
        (
            Some(CallReceiver::SelfField { path: actual }),
            OracleReceiver::SelfField { path: expected },
        ) => actual
            .iter()
            .map(String::as_str)
            .eq(expected.iter().copied()),
        (
            Some(CallReceiver::MethodResultField {
                method_name,
                field_path,
                ..
            }),
            OracleReceiver::MethodResultField { method, field },
        ) => {
            method_name == method
                && field_path
                    .iter()
                    .map(String::as_str)
                    .eq(field.iter().copied())
        }
        _ => false,
    }
}

fn rag_receiver_matches(actual: Option<&CallReceiverInfo>, expected: OracleReceiver) -> bool {
    match (actual, expected) {
        (
            Some(CallReceiverInfo::SelfField { path: actual }),
            OracleReceiver::SelfField { path: expected },
        ) => actual
            .iter()
            .map(String::as_str)
            .eq(expected.iter().copied()),
        (
            Some(CallReceiverInfo::MethodResultField {
                method_name,
                field_path,
                ..
            }),
            OracleReceiver::MethodResultField { method, field },
        ) => {
            method_name == method
                && field_path
                    .iter()
                    .map(String::as_str)
                    .eq(field.iter().copied())
        }
        _ => false,
    }
}

fn oracle_site_id(
    db: &Database,
    owner: Uuid,
    site: OracleSite,
    status: DbCallStatusKind,
    label: &str,
) -> Result<Uuid, Error> {
    let context = db.call_context_for_owner(owner)?;
    let row = context
        .iter()
        .find(|row| match site {
            OracleSite::Path(expected) => row.site.path.as_ref().is_some_and(|actual| {
                actual
                    .iter()
                    .map(String::as_str)
                    .eq(expected.iter().copied())
            }),
            OracleSite::Method { name, receiver } => {
                row.site.method.as_deref() == Some(name)
                    && db_receiver_matches(row.site.receiver.as_ref(), receiver)
            }
        })
        .unwrap_or_else(|| panic!("{label} should expose its oracle callsite: {context:#?}"));
    assert_eq!(row.status.status, status, "{label} DB status: {row:#?}");
    assert!(
        row.targets.is_empty(),
        "{label} should remain targetless in DB call context: {row:#?}"
    );
    Ok(row.site.id)
}

fn rag_site_matches(actual: &CallCalleeInfo, expected: OracleSite) -> bool {
    match (actual, expected) {
        (CallCalleeInfo::Path { path: actual }, OracleSite::Path(expected)) => actual
            .iter()
            .map(String::as_str)
            .eq(expected.iter().copied()),
        (
            CallCalleeInfo::Method {
                name: actual,
                receiver: actual_receiver,
            },
            OracleSite::Method { name, receiver },
        ) => actual == name && rag_receiver_matches(actual_receiver.as_ref(), receiver),
        _ => false,
    }
}

fn assert_rag_site(
    actual: &CallContextInfo,
    owner: Uuid,
    site_id: Uuid,
    status: CallStatusKind,
    expected: OracleSite,
    label: &str,
) {
    assert_eq!(actual.site_id, site_id, "{label} RAG site ID");
    assert_eq!(actual.owner_id, owner, "{label} RAG owner ID");
    assert_eq!(actual.status, status, "{label} RAG status");
    assert!(
        rag_site_matches(&actual.callee, expected),
        "{label} should preserve its exact RAG callee payload: {actual:#?}"
    );
    assert!(
        actual.targets.is_empty(),
        "{label} must not fabricate RAG target rows: {actual:#?}"
    );
}

fn oracle_summary_records(summary: OracleSummary, site: Uuid) -> Vec<serde_json::Value> {
    match summary {
        OracleSummary::RequestBuilder => {
            ploke_test_utils::axum_request_builder_summary_records(site)
        }
        OracleSummary::StdReplace => ploke_test_utils::axum_std_mem_replace_summary_records(site),
        OracleSummary::JsonFromSlice => {
            ploke_test_utils::axum_serde_json_from_slice_summary_records(site)
        }
        OracleSummary::BodySizeHint => ploke_test_utils::axum_body_size_hint_summary_records(site),
        OracleSummary::RouteOneshot => ploke_test_utils::axum_route_oneshot_summary_records(site),
    }
}

fn oracle_summary_id(summary: OracleSummary) -> &'static str {
    match summary {
        OracleSummary::RequestBuilder => ploke_test_utils::AXUM_REQUEST_BUILDER_SUMMARY_ID,
        OracleSummary::StdReplace => ploke_test_utils::AXUM_STD_MEM_REPLACE_SUMMARY_ID,
        OracleSummary::JsonFromSlice => ploke_test_utils::AXUM_SERDE_JSON_FROM_SLICE_SUMMARY_ID,
        OracleSummary::BodySizeHint => ploke_test_utils::AXUM_BODY_SIZE_HINT_SUMMARY_ID,
        OracleSummary::RouteOneshot => ploke_test_utils::AXUM_ROUTE_ONESHOT_SUMMARY_ID,
    }
}

fn assert_db_targetless(db: &Database, owner: Uuid, site: Uuid, label: &str) -> Result<(), Error> {
    let context = db.call_context_for_owner(owner)?;
    let row = context
        .iter()
        .find(|row| row.site.id == site)
        .unwrap_or_else(|| panic!("{label} should remain visible after summary: {context:#?}"));
    assert!(
        row.targets.is_empty(),
        "{label} summary must not fabricate a local edge: {row:#?}"
    );
    Ok(())
}

struct SpawnFixture {
    db: Arc<Database>,
    rag: RagService,
    start: Uuid,
    owner: Uuid,
    site: Uuid,
}

fn setup_spawn_fixture() -> Result<SpawnFixture, Error> {
    let (db, rag) = setup_axum_call_graph_rag()?;
    let start = function_id_by_name_in_module(
        &db,
        &["crate", "form", "tests"],
        "deserialize_error_status_codes",
    )?;
    let owner = function_id_by_name_in_module(
        &db,
        &["crate", "test_helpers", "test_client"],
        "spawn_service",
    )?;
    let site = oracle_site_id(
        &db,
        owner,
        OracleSite::Path(&["tokio", "spawn"]),
        DbCallStatusKind::External,
        "spawn_service::tokio::spawn",
    )?;
    Ok(SpawnFixture {
        db,
        rag,
        start,
        owner,
        site,
    })
}

fn assert_spawn_effect(
    effect: &CallReachEffectInfo,
    fixture: &SpawnFixture,
    exact_edges: bool,
    label: &str,
) {
    assert_eq!(effect.effect_class, "async_task_spawn", "{label}");
    assert_rag_site(
        &effect.call_site,
        fixture.owner,
        fixture.site,
        CallStatusKind::External,
        OracleSite::Path(&["tokio", "spawn"]),
        label,
    );
    let path = effect
        .paths_to_owner
        .iter()
        .find(|path| {
            path.start_id == fixture.start && path.end_id == fixture.owner && path.depth == 2
        })
        .unwrap_or_else(|| {
            panic!("{label} should preserve the path to spawn_service: {effect:#?}")
        });
    if exact_edges {
        assert_eq!(path.edges.len(), 2, "{label} path edges");
        assert_eq!(path.edges[0].caller_id, fixture.start, "{label} path start");
        assert_eq!(path.edges[1].callee_id, fixture.owner, "{label} path end");
    }
}

#[derive(Clone, Copy)]
enum SpawnQuery {
    Reachable,
    Policy,
    Guard,
    StoredPolicy,
}

struct SpawnCase {
    label: &'static str,
    seed: &'static str,
    query: SpawnQuery,
}

// Source-oracle chain retained locally for every row below:
// axum/src/form.rs:262 -> axum/src/test_helpers/test_client.rs:36 ->
// axum/src/test_helpers/test_client.rs:23 (`tokio::spawn`).
#[tokio::test]
async fn call_effects_exact_axum_spawn_matrix() -> Result<(), Error> {
    init_tracing_once();
    let cases = [
        SpawnCase {
            label: "reachable task-spawn effect",
            seed: "effect:axum-rag-test-client-task-spawn",
            query: SpawnQuery::Reachable,
        },
        SpawnCase {
            label: "caller-supplied task-spawn policy",
            seed: "effect:axum-rag-test-client-task-spawn-policy",
            query: SpawnQuery::Policy,
        },
        SpawnCase {
            label: "task-spawn guard report",
            seed: "effect:axum-rag-test-client-task-spawn-guard",
            query: SpawnQuery::Guard,
        },
        SpawnCase {
            label: "stored task-spawn policy",
            seed: "effect:axum-rag-test-client-task-spawn-stored-policy",
            query: SpawnQuery::StoredPolicy,
        },
    ];
    let options = CallPathOptions {
        max_depth: 3,
        max_paths: 16,
    };

    for case in cases {
        let fixture = setup_spawn_fixture()?;
        if matches!(case.query, SpawnQuery::Reachable) {
            let mut params = BTreeMap::new();
            params.insert(
                "site_id".to_string(),
                DataValue::Uuid(UuidWrapper(fixture.site)),
            );
            let relations = fixture.db.raw_query_params(
                r#"?[target_id] :=
                    *call_relation { source_id: $site_id, target_id @ 'NOW' }"#,
                params,
            )?;
            assert!(
                relations.rows.is_empty(),
                "tokio::spawn must remain an external frontier: {relations:#?}"
            );
        }

        let mut facts = vec![spawn_effect_seed(fixture.site, case.seed)];
        if matches!(case.query, SpawnQuery::StoredPolicy) {
            facts.push(owner_effect_policy(
                fixture.start,
                "effect-policy:axum-rag-test-client:stored-policy",
                &["ffi_boundary"],
            ));
        }
        fixture.db.upsert_proof_fact_values(&facts)?;

        match case.query {
            SpawnQuery::Reachable => {
                let effects = fixture
                    .rag
                    .exact_call_effects_reachable_from_owner(fixture.start, options)?
                    .expect("call context enabled");
                let effect = effects
                    .iter()
                    .find(|effect| effect.effect_seed_id == case.seed)
                    .unwrap_or_else(|| panic!("missing {}: {effects:#?}", case.label));
                assert_eq!(effect.confidence.as_deref(), Some("source-oracle"));
                assert_eq!(effect.blocker_if_unresolved, Some(false));
                assert!(effect.blocker_reasons.is_empty(), "{effect:#?}");
                assert_spawn_effect(effect, &fixture, true, case.label);
            }
            SpawnQuery::Policy => {
                let violations = fixture
                    .rag
                    .exact_call_effect_policy_violations_for_owner(
                        fixture.start,
                        options,
                        &["ffi_boundary"],
                    )?
                    .expect("call context enabled");
                let violation = violations
                    .iter()
                    .find(|row| row.effect.effect_seed_id == case.seed)
                    .unwrap_or_else(|| panic!("missing {}: {violations:#?}", case.label));
                assert_eq!(violation.allowed_effects, vec!["ffi_boundary".to_string()]);
                assert_spawn_effect(&violation.effect, &fixture, false, case.label);
                let allowed = fixture
                    .rag
                    .exact_call_effect_policy_violations_for_owner(
                        fixture.start,
                        options,
                        &["async_task_spawn"],
                    )?
                    .expect("call context enabled");
                assert!(
                    allowed.is_empty(),
                    "allowed effect was rejected: {allowed:#?}"
                );
            }
            SpawnQuery::Guard => {
                let guard =
                    method_id_by_name_and_body_substring(&fixture.db, "new", "spawn_service(svc)")?;
                let report = fixture
                    .rag
                    .exact_call_effect_guard_report_for_owner(
                        fixture.start,
                        guard,
                        "async_task_spawn",
                        options,
                    )?
                    .expect("call context enabled");
                assert_eq!(report.owner.id, fixture.start);
                assert_eq!(report.guard.id, guard);
                assert_eq!(report.effect_class, "async_task_spawn");
                assert!(report.guarded, "{report:#?}");
                assert_eq!(report.effects.len(), 1);
                assert!(report.violations.is_empty(), "{report:#?}");
                assert_eq!(report.effects[0].effect_seed_id, case.seed);
                assert_spawn_effect(&report.effects[0], &fixture, false, case.label);

                let unrelated = method_id_by_name_and_body_substring(
                    &fixture.db,
                    "new",
                    "default_fallback: true",
                )?;
                let unguarded = fixture
                    .rag
                    .exact_call_effect_guard_report_for_owner(
                        fixture.start,
                        unrelated,
                        "async_task_spawn",
                        options,
                    )?
                    .expect("call context enabled");
                assert!(!unguarded.guarded, "{unguarded:#?}");
                assert_eq!(unguarded.effects.len(), 1);
                assert_eq!(unguarded.violations.len(), 1);
                assert_eq!(unguarded.violations[0].call_site.site_id, fixture.site);
            }
            SpawnQuery::StoredPolicy => {
                let violations = fixture
                    .rag
                    .exact_call_effect_policy_violations_for_stored_owner_policy(
                        fixture.start,
                        options,
                    )?
                    .expect("call context enabled");
                let violation = violations
                    .iter()
                    .find(|row| row.effect.effect_seed_id == case.seed)
                    .unwrap_or_else(|| panic!("missing {}: {violations:#?}", case.label));
                assert_eq!(violation.allowed_effects, vec!["ffi_boundary".to_string()]);
                assert_spawn_effect(&violation.effect, &fixture, false, case.label);
            }
        }
    }

    Ok(())
}

struct SummaryEffectCase {
    label: &'static str,
    owner: OracleOwner,
    site: OracleSite,
    summary: OracleSummary,
    db_effect: bool,
}

// Source-oracle ledger:
// - axum/src/response/sse.rs:445-449: `std::mem::replace`;
// - axum-core/src/body.rs: `Body::size_hint` -> `self.0.size_hint()`;
// - axum/src/routing/route.rs:51,57: both Route `oneshot` receiver shapes.
#[tokio::test]
async fn call_effects_exact_axum_admitted_summary_matrix() -> Result<(), Error> {
    init_tracing_once();
    let cases = [
        SummaryEffectCase {
            label: "EventDataWriter::write_buf std::mem::replace",
            owner: OracleOwner::Method {
                name: "write_buf",
                body: "std::mem::replace",
            },
            site: OracleSite::Path(&["std", "mem", "replace"]),
            summary: OracleSummary::StdReplace,
            db_effect: true,
        },
        SummaryEffectCase {
            label: "Body::size_hint self-field call",
            owner: OracleOwner::Method {
                name: "size_hint",
                body: "self.0.size_hint()",
            },
            site: OracleSite::Method {
                name: "size_hint",
                receiver: OracleReceiver::SelfField { path: &["0"] },
            },
            summary: OracleSummary::BodySizeHint,
            db_effect: false,
        },
        SummaryEffectCase {
            label: "Route::oneshot_inner method-result-field call",
            owner: OracleOwner::Method {
                name: "oneshot_inner",
                body: "self.0.clone().oneshot(req)",
            },
            site: OracleSite::Method {
                name: "oneshot",
                receiver: OracleReceiver::MethodResultField {
                    method: "clone",
                    field: &["0"],
                },
            },
            summary: OracleSummary::RouteOneshot,
            db_effect: false,
        },
        SummaryEffectCase {
            label: "Route::oneshot_inner_owned self-field call",
            owner: OracleOwner::Method {
                name: "oneshot_inner_owned",
                body: "self.0.oneshot(req)",
            },
            site: OracleSite::Method {
                name: "oneshot",
                receiver: OracleReceiver::SelfField { path: &["0"] },
            },
            summary: OracleSummary::RouteOneshot,
            db_effect: false,
        },
    ];
    let options = CallPathOptions {
        max_depth: 1,
        max_paths: 16,
    };

    for case in cases {
        let (db, rag) = setup_axum_call_graph_rag()?;
        let owner = oracle_owner_id(&db, case.owner)?;
        let site = oracle_site_id(
            &db,
            owner,
            case.site,
            DbCallStatusKind::External,
            case.label,
        )?;
        let projected =
            db.project_call_proof_facts_for_owner(owner, "bd:corpus-axum-call-graph")?;
        assert!(projected >= 2, "{} projection: {projected}", case.label);
        db.upsert_proof_fact_values(&oracle_summary_records(case.summary, site))?;

        let summary = oracle_summary_id(case.summary);
        let effect_id = format!("summary-effect:{summary}:external_summary_boundary");
        if case.db_effect {
            let effects = db.call_effects_reachable_from_owner(owner, options)?;
            assert!(
                effects
                    .iter()
                    .any(|effect| effect.effect_seed_id == effect_id),
                "DB should expose {}: {effects:#?}",
                case.label
            );
        }
        let effects = rag
            .exact_call_effects_reachable_from_owner(owner, options)?
            .expect("call context enabled");
        let effect = effects
            .iter()
            .find(|effect| effect.effect_seed_id == effect_id && effect.call_site.site_id == site)
            .unwrap_or_else(|| panic!("missing {} summary effect: {effects:#?}", case.label));
        assert_eq!(effect.effect_class, "external_summary_boundary");
        assert_eq!(effect.confidence.as_deref(), Some("source-oracle-review"));
        assert_eq!(effect.blocker_if_unresolved, Some(false));
        assert!(effect.paths_to_owner.is_empty(), "{effect:#?}");
        assert!(effect.blocker_reasons.is_empty(), "{effect:#?}");
        assert_rag_site(
            &effect.call_site,
            owner,
            site,
            CallStatusKind::External,
            case.site,
            case.label,
        );
    }

    Ok(())
}

struct ExternalNeedCase {
    label: &'static str,
    owner: OracleOwner,
    site: OracleSite,
    summary: OracleSummary,
    depth: u32,
    paths: usize,
    post_targetless: bool,
}

// Source-oracle ledger:
// - axum/src/middleware/from_fn.rs:411: `Request::builder()`;
// - axum/src/response/sse.rs:449: `std::mem::replace`;
// - axum/src/json.rs:164,184 (feature `json`): serde_json `from_slice`.
#[tokio::test]
async fn external_summary_needs_exact_axum_matrix() -> Result<(), Error> {
    init_tracing_once();
    let cases = [
        ExternalNeedCase {
            label: "from_fn::tests::basic Request::builder",
            owner: OracleOwner::Function {
                module: &["crate", "middleware", "from_fn", "tests"],
                name: "basic",
            },
            site: OracleSite::Path(&["Request", "builder"]),
            summary: OracleSummary::RequestBuilder,
            depth: 3,
            paths: 64,
            post_targetless: false,
        },
        ExternalNeedCase {
            label: "EventDataWriter::write_buf std::mem::replace",
            owner: OracleOwner::Method {
                name: "write_buf",
                body: "std::mem::replace",
            },
            site: OracleSite::Path(&["std", "mem", "replace"]),
            summary: OracleSummary::StdReplace,
            depth: 3,
            paths: 64,
            post_targetless: true,
        },
        ExternalNeedCase {
            label: "Json::from_bytes serde_json::Deserializer::from_slice",
            owner: OracleOwner::MethodFile {
                name: "from_bytes",
                body: "serde_json::Deserializer::from_slice(bytes)",
                file: "axum/src/json.rs",
            },
            site: OracleSite::Path(&["serde_json", "Deserializer", "from_slice"]),
            summary: OracleSummary::JsonFromSlice,
            depth: 1,
            paths: 16,
            post_targetless: true,
        },
    ];

    for case in cases {
        let (db, rag) = setup_axum_call_graph_rag()?;
        let owner = oracle_owner_id(&db, case.owner)?;
        let site = oracle_site_id(
            &db,
            owner,
            case.site,
            DbCallStatusKind::External,
            case.label,
        )?;
        let projected =
            db.project_call_proof_facts_for_owner(owner, "bd:corpus-axum-call-graph")?;
        assert!(projected >= 2, "{} projection: {projected}", case.label);
        let options = CallPathOptions {
            max_depth: case.depth,
            max_paths: case.paths,
        };
        let needs = rag
            .exact_external_summary_needs_for_owner(owner, options)?
            .expect("call context enabled");
        let need = needs
            .iter()
            .find(|need| need.call_site.site_id == site)
            .unwrap_or_else(|| panic!("missing {} summary need: {needs:#?}", case.label));
        assert_rag_site(
            &need.call_site,
            owner,
            site,
            CallStatusKind::External,
            case.site,
            case.label,
        );
        assert!(need.paths_to_owner.is_empty(), "{need:#?}");
        assert!(
            need.blocker_reasons
                .iter()
                .any(|reason| reason == "external_dependency_summary_missing"),
            "{need:#?}"
        );

        db.upsert_proof_fact_values(&oracle_summary_records(case.summary, site))?;
        let after = rag
            .exact_external_summary_needs_for_owner(owner, options)?
            .expect("call context enabled");
        assert!(
            after.iter().all(|need| need.call_site.site_id != site),
            "{} summary should discharge its exact need: {after:#?}",
            case.label
        );
        if case.post_targetless {
            assert_db_targetless(&db, owner, site, case.label)?;
        }
    }

    Ok(())
}

// axum/src/error_handling/mod.rs:240,251 stores a dyn Future and calls
// `self.project().future.poll(cx)`; the exact method-result field is the contract.
#[tokio::test]
async fn runtime_dispatch_needs_exact_axum_dyn_future_poll() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;
    let owner_spec = OracleOwner::MethodFile {
        name: "poll",
        body: "self.project().future.poll(cx)",
        file: "axum/src/error_handling/mod.rs",
    };
    let site_spec = OracleSite::Method {
        name: "poll",
        receiver: OracleReceiver::MethodResultField {
            method: "project",
            field: &["future"],
        },
    };
    let owner = oracle_owner_id(&db, owner_spec)?;
    let site = oracle_site_id(
        &db,
        owner,
        site_spec,
        DbCallStatusKind::Unsupported,
        "HandleErrorFuture::poll dyn Future::poll",
    )?;
    db.project_call_proof_facts_for_owner(owner, "bd:corpus-axum-call-graph")?;
    db.upsert_proof_fact_values(&[ploke_test_utils::axum_dyn_future_poll_blocker(site)])?;

    let options = CallPathOptions {
        max_depth: 1,
        max_paths: 16,
    };
    let needs = rag
        .exact_runtime_dispatch_needs_for_owner(owner, options)?
        .expect("call context enabled");
    let need = needs
        .iter()
        .find(|need| need.call_site.site_id == site)
        .unwrap_or_else(|| panic!("missing dyn Future::poll runtime need: {needs:#?}"));
    assert_rag_site(
        &need.call_site,
        owner,
        site,
        CallStatusKind::Unsupported,
        site_spec,
        "HandleErrorFuture::poll dyn Future::poll",
    );
    assert!(need.paths_to_owner.is_empty(), "{need:#?}");
    assert!(
        need.blocker_reasons
            .iter()
            .any(|reason| reason == "dynamic_dispatch_unbounded"),
        "{need:#?}"
    );

    db.upsert_proof_fact_values(&[
        ploke_test_utils::axum_dyn_future_poll_runtime_dispatch_summary(site),
    ])?;
    let after = rag
        .exact_runtime_dispatch_needs_for_owner(owner, options)?
        .expect("call context enabled");
    assert!(
        after.iter().all(|need| need.call_site.site_id != site),
        "runtime summary should discharge dyn Future::poll: {after:#?}"
    );
    assert_db_targetless(&db, owner, site, "HandleErrorFuture::poll dyn Future::poll")?;

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

#[derive(Debug)]
struct ExpectedGeneratedCallSite {
    owner: Uuid,
    site: Uuid,
}

#[derive(Debug)]
struct ExpectedSelfMethod {
    method: &'static str,
    site: Uuid,
    target: Uuid,
}

fn axum_body_from_impl_generated_callers(
    db: &Database,
    target: Uuid,
) -> Result<Vec<ExpectedGeneratedCallSite>, Error> {
    let owners = method_ids_by_name_and_body_substring(
        db,
        "from",
        "Self::new(http_body_util::Full::from(buf))",
    )?;
    assert_eq!(
        owners.len(),
        7,
        "body_from_impl! should generate exactly seven From<T> for Body::from methods"
    );

    let expected_path = path(&["Self", "new"]);
    let mut expected = Vec::new();
    for owner in owners {
        let context = db.call_context_for_owner(owner)?;
        let row = context
            .iter()
            .find(|row| {
                row.site.kind == DbCallSiteKind::Path
                    && row.site.path.as_ref() == Some(&expected_path)
                    && row.targets.iter().any(|candidate| {
                        candidate.target_id == target
                            && candidate.relation == DbCallRelationKind::AssociatedFunction
                    })
            })
            .unwrap_or_else(|| {
                panic!("generated Body::from owner should call Body::new: {context:#?}")
            });
        assert_eq!(row.status.status, DbCallStatusKind::Resolved);
        assert_eq!(
            row.status.resolution,
            Some(DbCallResolutionKind::LocalExact)
        );
        expected.push(ExpectedGeneratedCallSite {
            owner,
            site: row.site.id,
        });
    }

    Ok(expected)
}

fn missing_extension_self_methods(
    db: &Database,
    owner: Uuid,
) -> Result<Vec<ExpectedSelfMethod>, Error> {
    let context = db.call_context_for_owner(owner)?;
    ["status", "body_text"]
        .into_iter()
        .map(|method| {
            let row = context
                .iter()
                .find(|row| {
                    row.site.kind == DbCallSiteKind::Method
                        && row.site.method.as_deref() == Some(method)
                        && row.site.receiver.as_ref() == Some(&CallReceiver::SelfValue)
                })
                .unwrap_or_else(|| {
                    panic!("generated MissingExtension::into_response should call self.{method}(): {context:#?}")
                });
            assert_eq!(row.status.status, DbCallStatusKind::Resolved);
            assert_eq!(
                row.status.resolution,
                Some(DbCallResolutionKind::LocalExact)
            );
            assert_eq!(
                row.targets.len(),
                1,
                "generated self.{method}() should resolve to one target: {row:#?}"
            );
            assert_eq!(row.targets[0].relation, DbCallRelationKind::Method);
            assert_eq!(row.targets[0].target_kind, DbCallTargetKind::Method);
            Ok(ExpectedSelfMethod {
                method,
                site: row.site.id,
                target: row.targets[0].target_id,
            })
        })
        .collect()
}

fn query_rejection_delegate_method(
    db: &Database,
    owner: Uuid,
) -> Result<ExpectedSelfMethod, Error> {
    let context = db.call_context_for_owner(owner)?;
    let receiver = CallReceiver::EnumVariantBinding {
        name: "inner".to_string(),
        enum_path: path(&["Self"]),
        variant_name: "FailedToDeserializeQueryString".to_string(),
        field_index: 0,
    };
    let row = context
        .iter()
        .find(|row| {
            row.site.kind == DbCallSiteKind::Method
                && row.site.method.as_deref() == Some("into_response")
                && row.site.receiver.as_ref() == Some(&receiver)
        })
        .unwrap_or_else(|| {
            panic!(
                "generated QueryRejection::into_response should delegate inner.into_response(): {context:#?}"
            )
        });
    assert_eq!(row.status.status, DbCallStatusKind::Resolved);
    assert_eq!(
        row.status.resolution,
        Some(DbCallResolutionKind::LocalExact)
    );
    assert_eq!(
        row.targets.len(),
        1,
        "generated inner.into_response() should resolve to one target: {row:#?}"
    );
    assert_eq!(row.targets[0].relation, DbCallRelationKind::Method);
    assert_eq!(row.targets[0].target_kind, DbCallTargetKind::Method);
    Ok(ExpectedSelfMethod {
        method: "into_response",
        site: row.site.id,
        target: row.targets[0].target_id,
    })
}

fn method_ids_by_name_and_body_substring(
    db: &Database,
    name: &str,
    body_marker: &str,
) -> Result<Vec<Uuid>, Error> {
    let mut params = BTreeMap::new();
    params.insert("name".to_string(), DataValue::from(name));

    let rows = db.raw_query_params(
        r#"?[id, body] :=
            *method { id, name: $name, body @ 'NOW' }"#,
        params,
    )?;
    let normalized_marker = body_key(body_marker);
    rows.rows
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
        .map(|id| to_uuid(&id).map_err(Error::from))
        .collect()
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

fn method_id_by_name_body_and_file_suffix(
    db: &Database,
    name: &str,
    body_marker: &str,
    file_suffix: &str,
) -> Result<Uuid, Error> {
    let ids = method_ids_by_file(db, name, body_marker, file_suffix)?;
    assert_eq!(
        ids.len(),
        1,
        "expected exactly one method {name:?} with body marker {body_marker:?} in {file_suffix}: {ids:#?}"
    );
    Ok(ids[0])
}

fn method_ids_by_file(
    db: &Database,
    name: &str,
    body_marker: &str,
    file_suffix: &str,
) -> Result<Vec<Uuid>, Error> {
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
    file_owner_for_module[mod_id, file_id],
    *file_mod{{ owner_id: file_id, file_path @ 'NOW' }}
"#
    );
    let rows = db.raw_query_params(&script, params)?;
    let normalized_marker = body_key(body_marker);
    rows.rows
        .iter()
        .filter_map(|row| {
            let body = match &row[1] {
                DataValue::Str(body) => body.as_str(),
                _ => return None,
            };
            let file = match &row[2] {
                DataValue::Str(path) => path.as_str(),
                _ => return None,
            };
            (body_key(body).contains(&normalized_marker) && file.ends_with(file_suffix))
                .then(|| row[0].clone())
        })
        .map(|id| to_uuid(&id).map_err(Error::from))
        .collect()
}

fn function_id_by_name_body_and_file_suffix(
    db: &Database,
    name: &str,
    body_marker: &str,
    file_suffix: &str,
) -> Result<Uuid, Error> {
    let mut params = BTreeMap::new();
    params.insert("name".to_string(), DataValue::from(name));

    let script = format!(
        r#"
ancestor[desc, desc] := *module{{ id: desc @ 'NOW' }}
{ANCESTOR_RULES_NOW}

module_has_file[mid] := *file_mod{{ owner_id: mid @ 'NOW' }}
file_owner_for_module[mod_id, file_id] := module_has_file[mod_id], file_id = mod_id
file_owner_for_module[mod_id, file_id] := ancestor[mod_id, parent], module_has_file[parent], file_id = parent

?[id, body, file_path] :=
    *function {{ id, name: $name, body, module_id @ 'NOW' }},
    file_owner_for_module[module_id, file_id],
    *file_mod{{ owner_id: file_id, file_path @ 'NOW' }}
"#
    );
    let rows = db.raw_query_params(&script, params)?;
    let normalized_marker = body_key(body_marker);
    let matching = rows
        .rows
        .iter()
        .filter_map(|row| {
            let body = match &row[1] {
                DataValue::Str(body) => body.as_str(),
                _ => return None,
            };
            let file = match &row[2] {
                DataValue::Str(path) => path.as_str(),
                _ => return None,
            };
            (body_key(body).contains(&normalized_marker) && file.ends_with(file_suffix))
                .then(|| row[0].clone())
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "expected exactly one function {name:?} with body marker {body_marker:?} in {file_suffix}: {:#?}",
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
