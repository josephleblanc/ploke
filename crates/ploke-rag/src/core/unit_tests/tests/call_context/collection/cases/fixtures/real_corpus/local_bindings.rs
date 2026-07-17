use super::*;
use cozo::DataValue;
use ploke_core::rag_types::{
    CallCalleeInfo, CallReceiverInfo, CallSiteKind, CallStatusKind, CallTargetKind,
    LocalBindingRelationKind,
};
use ploke_db::{
    Database,
    multi_embedding::db_ext::{ANCESTOR_RULES_NOW, METHOD_NODE_ANCESTOR_RULE},
    to_uuid,
};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[tokio::test]
async fn local_bindings_exact_expose_axum_tap_io_constructor_frontier() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    // Source oracle:
    //   axum/src/serve/listener.rs:116-123
    //   `tap_io<F>(self, tap_fn: F) -> TapIo<Self, F>` returns
    //   `TapIo { listener: self, tap_fn }`.
    //
    // Contract: exact RAG exposes the constructor-side local-binding frontier
    // without promoting `TapIo::accept`'s `(self.tap_fn)(&mut io)` call into a
    // traversal edge.
    let owner = method_id_by_name_and_body_substring(&db, "tap_io", "TapIo")?;
    let bindings = rag
        .exact_local_bindings_for_owner(owner)?
        .expect("call context is enabled");
    let return_binding = bindings
        .iter()
        .find(|binding| {
            binding.owner_id == owner
                && binding.kind == "ReturnExpression"
                && binding.name == "return"
                && binding.source_kind == "Constructed"
                && matches!(binding.source_path.as_deref(), Some([segment]) if segment == "TapIo")
        })
        .unwrap_or_else(|| {
            panic!("RAG should expose tap_io constructed return binding: {bindings:#?}")
        });
    let parameter = bindings
        .iter()
        .find(|binding| {
            binding.owner_id == owner
                && binding.kind == "ParameterBinding"
                && binding.name == "tap_fn"
                && binding.source_kind == "Parameter"
        })
        .unwrap_or_else(|| panic!("RAG should expose tap_io tap_fn parameter: {bindings:#?}"));
    let projection = bindings
        .iter()
        .find(|binding| {
            binding.owner_id == owner
                && binding.kind == "FieldProjection"
                && binding.name == "return.tap_fn"
                && binding.source_kind == "FieldProjection"
                && binding.source_id == Some(return_binding.id)
                && matches!(binding.source_path.as_deref(), Some([segment]) if segment == "tap_fn")
                && binding.callee_kind.as_deref() == Some("Path")
                && matches!(binding.callee_path.as_deref(), Some([segment]) if segment == "tap_fn")
        })
        .unwrap_or_else(|| {
            panic!("RAG should expose tap_io return.tap_fn field projection: {bindings:#?}")
        });

    let edges = rag
        .exact_local_binding_edges_for_owner(owner)?
        .expect("call context is enabled");
    assert!(
        edges.iter().any(|edge| {
            edge.source_id == owner
                && edge.target_id == return_binding.id
                && edge.relation == LocalBindingRelationKind::OwnerContainsBinding
                && edge.target_kind == "LocalBinding"
        }),
        "RAG should expose owner-to-return containment: {edges:#?}"
    );
    assert!(
        edges.iter().any(|edge| {
            edge.source_id == owner
                && edge.target_id == parameter.id
                && edge.relation == LocalBindingRelationKind::OwnerContainsBinding
                && edge.target_kind == "LocalBinding"
        }),
        "RAG should expose owner-to-parameter containment: {edges:#?}"
    );
    assert!(
        edges.iter().any(|edge| {
            edge.source_id == projection.id
                && edge.target_id == return_binding.id
                && edge.relation == LocalBindingRelationKind::BindingProjectsField
                && edge.source_kind == "LocalBinding"
                && edge.target_kind == "LocalBinding"
        }),
        "RAG should expose return.tap_fn projection-to-return proof: {edges:#?}"
    );

    Ok(())
}

