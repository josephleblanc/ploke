use super::helpers::*;
use super::*;

#[test]
fn context_for_owner_decodes_raw_method_receivers() -> Result<(), DbError> {
    let cases = vec![
        raw_receiver("SelfValue", DataValue::Null, CallReceiver::SelfValue),
        raw_receiver(
            "BorrowedLocalBinding",
            list(&["borrowed"]),
            CallReceiver::BorrowedLocalBinding {
                name: "borrowed".to_string(),
            },
        ),
        raw_receiver(
            "DereferencedLocalBinding",
            list(&["deref"]),
            CallReceiver::DereferencedLocalBinding {
                name: "deref".to_string(),
            },
        ),
        raw_receiver(
            "FieldLocalBinding",
            list(&["fielded", "inner"]),
            CallReceiver::FieldLocalBinding {
                name: "fielded".to_string(),
                field_path: vec!["inner".to_string()],
            },
        ),
        raw_receiver(
            "FieldTypedLocalBinding",
            list(&["fielded", "Holder", "", "inner"]),
            CallReceiver::FieldTypedLocalBinding {
                name: "fielded".to_string(),
                type_path: vec!["Holder".to_string()],
                field_path: vec!["inner".to_string()],
            },
        ),
        raw_receiver(
            "TupleReturnBinding",
            list(&["value", "0", "make_local_assoc_pair"]),
            CallReceiver::TupleReturnBinding {
                name: "value".to_string(),
                path: vec!["make_local_assoc_pair".to_string()],
                index: 0,
            },
        ),
        raw_receiver(
            "TupleMethodReturn",
            list(&["next", "0", "tuple_pair", "40981", "40999"]),
            CallReceiver::TupleMethodReturn {
                name: "next".to_string(),
                method_name: "tuple_pair".to_string(),
                method_span: (40981, 40999),
                index: 0,
            },
        ),
        raw_receiver(
            "MethodResultLocalBinding",
            list(&["iter", "into_iter", "44177", "44193"]),
            CallReceiver::MethodResultLocalBinding {
                name: "iter".to_string(),
                method_name: "into_iter".to_string(),
                method_span: (44177, 44193),
            },
        ),
        raw_receiver(
            "PathCallResult",
            list(&["make_local_assoc"]),
            CallReceiver::PathCallResult {
                path: vec!["make_local_assoc".to_string()],
            },
        ),
        raw_receiver(
            "MethodCallResult",
            list(&["clone_assoc"]),
            CallReceiver::MethodCallResult {
                method_name: "clone_assoc".to_string(),
            },
        ),
        raw_receiver(
            "TryMethodCallResult",
            list(&["try_clone_assoc"]),
            CallReceiver::TryMethodCallResult {
                method_name: "try_clone_assoc".to_string(),
            },
        ),
        raw_receiver(
            "AwaitMethodCallResult",
            list(&["send"]),
            CallReceiver::AwaitMethodCallResult {
                method_name: "send".to_string(),
            },
        ),
        raw_receiver(
            "IfBranchPaths",
            list(&["LocalAssoc", "", "LocalAssoc"]),
            CallReceiver::IfBranchPaths {
                paths: vec![
                    vec!["LocalAssoc".to_string()],
                    vec!["LocalAssoc".to_string()],
                ],
            },
        ),
        raw_receiver("AwaitResult", DataValue::Null, CallReceiver::AwaitResult),
        raw_receiver("TryResult", DataValue::Null, CallReceiver::TryResult),
        raw_receiver("Literal", DataValue::Null, CallReceiver::Literal),
        raw_receiver("Unsupported", DataValue::Null, CallReceiver::Unsupported),
    ];

    assert_receiver_cases(&cases)
}
