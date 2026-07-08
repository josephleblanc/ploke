use std::borrow::Cow;

use ploke_core::rag_types::{CallResolutionKind, CallTargetKind as RagCallTargetKind};
use ploke_db::{CallRelationKind, CallSiteKind as DbCallSiteKind};
use ploke_test_utils::{
    CallCorpusFixture, CallExpected, CallOwnerSelector, CallShapeCase, CallSiteSelector,
    CallTargetSelector,
};
use ploke_tui::tools::{code_item_lookup::LookupParams, get_code_edges::EdgesParams};

use super::*;

pub(crate) struct SharedCallShapeToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) case: &'static CallShapeCase,
    pub(crate) owner: Uuid,
    pub(crate) site: Uuid,
    pub(crate) target: Option<Uuid>,
    query: MatrixQuery,
}

struct MatrixQuery {
    file_path: PathBuf,
    module_path: Vec<String>,
    item_name: &'static str,
    node_kind: &'static str,
    owner_trait: Option<&'static str>,
    owner_type: Option<&'static str>,
    direction: QueryDirection,
}

#[derive(Clone, Copy)]
enum QueryDirection {
    Incoming,
    Outgoing,
}

impl SharedCallShapeToolFixture {
    pub(crate) async fn new(case: &'static CallShapeCase) -> Self {
        let db = Arc::new(
            fresh_backup_fixture_db(case.fixture.fixture())
                .unwrap_or_else(|err| panic!("{} fixture DB: {err}", case.name)),
        );
        assert!(
            db.has_call_graph_relations()
                .unwrap_or_else(|err| panic!("{} call graph relation check: {err}", case.name)),
            "{} must expose call graph relations",
            case.fixture.fixture().id
        );

        let owner = resolve_owner(db.as_ref(), case);
        let target = match case.expected {
            CallExpected::Resolved { target, .. } => Some(resolve_target(db.as_ref(), target)),
            CallExpected::Targetless { .. } => None,
        };
        let context = db
            .call_context_for_owner(owner.id)
            .unwrap_or_else(|err| panic!("{} owner call context: {err}", case.name));
        let row = select_site(&context, case);
        assert_db_expectation(row, case, target.as_ref().map(|node| node.id));

        let query = match target.as_ref() {
            Some(target) => query_for_target(target, case.expected),
            None => query_for_owner(&owner, case.owner),
        };
        let build_domain = build_domain(case.fixture);
        assert!(
            db.project_call_proof_facts_for_node(
                query_node_id(&query, &owner, target.as_ref()),
                build_domain
            )
            .unwrap_or_else(|err| panic!("{} project proof facts: {err}", case.name))
                >= 2,
            "{} should project node-scoped call proof rows",
            case.name
        );
        let state = app_state_with_rag(db, crate_root_from_file(&query.file_path, case.name)).await;

        Self {
            state,
            case,
            owner: owner.id,
            site: row.site.id,
            target: target.map(|node| node.id),
            query,
        }
    }