#[tokio::test]
async fn local_bindings_exact_expose_chrono_parse_internal_typed_setter_frontier()
-> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_chrono_call_graph_rag()?;

    // Source oracle:
    //   chrono/src/format/parse.rs:378 defines
    //   `type Setter = fn(&mut Parsed, i64) -> ParseResult<()>`.
    //   chrono/src/format/parse.rs:380-405 binds
    //   `(width, signed, set): (usize, bool, Setter)` from `match *spec`.
    //   chrono/src/format/parse.rs:421 calls `set(parsed, v)?`.
    //
    // Contract: exact RAG exposes the typed `set` local-binding frontier. The
    // callsite now carries finite ambiguous setter candidates, but still has no
    // resolved traversal edge because runtime `spec` selects the match arm.
    let owner = function_id_by_name_body_and_file_suffix(
        &db,
        "parse_internal",
        "set(parsed, v)?",
        "src/format/parse.rs",
    )?;
    let bindings = rag
        .exact_local_bindings_for_owner(owner)?
        .expect("call context is enabled");
    let binding = bindings
        .iter()
        .find(|binding| {
            binding.owner_id == owner
                && binding.kind == "LetBinding"
                && binding.name == "set"
                && binding.source_kind == "Typed"
                && matches!(binding.source_path.as_deref(), Some([segment]) if segment == "Setter")
                && binding.source_id.is_none()
                && binding.source_call_kind.is_none()
                && binding.callee_kind.is_none()
                && binding.callee_path.is_none()
        })
        .unwrap_or_else(|| {
            panic!("RAG should expose chrono parse_internal typed setter binding: {bindings:#?}")
        });

    let edges = rag
        .exact_local_binding_edges_for_owner(owner)?
        .expect("call context is enabled");
    assert!(
        edges.iter().any(|edge| {
            edge.source_id == owner
                && edge.target_id == binding.id
                && edge.relation == LocalBindingRelationKind::OwnerContainsBinding
                && edge.target_kind == "LocalBinding"
        }),
        "RAG should expose owner-to-typed setter binding containment: {edges:#?}"
    );

    let context = rag.exact_call_context(owner)?;
    let set_call = context
        .iter()
        .find(|call| {
            call.owner_id == owner
                && call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: path(&["set"]),
                    }
        })
        .unwrap_or_else(|| {
            panic!("RAG should expose chrono parse_internal set(parsed, v): {context:#?}")
        });
    assert_eq!(set_call.status, CallStatusKind::Ambiguous);
    assert_eq!(set_call.resolution, None);
    assert_eq!(
        set_call.targets.len(),
        21,
        "RAG should preserve every reviewed setter candidate: {set_call:#?}"
    );

    let free_targets = [
        chrono_function_id_by_name(&db, "set_weekday_with_num_days_from_sunday")?,
        chrono_function_id_by_name(&db, "set_weekday_with_number_from_monday")?,
    ];
    for target in free_targets {
        assert!(
            set_call.targets.iter().any(|candidate| {
                candidate.target_id == target && candidate.relation == CallTargetKind::Function
            }),
            "RAG should expose free setter function candidate {target}: {set_call:#?}"
        );
    }

    for method in CHRONO_PARSED_SETTER_METHODS {
        let target = chrono_parsed_method_id(&db, method)?;
        assert!(
            set_call.targets.iter().any(|candidate| {
                candidate.target_id == target
                    && candidate.relation == CallTargetKind::AssociatedFunction
            }),
            "RAG should expose Parsed::{method} associated-function candidate: {set_call:#?}"
        );
    }

    let actual = set_call
        .targets
        .iter()
        .map(|candidate| candidate.target_id)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        actual.len(),
        21,
        "RAG should not duplicate chrono setter candidates: {set_call:#?}"
    );

    Ok(())
}

const CHRONO_PARSED_SETTER_METHODS: &[&str] = &[
    "set_year",
    "set_year_div_100",
    "set_year_mod_100",
    "set_isoyear",
    "set_isoyear_div_100",
    "set_isoyear_mod_100",
    "set_quarter",
    "set_month",
    "set_day",
    "set_week_from_sun",
    "set_week_from_mon",
    "set_isoweek",
    "set_ordinal",
    "set_hour",
    "set_hour12",
    "set_minute",
    "set_second",
    "set_nanosecond",
    "set_timestamp",
];

fn chrono_function_id_by_name(db: &Database, name: &str) -> Result<Uuid, Error> {
    let mut params = BTreeMap::new();
    params.insert("name".to_string(), DataValue::from(name));
    let rows = db.raw_query_params(
        r#"?[id] :=
            *function { id, name: $name @ 'NOW' }"#,
        params,
    )?;
    assert_eq!(
        rows.rows.len(),
        1,
        "expected exactly one chrono function named {name:?}; rows: {:#?}",
        rows.rows
    );
    to_uuid(&rows.rows[0][0]).map_err(Error::from)
}

