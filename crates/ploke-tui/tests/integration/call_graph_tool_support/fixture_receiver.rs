use super::*;

pub(crate) struct FixtureDynamicCallableToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) file_path: PathBuf,
    pub(crate) owner_name: &'static str,
    pub(crate) owner: Uuid,
    pub(crate) target: Uuid,
}

pub(crate) struct FixtureBranchReceiverToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) file_path: PathBuf,
    pub(crate) owner_name: &'static str,
    pub(crate) owner: Uuid,
    pub(crate) target: Uuid,
}

pub(crate) struct FixtureSelfFieldReceiverToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) file_path: PathBuf,
    pub(crate) owner_type: &'static str,
    pub(crate) owner_name: &'static str,
    pub(crate) field_path: Vec<String>,
    pub(crate) owner: Uuid,
    pub(crate) target: Uuid,
}

pub(crate) struct FixtureMethodCallableArgumentToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) file_path: PathBuf,
    pub(crate) owner_type: &'static str,
    pub(crate) owner_name: &'static str,
    pub(crate) owner: Uuid,
    pub(crate) target: Uuid,
    pub(crate) parameter: Uuid,
    pub(crate) method_call_site: Uuid,
}

impl FixtureDynamicCallableToolFixture {
    pub(crate) async fn new_for_owner(owner_name: &'static str) -> Self {
        Self::with_db(fixture_graph_db(), owner_name).await
    }

    pub(crate) async fn with_db(db: Arc<Database>, owner_name: &'static str) -> Self {
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
        assert!(
            db.project_call_proof_facts_for_node(owner, "bd:fixture-call-graph")
                .expect("project dynamic callable owner proof facts")
                >= 3,
            "{owner_name} should project resolved dynamic proof rows"
        );

        let state = app_state_with_rag(db, crate_root).await;

        Self {
            state,
            file_path,
            owner_name,
            owner,
            target,
        }
    }

    pub(crate) fn ctx(&self, call_id: &'static str) -> Ctx {
        ctx_for_state(&self.state, call_id)
    }
}

impl FixtureMethodCallableArgumentToolFixture {
    pub(crate) async fn new() -> Self {
        let db = Arc::new(Database::new(
            setup_db_full_multi_embedding("fixture_call_graph").expect("fixture_call_graph db"),
        ));
        let crate_root = workspace_root().join("tests/fixture_crates/fixture_call_graph");
        let module_path = vec!["crate".to_string()];
        let file_path = crate_root.join("src/lib.rs");
        let owner_type = "LocalAssoc";
        let owner_name = "call_function_pointer_param";
        let owner = graph_resolve_exact(
            db.as_ref(),
            "method",
            file_path.as_path(),
            &module_path,
            owner_name,
        )
        .expect("resolve LocalAssoc::call_function_pointer_param")
        .pop()
        .expect("LocalAssoc::call_function_pointer_param row")
        .id;
        let caller = graph_resolve_exact(
            db.as_ref(),
            "function",
            file_path.as_path(),
            &module_path,
            "call_method_function_pointer_param_with_local_target",
        )
        .expect("resolve call_method_function_pointer_param_with_local_target")
        .pop()
        .expect("call_method_function_pointer_param_with_local_target row")
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
        let parameter = db
            .local_bindings_for_owner(owner)
            .expect("LocalAssoc::call_function_pointer_param bindings")
            .into_iter()
            .find(|binding| binding.kind == "ParameterBinding" && binding.name == "f")
            .expect("LocalAssoc::call_function_pointer_param should persist parameter f")
            .id;
        let receiver = CallReceiver::InitializedLocalBinding {
            name: "value".to_string(),
            init_path: vec!["LocalAssoc".to_string()],
        };
        let caller_context = db
            .call_context_for_owner(caller)
            .expect("call_method_function_pointer_param_with_local_target context");
        let matches = caller_context
            .iter()
            .filter(|row| {
                row.site.kind == DbCallSiteKind::Method
                    && row.site.method.as_deref() == Some(owner_name)
                    && row.site.receiver.as_ref() == Some(&receiver)
                    && row.targets.iter().any(|target| {
                        target.target_id == owner
                            && target.relation == DbCallRelationKind::Method
                            && target.target_kind == DbCallTargetKind::Method
                    })
            })
            .collect::<Vec<_>>();
        assert_eq!(
            matches.len(),
            1,
            "expected one method callsite supplying LocalAssoc::call_function_pointer_param: {caller_context:#?}"
        );
        let method_call_site = matches[0].site.id;
        assert!(
            db.project_call_proof_facts_for_node(owner, "bd:fixture-call-graph")
                .expect("project LocalAssoc::call_function_pointer_param proof facts")
                >= 1,
            "LocalAssoc::call_function_pointer_param should project proof rows"
        );
        let state = app_state_with_rag(db, crate_root).await;

        Self {
            state,
            file_path,
            owner_type,
            owner_name,
            owner,
            target,
            parameter,
            method_call_site,
        }
    }

