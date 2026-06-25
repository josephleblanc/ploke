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
            "BorrowedTypedLocalBinding",
            vec!["borrowed", "LocalAssoc"],
            CallReceiver::BorrowedTypedLocalBinding {
                name: "borrowed".to_string(),
                type_path: vec!["LocalAssoc".to_string()],
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
            "TryPathCallResult",
            vec!["try_local_assoc"],
            CallReceiver::TryPathCallResult {
                path: vec!["try_local_assoc".to_string()],
            },
        ),
    ];

    assert_receiver_cases(&cases)
}
