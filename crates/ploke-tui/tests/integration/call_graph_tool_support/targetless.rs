use super::*;

#[derive(Clone, Copy)]
pub(crate) struct DynamicToolCase {
    pub(crate) label: &'static str,
    pub(crate) method: &'static str,
    pub(crate) owner_type: &'static str,
    pub(crate) file_suffix: &'static str,
    pub(crate) body: &'static str,
}

pub(crate) struct DynamicToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) case: DynamicToolCase,
    pub(crate) file_path: PathBuf,
    pub(crate) module_path: Vec<String>,
    pub(crate) owner: Uuid,
}

#[derive(Clone, Copy)]
pub(crate) struct ReceiverToolCase {
    pub(crate) label: &'static str,
    pub(crate) method: &'static str,
    pub(crate) callee: &'static str,
    pub(crate) owner_type: &'static str,
    pub(crate) file_suffix: &'static str,
    pub(crate) body: &'static str,
    receiver: ReceiverShape,
}

#[derive(Clone, Copy)]
enum ReceiverShape {
    MethodResult { method: &'static str },
    SelfField { path: &'static [&'static str] },
}

pub(crate) struct ReceiverToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) case: ReceiverToolCase,
    pub(crate) file_path: PathBuf,
    pub(crate) module_path: Vec<String>,
    pub(crate) owner: Uuid,
}

impl DynamicToolCase {
    pub(crate) const AXUM: [Self; 4] = [
        Self {
            label: "axum/src/boxed.rs:85 MakeErasedHandler::into_route callable field",
            method: "into_route",
            owner_type: "MakeErasedHandler",
            file_suffix: "axum/src/boxed.rs",
            body: "(self.into_route)(self.handler, state)",
        },
        Self {
            label: "axum/src/boxed.rs:120 MakeErasedRouter::into_route callable field",
            method: "into_route",
            owner_type: "MakeErasedRouter",
            file_suffix: "axum/src/boxed.rs",
            body: "(self.into_route)(self.router, state)",
        },
        Self {
            label: "axum/src/boxed.rs:159 Map::into_route layer trait object",
            method: "into_route",
            owner_type: "Map",
            file_suffix: "axum/src/boxed.rs",
            body: "(self.layer)(self.inner.into_route(state))",
        },
        Self {
            label: "axum/src/serve/listener.rs:236 TapIo::accept callable field",
            method: "accept",
            owner_type: "TapIo",
            file_suffix: "axum/src/serve/listener.rs",
            body: "(self.tap_fn)(&mut io)",
        },
    ];
}

impl ReceiverToolCase {
    pub(crate) const ROUTE_ONESHOT: [Self; 2] = [
        Self {
            label: "axum/src/routing/route.rs:51 Route::oneshot_inner",
            method: "oneshot_inner",
            callee: "oneshot",
            owner_type: "Route",
            file_suffix: "axum/src/routing/route.rs",
            body: "self.0.clone().oneshot(req)",
            receiver: ReceiverShape::MethodResult { method: "clone" },
        },
        Self {
            label: "axum/src/routing/route.rs:57 Route::oneshot_inner_owned",
            method: "oneshot_inner_owned",
            callee: "oneshot",
            owner_type: "Route",
            file_suffix: "axum/src/routing/route.rs",
            body: "self.0.oneshot(req)",
            receiver: ReceiverShape::SelfField { path: &["0"] },
        },
    ];

    pub(crate) const SIZE_HINT: [Self; 1] = [Self {
        label: "axum-core/src/body.rs:127 Body::size_hint self field",
        method: "size_hint",
        callee: "size_hint",
        owner_type: "Body",
        file_suffix: "axum-core/src/body.rs",
        body: "self.0.size_hint()",
        receiver: ReceiverShape::SelfField { path: &["0"] },
    }];

    pub(crate) fn callee(self) -> CallCalleeInfo {
        let receiver = match self.receiver {
            ReceiverShape::MethodResult { method } => Some(CallReceiverInfo::MethodCallResult {
                method_name: method.to_string(),
            }),
            ReceiverShape::SelfField { path } => Some(CallReceiverInfo::SelfField {
                path: path.iter().map(|segment| (*segment).to_string()).collect(),
            }),
        };
        CallCalleeInfo::Method {
            name: self.callee.to_string(),
            receiver,
        }
    }
}

