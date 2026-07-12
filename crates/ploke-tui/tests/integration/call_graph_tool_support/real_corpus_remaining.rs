use super::*;

#[derive(Clone, Copy)]
pub(crate) enum AxumRemainingTarget {
    CoreTryDowncast,
    AxumTryDowncast,
    PositionFirst,
    HandleErrorNew,
    RequestExtExtract,
    RequestPartsExtExtract,
    FromRequest,
    FromRequestParts,
    FromRef,
    RouterNew,
    RouterClone,
    RoutingHelper,
    ExpandField,
    TestClientNew,
}

pub(crate) struct AxumRemainingToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) label: &'static str,
    pub(crate) item_name: &'static str,
    pub(crate) node_kind: &'static str,
    pub(crate) owner_trait: Option<&'static str>,
    pub(crate) owner_type: Option<&'static str>,
    pub(crate) file_path: PathBuf,
    pub(crate) module_path: Vec<String>,
    pub(crate) target: Uuid,
    pub(crate) callers: Vec<ExpectedRemainingCallSite>,
    pub(crate) dependency_root_sites: Vec<Uuid>,
}

#[derive(Clone)]
pub(crate) struct ExpectedRemainingCallSite {
    pub(crate) owner: Uuid,
    pub(crate) site: Uuid,
    pub(crate) callee: CallCalleeInfo,
}