    pub(crate) fn lookup_params(&self) -> LookupParams<'_> {
        LookupParams {
            item_name: Cow::Borrowed(self.query.item_name),
            file_path: Cow::Owned(self.query.file_path.display().to_string()),
            node_kind: Cow::Borrowed(self.query.node_kind),
            module_path: Cow::Owned(self.module_path_arg()),
            owner_trait: self.query.owner_trait.map(Cow::Borrowed),
            owner_type: self.query.owner_type.map(Cow::Borrowed),
            parent_name: None,
        }
    }

    pub(crate) fn edges_params(&self) -> EdgesParams<'_> {
        EdgesParams {
            item_name: Cow::Borrowed(self.query.item_name),
            file_path: Cow::Owned(self.query.file_path.display().to_string()),
            node_kind: Cow::Borrowed(self.query.node_kind),
            module_path: Cow::Owned(self.module_path_arg()),
            owner_trait: self.query.owner_trait.map(Cow::Borrowed),
            owner_type: self.query.owner_type.map(Cow::Borrowed),
            parent_name: None,
        }
    }

    pub(crate) fn ctx(&self, call_id: &'static str) -> Ctx {
        ctx_for_state(&self.state, call_id)
    }

    pub(crate) fn assert_call_context(&self, calls: &[serde_json::Value], tool: &str) {
        let matching = calls
            .iter()
            .filter_map(|call| serde_json::from_value::<CallContextInfo>(call.clone()).ok())
            .filter(|call| call.owner_id == self.owner && call.site_id == self.site)
            .collect::<Vec<_>>();
        assert_eq!(
            matching.len(),
            1,
            "{tool} should return exactly one selected shared-matrix row for {}: {calls:#?}",
            self.case.name
        );
        let call = &matching[0];
        assert_eq!(call.kind, rag_site_kind(self.case.site));
        assert_eq!(call.callee, rag_callee(self.case.site));

        match self.case.expected {
            CallExpected::Resolved {
                relation,
                edge_count,
                ..
            } => {
                let target = self
                    .target
                    .unwrap_or_else(|| panic!("{} expected target", self.case.name));
                assert_eq!(call.status, CallStatusKind::Resolved);
                assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
                assert_eq!(
                    call.targets.len(),
                    edge_count,
                    "{tool} should preserve expected target edge count for {}: {call:#?}",
                    self.case.name
                );
                assert!(
                    call.targets.iter().any(|candidate| {
                        candidate.target_id == target
                            && candidate.relation == rag_relation_kind(relation)
                    }),
                    "{tool} should expose the expected shared-matrix target for {}: {call:#?}",
                    self.case.name
                );
            }
            CallExpected::Targetless { status } => {
                assert_eq!(call.status, rag_status_kind(status));
                assert_eq!(call.resolution, None);
                assert!(
                    call.targets.is_empty(),
                    "{tool} should keep {} targetless: {call:#?}",
                    self.case.name
                );
            }
        }
    }

    pub(crate) fn assert_proof_context(&self, proofs: &[serde_json::Value], tool: &str) {
        let rows = proofs
            .iter()
            .filter_map(|proof| serde_json::from_value::<ProofContextInfo>(proof.clone()).ok())
            .collect::<Vec<_>>();
        let owner = self.owner.to_string();
        let site = self.site.to_string();
        let build_domain = build_domain(self.case.fixture);
        assert!(
            rows.iter().any(|proof| {
                proof.kind == "call_site"
                    && proof.caller_def_id.as_deref() == Some(owner.as_str())
                    && proof.call_site_id.as_deref() == Some(site.as_str())
                    && proof.build_domain_id.as_deref() == Some(build_domain)
            }),
            "{tool} should return the shared-matrix call_site proof row for {}: {proofs:#?}",
            self.case.name
        );

        match self.case.expected {
            CallExpected::Resolved { .. } => {
                let target = self
                    .target
                    .unwrap_or_else(|| panic!("{} expected target", self.case.name))
                    .to_string();
                assert!(
                    rows.iter().any(|proof| {
                        proof.kind == "call_resolution"
                            && proof.call_site_id.as_deref() == Some(site.as_str())
                            && proof.resolution_state.as_deref() == Some("resolved")
                            && proof.resolved_def_id.as_deref() == Some(target.as_str())
                    }),
                    "{tool} should return the resolved call_resolution proof row for {}: {proofs:#?}",
                    self.case.name
                );
                assert!(
                    rows.iter().any(|proof| {
                        proof.kind == "call_edge"
                            && proof.call_site_id.as_deref() == Some(site.as_str())
                            && proof.caller_def_id.as_deref() == Some(owner.as_str())
                            && proof.callee_def_id.as_deref() == Some(target.as_str())
                    }),
                    "{tool} should return the resolved call_edge proof row for {}: {proofs:#?}",
                    self.case.name
                );
            }
            CallExpected::Targetless { status } => {
                let (state, blocker) = targetless_proof_state(status, self.case.site);
                assert!(
                    rows.iter().any(|proof| {
                        proof.kind == "call_resolution"
                            && proof.call_site_id.as_deref() == Some(site.as_str())
                            && proof.resolution_state.as_deref() == Some(state)
                            && proof.blocker_reason.as_deref() == Some(blocker)
                    }),
                    "{tool} should return the targetless call_resolution proof row for {}: {proofs:#?}",
                    self.case.name
                );
                assert!(
                    rows.iter().all(|proof| {
                        proof.kind != "call_edge"
                            || proof.call_site_id.as_deref() != Some(site.as_str())
                    }),
                    "{tool} should not fabricate a call_edge proof row for {}: {proofs:#?}",
                    self.case.name
                );
            }
        }
    }

    pub(crate) fn assert_ui_counts(&self, ui: &ToolUiPayload, proof_count: usize) {
        match self.query.direction {
            QueryDirection::Incoming => assert!(
                ui_field(ui, "call_context_incoming")
                    .parse::<usize>()
                    .expect("incoming count")
                    >= 1,
                "shared matrix target query should expose incoming call context for {}",
                self.case.name
            ),
            QueryDirection::Outgoing => assert!(
                ui_field(ui, "call_context_outgoing")
                    .parse::<usize>()
                    .expect("outgoing count")
                    >= 1,
                "shared matrix owner query should expose outgoing call context for {}",
                self.case.name
            ),
        }
        assert_eq!(
            ui_field(ui, "proof_context"),
            proof_count.to_string().as_str()
        );
    }

    fn module_path_arg(&self) -> String {
        self.query.module_path.join("::")
    }
}

