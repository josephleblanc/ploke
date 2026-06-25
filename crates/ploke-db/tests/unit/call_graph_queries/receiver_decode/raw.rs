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
        raw_receiver("AwaitResult", DataValue::Null, CallReceiver::AwaitResult),
        raw_receiver("TryResult", DataValue::Null, CallReceiver::TryResult),
        raw_receiver("Literal", DataValue::Null, CallReceiver::Literal),
    ];

    assert_receiver_cases(&cases)
}
