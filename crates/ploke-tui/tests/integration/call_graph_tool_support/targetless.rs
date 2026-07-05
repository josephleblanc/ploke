use super::*;

#[derive(Clone, Copy)]
pub(crate) struct DynamicToolCase {
    pub(crate) label: &'static str,
    pub(crate) method: &'static str,
    pub(crate) owner_type: &'static str,
    pub(crate) file_suffix: &'static str,
    pub(crate) body: &'static str,
    pub(crate) expected_arg_count: Option<u32>,
    corpus: DynamicToolCorpus,
}

pub(crate) struct DynamicToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) case: DynamicToolCase,
    pub(crate) file_path: PathBuf,
    pub(crate) module_path: Vec<String>,
    pub(crate) owner: Uuid,
}

#[derive(Clone, Copy)]
enum DynamicToolCorpus {
    Axum,
    Memchr,
}

#[derive(Clone)]
pub(crate) struct ReceiverToolCase {
    pub(crate) label: &'static str,
    pub(crate) method: &'static str,
    pub(crate) callee: &'static str,
    pub(crate) status: CallStatusKind,
    pub(crate) owner_type: &'static str,
    pub(crate) module_path: Option<&'static [&'static str]>,
    pub(crate) file_suffix: &'static str,
    pub(crate) body: &'static str,
    receiver: ReceiverShape,
}

#[derive(Clone, Copy)]
enum ReceiverShape {
    MethodResult { method: &'static str },
    SelfField { path: &'static [&'static str] },
    LocalBinding { name: &'static str },
}

pub(crate) struct ReceiverToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) case: ReceiverToolCase,
    pub(crate) file_path: PathBuf,
    pub(crate) module_path: Vec<String>,
    pub(crate) owner: Uuid,
}

#[derive(Clone)]
pub(crate) struct PathToolCase {
    pub(crate) label: &'static str,
    pub(crate) item: &'static str,
    pub(crate) path: &'static [&'static str],
    pub(crate) status: CallStatusKind,
    pub(crate) proof: PathProof,
    owner: PathOwner,
}

#[derive(Clone, Copy)]
enum PathOwner {
    Function {
        module_path: &'static [&'static str],
        file_suffix: &'static str,
        body: &'static str,
    },
}

#[derive(Clone, Copy)]
pub(crate) enum PathProof {
    IdentityMismatch,
}

pub(crate) struct PathToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) case: PathToolCase,
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
            expected_arg_count: None,
            corpus: DynamicToolCorpus::Axum,
        },
        Self {
            label: "axum/src/boxed.rs:120 MakeErasedRouter::into_route callable field",
            method: "into_route",
            owner_type: "MakeErasedRouter",
            file_suffix: "axum/src/boxed.rs",
            body: "(self.into_route)(self.router, state)",
            expected_arg_count: None,
            corpus: DynamicToolCorpus::Axum,
        },
        Self {
            label: "axum/src/boxed.rs:159 Map::into_route layer trait object",
            method: "into_route",
            owner_type: "Map",
            file_suffix: "axum/src/boxed.rs",
            body: "(self.layer)(self.inner.into_route(state))",
            expected_arg_count: None,
            corpus: DynamicToolCorpus::Axum,
        },
        Self {
            label: "axum/src/serve/listener.rs:236 TapIo::accept callable field",
            method: "accept",
            owner_type: "TapIo",
            file_suffix: "axum/src/serve/listener.rs",
            body: "(self.tap_fn)(&mut io)",
            expected_arg_count: None,
            corpus: DynamicToolCorpus::Axum,
        },
    ];

    pub(crate) const MEMCHR: [Self; 2] = [
        Self {
            label: "memchr/src/memmem/searcher.rs:222 Searcher.call",
            method: "find",
            owner_type: "Searcher",
            file_suffix: "src/memmem/searcher.rs",
            body: "(self.call)(self, prestate, haystack, needle)",
            expected_arg_count: Some(4),
            corpus: DynamicToolCorpus::Memchr,
        },
        Self {
            label: "memchr/src/memmem/searcher.rs:718 Prefilter.call",
            method: "find",
            owner_type: "Prefilter",
            file_suffix: "src/memmem/searcher.rs",
            body: "(self.call)(self, haystack)",
            expected_arg_count: Some(2),
            corpus: DynamicToolCorpus::Memchr,
        },
    ];

    pub(crate) fn build_domain(self) -> &'static str {
        self.corpus.build_domain()
    }
}

