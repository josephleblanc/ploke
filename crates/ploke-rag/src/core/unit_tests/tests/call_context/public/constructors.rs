use super::super::super::*;

#[cfg(feature = "call_graph")]
#[tokio::test]
async fn call_context_sparse_get_context_expands_constructor_target_hits_to_fixture_callers()
-> Result<(), Error> {
    init_tracing_once();

    struct Case<'a> {
        label: &'a str,
        fixture: &'a str,
        query: &'a str,
        top_k: usize,
        target: ConstructorTarget<'a>,
        owner_module: &'a [&'a str],
        owner: &'a str,
        path: &'a [&'a str],
        relation: CallTargetKind,
    }

    enum ConstructorTarget<'a> {
        Struct {
            module: &'a [&'a str],
            name: &'a str,
        },
        Variant {
            enum_name: &'a str,
            name: &'a str,
        },
    }

    let cases = [
        Case {
            label: "tuple-struct constructor",
            fixture: "fixture_call_graph",
            query: "pub struct NewType",
            top_k: 1,
            target: ConstructorTarget::Struct {
                module: &["crate"],
                name: "NewType",
            },
            owner_module: &["crate"],
            owner: "call_new_type_constructor",
            path: &["NewType"],
            relation: CallTargetKind::TupleStructConstructor,
        },
        Case {
            label: "enum-variant constructor",
            fixture: "fixture_nodes",
            query: "Variant1",
            top_k: 10,
            target: ConstructorTarget::Variant {
                enum_name: "EnumWithData",
                name: "Variant1",
            },
            owner_module: &["crate", "imports"],
            owner: "use_imported_items",
            path: &["EnumWithData", "Variant1"],
            relation: CallTargetKind::EnumVariantConstructor,
        },
    ];

    for case in cases {
        let db = Arc::new(Database::new(setup_db_full_multi_embedding(case.fixture)?));
        let target = match case.target {
            ConstructorTarget::Struct { module, name } => {
                one_uuid(&db, &struct_in_module_query(module, name))?
            }
            ConstructorTarget::Variant { enum_name, name } => {
                one_uuid(&db, &variant_by_enum_query(enum_name, name))?
            }
        };
        let owner = one_uuid(
            &db,
            &function_in_module_query(case.owner_module, case.owner),
        )?;

        let mut cfg = crate::RagConfig::default();
        cfg.type_context.enabled = false;
        cfg.call_context.max_owner_hits = 64;
        cfg.call_context.max_caller_hits = 64;
        let rag = RagService::new_full(
            Arc::clone(&db),
            runtime_for(&db, EmbeddingProcessor::new_mock()),
            IoManagerHandle::new(),
            cfg,
        )?;
        assert!(
            !rag.call_context_degraded(),
            "fresh {} call_graph schema should enable public {} expansion",
            case.fixture,
            case.label
        );

        rag.bm25_rebuild().await?;
        let sparse_hits = rag
            .search_bm25_strict(case.query, case.top_k, LOADED_WORKSPACE_SCOPE)
            .await?;
        assert!(
            sparse_hits.iter().any(|(hit, _)| *hit == target),
            "{} query should seed get_context with the constructor target; hits: {sparse_hits:#?}",
            case.label
        );

        let assembled = rag
            .get_context(
                case.query,
                case.top_k,
                &TokenBudget {
                    max_total: 20_000,
                    per_file_max: 20_000,
                    per_part_max: 4_096,
                },
                &RetrievalStrategy::Sparse { strict: Some(true) },
                LOADED_WORKSPACE_SCOPE,
            )
            .await?;

        assert!(
            assembled.parts.iter().any(|part| part.id == target),
            "public get_context should materialize the {} target seed",
            case.label
        );

        let caller_part = assembled
            .parts
            .iter()
            .find(|part| part.id == owner)
            .unwrap_or_else(|| {
                panic!(
                    "public get_context should materialize the {} caller owner",
                    case.label
                )
            });
        let expected_path = case
            .path
            .iter()
            .map(|segment| (*segment).to_string())
            .collect::<Vec<_>>();
        let call = caller_part
            .call_context
            .iter()
            .find(|call| {
                call.kind == CallSiteKind::Path
                    && call.callee
                        == CallCalleeInfo::Path {
                            path: expected_path.clone(),
                        }
                    && call
                        .targets
                        .iter()
                        .any(|target_info| target_info.target_id == target)
            })
            .unwrap_or_else(|| {
                panic!(
                    "caller part should retain outgoing {} context to the seed target",
                    case.label
                )
            });
        assert_eq!(call.status, CallStatusKind::Resolved);
        assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(call.targets.len(), 1);
        assert_eq!(call.targets[0].target_id, target);
        assert_eq!(call.targets[0].relation, case.relation);
        assert_incoming_expansion(caller_part, call, target);
    }

    Ok(())
}
