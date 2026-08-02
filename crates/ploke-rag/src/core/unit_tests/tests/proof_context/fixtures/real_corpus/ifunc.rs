use super::super::super::super::*;
use super::helpers::{MEMCHR_DOMAIN, assert_site_blocker_in_domain, function_id, memchr_db};
use ploke_db::ProofGraphStore;

struct IfuncCase {
    label: &'static str,
    owner: &'static str,
    expected_arg_count: u32,
}

const IFUNC_CASES: [IfuncCase; 7] = [
    IfuncCase {
        label: "memchr/src/arch/x86_64/memchr.rs:180 memchr_raw unsafe_ifunc",
        owner: "memchr_raw",
        expected_arg_count: 3,
    },
    IfuncCase {
        label: "memchr/src/arch/x86_64/memchr.rs:203 memrchr_raw unsafe_ifunc",
        owner: "memrchr_raw",
        expected_arg_count: 3,
    },
    IfuncCase {
        label: "memchr/src/arch/x86_64/memchr.rs:227 memchr2_raw unsafe_ifunc",
        owner: "memchr2_raw",
        expected_arg_count: 4,
    },
    IfuncCase {
        label: "memchr/src/arch/x86_64/memchr.rs:252 memrchr2_raw unsafe_ifunc",
        owner: "memrchr2_raw",
        expected_arg_count: 4,
    },
    IfuncCase {
        label: "memchr/src/arch/x86_64/memchr.rs:278 memchr3_raw unsafe_ifunc",
        owner: "memchr3_raw",
        expected_arg_count: 5,
    },
    IfuncCase {
        label: "memchr/src/arch/x86_64/memchr.rs:305 memrchr3_raw unsafe_ifunc",
        owner: "memrchr3_raw",
        expected_arg_count: 5,
    },
    IfuncCase {
        label: "memchr/src/arch/x86_64/memchr.rs:326 count_raw unsafe_ifunc",
        owner: "count_raw",
        expected_arg_count: 3,
    },
];

#[tokio::test]
async fn proof_context_collection_preserves_memchr_generated_transmute_frontiers()
-> Result<(), Error> {
    init_tracing_once();
    let db = memchr_db()?;

    for case in IFUNC_CASES {
        let owner = function_id(&db, &["crate", "arch", "x86_64", "memchr"], case.owner)?;
        let projected = db.project_call_proof_facts_for_owner(owner, MEMCHR_DOMAIN)?;
        assert!(
            projected >= 4,
            "{} should project generated transmute path/dynamic proof rows",
            case.label
        );

        let rag = init_test_rag_mock(Arc::clone(&db));
        assert!(
            !rag.proof_context_degraded(),
            "projected memchr generated transmute facts should enable RAG proof context"
        );

        let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
        let calls = call_context
            .get(&owner)
            .unwrap_or_else(|| panic!("{} should receive outgoing call context", case.label));
        let sites = generated_transmute_sites(calls, owner, case.expected_arg_count, case.label);
        let rows = rag.exact_proof_context(owner)?;

        // Matrix:
        //   docs/active/agents/call-graph/
        //   2026-06-28_real-corpus-call-site-oracle-matrices.md
        //
        // Source chain:
        //   memchr/src/arch/x86_64/memchr.rs:153 generates
        //   `core::mem::transmute::<Fn, RealFn>(fun)(...)`.
        //   The `unsafe_ifunc!` instantiations at :180, :203, :227, :252,
        //   :278, :305, and :326 expand that arbitrary-expression callee.
        // Expected proof traversal: proof context preserves both generated
        // unsafe call_site rows and blocked external call_resolution facts,
        // while keeping the macro-loaded function pointer targetless.
        for site in [sites.0, sites.1] {
            assert_site_blocker_in_domain(
                &rows,
                owner,
                site,
                MEMCHR_DOMAIN,
                "external_dependency_summary_missing",
                case.label,
            );
            assert_unsafe_call_site(&rows, owner, site, case.label);
        }
    }

    Ok(())
}

fn generated_transmute_sites(
    calls: &[CallContextInfo],
    owner: Uuid,
    expected_arg_count: u32,
    label: &str,
) -> (Uuid, Uuid) {
    let callee = CallCalleeInfo::Path {
        path: ["core", "mem", "transmute"]
            .into_iter()
            .map(str::to_string)
            .collect(),
    };
    let path = calls
        .iter()
        .find(|call| {
            call.owner_id == owner
                && call.kind == CallSiteKind::Path
                && call.callee == callee
                && call.arg_count == Some(1)
                && call.generic_arg_count == Some(2)
                && call.status == CallStatusKind::External
                && call.resolution.is_none()
                && call.targets.is_empty()
        })
        .unwrap_or_else(|| {
            panic!("{label} should expose generated transmute path frontier: {calls:#?}")
        });
    let dynamic = calls
        .iter()
        .find(|call| {
            call.owner_id == owner
                && call.kind == CallSiteKind::Dynamic
                && call.callee == CallCalleeInfo::Dynamic
                && call.arg_count == Some(expected_arg_count)
                && call.status == CallStatusKind::External
                && call.resolution.is_none()
                && call.targets.is_empty()
                && call.path.as_ref().is_some_and(|path| {
                    path.iter()
                        .map(String::as_str)
                        .eq(["core", "mem", "transmute"])
                })
        })
        .unwrap_or_else(|| {
            panic!("{label} should expose generated returned-path dynamic frontier: {calls:#?}")
        });

    (path.site_id, dynamic.site_id)
}

fn assert_unsafe_call_site(rows: &[ProofContextInfo], owner: Uuid, site: Uuid, label: &str) {
    let owner = owner.to_string();
    let site = site.to_string();
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_site"
                && row.caller_def_id.as_deref() == Some(owner.as_str())
                && row.call_site_id.as_deref() == Some(site.as_str())
                && row.build_domain_id.as_deref() == Some(MEMCHR_DOMAIN)
                && row.unsafe_block == Some(true)
        }),
        "{label} proof context should preserve unsafe generated call_site fact: {rows:#?}"
    );
}
