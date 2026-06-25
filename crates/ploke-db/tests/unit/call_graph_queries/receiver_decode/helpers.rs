use super::*;

pub(super) enum ReceiverShape {
    Structured {
        kind: &'static str,
        parts: Vec<&'static str>,
    },
    Raw {
        kind: &'static str,
        path: DataValue,
    },
}

pub(super) struct ReceiverCase {
    seed: ReceiverShape,
    expected: CallReceiver,
}

pub(super) fn structured_receiver(
    kind: &'static str,
    parts: Vec<&'static str>,
    expected: CallReceiver,
) -> ReceiverCase {
    ReceiverCase {
        seed: ReceiverShape::Structured { kind, parts },
        expected,
    }
}

pub(super) fn raw_receiver(
    kind: &'static str,
    path: DataValue,
    expected: CallReceiver,
) -> ReceiverCase {
    ReceiverCase {
        seed: ReceiverShape::Raw { kind, path },
        expected,
    }
}

pub(super) fn assert_receiver_cases(cases: &[ReceiverCase]) -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(0x231);

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