fn chrono_parsed_method_id(db: &Database, name: &str) -> Result<Uuid, Error> {
    let mut params = BTreeMap::new();
    params.insert("name".to_string(), DataValue::from(name));
    params.insert("owner_type".to_string(), DataValue::from("Parsed"));
    params.insert(
        "owner_path".to_string(),
        DataValue::List(vec![DataValue::from("Parsed")]),
    );

    let script = format!(
        r#"
ancestor[desc, desc] := *module{{ id: desc @ 'NOW' }}
{ANCESTOR_RULES_NOW}
{METHOD_NODE_ANCESTOR_RULE}

impl_self_target[self_target_id] := *struct {{ id: self_target_id, name: $owner_type @ 'NOW' }}
impl_self_type[self_type_id] :=
    *type_relation {{
        source_id: self_type_id,
        target_id: self_target_id,
        relation_kind: "Ordinary" @ 'NOW'
    }},
    impl_self_target[self_target_id]
impl_self_type[self_type_id] :=
    *named_type {{ type_id: self_type_id, path @ 'NOW' }},
    path == $owner_path

?[id] :=
    *method {{ id, name: $name, owner_id: impl_id @ 'NOW' }},
    *impl {{ id: impl_id, self_type: self_type_id @ 'NOW' }},
    impl_self_type[self_type_id]
"#
    );
    let rows = db.raw_query_params(&script, params)?;
    assert_eq!(
        rows.rows.len(),
        1,
        "expected exactly one chrono Parsed::{name} method; rows: {:#?}",
        rows.rows
    );
    to_uuid(&rows.rows[0][0]).map_err(Error::from)
}

#[tokio::test]
async fn local_bindings_exact_expose_memchr_runner_setter_assignment() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_memchr_call_graph_rag()?;

    // Source oracle:
    //   memchr/src/tests/substring/mod.rs:133-138 stores the `search`
    //   parameter into `self.fwd` through `Some(Box::new(search))`.
    //
    // Contract: exact RAG exposes the setter-side field-assignment source
    // evidence without resolving the later boxed `dyn FnMut` call in
    // `Runner::run`.
    let owner = method_id_by_name_body_and_file_suffix(
        &db,
        "fwd",
        "self.fwd = Some(Box::new(search));",
        "src/tests/substring/mod.rs",
    )?;
    let bindings = rag
        .exact_local_bindings_for_owner(owner)?
        .expect("call context is enabled");
    let parameter = bindings
        .iter()
        .find(|binding| {
            binding.owner_id == owner
                && binding.kind == "ParameterBinding"
                && binding.name == "search"
                && binding.source_kind == "Parameter"
        })
        .unwrap_or_else(|| {
            panic!("RAG should expose memchr Runner::fwd search parameter: {bindings:#?}")
        });
    let assignment = bindings
        .iter()
        .find(|binding| {
            binding.owner_id == owner
                && binding.kind == "FieldAssignment"
                && binding.name == "self.fwd"
                && binding.source_kind == "SelfFieldAssignment"
                && matches!(binding.source_path.as_deref(), Some([segment]) if segment == "fwd")
                && binding.callee_kind.as_deref() == Some("Path")
                && matches!(binding.callee_path.as_deref(), Some([segment]) if segment == "search")
        })
        .unwrap_or_else(|| {
            panic!("RAG should expose memchr Runner::fwd self-field assignment: {bindings:#?}")
        });

    let edges = rag
        .exact_local_binding_edges_for_owner(owner)?
        .expect("call context is enabled");
    assert!(
        edges.iter().any(|edge| {
            edge.source_id == owner
                && edge.target_id == assignment.id
                && edge.relation == LocalBindingRelationKind::OwnerContainsBinding
                && edge.target_kind == "LocalBinding"
        }),
        "RAG should expose owner-to-field assignment containment: {edges:#?}"
    );
    assert!(
        edges.iter().any(|edge| {
            edge.source_id == assignment.id
                && edge.target_id == parameter.id
                && edge.relation == LocalBindingRelationKind::BindingSourceParameter
                && edge.source_kind == "LocalBinding"
                && edge.target_kind == "LocalBinding"
        }),
        "RAG should expose memchr self-field assignment-to-parameter proof: {edges:#?}"
    );

    Ok(())
}

