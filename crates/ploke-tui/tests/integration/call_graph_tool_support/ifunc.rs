use super::*;

#[derive(Clone, Copy)]
pub(crate) struct IfuncToolCase {
    pub(crate) label: &'static str,
    pub(crate) item: &'static str,
    pub(crate) expected_arg_count: u32,
}

pub(crate) struct IfuncToolFixture {
    pub(crate) state: Arc<AppState>,
    pub(crate) case: IfuncToolCase,
    pub(crate) file_path: PathBuf,
    pub(crate) module_path: Vec<String>,
    pub(crate) owner: Uuid,
}

impl IfuncToolCase {
    pub(crate) const MEMCHR: [Self; 7] = [
        Self {
            label: "memchr/src/arch/x86_64/memchr.rs:180 memchr_raw unsafe_ifunc",
            item: "memchr_raw",
            expected_arg_count: 3,
        },
        Self {
            label: "memchr/src/arch/x86_64/memchr.rs:203 memrchr_raw unsafe_ifunc",
            item: "memrchr_raw",
            expected_arg_count: 3,
        },
        Self {
            label: "memchr/src/arch/x86_64/memchr.rs:227 memchr2_raw unsafe_ifunc",
            item: "memchr2_raw",
            expected_arg_count: 4,
        },
        Self {
            label: "memchr/src/arch/x86_64/memchr.rs:252 memrchr2_raw unsafe_ifunc",
            item: "memrchr2_raw",
            expected_arg_count: 4,
        },
        Self {
            label: "memchr/src/arch/x86_64/memchr.rs:278 memchr3_raw unsafe_ifunc",
            item: "memchr3_raw",
            expected_arg_count: 5,
        },
        Self {
            label: "memchr/src/arch/x86_64/memchr.rs:305 memrchr3_raw unsafe_ifunc",
            item: "memrchr3_raw",
            expected_arg_count: 5,
        },
        Self {
            label: "memchr/src/arch/x86_64/memchr.rs:326 count_raw unsafe_ifunc",
            item: "count_raw",
            expected_arg_count: 3,
        },
    ];

    pub(crate) fn build_domain(&self) -> &'static str {
        "bd:corpus-memchr-call-graph"
    }
}