impl AxumRemainingTarget {
    pub(crate) const TOOL_REACHABLE_CASES: [Self; 12] = [
        Self::CoreTryDowncast,
        Self::AxumTryDowncast,
        Self::PositionFirst,
        Self::HandleErrorNew,
        Self::RequestExtExtract,
        Self::RequestPartsExtExtract,
        Self::FromRequest,
        Self::FromRequestParts,
        Self::FromRef,
        Self::RouterNew,
        Self::RouterClone,
        Self::RoutingHelper,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::CoreTryDowncast => "axum-core try_downcast",
            Self::AxumTryDowncast => "axum try_downcast",
            Self::PositionFirst => "axum-macros Position::First",
            Self::HandleErrorNew => "axum HandleError::new",
            Self::RequestExtExtract => "axum-core RequestExt extract self-call",
            Self::RequestPartsExtExtract => "axum-core RequestPartsExt extract self-call",
            Self::FromRequest => "axum-core FromRequest::from_request",
            Self::FromRequestParts => "axum-core FromRequestParts::from_request_parts",
            Self::FromRef => "axum-core FromRef::from_ref",
            Self::RouterNew => "axum Router::new",
            Self::RouterClone => "axum Router::clone",
            Self::RoutingHelper => "axum routing take_route_or_internal_error",
            Self::ExpandField => "axum-macros closure expand_field",
            Self::TestClientNew => "axum TestClient::new",
        }
    }

    fn item_name(self) -> &'static str {
        match self {
            Self::CoreTryDowncast | Self::AxumTryDowncast => "try_downcast",
            Self::PositionFirst => "First",
            Self::HandleErrorNew | Self::RouterNew | Self::TestClientNew => "new",
            Self::RoutingHelper => "take_route_or_internal_error",
            Self::ExpandField => "expand_field",
            Self::RouterClone => "clone",
            Self::RequestExtExtract | Self::RequestPartsExtExtract => "extract_with_state",
            Self::FromRequest => "from_request",
            Self::FromRequestParts => "from_request_parts",
            Self::FromRef => "from_ref",
        }
    }

    fn owner_trait(self) -> Option<&'static str> {
        match self {
            Self::FromRequest => Some("FromRequest"),
            Self::FromRequestParts => Some("FromRequestParts"),
            Self::FromRef => Some("FromRef"),
            Self::CoreTryDowncast
            | Self::AxumTryDowncast
            | Self::PositionFirst
            | Self::HandleErrorNew
            | Self::RequestExtExtract
            | Self::RequestPartsExtExtract
            | Self::RouterNew
            | Self::RouterClone
            | Self::RoutingHelper
            | Self::ExpandField
            | Self::TestClientNew => None,
        }
    }

    fn owner_type(self) -> Option<&'static str> {
        match self {
            Self::HandleErrorNew => Some("HandleError"),
            Self::RouterNew | Self::RouterClone => Some("Router"),
            Self::TestClientNew => Some("TestClient"),
            Self::RequestExtExtract => Some("Request"),
            Self::RequestPartsExtExtract => Some("Parts"),
            Self::CoreTryDowncast
            | Self::AxumTryDowncast
            | Self::PositionFirst
            | Self::ExpandField
            | Self::RoutingHelper
            | Self::FromRequest
            | Self::FromRequestParts
            | Self::FromRef => None,
        }
    }

    fn node_kind(self) -> &'static str {
        match self {
            Self::CoreTryDowncast
            | Self::AxumTryDowncast
            | Self::ExpandField
            | Self::RoutingHelper => "function",
            Self::PositionFirst => "variant",
            Self::HandleErrorNew
            | Self::RequestExtExtract
            | Self::RequestPartsExtExtract
            | Self::FromRequest
            | Self::FromRequestParts
            | Self::FromRef
            | Self::RouterClone
            | Self::RouterNew
            | Self::TestClientNew => "method",
        }
    }

    fn expected_callers(self) -> usize {
        match self {
            Self::CoreTryDowncast => 2,
            Self::AxumTryDowncast => 1,
            Self::PositionFirst => 1,
            Self::HandleErrorNew => 2,
            Self::RequestExtExtract => 1,
            Self::RequestPartsExtExtract => 3,
            Self::FromRequest => 18,
            Self::FromRequestParts => 261,
            Self::FromRef => 4,
            Self::RouterNew => 310,
            Self::RouterClone => 13,
            Self::RoutingHelper => 2,
            Self::ExpandField => 1,
            Self::TestClientNew => 168,
        }
    }

    fn resolve(self, db: &Database) -> TargetInfo {
        match self {
            Self::CoreTryDowncast => {
                function_target_by_name_and_file(db, "try_downcast", "axum-core/src/body.rs")
            }
            Self::AxumTryDowncast => {
                function_target_by_name_and_file(db, "try_downcast", "axum/src/util.rs")
            }
            Self::PositionFirst => variant_target_by_enum_and_name(
                db,
                "Position",
                "First",
                "axum-macros/src/with_position.rs",
            ),
            Self::HandleErrorNew => {
                inherent_method_target(db, "HandleError", "new", "axum/src/error_handling/mod.rs")
            }
            Self::RequestExtExtract => method_target_by_body_and_file(
                db,
                "extract_with_state",
                "E::from_request(self, state)",
                "axum-core/src/ext_traits/request.rs",
            ),
            Self::RequestPartsExtExtract => method_target_by_body_and_file(
                db,
                "extract_with_state",
                "E::from_request_parts(self, state)",
                "axum-core/src/ext_traits/request_parts.rs",
            ),
            Self::FromRequest => trait_method_target_by_name_and_file(
                db,
                "FromRequest",
                "from_request",
                "axum-core/src/extract/mod.rs",
            ),
            Self::FromRequestParts => trait_method_target_by_name_and_file(
                db,
                "FromRequestParts",
                "from_request_parts",
                "axum-core/src/extract/mod.rs",
            ),
            Self::FromRef => trait_method_target_by_name_and_file(
                db,
                "FromRef",
                "from_ref",
                "axum-core/src/extract/from_ref.rs",
            ),
            Self::RouterNew => {
                inherent_method_target(db, "Router", "new", "axum/src/routing/mod.rs")
            }
            Self::RouterClone => method_target_by_body_and_file(
                db,
                "clone",
                "inner: Arc::clone(&self.inner)",
                "axum/src/routing/mod.rs",
            ),
            Self::RoutingHelper => function_target_by_name_and_file(
                db,
                "take_route_or_internal_error",
                "axum/src/routing/mod.rs",
            ),
            Self::ExpandField => {
                function_target_by_name_and_file(db, "expand_field", "axum-macros/src/from_ref.rs")
            }
            Self::TestClientNew => associated_path_target_by_resolved_rows(
                db,
                &["TestClient", "new"],
                168,
                "axum/src/test_helpers/test_client.rs",
            ),
        }
    }
}