impl DynamicToolFixture {
    pub(crate) async fn new(case: DynamicToolCase) -> Self {
        let db = axum_call_graph_db();
        let owner = owner_by_body(&db, case.method, case.file_suffix, case.body, case.label);
        assert!(
            db.project_call_proof_facts_for_node(owner.id, "bd:corpus-axum-call-graph")
                .unwrap_or_else(|err| panic!("project {} proof facts: {err}", case.label))
                >= 2,
            "{} should project targetless dynamic call-site proof rows",
            case.label
        );
        let state = axum_state_for_target(Arc::clone(&db), &owner, case.label).await;

        Self {
            state,
            case,
            file_path: owner.file_path,
            module_path: owner.module_path,
            owner: owner.id,
        }
    }

    pub(crate) fn module_path_arg(&self) -> String {
        self.module_path.join("::")
    }

    pub(crate) fn ctx(&self, call_id: &'static str) -> Ctx {
        ctx_for_state(&self.state, call_id)
    }
}

impl ReceiverToolFixture {
    pub(crate) async fn new(case: ReceiverToolCase) -> Self {
        let db = axum_call_graph_db();
        let owner = owner_by_body(&db, case.method, case.file_suffix, case.body, case.label);
        assert!(
            db.project_call_proof_facts_for_node(owner.id, "bd:corpus-axum-call-graph")
                .unwrap_or_else(|err| panic!("project {} proof facts: {err}", case.label))
                >= 2,
            "{} should project targetless receiver proof rows",
            case.label
        );
        let state = axum_state_for_target(Arc::clone(&db), &owner, case.label).await;

        Self {
            state,
            case,
            file_path: owner.file_path,
            module_path: owner.module_path,
            owner: owner.id,
        }
    }

    pub(crate) fn module_path_arg(&self) -> String {
        self.module_path.join("::")
    }

    pub(crate) fn ctx(&self, call_id: &'static str) -> Ctx {
        ctx_for_state(&self.state, call_id)
    }
}

pub(crate) fn assert_dynamic_context(
    calls: &[serde_json::Value],
    owner: Uuid,
    label: &str,
    tool: &str,
) -> Uuid {
    let matching = calls
        .iter()
        .filter_map(|call| serde_json::from_value::<CallContextInfo>(call.clone()).ok())
        .filter(|call| {
            call.owner_id == owner
                && call.kind == CallSiteKind::Dynamic
                && call.callee == CallCalleeInfo::Dynamic
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "{tool} should return exactly one dynamic targetless row for {label}: {calls:#?}"
    );
    let call = &matching[0];
    assert_eq!(call.status, CallStatusKind::Unsupported);
    assert_eq!(call.resolution, None);
    assert!(
        call.targets.is_empty(),
        "{tool} should not fabricate traversal targets for {label}: {call:#?}"
    );
    call.site_id
}

pub(crate) fn assert_method_context(
    calls: &[serde_json::Value],
    owner: Uuid,
    callee: &CallCalleeInfo,
    label: &str,
    tool: &str,
) -> Uuid {
    let matching = calls
        .iter()
        .filter_map(|call| serde_json::from_value::<CallContextInfo>(call.clone()).ok())
        .filter(|call| {
            call.owner_id == owner && call.kind == CallSiteKind::Method && &call.callee == callee
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "{tool} should return exactly one method targetless row for {label}: {calls:#?}"
    );
    let call = &matching[0];
    assert_eq!(call.status, CallStatusKind::Unsupported);
    assert_eq!(call.resolution, None);
    assert!(
        call.targets.is_empty(),
        "{tool} should not fabricate traversal targets for {label}: {call:#?}"
    );
    call.site_id
}

pub(crate) fn assert_dynamic_proof(
    proofs: &[serde_json::Value],
    owner: Uuid,
    site_id: Uuid,
    label: &str,
    tool: &str,
) {
    let owner = owner.to_string();
    let site_id = site_id.to_string();
    let rows = proofs
        .iter()
        .filter_map(|proof| serde_json::from_value::<ProofContextInfo>(proof.clone()).ok())
        .collect::<Vec<_>>();
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "call_site"
                && proof.caller_def_id.as_deref() == Some(owner.as_str())
                && proof.call_site_id.as_deref() == Some(site_id.as_str())
                && proof.build_domain_id.as_deref() == Some("bd:corpus-axum-call-graph")
        }),
        "{tool} should return the dynamic call_site proof row for {label}: {proofs:#?}"
    );
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "call_resolution"
                && proof.call_site_id.as_deref() == Some(site_id.as_str())
                && proof.resolution_state.as_deref() == Some("blocked")
                && proof.blocker_reason.as_deref() == Some("dynamic_dispatch_unbounded")
        }),
        "{tool} should return the dynamic blocked resolution proof row for {label}: {proofs:#?}"
    );
}

