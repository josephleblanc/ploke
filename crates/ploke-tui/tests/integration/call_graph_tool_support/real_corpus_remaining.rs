use super::*;

#[derive(Clone, Copy)]
pub(crate) enum AxumRemainingTarget {
    CoreTryDowncast,
    AxumTryDowncast,
    FromRequest,
    FromRequestParts,
    FromRef,
}

pub(crate) struct AxumRemainingToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) label: &'static str,
    pub(crate) item_name: &'static str,
    pub(crate) node_kind: &'static str,
    pub(crate) owner_trait: Option<&'static str>,
    pub(crate) file_path: PathBuf,
    pub(crate) module_path: Vec<String>,
    pub(crate) target: Uuid,
    pub(crate) callers: Vec<ExpectedCallSite>,
}

impl AxumRemainingTarget {
    pub(crate) const TOOL_REACHABLE_CASES: [Self; 5] = [
        Self::CoreTryDowncast,
        Self::AxumTryDowncast,
        Self::FromRequest,
        Self::FromRequestParts,
        Self::FromRef,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::CoreTryDowncast => "axum-core try_downcast",
            Self::AxumTryDowncast => "axum try_downcast",
            Self::FromRequest => "axum-core FromRequest::from_request",
            Self::FromRequestParts => "axum-core FromRequestParts::from_request_parts",
            Self::FromRef => "axum-core FromRef::from_ref",
        }
    }

    fn item_name(self) -> &'static str {
        match self {
            Self::CoreTryDowncast | Self::AxumTryDowncast => "try_downcast",
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
            Self::CoreTryDowncast | Self::AxumTryDowncast => None,
        }
    }

    fn node_kind(self) -> &'static str {
        match self {
            Self::CoreTryDowncast | Self::AxumTryDowncast => "function",
            Self::FromRequest | Self::FromRequestParts | Self::FromRef => "method",
        }
    }

    fn expected_callers(self) -> usize {
        match self {
            Self::CoreTryDowncast => 2,
            Self::AxumTryDowncast => 1,
            Self::FromRequest => 2,
            Self::FromRequestParts => 3,
            Self::FromRef => 2,
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
            .map(|caller| ExpectedCallSite {
                owner: caller.site.owner_id,
                site: caller.site.id,
                path: caller
                    .site
                    .path
                    .unwrap_or_else(|| panic!("{} caller should carry a path", case.label())),
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
        let state = axum_state_for_target(Arc::clone(&db), &target, case.label()).await;

        Self {
            state,
            label: case.label(),
            item_name: case.item_name(),
            node_kind: case.node_kind(),
            owner_trait: case.owner_trait(),
            file_path: target.file_path,
            module_path: target.module_path,
            target: target.id,
            callers,
        }
    }

    pub(crate) fn module_path_arg(&self) -> String {
        self.module_path.join("::")
    }

    pub(crate) fn ctx(&self, call_id: &'static str) -> Ctx {
        ctx_for_state(&self.state, call_id)
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
