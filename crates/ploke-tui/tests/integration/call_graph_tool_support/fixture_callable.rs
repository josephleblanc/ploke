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
    pub(crate) async fn function_pointer_param() -> Self {
        Self::new_for_owner(
            "call_function_pointer_param",
            &["f"],
            CallableBlockerShape::Path,
            2,
        )
        .await
    }

    pub(crate) async fn multi_conflicting_function_pointer_param() -> Self {
        Self::new_for_owner(
            "call_multi_conflicting_function_pointer_param",
            &["f"],
            CallableBlockerShape::AmbiguousPath,
            8,
        )
        .await
    }

    pub(crate) async fn forwarded_conflicting_function_pointer_leaf() -> Self {
        Self::new_for_owner(
            "call_forwarded_conflicting_function_pointer_leaf",
            &["f"],
            CallableBlockerShape::AmbiguousPath,
            5,
        )
        .await
    }

    pub(crate) async fn returned_conflicting_function_pointer_local() -> Self {
        Self::returned_conflicting_function_pointer(
            "call_returned_conflicting_forwarded_function_pointer_param_with_local_target",
        )
        .await
    }

    pub(crate) async fn returned_conflicting_function_pointer_other() -> Self {
        Self::returned_conflicting_function_pointer(
            "call_returned_conflicting_forwarded_function_pointer_param_with_other_target",
        )
        .await
    }

    async fn returned_conflicting_function_pointer(owner_name: &'static str) -> Self {
        Self::new_for_owner(
            owner_name,
            &["return_conflicting_forwarded_function_pointer"],
            CallableBlockerShape::AmbiguousDynamic,
            5,
        )
        .await
    }

    pub(crate) async fn forwarded_conflicting_named_field_leaf() -> Self {
        Self::new_for_owner(
            "call_forwarded_conflicting_named_field_leaf",
            &["holder", "callback"],
            CallableBlockerShape::AmbiguousDynamic,
            5,
        )
        .await
    }

    pub(crate) async fn generic_fn_once_value_binding() -> Self {
        Self::new_for_owner(
            "call_generic_fn_once_value_binding",
            &["generic_f"],
            CallableBlockerShape::Path,
            2,
        )
        .await
    }

    pub(crate) async fn multi_conflicting_generic_fn_once_param() -> Self {
        Self::new_for_owner(
            "call_multi_conflicting_generic_fn_once_param",
            &["generic_f"],
            CallableBlockerShape::AmbiguousPath,
            8,
        )
        .await
    }

    pub(crate) async fn multi_conflicting_named_field_function_param() -> Self {
        Self::new_for_owner(
            "call_multi_conflicting_named_field_function_param",
            &["holder", "callback"],
            CallableBlockerShape::AmbiguousDynamic,
            8,
        )
        .await
    }

    pub(crate) async fn field_function_param() -> Self {
        Self::new_for_owner(
            "call_field_function_param",
            &["holder", "callback"],
            CallableBlockerShape::Dynamic,
            2,
        )
        .await
    }

    pub(crate) async fn indexed_function_pointer() -> Self {
        Self::new_for_owner(
            "call_indexed_function_pointer",
            &["funcs", "0"],
            CallableBlockerShape::Dynamic,
            2,
        )
        .await
    }

    pub(crate) async fn indexed_field_function_param() -> Self {
        Self::new_for_owner(
            "call_indexed_field_function_param",
            &["holder", "callbacks", "0"],
            CallableBlockerShape::Dynamic,
            2,
        )
        .await
    }

    pub(crate) async fn indexed_tuple_field_function_param() -> Self {
        Self::new_for_owner(
            "call_indexed_tuple_field_function_param",
            &["holder", "0", "0"],
            CallableBlockerShape::Dynamic,
            2,
        )
        .await
    }

    async fn new_for_owner(
        owner_name: &'static str,
        path: &[&str],
        shape: CallableBlockerShape,
        expected_projection_count: usize,
    ) -> Self {
        let db = Arc::new(Database::new(
            setup_db_full_multi_embedding("fixture_call_graph").expect("fixture_call_graph db"),
        ));
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

    pub(crate) fn ctx(&self, call_id: &'static str) -> Ctx {
        ctx_for_state(&self.state, call_id)
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

impl CallableParamResolvedFixture {
    pub(crate) async fn multi_function_pointer_param() -> Self {
        Self::new_for_owner("call_multi_function_pointer_param", &["f"], 9).await
    }

    pub(crate) async fn forwarded_function_pointer_leaf() -> Self {
        Self::new_for_owner("call_forwarded_function_pointer_leaf", &["f"], 6).await
    }

    pub(crate) async fn two_hop_forwarded_function_pointer_leaf() -> Self {
        Self::new_for_owner("call_two_hop_forwarded_function_pointer_leaf", &["f"], 6).await
    }

    pub(crate) async fn forwarded_referenced_dyn_fn_leaf() -> Self {
        Self::new_for_owner("call_forwarded_referenced_dyn_fn_leaf", &["f"], 6).await
    }

    pub(crate) async fn two_hop_forwarded_referenced_dyn_fn_leaf() -> Self {
        Self::new_for_owner("call_two_hop_forwarded_referenced_dyn_fn_leaf", &["f"], 6).await
    }

    pub(crate) async fn forwarded_boxed_dyn_fn_leaf() -> Self {
        Self::new_for_owner("call_forwarded_boxed_dyn_fn_leaf", &["f"], 6).await
    }

    pub(crate) async fn two_hop_forwarded_boxed_dyn_fn_leaf() -> Self {
        Self::new_for_owner("call_two_hop_forwarded_boxed_dyn_fn_leaf", &["f"], 6).await
    }

    pub(crate) async fn single_boxed_dyn_fn_param() -> Self {
        Self::new_for_owner("call_single_boxed_dyn_fn_param", &["f"], 6).await
    }

    pub(crate) async fn multi_generic_fn_once_param() -> Self {
        Self::new_for_owner("call_multi_generic_fn_once_param", &["generic_f"], 9).await
    }

    async fn new_for_owner(
        owner_name: &'static str,
        path: &[&str],
        expected_projection_count: usize,
    ) -> Self {
        let db = Arc::new(Database::new(
            setup_db_full_multi_embedding("fixture_call_graph").expect("fixture_call_graph db"),
        ));
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

    pub(crate) fn ctx(&self, call_id: &'static str) -> Ctx {
        ctx_for_state(&self.state, call_id)
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