    pub(crate) fn ctx(&self, call_id: &'static str) -> Ctx {
        ctx_for_state(&self.state, call_id)
    }
}

impl FixtureBranchReceiverToolFixture {
    pub(crate) async fn new_for_owner(owner_name: &'static str) -> Self {
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
            "method",
            file_path.as_path(),
            &module_path,
            "instance_value",
        )
        .expect("resolve LocalAssoc::instance_value")
        .pop()
        .expect("LocalAssoc::instance_value row")
        .id;
        assert!(
            db.project_call_proof_facts_for_node(owner, "bd:fixture-call-graph")
                .expect("project branch receiver owner proof facts")
                >= 3,
            "{owner_name} should project resolved branch receiver proof rows"
        );

        let state = app_state_with_rag(db, crate_root).await;

        Self {
            state,
            file_path,
            owner_name,
            owner,
            target,
        }
    }

    pub(crate) fn ctx(&self, call_id: &'static str) -> Ctx {
        ctx_for_state(&self.state, call_id)
    }
}

impl FixtureSelfFieldReceiverToolFixture {
    pub(crate) async fn nested_self_field() -> Self {
        Self::new(
            "NestedSelfFieldAssocOwner",
            "call_nested_self_field_instance_method",
            &["inner", "value"],
        )
        .await
    }

    async fn new(owner_type: &'static str, owner_name: &'static str, field_path: &[&str]) -> Self {
        let db = Arc::new(Database::new(
            setup_db_full_multi_embedding("fixture_call_graph").expect("fixture_call_graph db"),
        ));
        let crate_root = workspace_root().join("tests/fixture_crates/fixture_call_graph");
        let module_path = vec!["crate".to_string()];
        let file_path = crate_root.join("src/lib.rs");
        let owner = graph_resolve_exact(
            db.as_ref(),
            "method",
            file_path.as_path(),
            &module_path,
            owner_name,
        )
        .unwrap_or_else(|err| panic!("resolve {owner_type}::{owner_name}: {err}"))
        .pop()
        .unwrap_or_else(|| panic!("{owner_type}::{owner_name} row"))
        .id;
        let target = graph_resolve_exact(
            db.as_ref(),
            "method",
            file_path.as_path(),
            &module_path,
            "instance_value",
        )
        .expect("resolve LocalAssoc::instance_value")
        .pop()
        .expect("LocalAssoc::instance_value row")
        .id;
        assert!(
            db.project_call_proof_facts_for_node(owner, "bd:fixture-call-graph")
                .expect("project self-field receiver owner proof facts")
                >= 3,
            "{owner_type}::{owner_name} should project resolved self-field proof rows"
        );

        let state = app_state_with_rag(db, crate_root).await;

        Self {
            state,
            file_path,
            owner_type,
            owner_name,
            field_path: field_path
                .iter()
                .map(|segment| segment.to_string())
                .collect(),
            owner,
            target,
        }
    }

    pub(crate) fn ctx(&self, call_id: &'static str) -> Ctx {
        ctx_for_state(&self.state, call_id)
    }
}

