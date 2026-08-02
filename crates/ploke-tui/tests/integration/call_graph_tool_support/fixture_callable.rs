use super::*;

pub(crate) struct CallableBlockerFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) file_path: PathBuf,
    pub(crate) owner_name: &'static str,
    pub(crate) owner: Uuid,
    pub(crate) path: Vec<String>,
    pub(crate) candidates: Vec<Uuid>,
    pub(crate) build_domain: &'static str,
    pub(crate) shape: CallableBlockerShape,
}

pub(crate) struct CallableParamResolvedFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) file_path: PathBuf,
    pub(crate) owner_name: &'static str,
    pub(crate) owner: Uuid,
    pub(crate) target: Uuid,
    pub(crate) path: Vec<String>,
    pub(crate) build_domain: &'static str,
}

pub(crate) struct DirectSelfFieldDispatchFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) file_path: PathBuf,
    pub(crate) owner_name: &'static str,
    pub(crate) owner_type: &'static str,
    pub(crate) owner: Uuid,
    pub(crate) path: Vec<String>,
    pub(crate) candidates: Vec<Uuid>,
    pub(crate) build_domain: &'static str,
}

pub(crate) struct ResultCallbackFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) file_path: PathBuf,
    pub(crate) owner_name: &'static str,
    pub(crate) owner: Uuid,
    pub(crate) target: Uuid,
    pub(crate) build_domain: &'static str,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum CallableBlockerShape {
    Path,
    Dynamic,
    AmbiguousPath,
    AmbiguousDynamic,
}

impl CallableBlockerFixture {
    pub(crate) async fn with_db(
        db: Arc<Database>,
        owner_name: &'static str,
        path: &[&str],
        shape: CallableBlockerShape,
        expected_projection_count: usize,
    ) -> Self {
        let crate_root = workspace_root().join("tests/fixture_crates/fixture_call_graph");
        let module_path = vec!["crate".to_string()];
        let file_path = crate_root.join("src/lib.rs");
        let owner = graph_resolve_exact(
            db.as_ref(),
            "function",
            file_path.as_path(),
            &module_path,
            owner_name,
        )
        .unwrap_or_else(|err| panic!("resolve {owner_name}: {err}"))
        .pop()
        .unwrap_or_else(|| panic!("{owner_name} row"))
        .id;
        let mut candidates = vec![
            function_id(
                db.as_ref(),
                file_path.as_path(),
                &module_path,
                "local_target",
            ),
            function_id(
                db.as_ref(),
                file_path.as_path(),
                &module_path,
                "other_target",
            ),
        ];
        candidates.sort_unstable();
        assert_eq!(
            db.project_call_proof_facts_for_node(owner, "bd:fixture-call-graph")
                .expect("project callable blocker proof facts"),
            expected_projection_count,
            "{owner_name} should project the expected outgoing/incoming call proof fact rows"
        );

        let state = app_state_with_rag(db, crate_root).await;

        Self {
            state,
            file_path,
            owner_name,
            owner,
            path: path.iter().map(|part| (*part).to_string()).collect(),
            candidates,
            build_domain: "bd:fixture-call-graph",
            shape,
        }
    }
}

fn function_id(
    db: &Database,
    file_path: &std::path::Path,
    module_path: &[String],
    name: &str,
) -> Uuid {
    graph_resolve_exact(db, "function", file_path, module_path, name)
        .unwrap_or_else(|err| panic!("resolve {name}: {err}"))
        .pop()
        .unwrap_or_else(|| panic!("{name} row"))
        .id
}

impl DirectSelfFieldDispatchFixture {
    pub(crate) async fn with_db(db: Arc<Database>) -> Self {
        let crate_root = workspace_root().join("tests/fixture_crates/fixture_call_graph");
        let module_path = vec!["crate".to_string()];
        let file_path = crate_root.join("src/lib.rs");
        let owner_name = "invoke";
        let owner_type = "DirectSelfFieldDispatcher";
        let owner = method_id_by_self_type(db.as_ref(), owner_type, owner_name);
        let mut candidates = vec![
            function_id(
                db.as_ref(),
                file_path.as_path(),
                &module_path,
                "direct_self_field_local",
            ),
            function_id(
                db.as_ref(),
                file_path.as_path(),
                &module_path,
                "direct_self_field_other",
            ),
        ];
        candidates.sort_unstable();
        assert_eq!(
            db.project_call_proof_facts_for_node(owner, "bd:fixture-call-graph")
                .expect("project direct self-field dispatch proof facts"),
            3,
            "{owner_type}::{owner_name} should project candidate-only proof rows"
        );

        let state = app_state_with_rag(db, crate_root).await;

        Self {
            state,
            file_path,
            owner_name,
            owner_type,
            owner,
            path: vec!["self".to_string(), "call".to_string()],
            candidates,
            build_domain: "bd:fixture-call-graph",
        }
    }
}