impl AxumRemainingToolFixture {
    pub(crate) async fn new(case: AxumRemainingTarget) -> Self {
        let db = axum_call_graph_db();
        let target = case.resolve(&db);
        let callers = db
            .callers_for_target(target.id)
            .unwrap_or_else(|err| panic!("{} incoming callers: {err}", case.label()))
            .into_iter()
            .map(|caller| ExpectedRemainingCallSite {
                owner: caller.site.owner_id,
                site: caller.site.id,
                callee: callee_for_site(caller.site, case.label()),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            callers.len(),
            case.expected_callers(),
            "{} should expose the expected real-corpus caller count",
            case.label()
        );
        assert!(
            db.project_call_proof_facts_for_node(target.id, "bd:corpus-axum-call-graph")
                .unwrap_or_else(|err| panic!("project {} proof facts: {err}", case.label()))
                >= callers.len(),
            "{} should project target-scoped proof rows for real-corpus callers",
            case.label()
        );
        let dependency_root_sites = attach_root_proofs(&db, case, target.id, &callers);
        let state = axum_state_for_target(Arc::clone(&db), &target, case.label()).await;

        Self {
            state,
            label: case.label(),
            item_name: case.item_name(),
            node_kind: case.node_kind(),
            owner_trait: case.owner_trait(),
            owner_type: case.owner_type(),
            file_path: target.file_path,
            module_path: target.module_path,
            target: target.id,
            callers,
            dependency_root_sites,
        }
    }

    pub(crate) fn module_path_arg(&self) -> String {
        self.module_path.join("::")
    }

    pub(crate) fn ctx(&self, call_id: &'static str) -> Ctx {
        ctx_for_state(&self.state, call_id)
    }
}

pub(crate) fn assert_expected_remaining_incoming_context(
    calls: &[serde_json::Value],
    callers: &[ExpectedRemainingCallSite],
    target: Uuid,
    tool: &str,
    label: &str,
) {
    let incoming = calls
        .iter()
        .filter_map(|call| serde_json::from_value::<CallContextInfo>(call.clone()).ok())
        .filter(|call| {
            call.targets
                .iter()
                .any(|candidate| candidate.target_id == target)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        incoming.len(),
        callers.len(),
        "{tool} should return all incoming caller-site rows for {label}: {calls:#?}"
    );

    for expected in callers {
        assert!(
            incoming.iter().any(|call| {
                call.owner_id == expected.owner
                    && call.site_id == expected.site
                    && call.callee == expected.callee
            }),
            "{tool} should return incoming caller site {} for {label}: {calls:#?}",
            expected.site
        );
    }
}

fn attach_root_proofs(
    db: &Database,
    case: AxumRemainingTarget,
    target: Uuid,
    callers: &[ExpectedRemainingCallSite],
) -> Vec<Uuid> {
    let mut sites = Vec::new();
    let mut records = Vec::new();
    match case {
        AxumRemainingTarget::FromRef => {
            for caller in callers {
                let site = caller.site.to_string();
                let source = db
                    .proof_source_provenance(&site)
                    .unwrap_or_else(|err| panic!("{} source provenance: {err}", case.label()))
                    .unwrap_or_else(|| {
                        panic!("{} should have proof provenance for {site}", case.label())
                    });
                if source.source_file.ends_with("axum/src/extract/state.rs")
                    || source
                        .source_file
                        .ends_with("axum/src/middleware/from_extractor.rs")
                {
                    sites.push(caller.site);
                    records.push(axum_dependency_record(
                        "bd:corpus-axum-call-graph",
                        caller.site,
                        caller.owner,
                        target,
                    ));
                }
            }
            assert_eq!(
                sites.len(),
                2,
                "{} should identify both dependency-root caller sites",
                case.label()
            );
        }
        AxumRemainingTarget::TestClientNew => {
            for caller in callers {
                let site = caller.site.to_string();
                let source = db
                    .proof_source_provenance(&site)
                    .unwrap_or_else(|err| panic!("{} source provenance: {err}", case.label()))
                    .unwrap_or_else(|| {
                        panic!("{} should have proof provenance for {site}", case.label())
                    });
                if source
                    .source_file
                    .ends_with("axum-core/src/extract/request_parts.rs")
                {
                    sites.push(caller.site);
                    records.push(ploke_test_utils::axum_test_client_dependency_record(
                        "bd:corpus-axum-call-graph",
                        caller.site,
                        caller.owner,
                        target,
                    ));
                }
            }
            assert_eq!(
                sites.len(),
                1,
                "{} should identify the axum-core dependency-root caller site",
                case.label()
            );
        }
        AxumRemainingTarget::RouterNew => {
            for caller in callers {
                let site = caller.site.to_string();
                let source = db
                    .proof_source_provenance(&site)
                    .unwrap_or_else(|err| panic!("{} source provenance: {err}", case.label()))
                    .unwrap_or_else(|| {
                        panic!("{} should have proof provenance for {site}", case.label())
                    });
                if source
                    .source_file
                    .ends_with("axum-core/src/extract/request_parts.rs")
                {
                    sites.push(caller.site);
                    records.push(axum_router_new_dependency_record(
                        "bd:corpus-axum-call-graph",
                        caller.site,
                        caller.owner,
                        target,
                    ));
                }
            }
            assert_eq!(
                sites.len(),
                1,
                "{} should identify the axum-core dependency-root caller site",
                case.label()
            );
        }
        _ => return Vec::new(),
    }
    db.upsert_proof_fact_values(&records)
        .unwrap_or_else(|err| panic!("{} dependency-root proof insert: {err}", case.label()));
    sites
}

pub(crate) fn assert_dependency_root_proof(
    proofs: &[serde_json::Value],
    sites: &[Uuid],
    target: Uuid,
    tool: &str,
    label: &str,
) {
    if sites.is_empty() {
        return;
    }
    let expected = expected_dependency_root_target(label);
    let target = target.to_string();
    for site in sites {
        let site = site.to_string();
        assert!(
            proofs.iter().any(|proof| {
                proof.get("kind").and_then(serde_json::Value::as_str) == Some("dependency_root")
                    && proof
                        .get("call_site_id")
                        .and_then(serde_json::Value::as_str)
                        == Some(site.as_str())
                    && proof
                        .get("resolved_def_id")
                        .and_then(serde_json::Value::as_str)
                        == Some(target.as_str())
                    && proof.get("target_kind").and_then(serde_json::Value::as_str)
                        == Some(expected.target_kind)
                    && proof.get("target_name").and_then(serde_json::Value::as_str)
                        == Some(expected.target_name)
                    && proof.get("target_root").and_then(serde_json::Value::as_str)
                        == Some(expected.target_root)
                    && proof.get("status").and_then(serde_json::Value::as_str) == Some("admitted")
            }),
            "{tool} should expose dependency-root proof site {site} for {label}: {proofs:#?}"
        );
    }
}

struct ExpectedDependencyRoot<'a> {
    target_kind: &'a str,
    target_name: &'a str,
    target_root: &'a str,
}

fn expected_dependency_root_target(label: &str) -> ExpectedDependencyRoot<'_> {
    if label == AxumRemainingTarget::FromRef.label() {
        return ExpectedDependencyRoot {
            target_kind: "workspace_trait_method",
            target_name: "axum_core::extract::FromRef::from_ref",
            target_root: "axum-core/src/extract/from_ref.rs",
        };
    }
    if label == AxumRemainingTarget::TestClientNew.label() {
        return ExpectedDependencyRoot {
            target_kind: "workspace_inherent_method",
            target_name: "axum::test_helpers::TestClient::new",
            target_root: "axum/src/test_helpers/test_client.rs",
        };
    }
    if label == AxumRemainingTarget::RouterNew.label() {
        return ExpectedDependencyRoot {
            target_kind: "workspace_inherent_method",
            target_name: "axum::routing::Router::new",
            target_root: "axum/src/routing/mod.rs",
        };
    }
    panic!("{label} does not have dependency-root proof expectations")
}