pub(crate) fn assert_branch_receiver_context(
    calls: &[serde_json::Value],
    fixture: &FixtureBranchReceiverToolFixture,
    label: &str,
) -> CallContextInfo {
    let calls = calls
        .iter()
        .filter_map(|call| serde_json::from_value::<CallContextInfo>(call.clone()).ok())
        .filter(|call| {
            call.owner_id == fixture.owner
                && call.kind == CallSiteKind::Method
                && call.callee == branch_receiver_callee()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        calls.len(),
        1,
        "{label} should expose exactly one resolved branch receiver row for {}: {calls:#?}",
        fixture.owner_name
    );
    let call = calls[0].clone();
    assert_eq!(call.status, CallStatusKind::Resolved);
    assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(call.targets.len(), 1, "{call:#?}");
    assert_eq!(call.targets[0].target_id, fixture.target);
    assert_eq!(call.targets[0].relation, CallTargetKind::Method);
    call
}

pub(crate) fn assert_branch_receiver_proof(
    proofs: &[serde_json::Value],
    fixture: &FixtureBranchReceiverToolFixture,
    site: Uuid,
    label: &str,
) {
    assert_resolved_method_proof_rows(
        proofs,
        fixture.owner,
        fixture.target,
        site,
        label,
        fixture.owner_name,
    );
}

pub(crate) fn assert_initialized_local_receiver_context(
    calls: &[serde_json::Value],
    fixture: &FixtureBranchReceiverToolFixture,
    label: &str,
) -> CallContextInfo {
    let callee = initialized_local_receiver_callee();
    let calls = calls
        .iter()
        .filter_map(|call| serde_json::from_value::<CallContextInfo>(call.clone()).ok())
        .filter(|call| {
            call.owner_id == fixture.owner
                && call.kind == CallSiteKind::Method
                && call.callee == callee
        })
        .collect::<Vec<_>>();
    assert_eq!(
        calls.len(),
        1,
        "{label} should expose exactly one resolved initialized local receiver row for {}: {calls:#?}",
        fixture.owner_name
    );
    let call = calls[0].clone();
    assert_eq!(call.status, CallStatusKind::Resolved);
    assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(call.targets.len(), 1, "{call:#?}");
    assert_eq!(call.targets[0].target_id, fixture.target);
    assert_eq!(call.targets[0].relation, CallTargetKind::Method);
    call
}

pub(crate) fn assert_initialized_local_receiver_proof(
    proofs: &[serde_json::Value],
    fixture: &FixtureBranchReceiverToolFixture,
    site: Uuid,
    label: &str,
) {
    assert_resolved_method_proof_rows(
        proofs,
        fixture.owner,
        fixture.target,
        site,
        label,
        fixture.owner_name,
    );
}

pub(crate) fn assert_self_field_receiver_context(
    calls: &[serde_json::Value],
    fixture: &FixtureSelfFieldReceiverToolFixture,
    label: &str,
) -> CallContextInfo {
    let callee = CallCalleeInfo::Method {
        name: "instance_value".to_string(),
        receiver: Some(CallReceiverInfo::SelfField {
            path: fixture.field_path.clone(),
        }),
    };
    let calls = calls
        .iter()
        .filter_map(|call| serde_json::from_value::<CallContextInfo>(call.clone()).ok())
        .filter(|call| {
            call.owner_id == fixture.owner
                && call.kind == CallSiteKind::Method
                && call.callee == callee
        })
        .collect::<Vec<_>>();
    assert_eq!(
        calls.len(),
        1,
        "{label} should expose exactly one resolved self-field receiver row for {}::{}: {calls:#?}",
        fixture.owner_type,
        fixture.owner_name
    );
    let call = calls[0].clone();
    assert_eq!(call.status, CallStatusKind::Resolved);
    assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(call.targets.len(), 1, "{call:#?}");
    assert_eq!(call.targets[0].target_id, fixture.target);
    assert_eq!(call.targets[0].relation, CallTargetKind::Method);
    call
}

pub(crate) fn assert_self_field_receiver_proof(
    proofs: &[serde_json::Value],
    fixture: &FixtureSelfFieldReceiverToolFixture,
    site: Uuid,
    label: &str,
) {
    assert_resolved_method_proof_rows(
        proofs,
        fixture.owner,
        fixture.target,
        site,
        label,
        fixture.owner_name,
    );
}

fn assert_resolved_method_proof_rows(
    proofs: &[serde_json::Value],
    owner: Uuid,
    target: Uuid,
    site: Uuid,
    label: &str,
    owner_name: &str,
) {
    let owner = owner.to_string();
    let site = site.to_string();
    let target = target.to_string();
    let proofs = proofs
        .iter()
        .filter_map(|proof| serde_json::from_value::<ProofContextInfo>(proof.clone()).ok())
        .collect::<Vec<_>>();
    assert!(
        proofs.iter().any(|proof| {
            proof.kind == "call_site"
                && proof.caller_def_id.as_deref() == Some(owner.as_str())
                && proof.call_site_id.as_deref() == Some(site.as_str())
                && proof.build_domain_id.as_deref() == Some("bd:fixture-call-graph")
        }),
        "{label} should return the method call_site proof row for {owner_name}: {proofs:#?}"
    );
    assert!(
        proofs.iter().any(|proof| {
            proof.kind == "call_edge"
                && proof.call_site_id.as_deref() == Some(site.as_str())
                && proof.caller_def_id.as_deref() == Some(owner.as_str())
                && proof.callee_def_id.as_deref() == Some(target.as_str())
                && proof.resolution_state.as_deref() == Some("resolved")
        }),
        "{label} should return the resolved method call_edge proof row for {owner_name}: {proofs:#?}"
    );
    assert!(
        proofs.iter().any(|proof| {
            proof.kind == "call_resolution"
                && proof.call_site_id.as_deref() == Some(site.as_str())
                && proof.resolution_state.as_deref() == Some("resolved")
                && proof.resolved_def_id.as_deref() == Some(target.as_str())
        }),
        "{label} should return the resolved method call_resolution proof row for {owner_name}: {proofs:#?}"
    );
}

fn branch_receiver_callee() -> CallCalleeInfo {
    CallCalleeInfo::Method {
        name: "instance_value".to_string(),
        receiver: Some(CallReceiverInfo::IfBranchPaths {
            paths: vec![
                vec!["LocalAssoc".to_string()],
                vec!["LocalAssoc".to_string()],
            ],
        }),
    }
}

fn initialized_local_receiver_callee() -> CallCalleeInfo {
    CallCalleeInfo::Method {
        name: "instance_value".to_string(),
        receiver: Some(CallReceiverInfo::InitializedLocalBinding {
            name: "value".to_string(),
            init_path: vec!["LocalAssoc".to_string()],
        }),
    }
}
