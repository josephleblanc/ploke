use super::super::super::super::*;
use super::super::helpers::*;
struct SyntheticCallCase<'a> {
    label: &'a str,
    site: Uuid,
    target: Uuid,
    call_kind: &'a str,
    span: (i64, i64),
    path: Option<&'a [&'a str]>,
    method: Option<&'a str>,
    receiver: Option<(&'a str, &'a [&'a str])>,
    arg_count: Option<i64>,
    generic_arg_count: Option<i64>,
    target_relation: &'a str,
    target_kind: &'a str,
    expected_kind: CallSiteKind,
    expected_callee: ExpectedCallee<'a>,
    expected_target_relation: CallTargetKind,
}
enum ExpectedCallee<'a> {
    Path(&'a [&'a str]),
    MethodLocal {
        name: &'a str,
        binding: &'a str,
    },
    MethodInitialized {
        name: &'a str,
        binding: &'a str,
        init_path: &'a [&'a str],
    },
    MethodTryPath {
        name: &'a str,
        path: &'a [&'a str],
    },
    Dynamic,
}
impl ExpectedCallee<'_> {
    fn to_info(&self) -> CallCalleeInfo {
        match self {
            ExpectedCallee::Path(path) => CallCalleeInfo::Path {
                path: strings(path),
            },
            ExpectedCallee::MethodLocal { name, binding } => CallCalleeInfo::Method {
                name: (*name).to_string(),
                receiver: Some(CallReceiverInfo::LocalBinding {
                    name: (*binding).to_string(),
                }),
            },
            ExpectedCallee::MethodInitialized {
                name,
                binding,
                init_path,
            } => CallCalleeInfo::Method {
                name: (*name).to_string(),
                receiver: Some(CallReceiverInfo::InitializedLocalBinding {
                    name: (*binding).to_string(),
                    init_path: strings(init_path),
                }),
            },
            ExpectedCallee::MethodTryPath { name, path } => CallCalleeInfo::Method {
                name: (*name).to_string(),
                receiver: Some(CallReceiverInfo::TryPathCallResult {
                    path: strings(path),
                }),
            },
            ExpectedCallee::Dynamic => CallCalleeInfo::Dynamic,
        }
    }
}
fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_string()).collect()
}
fn insert_synthetic_call(
    db: &Database,
    owner: Uuid,
    case: &SyntheticCallCase<'_>,
) -> Result<(), Error> {
    insert_call_site(
        db,
        CallSeed {
            id: case.site,
            owner,
            kind: case.call_kind,
            span: case.span,
            path: case.path.map(|path| path.to_vec()),
            method: case.method,
            macro_name: None,
            receiver: case.receiver.map(|(kind, path)| (kind, path.to_vec())),
            arg_count: case.arg_count,
            generic_arg_count: case.generic_arg_count,
        },
    )?;
    insert_call_edge(db, owner, case.site, case.call_kind)?;
    insert_call_target(
        db,
        case.site,
        case.target,
        case.target_relation,
        case.call_kind,
        case.target_kind,
    )?;
    insert_call_status(
        db,
        case.site,
        case.call_kind,
        "Resolved",
        Some("LocalExact"),
    )
}
#[tokio::test]
async fn call_context_collection_attaches_outgoing_call_payloads() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::init_with_schema()?);
    let owner = Uuid::from_u128(0x101);
    let cases = [
        SyntheticCallCase {
            label: "path function",
            site: Uuid::from_u128(0x102),
            target: Uuid::from_u128(0x103),
            call_kind: "Path",
            span: (12, 25),
            path: Some(&["crate", "helper"]),
            method: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: Some(0),
            target_relation: "Function",
            target_kind: "Function",
            expected_kind: CallSiteKind::Path,
            expected_callee: ExpectedCallee::Path(&["crate", "helper"]),
            expected_target_relation: CallTargetKind::Function,
        },
        SyntheticCallCase {
            label: "associated function",
            site: Uuid::from_u128(0x104),
            target: Uuid::from_u128(0x105),
            call_kind: "Path",
            span: (40, 58),
            path: Some(&["LocalAssoc", "make"]),
            method: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: Some(0),
            target_relation: "AssociatedFunction",
            target_kind: "Method",
            expected_kind: CallSiteKind::Path,
            expected_callee: ExpectedCallee::Path(&["LocalAssoc", "make"]),
            expected_target_relation: CallTargetKind::AssociatedFunction,
        },
        SyntheticCallCase {
            label: "tuple constructor",
            site: Uuid::from_u128(0x106),
            target: Uuid::from_u128(0x107),
            call_kind: "Path",
            span: (60, 77),
            path: Some(&["TupleStruct"]),
            method: None,
            receiver: None,
            arg_count: Some(2),
            generic_arg_count: Some(0),
            target_relation: "TupleStructConstructor",
            target_kind: "Struct",
            expected_kind: CallSiteKind::Path,
            expected_callee: ExpectedCallee::Path(&["TupleStruct"]),
            expected_target_relation: CallTargetKind::TupleStructConstructor,
        },
        SyntheticCallCase {
            label: "enum variant constructor",
            site: Uuid::from_u128(0x108),
            target: Uuid::from_u128(0x109),
            call_kind: "Path",
            span: (80, 105),
            path: Some(&["EnumWithData", "Variant1"]),
            method: None,
            receiver: None,
            arg_count: Some(1),
            generic_arg_count: Some(0),
            target_relation: "EnumVariantConstructor",
            target_kind: "Variant",
            expected_kind: CallSiteKind::Path,
            expected_callee: ExpectedCallee::Path(&["EnumWithData", "Variant1"]),
            expected_target_relation: CallTargetKind::EnumVariantConstructor,
        },
        SyntheticCallCase {
            label: "local receiver method",
            site: Uuid::from_u128(0x10a),
            target: Uuid::from_u128(0x10b),
            call_kind: "Method",
            span: (110, 132),
            path: None,
            method: Some("instance_value"),
            receiver: Some(("LocalBinding", &["value"])),
            arg_count: Some(0),
            generic_arg_count: Some(0),
            target_relation: "Method",
            target_kind: "Method",
            expected_kind: CallSiteKind::Method,
            expected_callee: ExpectedCallee::MethodLocal {
                name: "instance_value",
                binding: "value",
            },
            expected_target_relation: CallTargetKind::Method,
        },
        SyntheticCallCase {
            label: "initialized receiver method",
            site: Uuid::from_u128(0x10c),
            target: Uuid::from_u128(0x10d),
            call_kind: "Method",
            span: (134, 156),
            path: None,
            method: Some("instance_value"),
            receiver: Some(("InitializedLocalBinding", &["value", "LocalAssoc"])),
            arg_count: Some(0),
            generic_arg_count: Some(0),
            target_relation: "Method",
            target_kind: "Method",
            expected_kind: CallSiteKind::Method,
            expected_callee: ExpectedCallee::MethodInitialized {
                name: "instance_value",
                binding: "value",
                init_path: &["LocalAssoc"],
            },
            expected_target_relation: CallTargetKind::Method,
        },
        SyntheticCallCase {
            label: "try-path receiver method",
            site: Uuid::from_u128(0x10e),
            target: Uuid::from_u128(0x10f),
            call_kind: "Method",
            span: (158, 184),
            path: None,
            method: Some("instance_value"),
            receiver: Some(("TryPathCallResult", &["try_local_assoc"])),
            arg_count: Some(0),
            generic_arg_count: Some(0),
            target_relation: "Method",
            target_kind: "Method",
            expected_kind: CallSiteKind::Method,
            expected_callee: ExpectedCallee::MethodTryPath {
                name: "instance_value",
                path: &["try_local_assoc"],
            },
            expected_target_relation: CallTargetKind::Method,
        },
        SyntheticCallCase {
            label: "dynamic function",
            site: Uuid::from_u128(0x110),
            target: Uuid::from_u128(0x111),
            call_kind: "Dynamic",
            span: (186, 202),
            path: None,
            method: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: None,
            target_relation: "DynamicFunction",
            target_kind: "Function",
            expected_kind: CallSiteKind::Dynamic,
            expected_callee: ExpectedCallee::Dynamic,
            expected_target_relation: CallTargetKind::DynamicFunction,
        },
    ];

    for case in &cases {
        insert_synthetic_call(&db, owner, case)?;
    }

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.call_context_degraded(),
        "fresh call_graph schema should enable call context collection"
    );

    let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
    let owner_context = call_context
        .get(&owner)
        .expect("owner should receive outgoing call context");
    assert_eq!(owner_context.len(), cases.len());

    for (call, case) in owner_context.iter().zip(cases.iter()) {
        assert_eq!(call.site_id, case.site, "{} site id", case.label);
        assert_eq!(&call.kind, &case.expected_kind, "{} kind", case.label);
        assert_eq!(
            call.callee,
            case.expected_callee.to_info(),
            "{} callee",
            case.label
        );
        assert_eq!(
            call.status,
            CallStatusKind::Resolved,
            "{} status",
            case.label
        );
        assert_eq!(
            call.resolution,
            Some(CallResolutionKind::LocalExact),
            "{} resolution",
            case.label
        );
        assert_eq!(call.targets.len(), 1, "{} targets", case.label);
        assert_eq!(
            call.targets[0].target_id, case.target,
            "{} target id",
            case.label
        );
        assert_eq!(
            &call.targets[0].relation, &case.expected_target_relation,
            "{} target relation",
            case.label
        );
    }

    Ok(())
}