#[tokio::test]
async fn self_field_assignment_flows_exact_expose_memchr_runner_run_frontier() -> Result<(), Error>
{
    init_tracing_once();
    let (db, rag) = setup_memchr_call_graph_rag()?;

    // Source oracle:
    //   memchr/src/tests/substring/mod.rs:94 and :110 call local path
    //   bindings `fwd(...)` and `rev(...)` after reading boxed `dyn FnMut`
    //   fields.
    //   memchr/src/tests/substring/mod.rs:133-154 stores the setter
    //   parameter `search` into `self.fwd` and `self.rev`.
    //
    // Contract: exact RAG exposes source-visible setter assignment proof for
    // the targetless boxed callable field calls without promoting trait-object
    // dispatch into local traversal edges.
    let run = method_id_by_name_body_and_file_suffix(
        &db,
        "run",
        "fwd(t.haystack.as_bytes(), t.needle.as_bytes())",
        "src/tests/substring/mod.rs",
    )?;
    let flows = rag
        .exact_self_field_assignment_flows_for_owner(run)?
        .expect("call context is enabled");
    assert_eq!(
        flows.len(),
        3,
        "RAG should expose same-type setter assignment flows for Runner::run, including the extra packedpair Runner::fwd setter candidate: {flows:#?}"
    );

    for (method_name, assignment, field_name) in [
        ("fwd", "self.fwd = Some(Box::new(search));", "fwd"),
        ("rev", "self.rev = Some(Box::new(search));", "rev"),
    ] {
        let setter = method_id_by_name_body_and_file_suffix(
            &db,
            method_name,
            assignment,
            "src/tests/substring/mod.rs",
        )?;
        let flow = flows
            .iter()
            .find(|flow| {
                flow.site.owner_id == run
                    && flow.setter_id == setter
                    && flow.site.kind == CallSiteKind::Path
                    && flow
                        .site
                        .path
                        .as_deref()
                        .is_some_and(|path| path.iter().map(String::as_str).eq([field_name]))
            })
            .unwrap_or_else(|| {
                panic!("RAG should expose the {field_name} assignment flow: {flows:#?}")
            });
        assert_eq!(flow.site.status, CallStatusKind::Unsupported);
        assert!(flow.site.targets.is_empty());
        assert_eq!(flow.owner_type, "Runner");
        assert_eq!(flow.setter_id, setter);
        assert_eq!(flow.assignment_binding.owner_id, setter);
        assert_eq!(flow.assignment_binding.kind, "FieldAssignment");
        assert_eq!(flow.assignment_binding.source_kind, "SelfFieldAssignment");
        assert!(
            flow.assignment_binding
                .source_path
                .as_deref()
                .is_some_and(|path| path.iter().map(String::as_str).eq([field_name])),
            "RAG should expose the {field_name} assignment source path: {flow:#?}"
        );
        assert_eq!(flow.assignment_binding.callee_kind.as_deref(), Some("Path"));
        assert!(
            flow.assignment_binding
                .callee_path
                .as_deref()
                .is_some_and(|path| path.iter().map(String::as_str).eq(["search"])),
            "RAG should expose the {field_name} assignment parameter path: {flow:#?}"
        );
        assert_eq!(flow.parameter_binding.owner_id, setter);
        assert_eq!(flow.parameter_binding.kind, "ParameterBinding");
        assert_eq!(flow.parameter_binding.name, "search");
        assert_eq!(flow.parameter_binding.source_kind, "Parameter");
        assert_eq!(
            flow.source_edge.relation,
            LocalBindingRelationKind::BindingSourceParameter
        );
        assert_eq!(flow.source_edge.source_id, flow.assignment_binding.id);
        assert_eq!(flow.source_edge.target_id, flow.parameter_binding.id);
    }

    Ok(())
}