fn query_node_id(query: &MatrixQuery, owner: &TargetInfo, target: Option<&TargetInfo>) -> Uuid {
    match query.direction {
        QueryDirection::Incoming => target.expect("incoming query target").id,
        QueryDirection::Outgoing => owner.id,
    }
}

fn resolve_owner(db: &Database, case: &CallShapeCase) -> TargetInfo {
    match case.owner {
        CallOwnerSelector::FunctionInModule { module_path, name } => {
            function_by_name_in_module(db, module_path, name, case.name)
        }
        CallOwnerSelector::MethodByBody { name, body, .. } => {
            method_by_name_and_body(db, name, body, None, case.name)
        }
        CallOwnerSelector::MethodByBodyFile {
            name,
            body,
            file_suffix,
        } => method_by_name_and_body(db, name, body, Some(file_suffix), case.name),
    }
}

fn resolve_target(db: &Database, target: CallTargetSelector) -> TargetInfo {
    match target {
        CallTargetSelector::FunctionInModule { module_path, name } => {
            function_by_name_in_module(db, module_path, name, name)
        }
        CallTargetSelector::Variant {
            enum_name,
            variant_name,
        } => variant_by_enum_and_name(db, enum_name, variant_name),
    }
}

fn select_site<'a>(
    context: &'a [ploke_db::CallContextRow],
    case: &CallShapeCase,
) -> &'a ploke_db::CallContextRow {
    let matches = context
        .iter()
        .filter(|row| db_site_matches(row, case.site))
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "{} should expose exactly one selected shared-matrix call-site row; source: {}; context: {context:#?}",
        case.name,
        case.source
    );
    matches[0]
}

fn db_site_matches(row: &ploke_db::CallContextRow, site: CallSiteSelector) -> bool {
    match site {
        CallSiteSelector::Path {
            segments,
            arg_count,
        } => {
            row.site.kind == DbCallSiteKind::Path
                && row.site.arg_count == arg_count
                && row.site.path.as_ref().is_some_and(|path| {
                    path.iter().map(String::as_str).eq(segments.iter().copied())
                })
        }
        CallSiteSelector::Dynamic { arg_count } => {
            row.site.kind == DbCallSiteKind::Dynamic && row.site.arg_count == arg_count
        }
    }
}

fn assert_db_expectation(
    row: &ploke_db::CallContextRow,
    case: &CallShapeCase,
    target: Option<Uuid>,
) {
    match case.expected {
        CallExpected::Resolved {
            relation,
            target_kind,
            edge_count,
            ..
        } => {
            let target = target.unwrap_or_else(|| panic!("{} expected target", case.name));
            assert_eq!(
                row.status.status,
                DbCallStatusKind::Resolved,
                "{} should be resolved in DB context",
                case.name
            );
            assert_eq!(
                row.targets.len(),
                edge_count,
                "{} should preserve expected DB edge count",
                case.name
            );
            assert!(
                row.targets.iter().any(|candidate| {
                    candidate.target_id == target
                        && candidate.relation == relation
                        && candidate.target_kind == target_kind
                }),
                "{} should preserve expected DB target edge: {row:#?}",
                case.name
            );
        }
        CallExpected::Targetless { status } => {
            assert_eq!(row.status.status, status);
            assert!(
                row.targets.is_empty(),
                "{} should remain targetless in DB context: {row:#?}",
                case.name
            );
        }
    }
}