fn method_id_by_self_type(db: &Database, self_type: &str, method: &str) -> Uuid {
    let rows = db
        .raw_query(&format!(
            r#"?[method_id] :=
                *method {{ id: method_id, name: "{method}", owner_id: impl_id @ 'NOW' }},
                *impl {{ id: impl_id @ 'NOW' }},
                *type_use {{
                    owner_id: impl_id,
                    root_type_id: self_type_id,
                    role: "ImplSelf" @ 'NOW'
                }},
                *type_relation {{
                    source_id: self_type_id,
                    target_id: self_target_id,
                    relation_kind: "Ordinary" @ 'NOW'
                }},
                *struct {{ id: self_target_id, name: "{self_type}" @ 'NOW' }}"#
        ))
        .unwrap_or_else(|err| panic!("resolve {self_type}::{method}: {err}"));
    assert_eq!(
        rows.rows.len(),
        1,
        "expected exactly one {self_type}::{method} method row: {:#?}",
        rows.rows
    );
    to_uuid(&rows.rows[0][0]).unwrap_or_else(|err| panic!("{self_type}::{method} uuid: {err}"))
}

impl CallableParamResolvedFixture {
    pub(crate) async fn with_db(
        db: Arc<Database>,
        owner_name: &'static str,
        path: &[&str],
        expected_projection_count: usize,
    ) -> Self {
        let crate_root = workspace_root().join("tests/fixture_crates/fixture_call_graph");
        let module_path = vec!["crate".to_string()];
        let file_path = crate_root.join("src/lib.rs");
        let owner = graph_resolve_exact(
            db.as_ref(),
            "function",
            file_path.as_path(),
            &module_path,
            owner_name,
        )
        .unwrap_or_else(|err| panic!("resolve {owner_name}: {err}"))
        .pop()
        .unwrap_or_else(|| panic!("{owner_name} row"))
        .id;
        let target = graph_resolve_exact(
            db.as_ref(),
            "function",
            file_path.as_path(),
            &module_path,
            "local_target",
        )
        .expect("resolve local_target")
        .pop()
        .expect("local_target row")
        .id;
        assert_eq!(
            db.project_call_proof_facts_for_node(owner, "bd:fixture-call-graph")
                .expect("project resolved callable parameter proof facts"),
            expected_projection_count,
            "{owner_name} should project the expected resolved call proof rows"
        );

        let state = app_state_with_rag(db, crate_root).await;

        Self {
            state,
            file_path,
            owner_name,
            owner,
            target,
            path: path.iter().map(|part| (*part).to_string()).collect(),
            build_domain: "bd:fixture-call-graph",
        }
    }
}

impl ResultCallbackFixture {
    pub(crate) async fn new() -> Self {
        let db = Arc::new(Database::new(
            setup_db_full_multi_embedding("fixture_call_graph").expect("fixture_call_graph db"),
        ));
        let crate_root = workspace_root().join("tests/fixture_crates/fixture_call_graph");
        let module_path = vec!["crate".to_string()];
        let file_path = crate_root.join("src/lib.rs");
        let owner_name = "call_single_result_callback";
        let owner = graph_resolve_exact(
            db.as_ref(),
            "function",
            file_path.as_path(),
            &module_path,
            owner_name,
        )
        .unwrap_or_else(|err| panic!("resolve {owner_name}: {err}"))
        .pop()
        .unwrap_or_else(|| panic!("{owner_name} row"))
        .id;
        let target = graph_resolve_exact(
            db.as_ref(),
            "function",
            file_path.as_path(),
            &module_path,
            "local_result_target",
        )
        .expect("resolve local_result_target")
        .pop()
        .expect("local_result_target row")
        .id;
        assert_eq!(
            db.project_call_proof_facts_for_node(owner, "bd:fixture-call-graph")
                .expect("project result callback proof facts"),
            8,
            "{owner_name} should project the expected result callback proof rows"
        );

        let state = app_state_with_rag(db, crate_root).await;

        Self {
            state,
            file_path,
            owner_name,
            owner,
            target,
            build_domain: "bd:fixture-call-graph",
        }
    }

    pub(crate) fn callee(&self) -> CallCalleeInfo {
        CallCalleeInfo::Method {
            name: "and_then".to_string(),
            receiver: Some(CallReceiverInfo::PathCallResult {
                path: vec!["Ok".to_string()],
            }),
        }
    }

    pub(crate) fn ctx(&self, call_id: &'static str) -> Ctx {
        ctx_for_state(&self.state, call_id)
    }
}