impl DynamicToolCorpus {
    fn db(self) -> Arc<Database> {
        match self {
            Self::Axum => axum_call_graph_db(),
            Self::Memchr => memchr_call_graph_db(),
        }
    }

    fn build_domain(self) -> &'static str {
        match self {
            Self::Axum => "bd:corpus-axum-call-graph",
            Self::Memchr => "bd:corpus-memchr-call-graph",
        }
    }
}

impl ReceiverToolCase {
    pub(crate) const ROUTE_ONESHOT: [Self; 2] = [
        Self {
            label: "axum/src/routing/route.rs:51 Route::oneshot_inner",
            method: "oneshot_inner",
            callee: "oneshot",
            status: CallStatusKind::Unsupported,
            owner_type: "Route",
            module_path: None,
            file_suffix: "axum/src/routing/route.rs",
            body: "self.0.clone().oneshot(req)",
            receiver: ReceiverShape::MethodResult { method: "clone" },
        },
        Self {
            label: "axum/src/routing/route.rs:57 Route::oneshot_inner_owned",
            method: "oneshot_inner_owned",
            callee: "oneshot",
            status: CallStatusKind::Unsupported,
            owner_type: "Route",
            module_path: None,
            file_suffix: "axum/src/routing/route.rs",
            body: "self.0.oneshot(req)",
            receiver: ReceiverShape::SelfField { path: &["0"] },
        },
    ];

    pub(crate) const SIZE_HINT: [Self; 1] = [Self {
        label: "axum-core/src/body.rs:127 Body::size_hint self field",
        method: "size_hint",
        callee: "size_hint",
        status: CallStatusKind::Unsupported,
        owner_type: "Body",
        module_path: None,
        file_suffix: "axum-core/src/body.rs",
        body: "self.0.size_hint()",
        receiver: ReceiverShape::SelfField { path: &["0"] },
    }];

    pub(crate) const REQUEST_PARTS: [Self; 1] = [Self {
        label: "axum-core/src/ext_traits/request_parts.rs:186 parts.extract_with_state",
        method: "from_request_parts",
        callee: "extract_with_state",
        status: CallStatusKind::Unresolved,
        owner_type: "WorksForCustomExtractor",
        module_path: Some(&["crate", "ext_traits", "request_parts", "tests"]),
        file_suffix: "axum-core/src/ext_traits/request_parts.rs",
        body: "parts.extract_with_state(state)",
        receiver: ReceiverShape::LocalBinding { name: "parts" },
    }];

    pub(crate) fn callee(&self) -> CallCalleeInfo {
        let receiver = match self.receiver {
            ReceiverShape::MethodResult { method } => Some(CallReceiverInfo::MethodCallResult {
                method_name: method.to_string(),
            }),
            ReceiverShape::SelfField { path } => Some(CallReceiverInfo::SelfField {
                path: path.iter().map(|segment| (*segment).to_string()).collect(),
            }),
            ReceiverShape::LocalBinding { name } => Some(CallReceiverInfo::LocalBinding {
                name: name.to_string(),
            }),
        };
        CallCalleeInfo::Method {
            name: self.callee.to_string(),
            receiver,
        }
    }
}

impl PathToolCase {
    pub(crate) const FROM_REF_DEP_ROOT: [Self; 1] = [Self {
        label: "axum/src/middleware/from_extractor.rs:328 Secret::from_ref dependency root",
        item: "test_from_extractor",
        path: &["Secret", "from_ref"],
        status: CallStatusKind::Unsupported,
        proof: PathProof::IdentityMismatch,
        owner: PathOwner::Function {
            module_path: &["crate", "middleware", "from_extractor", "tests"],
            file_suffix: "axum/src/middleware/from_extractor.rs",
            body: "Secret::from_ref(state)",
        },
    }];

    pub(crate) fn callee(&self) -> CallCalleeInfo {
        CallCalleeInfo::Path {
            path: self.path.iter().map(|part| (*part).to_string()).collect(),
        }
    }

    pub(crate) fn node_kind(&self) -> &'static str {
        match self.owner {
            PathOwner::Function { .. } => "function",
        }
    }

    pub(crate) fn owner_type(&self) -> Option<&'static str> {
        match self.owner {
            PathOwner::Function { .. } => None,
        }
    }
}