fn callee_for_site(site: ploke_db::CallSiteRow, label: &str) -> CallCalleeInfo {
    match site.kind {
        ploke_db::CallSiteKind::Path => CallCalleeInfo::Path {
            path: site
                .path
                .unwrap_or_else(|| panic!("{label} path caller should carry a path")),
        },
        ploke_db::CallSiteKind::Method => CallCalleeInfo::Method {
            name: site
                .method
                .unwrap_or_else(|| panic!("{label} method caller should carry a method name")),
            receiver: supported_receiver_info(site.receiver, label),
        },
        kind => panic!("{label} remaining supported caller should be path or method, got {kind:?}"),
    }
}

fn supported_receiver_info(
    receiver: Option<ploke_db::CallReceiver>,
    label: &str,
) -> Option<CallReceiverInfo> {
    match receiver {
        Some(ploke_db::CallReceiver::SelfValue) => Some(CallReceiverInfo::SelfValue),
        Some(ploke_db::CallReceiver::TypedLocalBinding { name, type_path }) => {
            Some(CallReceiverInfo::TypedLocalBinding { name, type_path })
        }
        Some(ploke_db::CallReceiver::SelfField { path }) => {
            Some(CallReceiverInfo::SelfField { path })
        }
        Some(ploke_db::CallReceiver::LocalBinding { name }) => {
            Some(CallReceiverInfo::LocalBinding { name })
        }
        Some(ploke_db::CallReceiver::TupleMethodReturn {
            name,
            method_name,
            method_span,
            index,
        }) => Some(CallReceiverInfo::TupleMethodReturn {
            name,
            method_name,
            method_span,
            index,
        }),
        other => {
            panic!("{label} remaining supported method caller has unexpected receiver {other:?}")
        }
    }
}

