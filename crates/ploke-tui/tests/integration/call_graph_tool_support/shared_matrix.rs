use std::borrow::Cow;

use ploke_core::rag_types::CallResolutionKind;
use ploke_test_utils::{CallExpected, CallShapeCase};
use ploke_tui::tools::{code_item_lookup::LookupParams, get_code_edges::EdgesParams};

use super::*;

#[path = "shared_matrix/db.rs"]
mod db;
#[path = "shared_matrix/query.rs"]
mod query;
#[path = "shared_matrix/rag.rs"]
mod rag;

use db::{assert_db_expectation, resolve_owner, resolve_target, select_site};
use query::{
    MatrixQuery, QueryDirection, build_domain, crate_root_from_file, query_for_owner,
    query_for_target, query_node_id,
};
use rag::{
    rag_callee_matches, rag_relation_kind, rag_site_kind, rag_status_kind, targetless_proof_state,
};

pub(crate) struct SharedCallShapeToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) case: &'static CallShapeCase,
    pub(crate) owner: Uuid,
    pub(crate) site: Uuid,
    pub(crate) target: Option<Uuid>,
    query: MatrixQuery,
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
        assert!(
            rag_callee_matches(&call.callee, self.case.site),
            "{tool} should expose the selected callee for {}: {call:#?}",
            self.case.name
        );

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
