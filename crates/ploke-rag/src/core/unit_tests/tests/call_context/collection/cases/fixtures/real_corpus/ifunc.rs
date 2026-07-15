use super::*;

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
async fn call_context_collection_reads_memchr_generated_transmute_frontiers() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_memchr_call_graph_rag()?;

    for case in IFUNC_CASES {
        let owner =
            function_id_by_name_in_module(&db, &["crate", "arch", "x86_64", "memchr"], case.owner)?;
        let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
        let context = call_context
            .get(&owner)
            .unwrap_or_else(|| panic!("{} should receive outgoing call context", case.label));
        let (path_row, dynamic_row) =
            generated_transmute_rows(context, owner, case.expected_arg_count, case.label);

        // Matrix:
        //   docs/active/agents/call-graph/
        //   2026-06-28_real-corpus-call-site-oracle-matrices.md
        //
        // Source chain:
        //   memchr/src/arch/x86_64/memchr.rs:153 generates
        //   `core::mem::transmute::<Fn, RealFn>(fun)(...)`.
        //   The `unsafe_ifunc!` instantiations at :180, :203, :227, :252,
        //   :278, :305, and :326 expand that arbitrary-expression callee.
        // Expected traversal: RAG preserves the generated inner external path
        // row and outer external returned-path dynamic row, but keeps both
        // targetless because there is no admitted function-pointer dispatch
        // proof for the macro-loaded pointer.
        assert_eq!(path_row.owner_id, owner);
        assert_eq!(path_row.arg_count, Some(1));
        assert_eq!(path_row.generic_arg_count, Some(2));
        assert_eq!(path_row.status, CallStatusKind::External);
        assert_eq!(path_row.resolution, None);
        assert!(
            path_row.targets.is_empty(),
            "{} generated transmute path row should stay targetless: {path_row:#?}",
            case.label
        );

        assert_eq!(dynamic_row.owner_id, owner);
        assert_eq!(dynamic_row.arg_count, Some(case.expected_arg_count));
        assert_eq!(dynamic_row.generic_arg_count, None);
        assert_eq!(dynamic_row.status, CallStatusKind::External);
        assert_eq!(dynamic_row.resolution, None);
        assert!(
            dynamic_row.targets.is_empty(),
            "{} generated returned-path dynamic row should stay targetless: {dynamic_row:#?}",
            case.label
        );
    }

    Ok(())
}

fn generated_transmute_rows<'a>(
    context: &'a [CallContextInfo],
    owner: Uuid,
    expected_arg_count: u32,
    label: &str,
) -> (&'a CallContextInfo, &'a CallContextInfo) {
    let callee = CallCalleeInfo::Path {
        path: path(&["core", "mem", "transmute"]),
    };
    let path_rows = context
        .iter()
        .filter(|call| call.owner_id == owner && call.kind == CallSiteKind::Path)
        .filter(|call| call.callee == callee)
        .collect::<Vec<_>>();
    assert_eq!(
        path_rows.len(),
        1,
        "{label} should expose one generated transmute path row: {context:#?}"
    );

    let dynamic_rows = context
        .iter()
        .filter(|call| {
            call.owner_id == owner
                && call.kind == CallSiteKind::Dynamic
                && call.callee == CallCalleeInfo::Dynamic
                && call.arg_count == Some(expected_arg_count)
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
        "{label} should expose one generated returned-path dynamic row: {context:#?}"
    );

    (path_rows[0], dynamic_rows[0])
}