pub(crate) fn assert_method_proof(
    proofs: &[serde_json::Value],
    owner: Uuid,
    site_id: Uuid,
    label: &str,
    tool: &str,
) {
    let owner = owner.to_string();
    let site_id = site_id.to_string();
    let rows = proofs
        .iter()
        .filter_map(|proof| serde_json::from_value::<ProofContextInfo>(proof.clone()).ok())
        .collect::<Vec<_>>();
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "call_site"
                && proof.caller_def_id.as_deref() == Some(owner.as_str())
                && proof.call_site_id.as_deref() == Some(site_id.as_str())
                && proof.build_domain_id.as_deref() == Some("bd:corpus-axum-call-graph")
        }),
        "{tool} should return the targetless method call_site proof row for {label}: {proofs:#?}"
    );
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "call_resolution"
                && proof.call_site_id.as_deref() == Some(site_id.as_str())
                && proof.resolution_state.as_deref() == Some("blocked")
                && proof.blocker_reason.as_deref() == Some("type_resolution_missing")
        }),
        "{tool} should return the targetless method blocked resolution proof row for {label}: {proofs:#?}"
    );
}

fn owner_by_body(
    db: &Database,
    method: &str,
    file_suffix: &str,
    body_marker: &str,
    label: &str,
) -> TargetInfo {
    let mut params = BTreeMap::new();
    params.insert("name".to_string(), DataValue::from(method));

    let script = format!(
        r#"
ancestor[desc, desc] := *module{{ id: desc @ 'NOW' }}
{ANCESTOR_RULES_NOW}
{METHOD_NODE_ANCESTOR_RULE}

module_has_file[mid] := *file_mod{{ owner_id: mid @ 'NOW' }}
file_owner_for_module[mod_id, file_id] := module_has_file[mod_id], file_id = mod_id
file_owner_for_module[mod_id, file_id] := ancestor[mod_id, parent], module_has_file[parent], file_id = parent

?[id, body, file_path, mod_path] :=
    *method {{ id, name: $name, body @ 'NOW' }},
    ancestor[id, mod_id],
    *module{{ id: mod_id, path: mod_path @ 'NOW' }},
    file_owner_for_module[mod_id, file_id],
    *file_mod{{ owner_id: file_id, file_path @ 'NOW' }}
"#
    );
    let rows = db
        .raw_query_params(&script, params)
        .unwrap_or_else(|err| panic!("query {label} owner: {err}"));
    let marker = body_key(body_marker);
    let matching = rows
        .rows
        .iter()
        .filter(|row| {
            let DataValue::Str(body) = &row[1] else {
                return false;
            };
            body_key(body).contains(&marker)
                && data_str(&row[2], "file_path").ends_with(file_suffix)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "expected exactly one owner for {label}; rows: {:#?}",
        rows.rows
    );
    let row = matching[0];

    TargetInfo {
        id: to_uuid(&row[0]).unwrap_or_else(|err| panic!("{label} uuid: {err}")),
        file_path: PathBuf::from(data_str(&row[2], "file_path")),
        module_path: data_path(&row[3], "module path"),
    }
}