impl DynamicToolFixture {
    pub(crate) async fn new(case: DynamicToolCase) -> Self {
        let db = case.corpus.db();
        let owner = owner_by_body(
            &db,
            case.method,
            case.owner_type,
            None,
            case.file_suffix,
            case.body,
            case.label,
        );
        assert!(
            db.project_call_proof_facts_for_node(owner.id, case.build_domain())
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
        let owner = owner_by_body(
            &db,
            case.method,
            case.owner_type,
            case.module_path,
            case.file_suffix,
            case.body,
            case.label,
        );
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

impl PathToolFixture {
    pub(crate) async fn new(case: PathToolCase) -> Self {
        let db = axum_call_graph_db();
        let owner = match case.owner {
            PathOwner::Function {
                module_path,
                file_suffix,
                body,
            } => function_owner_by_body(&db, case.item, module_path, file_suffix, body, case.label),
        };
        assert!(
            db.project_call_proof_facts_for_node(owner.id, "bd:corpus-axum-call-graph")
                .unwrap_or_else(|err| panic!("project {} proof facts: {err}", case.label))
                >= 2,
            "{} should project targetless path proof rows",
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
    expected_arg_count: Option<u32>,
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
    if let Some(expected) = expected_arg_count {
        assert_eq!(
            call.arg_count,
            Some(expected),
            "{tool} should preserve dynamic argument count for {label}: {call:#?}"
        );
    }
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
    status: &CallStatusKind,
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
    assert_eq!(&call.status, status);
    assert_eq!(call.resolution, None);
    assert!(
        call.targets.is_empty(),
        "{tool} should not fabricate traversal targets for {label}: {call:#?}"
    );
    call.site_id
}

pub(crate) fn assert_path_context(
    calls: &[serde_json::Value],
    owner: Uuid,
    callee: &CallCalleeInfo,
    status: &CallStatusKind,
    label: &str,
    tool: &str,
) -> Uuid {
    let matching = calls
        .iter()
        .filter_map(|call| serde_json::from_value::<CallContextInfo>(call.clone()).ok())
        .filter(|call| {
            call.owner_id == owner && call.kind == CallSiteKind::Path && &call.callee == callee
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "{tool} should return exactly one path targetless row for {label}: {calls:#?}"
    );
    let call = &matching[0];
    assert_eq!(&call.status, status);
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
    build_domain: &str,
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
                && proof.build_domain_id.as_deref() == Some(build_domain)
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
    status: &CallStatusKind,
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
    let state = match status {
        CallStatusKind::Resolved => "resolved",
        CallStatusKind::Unresolved => "unresolved",
        CallStatusKind::Ambiguous => "ambiguous",
        CallStatusKind::External | CallStatusKind::Unsupported => "blocked",
    };
    assert!(
        rows.iter().any(|proof| {
            proof.kind == "call_resolution"
                && proof.call_site_id.as_deref() == Some(site_id.as_str())
                && proof.resolution_state.as_deref() == Some(state)
                && proof.blocker_reason.as_deref() == Some("type_resolution_missing")
        }),
        "{tool} should return the targetless method {state} resolution proof row for {label}: {proofs:#?}"
    );
    assert!(
        rows.iter().all(|proof| {
            proof.kind != "call_edge" || proof.call_site_id.as_deref() != Some(site_id.as_str())
        }),
        "{tool} should not fabricate a call_edge for targetless method row {label}: {proofs:#?}"
    );
}

pub(crate) fn assert_path_proof(
    proofs: &[serde_json::Value],
    _owner: Uuid,
    site_id: Uuid,
    proof: PathProof,
    label: &str,
    tool: &str,
) {
    let site_id = site_id.to_string();
    let rows = proofs
        .iter()
        .filter_map(|proof| serde_json::from_value::<ProofContextInfo>(proof.clone()).ok())
        .collect::<Vec<_>>();
    match proof {
        PathProof::IdentityMismatch => assert!(
            rows.iter().any(|proof| {
                proof.blocker_reason.as_deref() == Some("canonical_identity_mismatch")
            }),
            "{tool} should return the canonical-identity proof blocker for {label}: {proofs:#?}"
        ),
    }
    assert!(
        rows.iter().all(|proof| {
            proof.kind != "call_edge" || proof.call_site_id.as_deref() != Some(site_id.as_str())
        }),
        "{tool} should not fabricate a call_edge for targetless path row {label}: {proofs:#?}"
    );
}

fn owner_by_body(
    db: &Database,
    method: &str,
    owner_type: &str,
    module_path: Option<&[&str]>,
    file_suffix: &str,
    body_marker: &str,
    label: &str,
) -> TargetInfo {
    let mut params = BTreeMap::new();
    params.insert("name".to_string(), DataValue::from(method));
    params.insert("owner_type".to_string(), DataValue::from(owner_type));
    params.insert(
        "owner_path".to_string(),
        DataValue::List(vec![DataValue::from(owner_type)]),
    );

    let script = format!(
        r#"
ancestor[desc, desc] := *module{{ id: desc @ 'NOW' }}
{ANCESTOR_RULES_NOW}
{METHOD_NODE_ANCESTOR_RULE}

module_has_file[mid] := *file_mod{{ owner_id: mid @ 'NOW' }}
file_owner_for_module[mod_id, file_id] := module_has_file[mod_id], file_id = mod_id
file_owner_for_module[mod_id, file_id] := ancestor[mod_id, parent], module_has_file[parent], file_id = parent

impl_self_target[self_target_id] := *struct {{ id: self_target_id, name: $owner_type @ 'NOW' }}
impl_self_target[self_target_id] := *enum {{ id: self_target_id, name: $owner_type @ 'NOW' }}
impl_self_target[self_target_id] := *union {{ id: self_target_id, name: $owner_type @ 'NOW' }}
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

?[id, body, file_path, mod_path] :=
    *method {{ id, name: $name, body, owner_id: impl_id @ 'NOW' }},
    *impl {{ id: impl_id, self_type: self_type_id @ 'NOW' }},
    impl_self_type[self_type_id],
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
            let module_matches = module_path.is_none_or(|expected| {
                let actual = data_path(&row[3], "module path");
                actual
                    .iter()
                    .map(String::as_str)
                    .eq(expected.iter().copied())
            });
            body_key(body).contains(&marker)
                && module_matches
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

fn function_owner_by_body(
    db: &Database,
    name: &str,
    module_path: &[&str],
    file_suffix: &str,
    body_marker: &str,
    label: &str,
) -> TargetInfo {
    let mut params = BTreeMap::new();
    params.insert("name".to_string(), DataValue::from(name));
    params.insert(
        "module_path".to_string(),
        DataValue::List(
            module_path
                .iter()
                .map(|part| DataValue::from(*part))
                .collect(),
        ),
    );

    let script = format!(
        r#"
ancestor[desc, desc] := *module{{ id: desc @ 'NOW' }}
{ANCESTOR_RULES_NOW}
{METHOD_NODE_ANCESTOR_RULE}

module_has_file[mid] := *file_mod{{ owner_id: mid @ 'NOW' }}
file_owner_for_module[mod_id, file_id] := module_has_file[mod_id], file_id = mod_id
file_owner_for_module[mod_id, file_id] := ancestor[mod_id, parent], module_has_file[parent], file_id = parent

?[id, body, file_path, mod_path] :=
    *function {{ id, name: $name, body, module_id @ 'NOW' }},
    *module{{ id: module_id, path: $module_path @ 'NOW' }},
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
    let mut matching = rows
        .rows
        .iter()
        .filter(|row| {
            let DataValue::Str(body) = &row[1] else {
                return false;
            };
            let module_matches = data_path(&row[3], "module path")
                .iter()
                .map(String::as_str)
                .eq(module_path.iter().copied());
            body_key(body).contains(&marker)
                && data_str(&row[2], "file_path").ends_with(file_suffix)
                && module_matches
        })
        .collect::<Vec<_>>();
    matching.sort_by_key(|row| to_uuid(&row[0]).expect("function owner uuid"));
    matching.dedup_by_key(|row| to_uuid(&row[0]).expect("function owner uuid"));
    assert_eq!(
        matching.len(),
        1,
        "expected exactly one function owner for {label}; rows: {:#?}",
        rows.rows
    );
    let row = matching[0];

    TargetInfo {
        id: to_uuid(&row[0]).unwrap_or_else(|err| panic!("{label} uuid: {err}")),
        file_path: PathBuf::from(data_str(&row[2], "file_path")),
        module_path: data_path(&row[3], "module path"),
    }
}