#[tokio::test]
async fn self_field_parameter_flows_exact_expose_axum_tap_io_accept_frontier() -> Result<(), Error>
{
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    // Source oracle:
    //   axum/src/serve/listener.rs:116-123
    //     `tap_io<F>(self, tap_fn: F) -> TapIo<Self, F>` returns
    //     `TapIo { listener: self, tap_fn }`.
    //   axum/src/serve/listener.rs:236
    //     `TapIo::accept` later calls `(self.tap_fn)(&mut io)`.
    //
    // Contract: exact RAG exposes the constructor-parameter source for the
    // targetless `self.tap_fn` dynamic call without promoting it into a local
    // traversal edge.
    let constructor = method_id_by_name_and_body_substring(&db, "tap_io", "TapIo")?;
    let accept = method_id_by_name_and_body_substring(&db, "accept", "(self.tap_fn)(&mut io)")?;

    let flows = rag
        .exact_self_field_parameter_flows_for_owner(accept)?
        .expect("call context is enabled");
    assert_eq!(
        flows.len(),
        1,
        "RAG should expose one TapIo::accept self-field parameter flow: {flows:#?}"
    );
    let flow = &flows[0];
    assert_eq!(flow.site.owner_id, accept);
    assert_eq!(flow.site.kind, CallSiteKind::Dynamic);
    assert_eq!(
        flow.site.path.as_deref(),
        Some(&["self".to_string(), "tap_fn".to_string()][..])
    );
    assert_eq!(flow.site.status, CallStatusKind::Unsupported);
    assert!(flow.site.targets.is_empty());
    assert_eq!(flow.constructor_id, constructor);
    assert_eq!(flow.return_binding.kind, "ReturnExpression");
    assert_eq!(flow.return_binding.source_kind, "Constructed");
    assert_eq!(flow.field_binding.kind, "FieldProjection");
    assert_eq!(flow.field_binding.source_id, Some(flow.return_binding.id));
    assert_eq!(
        flow.field_binding.source_path.as_deref(),
        Some(&["tap_fn".to_string()][..])
    );
    assert_eq!(flow.parameter_binding.kind, "ParameterBinding");
    assert_eq!(flow.parameter_binding.name, "tap_fn");
    assert_eq!(flow.parameter_binding.source_kind, "Parameter");

    Ok(())
}

#[tokio::test]
async fn local_bindings_exact_expose_axum_handle_error_returned_future_producer()
-> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    // Source oracle:
    //   axum/src/error_handling/mod.rs:140 creates
    //   `let future = Box::pin(async move { ... })`.
    //   axum/src/error_handling/mod.rs:147 returns
    //   `future::HandleErrorFuture { future }`.
    //   axum/src/error_handling/mod.rs:251 later calls
    //   `self.project().future.poll(cx)`.
    //
    // Contract: exact RAG exposes the producer-side returned-field binding
    // without promoting the later dyn Future::poll dispatch into a local edge.
    let owner = method_id_by_name_and_body_substring(
        &db,
        "call",
        "Err(err) => Ok(f(err).await.into_response())",
    )?;
    let context = rag.exact_call_context(owner)?;
    let box_pin = context
        .iter()
        .find(|row| {
            row.owner_id == owner
                && row.kind == CallSiteKind::Path
                && row
                    .path
                    .as_deref()
                    .is_some_and(|path| path.iter().map(String::as_str).eq(["Box", "pin"]))
        })
        .unwrap_or_else(|| {
            panic!("RAG should expose HandleError::call Box::pin site: {context:#?}")
        });

    let bindings = rag
        .exact_local_bindings_for_owner(owner)?
        .expect("call context is enabled");
    let return_binding = bindings
        .iter()
        .find(|binding| {
            binding.owner_id == owner
                && binding.kind == "ReturnExpression"
                && binding.name == "return"
                && binding.source_kind == "Constructed"
                && binding.source_path.as_deref().is_some_and(|path| {
                    path.iter()
                        .map(String::as_str)
                        .eq(["future", "HandleErrorFuture"])
                })
        })
        .unwrap_or_else(|| {
            panic!(
                "RAG should expose HandleError::call constructed returned future binding: {bindings:#?}"
            )
        });
    let future_field = bindings
        .iter()
        .find(|binding| {
            binding.owner_id == owner
                && binding.kind == "LetBinding"
                && binding.name == "return.future"
                && binding.source_kind == "PathCallResult"
                && binding.source_id == Some(box_pin.site_id)
                && binding.source_call_kind.as_deref() == Some("Path")
                && binding
                    .source_path
                    .as_deref()
                    .is_some_and(|path| path.iter().map(String::as_str).eq(["Box", "pin"]))
                && binding.callee_kind.is_none()
                && binding.callee_path.is_none()
        })
        .unwrap_or_else(|| {
            panic!(
                "RAG should expose HandleError::call return.future sourced by Box::pin: {bindings:#?}"
            )
        });

    let edges = rag
        .exact_local_binding_edges_for_owner(owner)?
        .expect("call context is enabled");
    assert!(
        edges.iter().any(|edge| {
            edge.source_id == owner
                && edge.target_id == return_binding.id
                && edge.relation == LocalBindingRelationKind::OwnerContainsBinding
                && edge.target_kind == "LocalBinding"
        }),
        "RAG should expose owner-to-return binding containment: {edges:#?}"
    );
    assert!(
        edges.iter().any(|edge| {
            edge.source_id == owner
                && edge.target_id == future_field.id
                && edge.relation == LocalBindingRelationKind::OwnerContainsBinding
                && edge.target_kind == "LocalBinding"
        }),
        "RAG should expose owner-to-return.future binding containment: {edges:#?}"
    );
    assert!(
        edges.iter().any(|edge| {
            edge.source_id == future_field.id
                && edge.target_id == box_pin.site_id
                && edge.relation == LocalBindingRelationKind::BindingSourceCallResult
                && edge.source_kind == "LocalBinding"
                && edge.target_kind == "Path"
        }),
        "RAG should expose return.future-to-Box::pin source proof: {edges:#?}"
    );

    Ok(())
}