fn function_target_by_name_and_file(db: &Database, name: &str, file_suffix: &str) -> TargetInfo {
    let mut params = BTreeMap::new();
    params.insert("name".to_string(), DataValue::from(name));

    let script = format!(
        r#"
ancestor[desc, desc] := *module{{ id: desc @ 'NOW' }}
{ANCESTOR_RULES_NOW}

module_has_file[mid] := *file_mod{{ owner_id: mid @ 'NOW' }}
file_owner_for_module[mod_id, file_id] := module_has_file[mod_id], file_id = mod_id
file_owner_for_module[mod_id, file_id] := ancestor[mod_id, parent], module_has_file[parent], file_id = parent

?[id, file_path, mod_path] :=
    *function {{ id, name: $name, module_id @ 'NOW' }},
    *module{{ id: module_id, path: mod_path @ 'NOW' }},
    file_owner_for_module[module_id, file_id],
    *file_mod{{ owner_id: file_id, file_path @ 'NOW' }}
"#
    );
    one_target_info(
        db.raw_query_params(&script, params)
            .unwrap_or_else(|err| panic!("query function {name}: {err}")),
        |row| data_str(&row[1], "file_path").ends_with(file_suffix),
        name,
    )
}

fn method_target_by_body_and_file(
    db: &Database,
    method_name: &str,
    body_needle: &str,
    file_suffix: &str,
) -> TargetInfo {
    let script = format!(
        r#"
ancestor[desc, desc] := *module{{ id: desc @ 'NOW' }}
{ANCESTOR_RULES_NOW}
{METHOD_NODE_ANCESTOR_RULE}

module_has_file[mid] := *file_mod{{ owner_id: mid @ 'NOW' }}
file_owner_for_module[mod_id, file_id] := module_has_file[mod_id], file_id = mod_id
file_owner_for_module[mod_id, file_id] := ancestor[mod_id, parent], module_has_file[parent], file_id = parent

?[id, body, file_path, mod_path] :=
    *method {{ id, name: $method_name, body @ 'NOW' }},
    *module{{ id: mod_id, path: mod_path @ 'NOW' }},
    ancestor[id, mod_id],
    file_owner_for_module[mod_id, file_id],
    *file_mod{{ owner_id: file_id, file_path @ 'NOW' }}
"#
    );
    let mut params = BTreeMap::new();
    params.insert("method_name".to_string(), DataValue::from(method_name));

    one_target_info(
        db.raw_query_params(&script, params).unwrap_or_else(|err| {
            panic!("query method {method_name} with body {body_needle}: {err}")
        }),
        |row| {
            let DataValue::Str(body) = &row[1] else {
                return false;
            };
            body_key(body.as_str()).contains(&body_key(body_needle))
                && data_str(&row[2], "file_path").ends_with(file_suffix)
        },
        method_name,
    )
}