fn query_for_target(target: &TargetInfo, expected: CallExpected) -> MatrixQuery {
    let (item_name, node_kind) = match expected {
        CallExpected::Resolved { target, .. } => match target {
            CallTargetSelector::FunctionInModule { name, .. } => (name, "function"),
            CallTargetSelector::Variant { variant_name, .. } => (variant_name, "variant"),
        },
        CallExpected::Targetless { .. } => unreachable!("target query requires resolved case"),
    };
    MatrixQuery {
        file_path: target.file_path.clone(),
        module_path: target.module_path.clone(),
        item_name,
        node_kind,
        owner_trait: None,
        owner_type: None,
        direction: QueryDirection::Incoming,
    }
}

fn query_for_owner(owner: &TargetInfo, selector: CallOwnerSelector) -> MatrixQuery {
    match selector {
        CallOwnerSelector::FunctionInModule { name, .. } => MatrixQuery {
            file_path: owner.file_path.clone(),
            module_path: owner.module_path.clone(),
            item_name: name,
            node_kind: "function",
            owner_trait: None,
            owner_type: None,
            direction: QueryDirection::Outgoing,
        },
        CallOwnerSelector::MethodByBody {
            name,
            owner_type,
            owner_trait,
            ..
        } => MatrixQuery {
            file_path: owner.file_path.clone(),
            module_path: owner.module_path.clone(),
            item_name: name,
            node_kind: "method",
            owner_trait,
            owner_type,
            direction: QueryDirection::Outgoing,
        },
        CallOwnerSelector::MethodByBodyFile { name, .. } => MatrixQuery {
            file_path: owner.file_path.clone(),
            module_path: owner.module_path.clone(),
            item_name: name,
            node_kind: "method",
            owner_trait: None,
            owner_type: None,
            direction: QueryDirection::Outgoing,
        },
    }
}

fn function_by_name_in_module(
    db: &Database,
    module_path: &[&str],
    name: &str,
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

module_has_file[mid] := *file_mod{{ owner_id: mid @ 'NOW' }}
file_owner_for_module[mod_id, file_id] := module_has_file[mod_id], file_id = mod_id
file_owner_for_module[mod_id, file_id] := ancestor[mod_id, parent], module_has_file[parent], file_id = parent

?[id, file_path, mod_path] :=
    *function {{ id, name: $name, module_id @ 'NOW' }},
    *module{{ id: module_id, path: mod_path @ 'NOW' }},
    mod_path == $module_path,
    file_owner_for_module[module_id, file_id],
    *file_mod{{ owner_id: file_id, file_path @ 'NOW' }}
"#
    );
    one_target_info(
        db.raw_query_params(&script, params)
            .unwrap_or_else(|err| panic!("query function {label}: {err}")),
        label,
    )
}

fn method_by_name_and_body(
    db: &Database,
    name: &str,
    body: &str,
    file_suffix: Option<&str>,
    label: &str,
) -> TargetInfo {
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
        .unwrap_or_else(|err| panic!("query method {label}: {err}"));
    let marker = body_key(body);
    let matching = rows
        .rows
        .iter()
        .filter(|row| {
            let DataValue::Str(body) = &row[1] else {
                return false;
            };
            body_key(body).contains(&marker)
                && file_suffix.is_none_or(|suffix| data_str(&row[2], "file_path").ends_with(suffix))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "expected exactly one method owner for {label}; rows: {:#?}",
        rows.rows
    );
    let row = matching[0];
    TargetInfo {
        id: to_uuid(&row[0]).unwrap_or_else(|err| panic!("{label} uuid: {err}")),
        file_path: PathBuf::from(data_str(&row[2], "file_path")),
        module_path: data_path(&row[3], "module path"),
    }
}

fn variant_by_enum_and_name(db: &Database, enum_name: &str, variant_name: &str) -> TargetInfo {
    let mut params = BTreeMap::new();
    params.insert("enum_name".to_string(), DataValue::from(enum_name));
    params.insert("variant_name".to_string(), DataValue::from(variant_name));

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
    one_target_info(
        db.raw_query_params(&script, params)
            .unwrap_or_else(|err| panic!("query variant {enum_name}::{variant_name}: {err}")),
        variant_name,
    )
}

fn one_target_info(rows: ploke_db::QueryResult, label: &str) -> TargetInfo {
    assert_eq!(
        rows.rows.len(),
        1,
        "expected exactly one target for {label}; rows: {:#?}",
        rows.rows
    );
    let row = &rows.rows[0];
    TargetInfo {
        id: to_uuid(&row[0]).unwrap_or_else(|err| panic!("{label} uuid: {err}")),
        file_path: PathBuf::from(data_str(&row[1], "file_path")),
        module_path: data_path(&row[2], "module path"),
    }
}

fn crate_root_from_file(file_path: &std::path::Path, label: &str) -> PathBuf {
    let src_dir = file_path
        .ancestors()
        .find(|path| path.file_name().and_then(|name| name.to_str()) == Some("src"))
        .unwrap_or_else(|| panic!("{label} file should live under a crate src directory"));
    src_dir
        .parent()
        .unwrap_or_else(|| panic!("{label} src directory should have a crate root"))
        .to_path_buf()
}

fn build_domain(fixture: CallCorpusFixture) -> &'static str {
    match fixture {
        CallCorpusFixture::Axum => "bd:corpus-axum-call-graph",
        CallCorpusFixture::Chrono => "bd:corpus-chrono-call-graph",
        CallCorpusFixture::Memchr => "bd:corpus-memchr-call-graph",
        CallCorpusFixture::GenericArray => "bd:corpus-generic-array-call-graph",
    }
}

