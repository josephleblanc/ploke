use super::*;

enum ReceiverShape {
    Structured {
        kind: &'static str,
        parts: Vec<&'static str>,
    },
    Raw {
        kind: &'static str,
        path: DataValue,
    },
}

struct ReceiverCase {
    seed: ReceiverShape,
    expected: CallReceiver,
}

fn structured_receiver(
    kind: &'static str,
    parts: Vec<&'static str>,
    expected: CallReceiver,
) -> ReceiverCase {
    ReceiverCase {
        seed: ReceiverShape::Structured { kind, parts },
        expected,
    }
}

fn raw_receiver(kind: &'static str, path: DataValue, expected: CallReceiver) -> ReceiverCase {
    ReceiverCase {
        seed: ReceiverShape::Raw { kind, path },
        expected,
    }
}

#[test]
fn context_for_owner_decodes_method_receivers() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(0x231);
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

    for (idx, case) in cases.iter().enumerate() {
        let site = Uuid::from_u128(0x240 + idx as u128);
        let target = Uuid::from_u128(0x280 + idx as u128);
        let span_start = 200 + idx as i64 * 10;

        match &case.seed {
            ReceiverShape::Structured { kind, parts } => {
                insert_call_site(
                    &db,
                    SiteSeed {
                        id: site,
                        owner,
                        kind: "Method",
                        span: (span_start, span_start + 9),
                        path: None,
                        method: Some("instance_value"),
                        macro_name: None,
                        receiver: Some((*kind, parts.clone())),
                        arg_count: Some(0),
                        generic_arg_count: Some(0),
                    },
                )?;
            }
            ReceiverShape::Raw { kind, path } => {
                insert_call_site_raw_receiver(
                    &db,
                    SiteSeed {
                        id: site,
                        owner,
                        kind: "Method",
                        span: (span_start, span_start + 9),
                        path: None,
                        method: Some("instance_value"),
                        macro_name: None,
                        receiver: None,
                        arg_count: Some(0),
                        generic_arg_count: Some(0),
                    },
                    DataValue::from(*kind),
                    path.clone(),
                )?;
            }
        }
        insert_edge(&db, owner, site, "Method")?;
        insert_relation(&db, site, target, "Method", "Method", "Method")?;
        insert_status(&db, site, "Method", "Resolved", Some("LocalExact"))?;
    }

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), cases.len(), "context rows: {context:#?}");
    for (idx, case) in cases.iter().enumerate() {
        assert_eq!(context[idx].site.method.as_deref(), Some("instance_value"));
        assert_eq!(context[idx].site.receiver.as_ref(), Some(&case.expected));
        assert_eq!(context[idx].targets[0].relation, CallRelationKind::Method);
    }

    Ok(())
}