#[tokio::test]
async fn future_poll_field_producer_flows_exact_expose_axum_handle_error_poll_frontier()
-> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    // Source oracle:
    //   axum/src/error_handling/mod.rs:140 creates
    //   `let future = Box::pin(async move { ... })`.
    //   axum/src/error_handling/mod.rs:147 returns
    //   `future::HandleErrorFuture { future }`.
    //   axum/src/error_handling/mod.rs:251 later calls
    //   `self.project().future.poll(cx)`.
    //
    // Contract: exact RAG surfaces a proof payload tying the targetless dyn
    // Future::poll frontier back to its producer-side returned field. This is
    // proof context, not a promoted traversal edge.
    let producer = method_id_by_name_and_body_substring(
        &db,
        "call",
        "Err(err) => Ok(f(err).await.into_response())",
    )?;
    let poll_owner = method_id_by_file(
        &db,
        "poll",
        "self.project().future.poll(cx)",
        "axum/src/error_handling/mod.rs",
    )?;

    let flows = rag
        .exact_future_poll_field_producer_flows_for_owner(poll_owner)?
        .expect("call context is enabled");
    assert_eq!(
        flows.len(),
        1,
        "RAG should expose one HandleErrorFuture::poll producer flow: {flows:#?}"
    );
    let flow = &flows[0];
    assert_eq!(flow.site.owner_id, poll_owner);
    assert_eq!(flow.site.kind, CallSiteKind::Method);
    assert_eq!(flow.site.status, CallStatusKind::Unsupported);
    assert!(flow.site.targets.is_empty());
    assert!(
        matches!(
            &flow.site.callee,
            CallCalleeInfo::Method {
                name,
                receiver: Some(CallReceiverInfo::MethodResultField {
                    method_name,
                    field_path,
                    ..
                }),
            } if name == "poll"
                && method_name == "project"
                && field_path.iter().map(String::as_str).eq(["future"])
        ),
        "RAG should preserve the project().future poll receiver: {flow:#?}"
    );
    assert_eq!(flow.poll_owner_type, "HandleErrorFuture");
    assert_eq!(flow.producer_id, producer);
    assert_eq!(flow.return_binding.owner_id, producer);
    assert_eq!(flow.return_binding.kind, "ReturnExpression");
    assert_eq!(flow.return_binding.source_kind, "Constructed");
    assert!(
        flow.return_binding
            .source_path
            .as_deref()
            .is_some_and(|path| path
                .iter()
                .map(String::as_str)
                .eq(["future", "HandleErrorFuture"]))
    );
    assert_eq!(flow.field_binding.owner_id, producer);
    assert_eq!(flow.field_binding.kind, "LetBinding");
    assert_eq!(flow.field_binding.name, "return.future");
    assert_eq!(flow.field_binding.source_kind, "PathCallResult");
    assert_eq!(flow.field_binding.source_id, Some(flow.source_site.site_id));
    assert_eq!(flow.source_site.owner_id, producer);
    assert_eq!(flow.source_site.kind, CallSiteKind::Path);
    assert!(
        flow.source_site
            .path
            .as_deref()
            .is_some_and(|path| path.iter().map(String::as_str).eq(["Box", "pin"]))
    );
    assert_eq!(
        flow.source_edge.relation,
        LocalBindingRelationKind::BindingSourceCallResult
    );
    assert_eq!(flow.source_edge.source_id, flow.field_binding.id);
    assert_eq!(flow.source_edge.target_id, flow.source_site.site_id);

    let context = rag.exact_call_context(poll_owner)?;
    let poll_site = context
        .iter()
        .find(|site| site.site_id == flow.site.site_id)
        .unwrap_or_else(|| panic!("RAG should expose poll site in call context: {context:#?}"));
    assert!(
        poll_site.targets.is_empty(),
        "RAG must not fabricate a dyn Future::poll target from producer proof: {poll_site:#?}"
    );

    Ok(())
}
