use super::helpers::*;
use super::*;

#[test]
fn context_for_owner_decodes_structured_method_receivers() -> Result<(), DbError> {
    let cases = vec![
        structured_receiver(
            "SelfField",
            vec!["secret"],
            CallReceiver::SelfField {
                path: vec!["secret".to_string()],
            },
        ),
        structured_receiver(
            "LocalBinding",
            vec!["value"],
            CallReceiver::LocalBinding {
                name: "value".to_string(),
            },
        ),
        structured_receiver(
            "TypedLocalBinding",
            vec!["typed", "LocalAssoc"],
            CallReceiver::TypedLocalBinding {
                name: "typed".to_string(),
                type_path: vec!["LocalAssoc".to_string()],
            },
        ),
        structured_receiver(
            "InitializedLocalBinding",
            vec!["init", "LocalAssoc"],
            CallReceiver::InitializedLocalBinding {
                name: "init".to_string(),
                init_path: vec!["LocalAssoc".to_string()],
            },
        ),
        structured_receiver(
            "TupleReturnBinding",
            vec!["value", "0", "make_local_assoc_pair"],
            CallReceiver::TupleReturnBinding {
                name: "value".to_string(),
                path: vec!["make_local_assoc_pair".to_string()],
                index: 0,
            },
        ),
        structured_receiver(
            "TupleMethodReturn",
            vec!["next", "0", "tuple_pair", "40981", "40999"],
            CallReceiver::TupleMethodReturn {
                name: "next".to_string(),
                method_name: "tuple_pair".to_string(),
                method_span: (40981, 40999),
                index: 0,
            },
        ),
        structured_receiver(
            "MethodResultLocalBinding",
            vec!["iter", "into_iter", "44177", "44193"],
            CallReceiver::MethodResultLocalBinding {
                name: "iter".to_string(),
                method_name: "into_iter".to_string(),
                method_span: (44177, 44193),
            },
        ),
        structured_receiver(
            "BorrowedTypedLocalBinding",
            vec!["borrowed", "LocalAssoc"],
            CallReceiver::BorrowedTypedLocalBinding {
                name: "borrowed".to_string(),
                type_path: vec!["LocalAssoc".to_string()],
            },
        ),
        structured_receiver(
            "BorrowedInitializedLocalBinding",
            vec!["borrowed", "LocalAssoc"],
            CallReceiver::BorrowedInitializedLocalBinding {
                name: "borrowed".to_string(),
                init_path: vec!["LocalAssoc".to_string()],
            },
        ),
        structured_receiver(
            "DereferencedInitializedLocalBinding",
            vec!["deref", "LocalAssoc"],
            CallReceiver::DereferencedInitializedLocalBinding {
                name: "deref".to_string(),
                init_path: vec!["LocalAssoc".to_string()],
            },
        ),
        structured_receiver(
            "FieldInitializedLocalBinding",
            vec!["fielded", "TupleFieldMethodReceiver", "", "0"],
            CallReceiver::FieldInitializedLocalBinding {
                name: "fielded".to_string(),
                init_path: vec!["TupleFieldMethodReceiver".to_string()],
                field_path: vec!["0".to_string()],
            },
        ),
        structured_receiver(
            "AwaitPathCallResult",
            vec!["make_ready_local_assoc"],
            CallReceiver::AwaitPathCallResult {
                path: vec!["make_ready_local_assoc".to_string()],
            },
        ),
        structured_receiver(
            "AwaitMethodCallResult",
            vec!["acquire_owned"],
            CallReceiver::AwaitMethodCallResult {
                method_name: "acquire_owned".to_string(),
            },
        ),
        structured_receiver(
            "TryPathCallResult",
            vec!["try_local_assoc"],
            CallReceiver::TryPathCallResult {
                path: vec!["try_local_assoc".to_string()],
            },
        ),
        structured_receiver(
            "TryMethodCallResult",
            vec!["try_clone_assoc"],
            CallReceiver::TryMethodCallResult {
                method_name: "try_clone_assoc".to_string(),
            },
        ),
        structured_receiver(
            "IfBranchPaths",
            vec!["LocalAssoc", "", "LocalAssoc"],
            CallReceiver::IfBranchPaths {
                paths: vec![
                    vec!["LocalAssoc".to_string()],
                    vec!["LocalAssoc".to_string()],
                ],
            },
        ),
    ];

    assert_receiver_cases(&cases)
}