fn trait_method_target_by_name_and_file(
    db: &Database,
    trait_name: &str,
    method_name: &str,
    file_suffix: &str,
) -> TargetInfo {
    let script = format!(
        r#"
ancestor[desc, desc] := *module{{ id: desc @ 'NOW' }}
{ANCESTOR_RULES_NOW}
{METHOD_NODE_ANCESTOR_RULE}

module_has_file[mid] := *file_mod{{ owner_id: mid @ 'NOW' }}
file_owner_for_module[mod_id, file_id] := module_has_file[mod_id], file_id = mod_id
file_owner_for_module[mod_id, file_id] := ancestor[mod_id, parent], module_has_file[parent], file_id = parent

?[id, file_path, mod_path] :=
    *trait {{ id: trait_id, name: $trait_name @ 'NOW' }},
    *method {{ id, name: $method_name, owner_id: trait_id @ 'NOW' }},
    ancestor[id, mod_id],
    *module{{ id: mod_id, path: mod_path @ 'NOW' }},
    file_owner_for_module[mod_id, file_id],
    *file_mod{{ owner_id: file_id, file_path @ 'NOW' }}
"#
    );
    let mut params = BTreeMap::new();
    params.insert("trait_name".to_string(), DataValue::from(trait_name));
    params.insert("method_name".to_string(), DataValue::from(method_name));

    one_target_info(
        db.raw_query_params(&script, params)
            .unwrap_or_else(|err| panic!("query trait method {trait_name}::{method_name}: {err}")),
        |row| data_str(&row[1], "file_path").ends_with(file_suffix),
        method_name,
    )
}

fn inherent_method_target(
    db: &Database,
    type_name: &str,
    method_name: &str,
    file_suffix: &str,
) -> TargetInfo {
    let script = format!(
        r#"
ancestor[desc, desc] := *module{{ id: desc @ 'NOW' }}
{ANCESTOR_RULES_NOW}
{METHOD_NODE_ANCESTOR_RULE}

module_has_file[mid] := *file_mod{{ owner_id: mid @ 'NOW' }}
file_owner_for_module[mod_id, file_id] := module_has_file[mod_id], file_id = mod_id
file_owner_for_module[mod_id, file_id] := ancestor[mod_id, parent], module_has_file[parent], file_id = parent

impl_self_target[self_target_id] := *struct {{ id: self_target_id, name: $type_name @ 'NOW' }}
impl_self_target[self_target_id] := *enum {{ id: self_target_id, name: $type_name @ 'NOW' }}
impl_self_target[self_target_id] := *union {{ id: self_target_id, name: $type_name @ 'NOW' }}
impl_self_type[self_type_id] :=
    *type_relation {{
        source_id: self_type_id,
        target_id: self_target_id,
        relation_kind: "Ordinary" @ 'NOW'
    }},
    impl_self_target[self_target_id]
impl_self_type[self_type_id] :=
    *named_type {{ type_id: self_type_id, path @ 'NOW' }},
    path == $type_path

?[id, file_path, mod_path] :=
    *method {{ id, name: $method_name, owner_id: impl_id @ 'NOW' }},
    *impl {{ id: impl_id, self_type: self_type_id @ 'NOW' }},
    impl_self_type[self_type_id],
    ancestor[id, mod_id],
    *module{{ id: mod_id, path: mod_path @ 'NOW' }},
    file_owner_for_module[mod_id, file_id],
    *file_mod{{ owner_id: file_id, file_path @ 'NOW' }}
"#
    );
    let mut params = BTreeMap::new();
    params.insert("type_name".to_string(), DataValue::from(type_name));
    params.insert(
        "type_path".to_string(),
        DataValue::List(vec![DataValue::from(type_name)]),
    );
    params.insert("method_name".to_string(), DataValue::from(method_name));

    one_target_info(
        db.raw_query_params(&script, params).unwrap_or_else(|err| {
            panic!("query inherent method {type_name}::{method_name}: {err}")
        }),
        |row| data_str(&row[1], "file_path").ends_with(file_suffix),
        method_name,
    )
}

fn variant_target_by_enum_and_name(
    db: &Database,
    enum_name: &str,
    variant_name: &str,
    file_suffix: &str,
) -> TargetInfo {
    let script = format!(
        r#"
ancestor[desc, desc] := *module{{ id: desc @ 'NOW' }}
{ANCESTOR_RULES_NOW}

module_has_file[mid] := *file_mod{{ owner_id: mid @ 'NOW' }}
file_owner_for_module[mod_id, file_id] := module_has_file[mod_id], file_id = mod_id
file_owner_for_module[mod_id, file_id] := ancestor[mod_id, parent], module_has_file[parent], file_id = parent

?[id, file_path, mod_path] :=
    *enum {{ id: enum_id, name: $enum_name @ 'NOW' }},
    *variant {{ id, name: $variant_name, owner_id: enum_id @ 'NOW' }},
    ancestor[enum_id, module_id],
    *module{{ id: module_id, path: mod_path @ 'NOW' }},
    file_owner_for_module[module_id, file_id],
    *file_mod{{ owner_id: file_id, file_path @ 'NOW' }}
"#
    );
    let mut params = BTreeMap::new();
    params.insert("enum_name".to_string(), DataValue::from(enum_name));
    params.insert("variant_name".to_string(), DataValue::from(variant_name));

    one_target_info(
        db.raw_query_params(&script, params)
            .unwrap_or_else(|err| panic!("query enum variant {enum_name}::{variant_name}: {err}")),
        |row| data_str(&row[1], "file_path").ends_with(file_suffix),
        variant_name,
    )
}