impl IfuncToolFixture {
    pub(crate) async fn new(case: IfuncToolCase) -> Self {
        let db = memchr_call_graph_db();
        let owner = function_target_by_name_in_module(
            &db,
            &["crate", "arch", "x86_64", "memchr"],
            case.item,
            case.label,
        );
        assert!(
            db.project_call_proof_facts_for_node(owner.id, case.build_domain())
                .unwrap_or_else(|err| panic!("project {} proof facts: {err}", case.label))
                >= 4,
            "{} should project generated unsafe_ifunc path/dynamic proof rows",
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

pub(crate) fn assert_ifunc_context(
    calls: &[serde_json::Value],
    owner: Uuid,
    expected_arg_count: u32,
    label: &str,
    tool: &str,
) -> (Uuid, Uuid) {
    let rows = calls
        .iter()
        .filter_map(|call| serde_json::from_value::<CallContextInfo>(call.clone()).ok())
        .collect::<Vec<_>>();
    let callee = CallCalleeInfo::Path {
        path: transmute_path(),
    };
    let path_rows = rows
        .iter()
        .filter(|call| call.owner_id == owner && call.kind == CallSiteKind::Path)
        .filter(|call| call.callee == callee)
        .collect::<Vec<_>>();
    assert_eq!(
        path_rows.len(),
        1,
        "{tool} should return one generated transmute path row for {label}: {calls:#?}"
    );
    let path = path_rows[0];
    assert_eq!(path.status, CallStatusKind::External);
    assert_eq!(path.resolution, None);
    assert_eq!(path.arg_count, Some(1));
    assert_eq!(path.generic_arg_count, Some(2));
    assert!(
        path.targets.is_empty(),
        "{tool} should not fabricate a target for generated transmute path row {label}: {path:#?}"
    );

    let dynamic_rows = rows
        .iter()
        .filter(|call| {
            call.owner_id == owner
                && call.kind == CallSiteKind::Dynamic
                && call.callee == CallCalleeInfo::Dynamic
                && call.path.as_ref().is_some_and(|path| {
                    path.iter()
                        .map(String::as_str)
                        .eq(["core", "mem", "transmute"])
                })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        dynamic_rows.len(),
        1,
        "{tool} should return one generated returned-path dynamic row for {label}: {calls:#?}"
    );
    let dynamic = dynamic_rows[0];
    assert_eq!(dynamic.status, CallStatusKind::External);
    assert_eq!(dynamic.resolution, None);
    assert_eq!(dynamic.arg_count, Some(expected_arg_count));
    assert_eq!(dynamic.generic_arg_count, None);
    assert!(
        dynamic.targets.is_empty(),
        "{tool} should not fabricate a target for generated returned-path dynamic row {label}: {dynamic:#?}"
    );

    (path.site_id, dynamic.site_id)
}

pub(crate) fn assert_ifunc_proof(
    proofs: &[serde_json::Value],
    owner: Uuid,
    sites: (Uuid, Uuid),
    build_domain: &str,
    label: &str,
    tool: &str,
) {
    let rows = proofs
        .iter()
        .filter_map(|proof| serde_json::from_value::<ProofContextInfo>(proof.clone()).ok())
        .collect::<Vec<_>>();
    for site in [sites.0, sites.1] {
        let site_id = site.to_string();
        let owner_id = owner.to_string();
        assert!(
            rows.iter().any(|proof| {
                proof.kind == "call_site"
                    && proof.caller_def_id.as_deref() == Some(owner_id.as_str())
                    && proof.call_site_id.as_deref() == Some(site_id.as_str())
                    && proof.build_domain_id.as_deref() == Some(build_domain)
                    && proof.unsafe_block == Some(true)
            }),
            "{tool} should return the unsafe generated call_site proof row for {label}: {proofs:#?}"
        );
        assert!(
            rows.iter().any(|proof| {
                proof.kind == "call_resolution"
                    && proof.call_site_id.as_deref() == Some(site_id.as_str())
                    && proof.resolution_state.as_deref() == Some("blocked")
                    && proof.blocker_reason.as_deref()
                        == Some("external_dependency_summary_missing")
            }),
            "{tool} should return the external generated call_resolution proof row for {label}: {proofs:#?}"
        );
        assert!(
            rows.iter().all(|proof| {
                proof.kind != "call_edge" || proof.call_site_id.as_deref() != Some(site_id.as_str())
            }),
            "{tool} should not fabricate generated transmute call_edge rows for {label}: {proofs:#?}"
        );
    }
}

pub(crate) fn assert_ifunc_unsafe_calls(
    calls: &[serde_json::Value],
    owner: Uuid,
    sites: (Uuid, Uuid),
    expected_arg_count: u32,
    label: &str,
    tool: &str,
) {
    let rows = calls
        .iter()
        .filter_map(|call| serde_json::from_value::<UnsafeBlockCallInfo>(call.clone()).ok())
        .collect::<Vec<_>>();
    assert_eq!(
        rows.len(),
        2,
        "{tool} should return the generated unsafe transmute path and returned-path dynamic rows for {label}: {calls:#?}"
    );
    assert!(
        rows.iter().all(|row| row.paths_to_owner.is_empty()),
        "{tool} should not invent intermediate paths for direct generated unsafe rows in {label}: {rows:#?}"
    );

    let callee = CallCalleeInfo::Path {
        path: transmute_path(),
    };
    let path_rows = rows
        .iter()
        .filter(|row| {
            row.call_site.owner_id == owner
                && row.call_site.site_id == sites.0
                && row.call_site.kind == CallSiteKind::Path
                && row.call_site.callee == callee
        })
        .collect::<Vec<_>>();
    assert_eq!(
        path_rows.len(),
        1,
        "{tool} unsafe-block calls should include the generated transmute path row for {label}: {calls:#?}"
    );
    let path = &path_rows[0].call_site;
    assert_eq!(path.status, CallStatusKind::External);
    assert_eq!(path.resolution, None);
    assert_eq!(path.arg_count, Some(1));
    assert_eq!(path.generic_arg_count, Some(2));
    assert!(
        path.targets.is_empty(),
        "{tool} should keep generated unsafe transmute path targetless for {label}: {path:#?}"
    );

    let dynamic_rows = rows
        .iter()
        .filter(|row| {
            row.call_site.owner_id == owner
                && row.call_site.site_id == sites.1
                && row.call_site.kind == CallSiteKind::Dynamic
                && row.call_site.callee == CallCalleeInfo::Dynamic
                && row.call_site.arg_count == Some(expected_arg_count)
                && row.call_site.path.as_ref().is_some_and(|path| {
                    path.iter()
                        .map(String::as_str)
                        .eq(["core", "mem", "transmute"])
                })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        dynamic_rows.len(),
        1,
        "{tool} unsafe-block calls should include the generated returned-path dynamic row for {label}: {calls:#?}"
    );
    let dynamic = &dynamic_rows[0].call_site;
    assert_eq!(dynamic.status, CallStatusKind::External);
    assert_eq!(dynamic.resolution, None);
    assert_eq!(dynamic.generic_arg_count, None);
    assert!(
        dynamic.targets.is_empty(),
        "{tool} should keep generated unsafe returned-path dynamic row targetless for {label}: {dynamic:#?}"
    );
}

fn transmute_path() -> Vec<String> {
    ["core", "mem", "transmute"]
        .into_iter()
        .map(str::to_string)
        .collect()
}