fn rag_site_kind(site: CallSiteSelector) -> CallSiteKind {
    match site {
        CallSiteSelector::Path { .. } => CallSiteKind::Path,
        CallSiteSelector::Dynamic { .. } => CallSiteKind::Dynamic,
    }
}

fn rag_callee(site: CallSiteSelector) -> CallCalleeInfo {
    match site {
        CallSiteSelector::Path { segments, .. } => CallCalleeInfo::Path {
            path: segments
                .iter()
                .map(|segment| (*segment).to_string())
                .collect(),
        },
        CallSiteSelector::Dynamic { .. } => CallCalleeInfo::Dynamic,
    }
}

fn rag_relation_kind(relation: CallRelationKind) -> RagCallTargetKind {
    match relation {
        CallRelationKind::Function => RagCallTargetKind::Function,
        CallRelationKind::DynamicFunction => RagCallTargetKind::DynamicFunction,
        CallRelationKind::Closure => RagCallTargetKind::Closure,
        CallRelationKind::LocalFunction => RagCallTargetKind::LocalFunction,
        CallRelationKind::DynamicClosure => RagCallTargetKind::DynamicClosure,
        CallRelationKind::Method => RagCallTargetKind::Method,
        CallRelationKind::AssociatedFunction => RagCallTargetKind::AssociatedFunction,
        CallRelationKind::TupleStructConstructor => RagCallTargetKind::TupleStructConstructor,
        CallRelationKind::EnumVariantConstructor => RagCallTargetKind::EnumVariantConstructor,
    }
}

fn rag_status_kind(status: DbCallStatusKind) -> CallStatusKind {
    match status {
        DbCallStatusKind::Resolved => CallStatusKind::Resolved,
        DbCallStatusKind::Unresolved => CallStatusKind::Unresolved,
        DbCallStatusKind::Ambiguous => CallStatusKind::Ambiguous,
        DbCallStatusKind::External => CallStatusKind::External,
        DbCallStatusKind::Unsupported => CallStatusKind::Unsupported,
    }
}

fn targetless_proof_state(
    status: DbCallStatusKind,
    site: CallSiteSelector,
) -> (&'static str, &'static str) {
    match (status, site) {
        (DbCallStatusKind::Unresolved, CallSiteSelector::Path { .. }) => {
            ("unresolved", "type_resolution_missing")
        }
        (DbCallStatusKind::Unresolved, CallSiteSelector::Dynamic { .. }) => {
            ("unresolved", "dynamic_dispatch_unbounded")
        }
        (DbCallStatusKind::Unsupported, CallSiteSelector::Dynamic { .. }) => {
            ("blocked", "dynamic_dispatch_unbounded")
        }
        (DbCallStatusKind::Unsupported, _) => ("blocked", "type_resolution_missing"),
        (DbCallStatusKind::External, _) => ("blocked", "external_dependency_summary_missing"),
        (DbCallStatusKind::Ambiguous, _) => ("ambiguous", "type_resolution_missing"),
        (DbCallStatusKind::Resolved, _) => unreachable!("resolved rows are not targetless"),
    }
}