fn associated_path_target_by_resolved_rows(
    db: &Database,
    path_parts: &[&str],
    expected_count: usize,
    file_suffix: &str,
) -> TargetInfo {
    let script = format!(
        r#"
ancestor[desc, desc] := *module{{ id: desc @ 'NOW' }}
{ANCESTOR_RULES_NOW}
{METHOD_NODE_ANCESTOR_RULE}

module_has_file[mid] := *file_mod{{ owner_id: mid @ 'NOW' }}
file_owner_for_module[mod_id, file_id] := module_has_file[mod_id], file_id = mod_id
file_owner_for_module[mod_id, file_id] := ancestor[mod_id, parent], module_has_file[parent], file_id = parent

?[target_id, file_path, mod_path, count(site_id)] :=
    *call_site {{
        id: site_id,
        call_kind: "Path",
        path: $path @ 'NOW'
    }},
    *call_resolution_status {{
        source_id: site_id,
        source_kind: "Path",
        status_kind: "Resolved",
        resolution_kind: "LocalExact" @ 'NOW'
    }},
    *call_relation {{
        source_id: site_id,
        source_kind: "Path",
        relation_kind: "AssociatedFunction",
        target_id,
        target_kind: "Method" @ 'NOW'
    }},
    ancestor[target_id, mod_id],
    *module{{ id: mod_id, path: mod_path @ 'NOW' }},
    file_owner_for_module[mod_id, file_id],
    *file_mod{{ owner_id: file_id, file_path @ 'NOW' }}
"#
    );
    let mut params = BTreeMap::new();
    params.insert(
        "path".to_string(),
        DataValue::List(
            path_parts
                .iter()
                .map(|part| DataValue::from(*part))
                .collect(),
        ),
    );

    let rows = db
        .raw_query_params(&script, params)
        .unwrap_or_else(|err| panic!("query associated path {path_parts:?}: {err}"));
    let matching = rows
        .rows
        .iter()
        .filter(|row| data_str(&row[1], "file_path").ends_with(file_suffix))
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "expected exactly one associated path target for {path_parts:?}; rows: {:#?}",
        rows.rows
    );
    let row = matching[0];
    let DataValue::Num(cozo::Num::Int(count)) = &row[3] else {
        panic!(
            "associated path {path_parts:?} row count should be an integer: {:#?}",
            rows.rows
        );
    };
    assert_eq!(
        *count as usize, expected_count,
        "associated path {path_parts:?} should expose exactly {expected_count} resolved rows"
    );

    TargetInfo {
        id: to_uuid(&row[0])
            .unwrap_or_else(|err| panic!("associated path {path_parts:?} uuid: {err}")),
        file_path: PathBuf::from(data_str(&row[1], "file_path")),
        module_path: data_path(&row[2], "module path"),
    }
}

fn one_target_info(
    rows: ploke_db::QueryResult,
    keep: impl Fn(&[DataValue]) -> bool,
    label: &str,
) -> TargetInfo {
    let matching = rows.rows.iter().filter(|row| keep(row)).collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "expected exactly one target for {label}; rows: {:#?}",
        rows.rows
    );
    let row = matching[0];
    let file_path_index = row.len() - 2;
    let module_path_index = row.len() - 1;

    TargetInfo {
        id: to_uuid(&row[0]).unwrap_or_else(|err| panic!("{label} uuid: {err}")),
        file_path: PathBuf::from(data_str(&row[file_path_index], "file_path")),
        module_path: data_path(&row[module_path_index], "module path"),
    }
}
