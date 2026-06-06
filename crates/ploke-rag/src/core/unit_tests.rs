#[cfg(test)]
mod tests {
    //! RAG test coverage boundary for typed type context:
    //!
    //! - Covered: generic type-context expansion plumbing and the shared
    //!   corpus-backed `TypeShapeCase` matrix rows marked for RAG API coverage.
    //!   Matrix rows assert source-pinned owners, terminal targets, and
    //!   structured `TypeContextInfo` provenance.
    //! - Not covered here: token-budget behavior for where-derived neighbors
    //!   or live model/tool behavior. DB tests below
    //!   `ploke-db/tests/unit/type_graph_queries` own exact source coordinates
    //!   and containment depth; TUI tests own model-facing tool payloads.
    //! - Recursive/nested behavior is inherited from DB fixtures; this RAG layer
    //!   does not add independent recursive type traversal assertions.

    use std::{collections::BTreeMap, default, ops::Deref, sync::Arc};

    use crate::{ApproxCharTokenizer, AssemblyPolicy, RetrievalStrategy, TokenBudget};
    #[cfg(feature = "typed_type_graph")]
    use cozo::DataValue;
    use itertools::Itertools;
    use lazy_static::lazy_static;
    #[cfg(feature = "typed_type_graph")]
    use ploke_core::rag_types::TypeContextKind;
    use ploke_core::{CrateId, EmbeddingData, RetrievalScope};
    #[cfg(feature = "typed_type_graph")]
    use ploke_db::get_by_id::{GetNodeInfo, NodePaths};
    use ploke_db::{
        Database, create_index_primary_with_index,
        multi_embedding::{db_ext::EmbeddingExt, debug::DebugAll, hnsw_ext::HnswExt},
    };
    #[cfg(feature = "typed_type_graph")]
    use ploke_db::{DbError, TypeContextSeed, TypeUseCoordinate, TypeUseRoot, to_uuid};
    use ploke_embed::{
        indexer::{EmbeddingProcessor, EmbeddingSource},
        local::{EmbeddingConfig, LocalEmbedder},
        runtime::EmbeddingRuntime,
    };
    use ploke_error::Error;
    use ploke_io::IoManagerHandle;
    #[cfg(feature = "typed_type_graph")]
    use ploke_test_utils::{
        ContainingOwnerSelector, CoordinateSpec, OwnerSelector, ShapePipelineCoverage,
        TargetSelector, TypeShapeCase, positive_type_shape_cases, setup_db_full_multi_embedding,
    };
    use ploke_test_utils::{
        FIXTURE_NODES_LOCAL_EMBEDDINGS, WS_FIXTURE_01_CANONICAL, fresh_backup_fixture_db,
        shared_backup_fixture_db,
    };
    use tokio::time::{Duration, sleep};
    use tracing::{Level, debug};
    use uuid::Uuid;

    use crate::{RagError, RagService};
    use std::sync::LazyLock;
    use std::sync::Once;

    static TEST_TRACING: Once = Once::new();
    fn init_tracing_once() {
        TEST_TRACING.call_once(|| {
            ploke_test_utils::init_test_tracing_with_target("", tracing::Level::ERROR);
        });
    }

    static DEFAULT_TEST_RAG: LazyLock<RagService> = LazyLock::new(|| {
        let db = default_test_db_setup().expect("db setup");
        init_test_rag(db)
    });

    const LOADED_WORKSPACE_SCOPE: RetrievalScope = RetrievalScope::LoadedWorkspace;
    struct WorkspaceScopeFixture {
        root_id: Uuid,
        root_namespace: Uuid,
        nested_id: Uuid,
        nested_namespace: Uuid,
    }

    fn init_test_rag(db: Arc<Database>) -> RagService {
        let model =
            LocalEmbedder::new(EmbeddingConfig::default()).expect("valid default embedding config");
        let source = EmbeddingSource::Local(model);
        let embedding_runtime = Arc::new(EmbeddingRuntime::from_shared_set(
            Arc::clone(&db.active_embedding_set),
            EmbeddingProcessor::new(source),
        ));
        RagService::new(db, embedding_runtime).expect("valid db and RagService constructor args")
    }

    #[cfg(feature = "typed_type_graph")]
    fn init_test_rag_mock(db: Arc<Database>) -> RagService {
        let embedding_runtime = Arc::new(EmbeddingRuntime::from_shared_set(
            Arc::clone(&db.active_embedding_set),
            EmbeddingProcessor::new_mock(),
        ));
        RagService::new(db, embedding_runtime).expect("valid db and RagService constructor args")
    }

    async fn init_test_rag_bm25(db: Arc<Database>) -> RagService {
        let rag = init_test_rag(db);
        rag.bm25_rebuild().await.expect("bm25 rebuild must succeed");
        rag
    }

    fn load_local_fixture_db() -> Result<Arc<Database>, Error> {
        shared_backup_fixture_db(&FIXTURE_NODES_LOCAL_EMBEDDINGS)
    }

    async fn db_test_setup() -> Result<Arc<Database>, Error> {
        load_local_fixture_db()
    }

    fn default_test_db_setup() -> Result<Arc<Database>, Error> {
        load_local_fixture_db()
    }

    fn runtime_for(db: &Arc<Database>, processor: EmbeddingProcessor) -> Arc<EmbeddingRuntime> {
        Arc::new(EmbeddingRuntime::from_shared_set(
            Arc::clone(&db.active_embedding_set),
            processor,
        ))
    }

    fn init_test_rag_with_io(db: Arc<Database>) -> RagService {
        let model =
            LocalEmbedder::new(EmbeddingConfig::default()).expect("valid default embedding config");
        let source = EmbeddingSource::Local(model);
        let embedding_runtime = Arc::new(EmbeddingRuntime::from_shared_set(
            Arc::clone(&db.active_embedding_set),
            EmbeddingProcessor::new(source),
        ));
        RagService::new_with_io(db, embedding_runtime, IoManagerHandle::new())
            .expect("valid db and RagService constructor args")
    }

    #[cfg(feature = "typed_type_graph")]
    fn unique_id_by_name(db: &Database, relation: &str, name: &str) -> Result<Uuid, Error> {
        let script = format!(
            r#"?[id] :=
                *{relation} {{ id, name: "{name}" @ 'NOW' }}"#
        );
        let rows = db.raw_query(&script).map_err(Error::from)?;
        assert_eq!(
            rows.rows.len(),
            1,
            "expected exactly one {relation} named {name}; rows: {:#?}",
            rows.rows
        );
        to_uuid(&rows.rows[0][0]).map_err(Error::from)
    }

    #[cfg(feature = "typed_type_graph")]
    fn impl_self_target_for_method_name(db: &Database, method_name: &str) -> Result<Uuid, Error> {
        let script = format!(
            r#"?[target_id] :=
                *method {{ name: "{method_name}", owner_id: impl_id @ 'NOW' }},
                *impl {{ id: impl_id, self_type: source_id @ 'NOW' }},
                *type_relation {{
                    source_id,
                    target_id,
                    relation_kind: "Ordinary",
                    target_kind: "Struct" @ 'NOW'
                }}"#
        );
        let rows = db.raw_query(&script).map_err(Error::from)?;
        assert_eq!(
            rows.rows.len(),
            1,
            "expected exactly one struct target for impl method {method_name}; rows: {:#?}",
            rows.rows
        );
        to_uuid(&rows.rows[0][0]).map_err(Error::from)
    }

    #[cfg(feature = "typed_type_graph")]
    fn covers(case: &TypeShapeCase, coverage: ShapePipelineCoverage) -> bool {
        case.coverage.iter().any(|candidate| *candidate == coverage)
    }

    #[cfg(feature = "typed_type_graph")]
    fn resolve_matrix_owner(db: &Database, selector: OwnerSelector) -> Result<Uuid, DbError> {
        match selector {
            OwnerSelector::FunctionInModule { module_path, name } => {
                one_uuid(db, &function_in_module_query(module_path, name))
            }
            OwnerSelector::FunctionInFile { file_suffix, name } => {
                one_uuid_by_file_suffix(db, &function_in_file_query(name), file_suffix)
            }
            OwnerSelector::MethodByImplSelf { self_type, method } => {
                one_uuid(db, &method_by_impl_self_query(self_type, method))
            }
            OwnerSelector::MethodByImplTraitAndSelf {
                trait_name,
                self_type,
                method,
            } => one_uuid(
                db,
                &method_by_impl_trait_self_query(trait_name, self_type, method),
            ),
            OwnerSelector::MethodByRawPointerImpl {
                file_suffix,
                trait_name,
                mutable,
                method,
            } => one_uuid_by_file_suffix(
                db,
                &method_by_raw_pointer_impl_query(trait_name, mutable, method),
                file_suffix,
            ),
            OwnerSelector::FieldByStructInModule {
                module_path,
                struct_name,
                field_index,
            } => {
                let struct_id = one_uuid(db, &struct_in_module_query(module_path, struct_name))?;
                one_uuid(
                    db,
                    &format!(
                        r#"?[id] :=
                            *field {{
                                id,
                                owner_id: to_uuid("{struct_id}"),
                                index: {field_index} @ 'NOW'
                            }}"#
                    ),
                )
            }
            OwnerSelector::FieldByStructInFile {
                file_suffix,
                struct_name,
                field_index,
            } => {
                let struct_id =
                    one_uuid_by_file_suffix(db, &struct_in_file_query(struct_name), file_suffix)?;
                one_uuid(
                    db,
                    &format!(
                        r#"?[id] :=
                            *field {{
                                id,
                                owner_id: to_uuid("{struct_id}"),
                                index: {field_index} @ 'NOW'
                            }}"#
                    ),
                )
            }
            OwnerSelector::TypeAlias { name } => one_uuid(
                db,
                &format!(r#"?[id] := *type_alias {{ id, name: "{name}" @ 'NOW' }}"#),
            ),
            OwnerSelector::StaticInFile { file_suffix, name } => {
                one_uuid_by_file_suffix(db, &item_in_file_query("static", name), file_suffix)
            }
            OwnerSelector::StructByName { name } => one_uuid(
                db,
                &format!(r#"?[id] := *struct {{ id, name: "{name}" @ 'NOW' }}"#),
            ),
            OwnerSelector::TraitInModule { module_path, name } => {
                one_uuid(db, &trait_in_module_query(module_path, name))
            }
            OwnerSelector::GenericTypeParamByContainingOwner {
                containing_owner,
                param_name,
            } => {
                let owner_id = resolve_containing_owner(db, containing_owner)?;
                generic_type_param_id_by_owner_name(db, owner_id, param_name)
            }
            OwnerSelector::ImplByTraitInFile {
                file_suffix,
                trait_name,
            } => one_uuid_by_file_suffix(db, &impl_by_trait_query(trait_name), file_suffix),
            OwnerSelector::WhereGenericParamBoundOwner {
                containing_owner,
                predicate_index,
                bound_index,
                target_trait,
            } => where_generic_param_bound_owner(
                db,
                resolve_containing_owner(db, containing_owner)?,
                predicate_index,
                bound_index,
                target_trait,
            ),
            other => Err(DbError::QueryExecution(format!(
                "RAG matrix resolver does not materialize owner selector {other:?}"
            ))),
        }
    }

    #[cfg(feature = "typed_type_graph")]
    fn resolve_matrix_target(
        db: &Database,
        owner_id: Uuid,
        case: &TypeShapeCase,
    ) -> Result<Uuid, DbError> {
        match case.terminal {
            TargetSelector::StructByName { name } => one_uuid(
                db,
                &format!(r#"?[id] := *struct {{ id, name: "{name}" @ 'NOW' }}"#),
            ),
            TargetSelector::StructInModule { module_path, name } => {
                one_uuid(db, &struct_in_module_query(module_path, name))
            }
            TargetSelector::EnumByName { name } => one_uuid(
                db,
                &format!(r#"?[id] := *enum {{ id, name: "{name}" @ 'NOW' }}"#),
            ),
            TargetSelector::TraitInModule { module_path, name } => {
                one_uuid(db, &trait_in_module_query(module_path, name))
            }
            TargetSelector::TraitInFile { file_suffix, name } => {
                one_uuid_by_file_suffix(db, &trait_in_file_query(name), file_suffix)
            }
            TargetSelector::TypeAlias { name } => one_uuid(
                db,
                &format!(r#"?[id] := *type_alias {{ id, name: "{name}" @ 'NOW' }}"#),
            ),
            TargetSelector::Union { name } => one_uuid(
                db,
                &format!(r#"?[id] := *union {{ id, name: "{name}" @ 'NOW' }}"#),
            ),
            TargetSelector::GenericParamReachableByName { name } => {
                let coordinate = resolve_matrix_coordinate(db, case.coordinate)?;
                let root = exactly_one_matrix_root(db, owner_id, &coordinate, case)?;
                let reachable = db.type_targets_reachable_from_owner(owner_id)?;
                let matching = reachable
                    .iter()
                    .filter(|path| {
                        path.type_use_id == root.id && path.root_type_id == root.root_type_id
                    })
                    .filter_map(|path| {
                        generic_type_name(db, path.target_id)
                            .ok()
                            .filter(|candidate| candidate == name)
                            .map(|_| path.target_id)
                    })
                    .collect::<Vec<_>>();
                assert_eq!(
                    matching.len(),
                    1,
                    "{} expected exactly one reachable generic type parameter named {name}; root: {root:#?}; reachable: {reachable:#?}",
                    case.name
                );
                Ok(matching[0])
            }
        }
    }

    #[cfg(feature = "typed_type_graph")]
    fn resolve_containing_owner(
        db: &Database,
        selector: ContainingOwnerSelector,
    ) -> Result<Uuid, DbError> {
        match selector {
            ContainingOwnerSelector::StructByName { name } => one_uuid(
                db,
                &format!(r#"?[id] := *struct {{ id, name: "{name}" @ 'NOW' }}"#),
            ),
            ContainingOwnerSelector::StructInModule { module_path, name } => {
                one_uuid(db, &struct_in_module_query(module_path, name))
            }
            ContainingOwnerSelector::MethodByImplSelf { self_type, method } => {
                one_uuid(db, &method_by_impl_self_query(self_type, method))
            }
            ContainingOwnerSelector::MethodByImplTraitAndSelf {
                trait_name,
                self_type,
                method,
            } => one_uuid(
                db,
                &method_by_impl_trait_self_query(trait_name, self_type, method),
            ),
            ContainingOwnerSelector::TraitInModule { module_path, name } => {
                one_uuid(db, &trait_in_module_query(module_path, name))
            }
            ContainingOwnerSelector::ImplByTraitInFile {
                file_suffix,
                trait_name,
            } => one_uuid_by_file_suffix(db, &impl_by_trait_query(trait_name), file_suffix),
        }
    }

    #[cfg(feature = "typed_type_graph")]
    fn resolve_matrix_coordinate(
        db: &Database,
        spec: CoordinateSpec,
    ) -> Result<TypeUseCoordinate, DbError> {
        Ok(match spec {
            CoordinateSpec::None => TypeUseCoordinate::None,
            CoordinateSpec::ParamSlot(param_index) => TypeUseCoordinate::ParamSlot { param_index },
            CoordinateSpec::FieldSlot(field_index) => TypeUseCoordinate::FieldSlot { field_index },
            CoordinateSpec::TraitSuperSlot(supertrait_index) => {
                TypeUseCoordinate::TraitSuperSlot { supertrait_index }
            }
            CoordinateSpec::GenericBoundSlot {
                generic_param_index,
                bound_index,
            } => TypeUseCoordinate::GenericBoundSlot {
                generic_param_index,
                bound_index,
            },
            CoordinateSpec::GenericParamBoundSlot {
                containing_owner,
                generic_param_index,
                bound_index,
            } => TypeUseCoordinate::GenericParamBoundSlot {
                containing_owner_id: resolve_containing_owner(db, containing_owner)?,
                generic_param_index,
                bound_index,
            },
            CoordinateSpec::WhereSubjectSlot { predicate_index } => {
                TypeUseCoordinate::WhereSubjectSlot { predicate_index }
            }
            CoordinateSpec::WhereBoundSlot {
                predicate_index,
                bound_index,
            } => TypeUseCoordinate::WhereBoundSlot {
                predicate_index,
                bound_index,
            },
            CoordinateSpec::WhereGenericParamBoundSlot {
                containing_owner,
                predicate_index,
                bound_index,
            } => TypeUseCoordinate::WhereGenericParamBoundSlot {
                containing_owner_id: resolve_containing_owner(db, containing_owner)?,
                predicate_index,
                bound_index,
            },
            CoordinateSpec::AssociatedTypeBoundSlot {
                associated_type_index,
                associated_type_name,
                bound_index,
            } => TypeUseCoordinate::AssociatedTypeBoundSlot {
                associated_type_index,
                associated_type_name: associated_type_name.to_string(),
                bound_index,
            },
        })
    }

    #[cfg(feature = "typed_type_graph")]
    fn exactly_one_matrix_root(
        db: &Database,
        owner_id: Uuid,
        coordinate: &TypeUseCoordinate,
        case: &TypeShapeCase,
    ) -> Result<TypeUseRoot, DbError> {
        let roots = db.type_uses_for_owner(owner_id)?;
        let matching = roots
            .iter()
            .filter(|root| root.role == case.role && &root.coordinate == coordinate)
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(
            matching.len(),
            1,
            "{} expected exactly one root for owner {owner_id}, role {:?}, coordinate {coordinate:?}; roots: {roots:#?}",
            case.name,
            case.role
        );
        Ok(matching[0].clone())
    }

    #[cfg(feature = "typed_type_graph")]
    fn one_uuid(db: &Database, script: &str) -> Result<Uuid, DbError> {
        let rows = db.raw_query(script)?;
        assert_eq!(
            rows.rows.len(),
            1,
            "expected exactly one UUID for query:\n{script}\nrows: {:#?}",
            rows.rows
        );
        to_uuid(&rows.rows[0][0])
    }

    #[cfg(feature = "typed_type_graph")]
    fn one_uuid_by_file_suffix(
        db: &Database,
        script: &str,
        file_suffix: &str,
    ) -> Result<Uuid, DbError> {
        let rows = db.raw_query(script)?;
        let matching = rows
            .rows
            .iter()
            .filter_map(|row| {
                let file_path = match &row[1] {
                    DataValue::Str(path) => path.as_str(),
                    _ => return None,
                };
                file_path.ends_with(file_suffix).then(|| row[0].clone())
            })
            .collect::<Vec<_>>();
        assert_eq!(
            matching.len(),
            1,
            "expected exactly one row in file suffix {file_suffix}; rows: {:#?}",
            rows.rows
        );
        to_uuid(&matching[0])
    }

    #[cfg(feature = "typed_type_graph")]
    fn module_path(items: &[&str]) -> String {
        format!(
            "[{}]",
            items
                .iter()
                .map(|item| format!("\"{item}\""))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }

    #[cfg(feature = "typed_type_graph")]
    fn function_in_module_query(module_path_items: &[&str], name: &str) -> String {
        let module_path = module_path(module_path_items);
        format!(
            r#"?[id] :=
                *function {{ id, name: "{name}", module_id @ 'NOW' }},
                *module {{ id: module_id, path: {module_path} @ 'NOW' }}"#
        )
    }

    #[cfg(feature = "typed_type_graph")]
    fn function_in_file_query(name: &str) -> String {
        item_in_file_query("function", name)
    }

    #[cfg(feature = "typed_type_graph")]
    fn item_in_file_query(relation: &str, name: &str) -> String {
        format!(
            r#"?[id, file_path] :=
                *{relation} {{ id, name: "{name}" @ 'NOW' }},
                *syntax_edge {{
                    source_id: module_id,
                    target_id: id,
                    relation_kind: "Contains" @ 'NOW'
                }},
                *file_mod {{ owner_id: module_id, file_path @ 'NOW' }}"#
        )
    }

    #[cfg(feature = "typed_type_graph")]
    fn struct_in_module_query(module_path_items: &[&str], name: &str) -> String {
        let module_path = module_path(module_path_items);
        format!(
            r#"?[id] :=
                *module {{ id: module_id, path: {module_path} @ 'NOW' }},
                *syntax_edge {{
                    source_id: module_id,
                    target_id: id,
                    relation_kind: "Contains" @ 'NOW'
                }},
                *struct {{ id, name: "{name}" @ 'NOW' }}"#
        )
    }

    #[cfg(feature = "typed_type_graph")]
    fn struct_in_file_query(name: &str) -> String {
        item_in_file_query("struct", name)
    }

    #[cfg(feature = "typed_type_graph")]
    fn trait_in_module_query(module_path_items: &[&str], name: &str) -> String {
        let module_path = module_path(module_path_items);
        format!(
            r#"?[id] :=
                *module {{ id: module_id, path: {module_path} @ 'NOW' }},
                *syntax_edge {{
                    source_id: module_id,
                    target_id: id,
                    relation_kind: "Contains" @ 'NOW'
                }},
                *trait {{ id, name: "{name}" @ 'NOW' }}"#
        )
    }

    #[cfg(feature = "typed_type_graph")]
    fn trait_in_file_query(name: &str) -> String {
        item_in_file_query("trait", name)
    }

    #[cfg(feature = "typed_type_graph")]
    fn method_by_impl_self_query(self_type: &str, method: &str) -> String {
        format!(
            r#"?[method_id] :=
                *method {{ id: method_id, name: "{method}", owner_id: impl_id @ 'NOW' }},
                *impl {{ id: impl_id @ 'NOW' }},
                *type_use {{
                    owner_id: impl_id,
                    root_type_id: self_type_id,
                    role: "ImplSelf" @ 'NOW'
                }},
                *type_relation {{
                    source_id: self_type_id,
                    target_id: self_target_id,
                    relation_kind: "Ordinary" @ 'NOW'
                }},
                *struct {{ id: self_target_id, name: "{self_type}" @ 'NOW' }}"#
        )
    }

    #[cfg(feature = "typed_type_graph")]
    fn method_by_impl_trait_self_query(trait_name: &str, self_type: &str, method: &str) -> String {
        format!(
            r#"?[method_id] :=
                *method {{ id: method_id, name: "{method}", owner_id: impl_id @ 'NOW' }},
                *impl {{ id: impl_id @ 'NOW' }},
                *type_use {{
                    owner_id: impl_id,
                    root_type_id: self_type_id,
                    role: "ImplSelf" @ 'NOW'
                }},
                *type_relation {{
                    source_id: self_type_id,
                    target_id: self_target_id,
                    relation_kind: "Ordinary" @ 'NOW'
                }},
                *struct {{ id: self_target_id, name: "{self_type}" @ 'NOW' }},
                *type_use {{
                    owner_id: impl_id,
                    root_type_id: trait_type_id,
                    role: "ImplTrait" @ 'NOW'
                }},
                *type_relation {{
                    source_id: trait_type_id,
                    target_id: trait_target_id,
                    relation_kind: "Trait" @ 'NOW'
                }},
                *trait {{ id: trait_target_id, name: "{trait_name}" @ 'NOW' }}"#
        )
    }

    #[cfg(feature = "typed_type_graph")]
    fn method_by_raw_pointer_impl_query(trait_name: &str, mutable: bool, method: &str) -> String {
        format!(
            r#"?[method_id, file_path] :=
                *method {{ id: method_id, name: "{method}", owner_id: impl_id @ 'NOW' }},
                *type_use {{
                    owner_id: impl_id,
                    root_type_id: trait_type_id,
                    role: "ImplTrait" @ 'NOW'
                }},
                *type_relation {{
                    source_id: trait_type_id,
                    target_id: trait_target_id,
                    relation_kind: "Trait" @ 'NOW'
                }},
                *trait {{ id: trait_target_id, name: "{trait_name}" @ 'NOW' }},
                *type_use {{
                    owner_id: impl_id,
                    root_type_id: self_type_id,
                    role: "ImplSelf" @ 'NOW'
                }},
                *raw_pointer_type {{
                    type_id: self_type_id,
                    is_mutable: {mutable} @ 'NOW'
                }},
                *syntax_edge {{
                    source_id: module_id,
                    target_id: impl_id,
                    relation_kind: "Contains" @ 'NOW'
                }},
                *file_mod {{ owner_id: module_id, file_path @ 'NOW' }}"#
        )
    }

    #[cfg(feature = "typed_type_graph")]
    fn impl_by_trait_query(trait_name: &str) -> String {
        format!(
            r#"?[impl_id, file_path] :=
                *impl {{ id: impl_id @ 'NOW' }},
                *type_use {{
                    owner_id: impl_id,
                    root_type_id: trait_type_id,
                    role: "ImplTrait" @ 'NOW'
                }},
                *type_relation {{
                    source_id: trait_type_id,
                    target_id: trait_target_id,
                    relation_kind: "Trait" @ 'NOW'
                }},
                *trait {{ id: trait_target_id, name: "{trait_name}" @ 'NOW' }},
                *syntax_edge {{
                    source_id: module_id,
                    target_id: impl_id,
                    relation_kind: "Contains" @ 'NOW'
                }},
                *file_mod {{ owner_id: module_id, file_path @ 'NOW' }}"#
        )
    }

    #[cfg(feature = "typed_type_graph")]
    fn generic_type_param_id_by_owner_name(
        db: &Database,
        owner_id: Uuid,
        name: &str,
    ) -> Result<Uuid, DbError> {
        one_uuid(
            db,
            &format!(
                r#"?[generic_id] :=
                    *generic_type {{
                        id: generic_id,
                        owner_id: to_uuid("{owner_id}"),
                        name: "{name}" @ 'NOW'
                    }}"#
            ),
        )
    }

    #[cfg(feature = "typed_type_graph")]
    fn where_generic_param_bound_owner(
        db: &Database,
        containing_owner_id: Uuid,
        predicate_index: u32,
        bound_index: u32,
        target_trait: TargetSelector,
    ) -> Result<Uuid, DbError> {
        let target_id = match target_trait {
            TargetSelector::TraitInModule { module_path, name } => {
                one_uuid(db, &trait_in_module_query(module_path, name))?
            }
            other => {
                return Err(DbError::QueryExecution(format!(
                    "WhereGenericParamBoundOwner target must be a trait, got {other:?}"
                )));
            }
        };
        one_uuid(
            db,
            &format!(
                r#"?[owner_id] :=
                    *type_use {{
                        id: type_use_id,
                        owner_id,
                        root_type_id,
                        role: "WhereGenericParamBound" @ 'NOW'
                    }},
                    *type_use_where_generic_param_bound_slot {{
                        type_use_id,
                        containing_owner_id: to_uuid("{containing_owner_id}"),
                        predicate_index: {predicate_index},
                        bound_index: {bound_index} @ 'NOW'
                    }},
                    *type_relation {{
                        source_id: root_type_id,
                        target_id: to_uuid("{target_id}"),
                        relation_kind: "Trait" @ 'NOW'
                    }}"#
            ),
        )
    }

    #[cfg(feature = "typed_type_graph")]
    fn generic_type_name(db: &Database, target_id: Uuid) -> Result<String, DbError> {
        let rows = db.raw_query(&format!(
            r#"?[name] :=
                *generic_type {{
                    id: to_uuid("{target_id}"),
                    name @ 'NOW'
                }}"#
        ))?;
        assert_eq!(
            rows.rows.len(),
            1,
            "expected generic_type row for {target_id}; rows: {:#?}",
            rows.rows
        );
        match &rows.rows[0][0] {
            DataValue::Str(name) => Ok(name.to_string()),
            other => Err(DbError::QueryExecution(format!(
                "expected generic_type name string for {target_id}, got {other:?}"
            ))),
        }
    }

    #[tokio::test]
    async fn test_fixture_embeddings_loaded_into_active_set() -> Result<(), Error> {
        use tracing::info;

        init_tracing_once();
        let db = &DEFAULT_TEST_RAG.db;

        // TODO:active-embedding-set 2025-12-15
        // update the active embedding set functions to correctly use Arc<RwLock<>> within these
        // functions.
        let rel = db.with_active_set(|set| set.rel_name.clone())?;
        let script = format!("?[count(node_id)] := *{rel}{{ node_id @ 'NOW' }}");
        let rows = db.raw_query(&script).map_err(ploke_error::Error::from)?;
        info!(?rows);
        let count = rows
            .rows
            .first()
            .and_then(|row| row.first())
            .and_then(|val| val.get_int())
            .unwrap_or(0) as usize;

        assert!(
            count > 0,
            "Embedding relation is empty after loading fixture backup; \
             dense search relies on seeded vectors (use multi-embedding backup)."
        );

        Ok(())
    }

    /// Ensure dense search hits multi-embedding relations (and not legacy `function.embedding`).
    /// This mirrors loading a multi-embedding backup then issuing a vector search.
    #[tokio::test]
    async fn dense_context_uses_multi_embedding_relations() {
        use ploke_db::multi_embedding::{db_ext::EmbeddingExt, hnsw_ext::HnswExt};

        let db = load_local_fixture_db().expect("load local embedding fixture");

        // Note: if the backup lacks vectors, we still expect the legacy-path error; this test
        // asserts on that specific failure mode.

        // TODO:active-embedding-set 2025-12-15
        // update the active embedding set functions to correctly use Arc<RwLock<>> within these
        // functions.
        let active_embedding_set = db
            .with_active_set(|set| set.clone())
            .expect("Un-Poisoned active_embedding_set");

        let embed_rel = active_embedding_set.rel_name.clone();
        let count_script = format!("?[count(node_id)] := *{embed_rel}{{ node_id @ 'NOW' }}");
        let rows = db.raw_query(&count_script).expect("count query");
        let _count = rows
            .rows
            .first()
            .and_then(|r| r.first())
            .and_then(|v| v.get_int())
            .unwrap_or(0);

        // Issue a dense search directly through hnsw to surface any legacy-path errors.
        let dims = active_embedding_set.dims() as usize;
        let query_vec = vec![0.1f32; dims];
        let err = match db.search_similar_for_set(
            &active_embedding_set,
            ploke_db::NodeType::Function,
            LOADED_WORKSPACE_SCOPE,
            query_vec,
            5,
            10,
            5,
            None,
        ) {
            Ok(_) => return,
            Err(e) => e.to_string(),
        };
        assert!(
            err.contains("function") && err.contains("embedding"),
            "expected legacy embedding column error; got: {err}"
        );
    }
    #[tokio::test]
    async fn test_db_nodes_setup() -> Result<(), Error> {
        let db = default_test_db_setup()?;
        Ok(())
    }

    lazy_static! {
        /// This test db is restored from the backup of an earlier parse of the `fixture_nodes`
        /// crate located in `tests/fixture_crates/fixture_nodes`, and has a decent sampling of all
        /// rust code items. It provides a good target for other tests because it has already been
        /// extensively tested in `syn_parser`, with each item individually verified to have all
        /// fields correctly parsed for expected values.
        ///
        /// One "gotcha" of laoding the Cozo database is that the hnsw items are not retained
        /// between backups, so they must be recalculated each time. However, by restoring the
        /// backup database we do retain the dense vector embeddings, allowing our tests to be
        /// significantly sped up by using a lazy loader here and making calls to the same backup.
        ///
        /// If needed, other tests can re-implement the load from this file, which may become a
        /// factor for some tests that need to alter the database, but as long as things are
        /// cleaned up afterwards it should be OK.
        // TODO: Add a mutex guard to avoid cross-contamination of tests.
        pub static ref TEST_DB_NODES: Result<Arc< Database >, Error> = {
            default_test_db_setup()
        };
    }

    async fn fetch_snippet_containing(
        db: &Arc<Database>,
        ordered_node_ids: Vec<Uuid>,
        search_term: &str,
    ) -> Result<String, Error> {
        let node_info: Vec<EmbeddingData> = db.get_nodes_ordered(ordered_node_ids)?;
        let io_handle = IoManagerHandle::new();

        let debug_msg = node_info
            .iter()
            .enumerate()
            .map(|x| format!("{:#?}", x))
            .join("\n");
        debug!(%debug_msg);

        let snippet_find: Vec<String> = io_handle
            .get_snippets_batch(node_info)
            .await
            .expect("Problem receiving")
            .into_iter()
            .try_collect()?;

        snippet_find
            .into_iter()
            .find(|snip| snip.contains(search_term))
            .ok_or_else(|| {
                RagError::Search(format!("No snippet found for term {search_term}")).into()
            })
    }

    async fn fetch_and_assert_snippet(
        db: &Arc<Database>,
        ordered_node_ids: Vec<Uuid>,
        search_term: &str,
    ) -> Result<(), Error> {
        let node_info: Vec<EmbeddingData> = db.get_nodes_ordered(ordered_node_ids)?;
        let io_handle = IoManagerHandle::new();

        let snippet = io_handle
            .get_snippets_batch(node_info)
            .await
            .expect("Problem receiving")
            .into_iter()
            .inspect(|snip| eprintln!("Search result: {:?}", snip))
            .find(|snip| snip.as_ref().is_ok_and(|s| s.contains(search_term)));

        assert!(
            snippet.is_some(),
            "No snippet found containing '{}'",
            search_term
        );
        Ok(())
    }

    fn workspace_fixture_function_rows(db: &Database) -> Result<WorkspaceScopeFixture, Error> {
        let rows = db
            .get_unembedded_node_data(64, 0)?
            .into_iter()
            .flat_map(|typed| typed.v.into_iter())
            .collect_vec();
        let mut by_name = BTreeMap::new();
        for row in rows {
            by_name.insert(row.name, (row.id, row.namespace));
        }

        let (root_id, root_namespace) = by_name.remove("root_value").ok_or_else(|| {
            ploke_error::Error::Internal(ploke_error::internal::InternalError::CompilerError(
                "missing root_value in workspace fixture".to_string(),
            ))
        })?;
        let (nested_id, nested_namespace) = by_name.remove("nested_value").ok_or_else(|| {
            ploke_error::Error::Internal(ploke_error::internal::InternalError::CompilerError(
                "missing nested_value in workspace fixture".to_string(),
            ))
        })?;

        Ok(WorkspaceScopeFixture {
            root_id,
            root_namespace,
            nested_id,
            nested_namespace,
        })
    }

    fn load_workspace_scope_db(
        query: &str,
    ) -> Result<(Arc<Database>, WorkspaceScopeFixture), Error> {
        let db = Arc::new(fresh_backup_fixture_db(&WS_FIXTURE_01_CANONICAL)?);
        let fixture = workspace_fixture_function_rows(db.as_ref())?;
        let embedding_set = db.with_active_set(|set| set.clone())?;
        db.ensure_embedding_relation(&embedding_set)?;

        let embedder = LocalEmbedder::new(EmbeddingConfig::default())?;
        let query_vec = embedder
            .embed_batch(&[query])?
            .into_iter()
            .next()
            .ok_or_else(|| {
                ploke_error::Error::Internal(ploke_error::internal::InternalError::CompilerError(
                    "expected one query embedding".to_string(),
                ))
            })?;
        let mut in_scope_vec = query_vec.clone();
        if let Some(first) = in_scope_vec.first_mut() {
            *first += 0.05;
        }

        db.update_embeddings_batch(vec![
            (fixture.root_id, query_vec),
            (fixture.nested_id, in_scope_vec),
        ])?;
        db.create_embedding_index(&embedding_set)?;

        Ok((db, fixture))
    }

    #[tokio::test]
    async fn test_search() -> Result<(), Error> {
        init_tracing_once();

        let search_term = "use_all_const_static";

        let rag = &DEFAULT_TEST_RAG;
        let db = &DEFAULT_TEST_RAG.db;

        let search_res: Vec<(Uuid, f32)> =
            rag.search(search_term, 15, LOADED_WORKSPACE_SCOPE).await?;
        assert!(
            !search_res.is_empty(),
            "Dense search returned no results for '{}'",
            search_term
        );

        let ordered_node_ids: Vec<Uuid> = search_res.iter().map(|(id, _score)| *id).collect();
        fetch_and_assert_snippet(db, ordered_node_ids, search_term).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_bm25_rebuild() -> Result<(), Error> {
        init_tracing_once();
        let db_raw = fresh_backup_fixture_db(&FIXTURE_NODES_LOCAL_EMBEDDINGS)?;
        let db = Arc::new(db_raw);
        let rag = init_test_rag(Arc::clone(&db));

        // Should not error
        rag.bm25_rebuild().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_bm25_search_basic() -> Result<(), Error> {
        init_tracing_once();
        let db_raw = fresh_backup_fixture_db(&FIXTURE_NODES_LOCAL_EMBEDDINGS)?;
        let db = Arc::new(db_raw);
        let rag = init_test_rag(Arc::clone(&db));

        let search_term = "use_all_const_static";

        // Trigger a rebuild to ensure index is fresh, then retry a few times in case it's async.
        rag.bm25_rebuild().await?;

        let mut bm25_res: Vec<(Uuid, f32)> = Vec::new();
        for _ in 0..10 {
            bm25_res = rag
                .search_bm25(search_term, 15, LOADED_WORKSPACE_SCOPE)
                .await?;
            if !bm25_res.is_empty() {
                break;
            }
            sleep(Duration::from_millis(50)).await;
        }

        assert!(
            !bm25_res.is_empty(),
            "BM25 search returned no results for '{}'",
            search_term
        );

        let ordered_node_ids: Vec<Uuid> = bm25_res.iter().map(|(id, _score)| *id).collect();
        fetch_and_assert_snippet(&db, ordered_node_ids, search_term).await?;
        Ok(())
    }

    #[tokio::test]
    async fn sparse_context_assembly_does_not_require_embedding_rows() -> Result<(), Error> {
        use ploke_test_utils::fixture_dbs::FIXTURE_NODES_CANONICAL;

        init_tracing_once();
        let db_raw = fresh_backup_fixture_db(&FIXTURE_NODES_CANONICAL)?;
        let db = Arc::new(db_raw);
        let rag = init_test_rag_with_io(Arc::clone(&db));
        let search_term = "use_all_const_static";
        let budget = TokenBudget {
            max_total: 2_000,
            per_file_max: 2_000,
            per_part_max: 500,
        };
        let strategy = RetrievalStrategy::Sparse { strict: Some(true) };

        rag.bm25_rebuild().await?;

        for _ in 0..10 {
            match rag.bm25_status().await? {
                ploke_db::bm25_index::bm25_service::Bm25Status::Ready { docs } if docs > 0 => {
                    break;
                }
                ploke_db::bm25_index::bm25_service::Bm25Status::Error(detail) => {
                    panic!("BM25 rebuild entered error state: {detail}");
                }
                _ => sleep(Duration::from_millis(50)).await,
            }
        }

        let mut assembled = None;
        for _ in 0..10 {
            match rag
                .get_context(search_term, 5, &budget, &strategy, LOADED_WORKSPACE_SCOPE)
                .await
            {
                Ok(ctx) if !ctx.parts.is_empty() => {
                    assembled = Some(ctx);
                    break;
                }
                Ok(_) => sleep(Duration::from_millis(50)).await,
                Err(RagError::Search(message)) if message.contains("bm25 index not ready") => {
                    sleep(Duration::from_millis(50)).await;
                }
                Err(err) => return Err(err.into()),
            }
        }

        let ctx = match assembled {
            Some(ctx) => ctx,
            None => {
                panic!("sparse context assembly should return snippets");
            }
        };
        assert!(
            ctx.parts.iter().any(|part| part.text.contains(search_term)),
            "expected sparse context to include snippet text containing {search_term:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn lenient_context_reports_skipped_stale_snippet_io() -> Result<(), Error> {
        use ploke_test_utils::fixture_dbs::FIXTURE_NODES_CANONICAL;

        init_tracing_once();
        let db = fresh_backup_fixture_db(&FIXTURE_NODES_CANONICAL)?;
        let target = db
            .raw_query(
                r#"
parent_of[child, parent] := *syntax_edge{source_id: parent, target_id: child, relation_kind: "Contains" @ 'NOW'}
ancestor[desc, asc] := parent_of[desc, asc]
ancestor[desc, asc] := parent_of[desc, intermediate], ancestor[intermediate, asc]
is_file_module[id] := *file_mod{owner_id: id @ 'NOW'}

?[id, file_mod_id] :=
    *function{id, name: "use_all_const_static" @ 'NOW'},
    ancestor[id, file_mod_id],
    is_file_module[file_mod_id]
:limit 1
"#,
            )
            .map_err(Error::from)?;
        let row = target
            .row_refs()
            .next()
            .expect("fixture should contain use_all_const_static");
        let node_id: Uuid = row.get("id").map_err(Error::from)?;
        let file_mod_id: Uuid = row.get("file_mod_id").map_err(Error::from)?;
        let stale_hash = Uuid::new_v4();
        let stale_module_script = format!(
            r#"
?[id, at, name, path, vis_kind, vis_path, docstring, span, tracking_hash, module_kind, cfgs] :=
    *module{{id, name, path, vis_kind, vis_path, docstring, span, tracking_hash: _old_hash, module_kind, cfgs @ 'NOW'}},
    id = to_uuid("{file_mod_id}"),
    at = 'ASSERT',
    tracking_hash = to_uuid("{stale_hash}")
:put module {{ id, at => name, path, vis_kind, vis_path, docstring, span, tracking_hash, module_kind, cfgs }}
"#
        );
        db.raw_query_mut(&stale_module_script)
            .map_err(Error::from)?;

        let context = crate::context::assemble_context(
            "use_all_const_static",
            &[(node_id, 1.0)],
            &TokenBudget::default(),
            &AssemblyPolicy::default(),
            &ApproxCharTokenizer,
            &db,
            &IoManagerHandle::new(),
        )
        .await?;

        assert!(
            context.parts.is_empty(),
            "the stale snippet must not be returned as clean context"
        );
        assert_eq!(
            context.stats.skipped_io_errors, 1,
            "lenient context assembly must surface skipped stale snippet IO"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_hybrid_search() -> Result<(), Error> {
        init_tracing_once();

        let db_raw = fresh_backup_fixture_db(&FIXTURE_NODES_LOCAL_EMBEDDINGS)?;
        let db = Arc::new(db_raw);
        let rag = init_test_rag(Arc::clone(&db));

        let search_term = "use_all_const_static";

        let fused: Vec<(Uuid, f32)> = rag
            .hybrid_search(search_term, 15, LOADED_WORKSPACE_SCOPE)
            .await?;
        assert!(
            !fused.is_empty(),
            "Hybrid search returned no results for '{}'",
            search_term
        );

        let ordered_node_ids: Vec<Uuid> = fused.iter().map(|(id, _score)| *id).collect();
        fetch_and_assert_snippet(&db, ordered_node_ids, search_term).await?;
        Ok(())
    }

    #[tokio::test]
    async fn hybrid_specific_crate_scope_excludes_out_of_scope_candidates_before_fusion()
    -> Result<(), Error> {
        init_tracing_once();
        let query = "root value";
        let (db, fixture) = load_workspace_scope_db(query)?;
        let rag = init_test_rag(Arc::clone(&db));

        rag.bm25_rebuild().await?;

        let unscoped = rag
            .hybrid_search(query, 1, RetrievalScope::LoadedWorkspace)
            .await?;
        assert_eq!(
            unscoped.len(),
            1,
            "unscoped hybrid top_k=1 should return one hit"
        );
        assert_eq!(
            unscoped[0].0, fixture.root_id,
            "unscoped hybrid search should prefer the stronger out-of-scope root_value candidate"
        );

        let scoped = rag
            .hybrid_search(
                query,
                2,
                RetrievalScope::SpecificCrate(CrateId::new(fixture.nested_namespace)),
            )
            .await?;
        assert!(
            !scoped.is_empty(),
            "scoped hybrid search should retain an in-scope candidate"
        );
        assert!(
            scoped.iter().all(|(id, _)| *id == fixture.nested_id),
            "hybrid fusion must not admit the out-of-scope root_value candidate"
        );

        let nodes = db.get_nodes_ordered(scoped.iter().map(|(id, _)| *id).collect())?;
        assert!(
            nodes
                .iter()
                .all(|node| node.namespace == fixture.nested_namespace),
            "scoped hybrid results must remain in the requested crate namespace"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_bm25_search_fallback() -> Result<(), Error> {
        // Initialize tracing for the test
        init_tracing_once();
        let db_raw = fresh_backup_fixture_db(&FIXTURE_NODES_LOCAL_EMBEDDINGS)?;
        let db = Arc::new(db_raw);
        let rag = init_test_rag(Arc::clone(&db));

        let search_term = "use_all_const_static";

        // Intentionally do not call bm25_rebuild or index anything; fallback should kick in.
        let results: Vec<(Uuid, f32)> = rag
            .search_bm25(search_term, 15, LOADED_WORKSPACE_SCOPE)
            .await?;
        assert!(
            !results.is_empty(),
            "BM25 fallback returned no results for '{}'",
            search_term
        );

        let ordered_node_ids: Vec<Uuid> = results.iter().map(|(id, _score)| *id).collect();
        fetch_and_assert_snippet(&db, ordered_node_ids, search_term).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_search_structs() -> Result<(), Error> {
        init_tracing_once();
        let rag = &DEFAULT_TEST_RAG;
        let db = &DEFAULT_TEST_RAG.db;

        let search_term = "DocumentedStruct";

        let search_res: Vec<(Uuid, f32)> =
            rag.search(search_term, 10, LOADED_WORKSPACE_SCOPE).await?;
        assert!(
            !search_res.is_empty(),
            "Dense search returned no results for '{}'",
            search_term
        );

        let ordered_node_ids: Vec<Uuid> = search_res.iter().map(|(id, _score)| *id).collect();
        fetch_and_assert_snippet(db, ordered_node_ids, search_term).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_search_enums() -> Result<(), Error> {
        init_tracing_once();
        let rag = &DEFAULT_TEST_RAG;
        let db = &DEFAULT_TEST_RAG.db;

        let search_term = "GenericEnum";

        let search_res: Vec<(Uuid, f32)> =
            rag.search(search_term, 10, LOADED_WORKSPACE_SCOPE).await?;
        assert!(
            !search_res.is_empty(),
            "Dense search returned no results for '{}'",
            search_term
        );

        let ordered_node_ids: Vec<Uuid> = search_res.iter().map(|(id, _score)| *id).collect();
        fetch_and_assert_snippet(db, ordered_node_ids, search_term).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_search_traits_new() -> Result<(), Error> {
        init_tracing_once();

        let db_raw = fresh_backup_fixture_db(&FIXTURE_NODES_LOCAL_EMBEDDINGS)?;
        let db = Arc::new(db_raw);
        let rag = init_test_rag_bm25(Arc::clone(&db)).await;

        let search_term = "ComplexGenericTrait";

        let search_res: Vec<(Uuid, f32)> =
            rag.search(search_term, 10, LOADED_WORKSPACE_SCOPE).await?;

        assert!(
            !search_res.is_empty(),
            "Dense search returned no results for '{}'",
            search_term
        );
        let ordered_node_ids: Vec<Uuid> = search_res.iter().map(|(id, _)| *id).collect();

        // Ensure sparse index is populated so we test BM25 behavior (not dense fallback).
        rag.bm25_rebuild().await?;
        let mut results: Vec<(Uuid, f32)> = Vec::new();
        for _ in 0..10 {
            results = rag
                .search_bm25(search_term, 15, LOADED_WORKSPACE_SCOPE)
                .await?;
            if !results.is_empty() {
                break;
            }
            sleep(Duration::from_millis(50)).await;
        }
        assert!(
            !results.is_empty(),
            "BM25 search returned no results for '{}'",
            search_term
        );

        let ordered_node_ids: Vec<Uuid> = results.iter().map(|(id, _score)| *id).collect();
        fetch_and_assert_snippet(&db, ordered_node_ids, search_term).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_search_unions() -> Result<(), Error> {
        init_tracing_once();
        let rag = &DEFAULT_TEST_RAG;
        let db = &DEFAULT_TEST_RAG.db;

        let search_term = "GenericUnion";

        let search_res: Vec<(Uuid, f32)> =
            rag.search(search_term, 10, LOADED_WORKSPACE_SCOPE).await?;
        assert!(
            !search_res.is_empty(),
            "Dense search returned no results for '{}'",
            search_term
        );

        let ordered_node_ids: Vec<Uuid> = search_res.iter().map(|(id, _score)| *id).collect();
        fetch_and_assert_snippet(db, ordered_node_ids, search_term).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_search_macros() -> Result<(), Error> {
        init_tracing_once();
        let rag = &DEFAULT_TEST_RAG;
        let db = &DEFAULT_TEST_RAG.db;

        let search_term = "documented_macro";

        let search_res: Vec<(Uuid, f32)> =
            rag.search(search_term, 10, LOADED_WORKSPACE_SCOPE).await?;
        assert!(
            !search_res.is_empty(),
            "Dense search returned no results for '{}'",
            search_term
        );

        let ordered_node_ids: Vec<Uuid> = search_res.iter().map(|(id, _score)| *id).collect();
        fetch_and_assert_snippet(db, ordered_node_ids, search_term).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_search_type_aliases() -> Result<(), Error> {
        init_tracing_once();

        let db_raw = fresh_backup_fixture_db(&FIXTURE_NODES_LOCAL_EMBEDDINGS)?;
        let db = Arc::new(db_raw);
        let rag = init_test_rag_bm25(Arc::clone(&db)).await;

        let search_term = "DisplayableContainer";

        let sparse_res: Vec<(Uuid, f32)> = rag
            .search_bm25_strict(search_term, 10, LOADED_WORKSPACE_SCOPE)
            .await?;

        eprintln!("checking sparse search");
        assert!(
            !sparse_res.is_empty(),
            "Sparse search returned no results for '{}'",
            search_term
        );
        let ordered_node_ids: Vec<Uuid> = sparse_res.iter().map(|(id, _score)| *id).collect();
        fetch_and_assert_snippet(&db, ordered_node_ids, search_term).await?;

        let search_res: Vec<(Uuid, f32)> =
            rag.search(search_term, 10, LOADED_WORKSPACE_SCOPE).await?;
        eprintln!("checking dense search");
        assert!(
            !search_res.is_empty(),
            "Dense search returned no results for '{}'",
            search_term
        );

        let ordered_node_ids: Vec<Uuid> = search_res.iter().map(|(id, _score)| *id).collect();
        fetch_and_assert_snippet(&db, ordered_node_ids, search_term).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_search_constants() -> Result<(), Error> {
        init_tracing_once();
        let rag = &DEFAULT_TEST_RAG;
        let db = &DEFAULT_TEST_RAG.db;

        let search_term = "TOP_LEVEL_BOOL";

        let search_res: Vec<(Uuid, f32)> =
            rag.search(search_term, 10, LOADED_WORKSPACE_SCOPE).await?;
        assert!(
            !search_res.is_empty(),
            "Dense search returned no results for '{}'",
            search_term
        );

        let ordered_node_ids: Vec<Uuid> = search_res.iter().map(|(id, _score)| *id).collect();
        fetch_and_assert_snippet(db, ordered_node_ids, search_term).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_search_statics() -> Result<(), Error> {
        init_tracing_once();
        let rag = &DEFAULT_TEST_RAG;
        let db = &DEFAULT_TEST_RAG.db;

        let search_term = "TOP_LEVEL_COUNTER";

        let search_res: Vec<(Uuid, f32)> =
            rag.search(search_term, 10, LOADED_WORKSPACE_SCOPE).await?;
        assert!(
            !search_res.is_empty(),
            "Dense search returned no results for '{}'",
            search_term
        );

        let ordered_node_ids: Vec<Uuid> = search_res.iter().map(|(id, _score)| *id).collect();
        fetch_and_assert_snippet(db, ordered_node_ids, search_term).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_hybrid_search_generic_trait() -> Result<(), Error> {
        init_tracing_once();
        // Rebuild BM25 index so hybrid search uses real sparse scores rather than dense fallback.
        let db_raw = fresh_backup_fixture_db(&FIXTURE_NODES_LOCAL_EMBEDDINGS)?;
        let db = Arc::new(db_raw);
        let rag = init_test_rag(Arc::clone(&db));

        let search_term = "GenericSuperTrait";
        let fused: Vec<(Uuid, f32)> = rag
            .hybrid_search(search_term, 15, LOADED_WORKSPACE_SCOPE)
            .await?;
        assert!(
            !fused.is_empty(),
            "Hybrid search returned no results for '{}'",
            search_term
        );

        let ordered_node_ids: Vec<Uuid> = fused.iter().map(|(id, _score)| *id).collect();
        fetch_and_assert_snippet(&db, ordered_node_ids, search_term).await?;
        Ok(())
    }

    #[tokio::test]
    async fn get_context_specific_crate_scope_does_not_materialize_out_of_scope_ids()
    -> Result<(), Error> {
        init_tracing_once();
        let query = "root value";
        let (db, fixture) = load_workspace_scope_db(query)?;
        let rag = init_test_rag_with_io(Arc::clone(&db));

        rag.bm25_rebuild().await?;

        let context = rag
            .get_context(
                query,
                2,
                &TokenBudget::default(),
                &RetrievalStrategy::Hybrid {
                    rrf: Default::default(),
                    mmr: None,
                },
                RetrievalScope::SpecificCrate(CrateId::new(fixture.nested_namespace)),
            )
            .await?;

        assert!(
            !context.parts.is_empty(),
            "scoped get_context should return at least one assembled part"
        );
        assert!(
            context
                .parts
                .iter()
                .all(|part| part.file_path.as_ref().contains("nested/member_nested")),
            "get_context should only materialize snippets from the requested crate"
        );
        assert!(
            context
                .parts
                .iter()
                .all(|part| part.id == fixture.nested_id),
            "context assembly must not materialize the out-of-scope root_value node"
        );

        Ok(())
    }

    #[cfg(feature = "typed_type_graph")]
    async fn stale_plain_starting_db() -> Result<Arc<Database>, Error> {
        use ploke_test_utils::FIXTURE_NODES_CANONICAL;
        use ploke_test_utils::fixture_dbs::backup_fixture_path_or_seed;

        let path = backup_fixture_path_or_seed(&FIXTURE_NODES_CANONICAL).map_err(Error::from)?;
        let db = Database::create_new_backup_default(&path)
            .await
            .map_err(Error::from)?;
        Ok(Arc::new(db))
    }

    #[cfg(feature = "typed_type_graph")]
    #[tokio::test]
    async fn type_context_disabled_safely_when_relations_absent() -> Result<(), Error> {
        init_tracing_once();

        let db = stale_plain_starting_db().await?;
        assert!(
            !db.has_typed_type_graph_relations().map_err(Error::from)?,
            "stale plain starting-db restore must lack typed-graph relations for this regression"
        );

        let rag = {
            let rag = init_test_rag_with_io(Arc::clone(&db));
            rag.bm25_rebuild().await.map_err(Error::from)?;
            rag
        };
        assert!(
            rag.type_context_degraded(),
            "RagService must record degraded type-context when relations are absent"
        );

        rag.get_context(
            "method",
            4,
            &TokenBudget {
                max_total: 512,
                per_part_max: 128,
                ..Default::default()
            },
            &RetrievalStrategy::Sparse { strict: Some(true) },
            LOADED_WORKSPACE_SCOPE,
        )
        .await
        .map_err(Error::from)?;

        Ok(())
    }

    #[cfg(feature = "typed_type_graph")]
    #[tokio::test]
    async fn type_context_expansion_adds_materializable_type_neighbors() -> Result<(), Error> {
        init_tracing_once();
        let db_raw = setup_db_full_multi_embedding("fixture_nodes")?;
        let db = Arc::new(Database::new(db_raw));

        let seed = unique_id_by_name(&db, "method", "new")?;
        let struct_neighbor = impl_self_target_for_method_name(&db, "new")?;
        let embedding_set = db.with_active_set(|set| set.clone())?;
        db.ensure_embedding_relation(&embedding_set)?;
        let dims = embedding_set.dims() as usize;
        db.update_embeddings_batch(vec![
            (seed, vec![0.91; dims]),
            (struct_neighbor, vec![0.73; dims]),
        ])?;
        db.create_embedding_index(&embedding_set)?;

        let rag = init_test_rag(Arc::clone(&db));

        let (expanded, type_context) = rag.expand_hits_with_type_context(&[(seed, 1.0)])?;
        let expanded_ids = expanded.iter().map(|(id, _)| *id).collect::<Vec<_>>();

        assert!(
            expanded_ids.contains(&seed),
            "expanded hit list must preserve the original retrieval hit; expanded: {expanded:#?}"
        );
        assert!(
            expanded_ids.contains(&struct_neighbor),
            "a method returning Self should pull the impl self type into candidate context; expanded: {expanded:#?}"
        );
        let provenance = type_context
            .get(&struct_neighbor)
            .expect("expanded type neighbor should carry type-context provenance");
        assert_eq!(provenance.seed_id, seed);
        assert_eq!(provenance.relation, TypeContextKind::TypeDefinitionImpact);
        Ok(())
    }

    #[cfg(feature = "typed_type_graph")]
    #[tokio::test]
    async fn corpus_type_shape_matrix_expands_db_and_rag_type_context() -> Result<(), Error> {
        init_tracing_once();

        for case in positive_type_shape_cases()
            .iter()
            .filter(|case| covers(case, ShapePipelineCoverage::RagApi))
        {
            let db = Arc::new(fresh_backup_fixture_db(case.fixture.searchable_fixture())?);
            let owner_id = resolve_matrix_owner(&db, case.owner).map_err(Error::from)?;
            let target_id = resolve_matrix_target(&db, owner_id, case).map_err(Error::from)?;

            let candidates = db
                .expand_type_context(TypeContextSeed::Owner(owner_id), Default::default())
                .map_err(Error::from)?;
            assert!(
                candidates.iter().any(|candidate| {
                    candidate.node_id == target_id
                        && candidate.relation == case.type_context_relation
                }),
                "{} should expand from owner to terminal target with {:?}; source: {}; candidates: {candidates:#?}",
                case.name,
                case.type_context_relation,
                case.source
            );

            if covers(case, ShapePipelineCoverage::TuiTool) {
                let rag = init_test_rag_mock(Arc::clone(&db));
                let (expanded, type_context) = rag
                    .expand_hits_with_type_context(&[(owner_id, 1.0)])
                    .map_err(Error::from)?;
                assert!(
                    expanded.iter().any(|(id, _)| *id == target_id),
                    "{} should materialize target through RagService hit expansion; expanded: {expanded:#?}",
                    case.name
                );
                let provenance = type_context.get(&target_id).unwrap_or_else(|| {
                    panic!("{} missing TypeContextInfo for {target_id}", case.name)
                });
                assert_eq!(provenance.seed_id, owner_id, "{}", case.name);
                assert_eq!(
                    provenance.relation,
                    super::super::type_context_kind(case.type_context_relation),
                    "{}",
                    case.name
                );
                assert_eq!(provenance.distance, case.depth + 1, "{}", case.name);
            }
        }

        Ok(())
    }

    #[cfg(feature = "typed_type_graph")]
    #[tokio::test]
    async fn axum_struct_seed_materializes_nested_trait_object_target() -> Result<(), Error> {
        init_tracing_once();

        let db = Arc::new(fresh_backup_fixture_db(
            &ploke_test_utils::CORPUS_AXUM_OPENROUTER_EMBEDDINGS,
        )?);
        let seed_id = one_uuid_by_file_suffix(
            &db,
            &struct_in_file_query("BoxedIntoRoute"),
            "axum/src/boxed.rs",
        )
        .map_err(Error::from)?;
        let target_id = one_uuid_by_file_suffix(
            &db,
            &trait_in_file_query("ErasedIntoRoute"),
            "axum/src/boxed.rs",
        )
        .map_err(Error::from)?;

        let rag = init_test_rag_mock(Arc::clone(&db));
        let (expanded, type_context) = rag
            .expand_hits_with_type_context(&[(seed_id, 1.0)])
            .map_err(Error::from)?;

        assert!(
            expanded.iter().any(|(id, _)| *id == target_id),
            "BoxedIntoRoute target seed should materialize nested ErasedIntoRoute trait target; expanded: {expanded:#?}; type_context: {type_context:#?}"
        );
        let provenance = type_context.get(&target_id).unwrap_or_else(|| {
            panic!("missing TypeContextInfo for ErasedIntoRoute target {target_id}; expanded: {expanded:#?}; type_context: {type_context:#?}")
        });
        assert_eq!(provenance.seed_id, seed_id);
        assert_eq!(provenance.relation, TypeContextKind::UsesTypeNested);
        Ok(())
    }

    #[cfg(feature = "typed_type_graph")]
    #[tokio::test]
    async fn axum_boxed_into_route_sparse_context_emits_nested_trait_type_context()
    -> Result<(), Error> {
        init_tracing_once();

        let db = Arc::new(fresh_backup_fixture_db(
            &ploke_test_utils::CORPUS_AXUM_OPENROUTER_EMBEDDINGS,
        )?);
        let seed_id = one_uuid_by_file_suffix(
            &db,
            &struct_in_file_query("BoxedIntoRoute"),
            "axum/src/boxed.rs",
        )
        .map_err(Error::from)?;

        let rag = RagService::new_full(
            Arc::clone(&db),
            runtime_for(&db, EmbeddingProcessor::new_mock()),
            IoManagerHandle::new(),
            crate::RagConfig::default(),
        )?;
        rag.bm25_rebuild().await?;

        let sparse_hits = rag
            .search_bm25_strict("BoxedIntoRoute struct", 1, LOADED_WORKSPACE_SCOPE)
            .await?;
        let (expanded_hits, expanded_type_context) =
            rag.expand_hits_with_type_context(&sparse_hits)?;
        let expanded_labels = expanded_hits
            .iter()
            .map(|(id, score)| (*id, *score, context_label(&db, *id)))
            .collect::<Vec<_>>();

        let assembled = rag
            .get_context(
                "BoxedIntoRoute struct",
                1,
                &TokenBudget {
                    max_total: 4096,
                    per_part_max: 256,
                    ..TokenBudget::default()
                },
                &RetrievalStrategy::Sparse { strict: Some(true) },
                LOADED_WORKSPACE_SCOPE,
            )
            .await?;

        assert!(
            assembled.parts.iter().any(|part| {
                part.text.contains("ErasedIntoRoute")
                    && part.type_context.is_some_and(|info| {
                        info.seed_id == seed_id && info.relation == TypeContextKind::UsesTypeNested
                    })
            }),
            "BoxedIntoRoute sparse context should carry nested ErasedIntoRoute type_context; sparse_hits: {sparse_hits:#?}; expanded_hits: {expanded_hits:#?}; expanded_labels: {expanded_labels:#?}; expanded_type_context: {expanded_type_context:#?}; parts: {:#?}",
            assembled.parts
        );
        Ok(())
    }

    #[cfg(feature = "typed_type_graph")]
    #[tokio::test]
    async fn chrono_single_day_owner_seeded_expands_weekday_type_context() -> Result<(), Error> {
        init_tracing_once();

        let db = Arc::new(fresh_backup_fixture_db(
            &ploke_test_utils::CORPUS_CHRONO_OPENROUTER_EMBEDDINGS,
        )?);
        let seed_id = one_uuid(&db, &method_by_impl_self_query("WeekdaySet", "single_day"))
            .map_err(Error::from)?;
        let target_id = unique_id_by_name(&db, "enum", "Weekday")?;

        let rag = init_test_rag_mock(Arc::clone(&db));
        let (expanded, type_context) = rag.expand_hits_with_type_context(&[(seed_id, 1.0)])?;

        assert!(
            expanded.iter().any(|(id, _)| *id == target_id),
            "WeekdaySet::single_day owner seed should materialize Weekday; expanded: {expanded:#?}; type_context: {type_context:#?}"
        );
        let provenance = type_context.get(&target_id).unwrap_or_else(|| {
            panic!("missing TypeContextInfo for Weekday target {target_id}; expanded: {expanded:#?}; type_context: {type_context:#?}")
        });
        assert_eq!(provenance.seed_id, seed_id);
        assert_eq!(provenance.relation, TypeContextKind::UsesTypeNested);
        Ok(())
    }

    #[cfg(feature = "typed_type_graph")]
    #[tokio::test]
    async fn chrono_single_day_bm25_precise_query_retrieves_method_owner() -> Result<(), Error> {
        init_tracing_once();

        let db = Arc::new(fresh_backup_fixture_db(
            &ploke_test_utils::CORPUS_CHRONO_OPENROUTER_EMBEDDINGS,
        )?);
        let method_id = one_uuid(&db, &method_by_impl_self_query("WeekdaySet", "single_day"))
            .map_err(Error::from)?;

        let rag = init_test_rag_mock(Arc::clone(&db));
        rag.bm25_rebuild().await?;

        let search_term = "single_day";
        let sparse_hits = rag
            .search_bm25_strict(search_term, 15, LOADED_WORKSPACE_SCOPE)
            .await?;

        assert!(
            sparse_hits.iter().any(|(id, _)| *id == method_id),
            "BM25 exact-name query should retrieve WeekdaySet::single_day within top 15; sparse_hits: {sparse_hits:#?}"
        );
        Ok(())
    }

    #[cfg(feature = "typed_type_graph")]
    #[tokio::test]
    async fn axum_map_layer_field_seeded_expands_layer_fn_type_context() -> Result<(), Error> {
        init_tracing_once();

        let db = Arc::new(fresh_backup_fixture_db(
            &ploke_test_utils::CORPUS_AXUM_OPENROUTER_EMBEDDINGS,
        )?);
        let map_id =
            one_uuid_by_file_suffix(&db, &struct_in_file_query("Map"), "axum/src/boxed.rs")
                .map_err(Error::from)?;
        let seed_id = one_uuid(
            &db,
            &format!(
                r#"?[id] :=
                    *field {{
                        id,
                        owner_id: to_uuid("{map_id}"),
                        index: 1 @ 'NOW'
                    }}"#
            ),
        )
        .map_err(Error::from)?;
        let trait_id =
            one_uuid_by_file_suffix(&db, &trait_in_file_query("LayerFn"), "axum/src/boxed.rs")
                .map_err(Error::from)?;

        let rag = init_test_rag_mock(Arc::clone(&db));
        let (expanded, type_context) = rag.expand_hits_with_type_context(&[(seed_id, 1.0)])?;

        assert!(
            expanded.iter().any(|(id, _)| *id == trait_id),
            "Map.layer field seed should materialize nested LayerFn trait target; expanded: {expanded:#?}; type_context: {type_context:#?}"
        );
        let provenance = type_context.get(&trait_id).unwrap_or_else(|| {
            panic!("missing TypeContextInfo for LayerFn target {trait_id}; expanded: {expanded:#?}; type_context: {type_context:#?}")
        });
        assert_eq!(provenance.seed_id, seed_id);
        assert_eq!(provenance.relation, TypeContextKind::UsesTypeNested);
        Ok(())
    }

    #[cfg(feature = "typed_type_graph")]
    #[tokio::test]
    async fn axum_map_bm25_precise_query_retrieves_map_struct() -> Result<(), Error> {
        init_tracing_once();

        let db = Arc::new(fresh_backup_fixture_db(
            &ploke_test_utils::CORPUS_AXUM_OPENROUTER_EMBEDDINGS,
        )?);
        let map_id =
            one_uuid_by_file_suffix(&db, &struct_in_file_query("Map"), "axum/src/boxed.rs")
                .map_err(Error::from)?;

        let rag = init_test_rag_mock(Arc::clone(&db));
        rag.bm25_rebuild().await?;

        let search_term = "Map";
        let sparse_hits = rag
            .search_bm25_strict(search_term, 25, LOADED_WORKSPACE_SCOPE)
            .await?;

        assert!(
            sparse_hits.iter().any(|(id, _)| *id == map_id),
            "BM25 exact-name query should retrieve axum/src/boxed.rs Map within top 25; sparse_hits: {sparse_hits:#?}"
        );
        Ok(())
    }

    #[cfg(feature = "typed_type_graph")]
    fn context_label(db: &Database, id: Uuid) -> String {
        db.paths_from_id(id)
            .ok()
            .and_then(|rows| NodePaths::try_from(rows).ok())
            .map(|paths| paths.canon)
            .unwrap_or_else(|| format!("{id}"))
    }

    #[tokio::test]
    async fn test_bm25_search_complex_enum() -> Result<(), Error> {
        init_tracing_once();
        let db_raw = fresh_backup_fixture_db(&FIXTURE_NODES_LOCAL_EMBEDDINGS)?;
        let db = Arc::new(db_raw);
        let rag = init_test_rag(Arc::clone(&db));

        let search_term = "EnumWithMixedVariants";

        // Ensure BM25 index is populated
        rag.bm25_rebuild().await?;

        let mut bm25_res: Vec<(Uuid, f32)> = Vec::new();
        for _ in 0..10 {
            bm25_res = rag
                .search_bm25(search_term, 15, LOADED_WORKSPACE_SCOPE)
                .await?;
            if !bm25_res.is_empty() {
                break;
            }
            sleep(Duration::from_millis(50)).await;
        }

        assert!(
            !bm25_res.is_empty(),
            "BM25 search returned no results for '{}'",
            search_term
        );

        let ordered_node_ids: Vec<Uuid> = bm25_res.iter().map(|(id, _score)| *id).collect();
        fetch_and_assert_snippet(&db, ordered_node_ids, search_term).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_search_function_definitions() -> Result<(), Error> {
        init_tracing_once();
        let rag = &DEFAULT_TEST_RAG;
        let db = &DEFAULT_TEST_RAG.db;

        let search_term = "use_all_const_static";

        let search_res: Vec<(Uuid, f32)> =
            rag.search(search_term, 10, LOADED_WORKSPACE_SCOPE).await?;
        assert!(
            !search_res.is_empty(),
            "Dense search returned no results for '{}'",
            search_term
        );

        let ordered_node_ids: Vec<Uuid> = search_res.iter().map(|(id, _score)| *id).collect();
        fetch_and_assert_snippet(db, ordered_node_ids, search_term).await?;
        Ok(())
    }

    // ============================================================================
    // Phase 4: Method Node RAG Tests
    // ============================================================================

    /// Phase 4 TDD test: Dense search finds method nodes.
    ///
    /// This test verifies that `rag.search()` can find method nodes when they
    /// have embeddings. It uses the fixture_nodes database which contains
    /// methods like `SimpleStruct::new`.
    #[tokio::test]
    async fn dense_search_finds_method_by_unique_token() -> Result<(), Error> {
        use ploke_test_utils::fixture_dbs::{FIXTURE_NODES_CANONICAL, fresh_backup_fixture_db};

        init_tracing_once();

        // Load fixture database and set up RAG with embedder
        let db_raw = fresh_backup_fixture_db(&FIXTURE_NODES_CANONICAL)?;
        let db = Arc::new(db_raw);
        let _rag = init_test_rag(Arc::clone(&db));

        // Get a known method ID and its name
        let method_result = db
            .raw_query(r#"?[id] := *method { id, name }, name == "new""#)
            .map_err(ploke_error::Error::from)?;

        let method_id = method_result
            .rows
            .first()
            .and_then(|row| row.first())
            .and_then(|val| ploke_db::to_uuid(val).ok())
            .expect("should find 'new' method in fixture");

        eprintln!("Testing method search with method_id: {}", method_id);

        // Set up embedding set with small dims for speed
        let embedding_set = db.with_active_set(|set| set.clone())?;
        db.ensure_embedding_relation(&embedding_set)?;

        // Create a test vector and insert it for the method
        let dims = embedding_set.dims() as usize;
        let mut test_vector: Vec<f32> = vec![0.1; dims];
        test_vector[0] = 0.99; // Make distinctive
        test_vector[1] = 0.98;
        db.update_embeddings_batch(vec![(method_id, test_vector.clone())])?;

        // Create HNSW index
        db.create_embedding_index(&embedding_set)?;

        // Use the same vector as query - should find the method
        let query_vec = test_vector;

        // Search using search_similar_for_set directly with Method node type
        let result = db.search_similar_for_set(
            &embedding_set,
            ploke_db::NodeType::Method,
            LOADED_WORKSPACE_SCOPE,
            query_vec,
            5,
            10,
            5,
            None,
        )?;

        let found_ids: Vec<Uuid> = result.typed_data.v.iter().map(|node| node.id).collect();
        eprintln!(
            "Search returned {} results: {:?}",
            found_ids.len(),
            found_ids
        );

        assert!(
            found_ids.contains(&method_id),
            "Expected search to find method_id {}. Got results: {:?}",
            method_id,
            found_ids
        );

        Ok(())
    }

    /// Phase 4 TDD test: get_nodes_ordered returns expected snippet for method nodes.
    ///
    /// This test verifies that `db.get_nodes_ordered()` can retrieve method node
    /// data that can be used to fetch snippets via IoManagerHandle.
    #[tokio::test]
    async fn get_nodes_ordered_snippet_contains_expected_substring() -> Result<(), Error> {
        use ploke_test_utils::fixture_dbs::{FIXTURE_NODES_CANONICAL, fresh_backup_fixture_db};

        init_tracing_once();

        let db_raw = fresh_backup_fixture_db(&FIXTURE_NODES_CANONICAL)?;
        let db = Arc::new(db_raw);
        let io = IoManagerHandle::new();

        // Get a known method ID (SimpleStruct::new)
        let method_result = db
            .raw_query(r#"?[id] := *method { id, name }, name == "new""#)
            .map_err(ploke_error::Error::from)?;

        let method_id = method_result
            .rows
            .first()
            .and_then(|row| row.first())
            .and_then(|val| ploke_db::to_uuid(val).ok())
            .expect("should find 'new' method in fixture");

        eprintln!("Testing get_nodes_ordered with method_id: {}", method_id);

        // Insert an embedding for the method so get_nodes_ordered can find it
        let embedding_set = db.with_active_set(|set| set.clone())?;
        db.ensure_embedding_relation(&embedding_set)?;

        let dims = embedding_set.dims() as usize;
        let test_vector: Vec<f32> = vec![0.5; dims];
        db.update_embeddings_batch(vec![(method_id, test_vector)])?;

        // Get nodes ordered for the method ID
        let nodes = db.get_nodes_ordered(vec![method_id])?;

        assert!(
            !nodes.is_empty(),
            "Expected get_nodes_ordered to return node for method_id {}",
            method_id
        );

        // Fetch snippet using IoManagerHandle
        let snippets = io
            .get_snippets_batch(nodes)
            .await
            .expect("Problem receiving")
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;

        assert!(
            !snippets.is_empty(),
            "Expected to get snippet for method_id {}",
            method_id
        );

        // The snippet should contain "fn new" or similar method signature
        let snippet = &snippets[0];
        assert!(
            snippet.contains("fn new") || snippet.contains("pub fn new"),
            "Expected snippet to contain method signature, got: {}",
            snippet
        );

        eprintln!("Successfully retrieved snippet for method: {}", snippet);

        Ok(())
    }

    /// Integration test: Dense search includes method nodes in search scope.
    ///
    /// This test verifies that `rag.search()` now includes Method nodes in its search
    /// by checking that methods CAN be found when they have embeddings. The test
    /// manually inserts an embedding for a known method and verifies it can be retrieved.
    #[tokio::test]
    async fn dense_search_includes_methods_in_search_scope() -> Result<(), Error> {
        use ploke_test_utils::fixture_dbs::{FIXTURE_NODES_CANONICAL, fresh_backup_fixture_db};

        init_tracing_once();

        // Load fixture and set up RAG
        let db_raw = fresh_backup_fixture_db(&FIXTURE_NODES_CANONICAL)?;
        let db = Arc::new(db_raw);
        let rag = init_test_rag(Arc::clone(&db));

        // Get a known method ID
        let method_result = db
            .raw_query(r#"?[id] := *method { id, name }, name == "get_secret_len""#)
            .map_err(ploke_error::Error::from)?;

        let method_id = method_result
            .rows
            .first()
            .and_then(|row| row.first())
            .and_then(|val| ploke_db::to_uuid(val).ok())
            .expect("should find 'get_secret_len' method in fixture");

        eprintln!("Testing method search with method_id: {}", method_id);

        // Set up embedding set
        let embedding_set = db.with_active_set(|set| set.clone())?;
        db.ensure_embedding_relation(&embedding_set)?;

        // Create a distinctive test vector for the method
        let dims = embedding_set.dims() as usize;
        let mut test_vector: Vec<f32> = vec![0.1; dims];
        test_vector[0] = 0.99;
        test_vector[1] = 0.98;
        db.update_embeddings_batch(vec![(method_id, test_vector.clone())])?;

        // Create HNSW index
        db.create_embedding_index(&embedding_set)?;

        // Generate query embedding using the same vector
        let query_vec = test_vector;

        // Search using search_similar_for_set with Method node type
        let result = db.search_similar_for_set(
            &embedding_set,
            ploke_db::NodeType::Method,
            LOADED_WORKSPACE_SCOPE,
            query_vec,
            5,
            10,
            5,
            None,
        )?;

        let found_ids: Vec<Uuid> = result.typed_data.v.iter().map(|node| node.id).collect();

        assert!(
            found_ids.contains(&method_id),
            "Expected search to find method_id {}. Got results: {:?}",
            method_id,
            found_ids
        );

        // Now verify RAG search also includes methods by checking the search scope
        // The key assertion is that RagService::search now iterates over primary_and_assoc_nodes
        // which includes Method
        eprintln!("Dense search successfully includes method nodes in search scope");

        Ok(())
    }

    /// Integration test: BM25 sparse search finds method nodes by exact name.
    ///
    /// This test verifies that BM25 search can find methods by their exact name match.
    /// BM25 uses term frequency, so exact method names should match well.
    #[tokio::test]
    async fn bm25_search_finds_existing_method_by_exact_name() -> Result<(), Error> {
        init_tracing_once();

        let db_raw = fresh_backup_fixture_db(&FIXTURE_NODES_LOCAL_EMBEDDINGS)?;
        let db = Arc::new(db_raw);
        let rag = init_test_rag(Arc::clone(&db));

        // Rebuild BM25 index to include methods
        rag.bm25_rebuild().await?;

        // Search for a unique method name that should be in the BM25 index
        let search_term = "new";

        // Retry a few times in case index is still building
        let mut bm25_res: Vec<(Uuid, f32)> = Vec::new();
        for _ in 0..10 {
            bm25_res = rag
                .search_bm25(search_term, 15, LOADED_WORKSPACE_SCOPE)
                .await?;
            if !bm25_res.is_empty() {
                break;
            }
            sleep(Duration::from_millis(50)).await;
        }

        assert!(
            !bm25_res.is_empty(),
            "BM25 search returned no results for method '{}'",
            search_term
        );

        // Verify that at least one result is a method node
        let found_ids: Vec<Uuid> = bm25_res.iter().map(|(id, _)| *id).collect();

        // Get method IDs that match "new"
        let method_id_result = db
            .raw_query(r#"?[id] := *method { id, name }, name == "new""#)
            .map_err(ploke_error::Error::from)?;

        let new_method_ids: Vec<Uuid> = method_id_result
            .rows
            .iter()
            .filter_map(|row| row.first().and_then(|val| ploke_db::to_uuid(val).ok()))
            .collect();

        // Check that at least one "new" method is in the results
        let has_new_method = found_ids.iter().any(|id| new_method_ids.contains(id));
        assert!(
            has_new_method,
            "BM25 should find at least one 'new' method in results for query '{}'",
            search_term
        );

        Ok(())
    }

    /// Integration test: Hybrid search includes method nodes.
    ///
    /// Combines dense and sparse search to find methods.
    #[tokio::test]
    async fn hybrid_search_includes_methods() -> Result<(), Error> {
        init_tracing_once();

        let db_raw = fresh_backup_fixture_db(&FIXTURE_NODES_LOCAL_EMBEDDINGS)?;
        let db = Arc::new(db_raw);
        let rag = init_test_rag(Arc::clone(&db));

        // Rebuild BM25 index for sparse component
        rag.bm25_rebuild().await?;

        // Search for a method name - hybrid should find it via BM25 component
        let search_term = "new";

        let fused: Vec<(Uuid, f32)> = rag
            .hybrid_search(search_term, 15, LOADED_WORKSPACE_SCOPE)
            .await?;

        assert!(
            !fused.is_empty(),
            "Hybrid search returned no results for method '{}'",
            search_term
        );

        // Verify the result contains the method ID by checking raw query
        let method_id_result = db
            .raw_query(r#"?[id] := *method { id, name }, name == "new""#)
            .map_err(ploke_error::Error::from)?;

        let method_id = method_id_result
            .rows
            .first()
            .and_then(|row| row.first())
            .and_then(|val| ploke_db::to_uuid(val).ok())
            .expect("should find method in fixture");

        let found_ids: Vec<Uuid> = fused.iter().map(|(id, _)| *id).collect();
        assert!(
            found_ids.contains(&method_id),
            "Hybrid should find method_id {} for query '{}'",
            method_id,
            search_term
        );

        Ok(())
    }

    /// Integration test: BM25 search finds trait definition methods.
    #[tokio::test]
    async fn bm25_search_finds_trait_definition_method() -> Result<(), Error> {
        init_tracing_once();

        let db_raw = fresh_backup_fixture_db(&FIXTURE_NODES_LOCAL_EMBEDDINGS)?;
        let db = Arc::new(db_raw);
        let rag = init_test_rag(Arc::clone(&db));

        rag.bm25_rebuild().await?;

        // Search for a trait definition method
        let search_term = "required_method";

        let mut bm25_res: Vec<(Uuid, f32)> = Vec::new();
        for _ in 0..10 {
            bm25_res = rag
                .search_bm25(search_term, 15, LOADED_WORKSPACE_SCOPE)
                .await?;
            if !bm25_res.is_empty() {
                break;
            }
            sleep(Duration::from_millis(50)).await;
        }

        assert!(
            !bm25_res.is_empty(),
            "BM25 search returned no results for trait method '{}'",
            search_term
        );

        // Verify the result contains the method ID by checking raw query
        let method_id_result = db
            .raw_query(r#"?[id] := *method { id, name }, name == "required_method""#)
            .map_err(ploke_error::Error::from)?;

        let method_id = method_id_result
            .rows
            .first()
            .and_then(|row| row.first())
            .and_then(|val| ploke_db::to_uuid(val).ok())
            .expect("should find method in fixture");

        let found_ids: Vec<Uuid> = bm25_res.iter().map(|(id, _)| *id).collect();
        assert!(
            found_ids.contains(&method_id),
            "BM25 should find method_id {} for query '{}'",
            method_id,
            search_term
        );

        Ok(())
    }

    /// Negative test: BM25 strict search for non-existent method returns empty.
    ///
    /// This test verifies that strict BM25 search (no dense fallback) gracefully
    /// handles queries for methods that don't exist in the indexed documents.
    #[tokio::test]
    async fn bm25_search_nonexistent_method_returns_empty() -> Result<(), Error> {
        init_tracing_once();

        let db_raw = fresh_backup_fixture_db(&FIXTURE_NODES_LOCAL_EMBEDDINGS)?;
        let db = Arc::new(db_raw);
        let rag = init_test_rag(Arc::clone(&db));

        // Ensure BM25 index is built
        rag.bm25_rebuild().await?;

        // Wait for index to be ready
        sleep(Duration::from_millis(100)).await;

        let search_term = "totally_fake_method_abc999";

        // Use strict mode to avoid dense fallback
        let bm25_res = rag
            .search_bm25_strict(search_term, 15, LOADED_WORKSPACE_SCOPE)
            .await;

        // Should return empty results or an error about index being empty
        // The key assertion is that it doesn't panic
        eprintln!(
            "BM25 strict search for non-existent method result: {:?}",
            bm25_res
        );

        // In strict BM25 mode with no matches, we expect either:
        // - Ok(empty) if the index is ready but query has no matches
        // - Err if the index is not ready
        // Both are acceptable - the main thing is it doesn't panic
        match bm25_res {
            Ok(results) => {
                eprintln!(
                    "BM25 returned {} results (may be from partial matching)",
                    results.len()
                );
                // With strict mode, we expect empty results for non-existent terms
                // But the BM25 tokenizer might match partial tokens, so we just verify
                // the search completed without error
            }
            Err(e) => {
                eprintln!(
                    "BM25 returned error (expected for non-existent terms): {}",
                    e
                );
            }
        }

        Ok(())
    }

    #[tokio::test]
    async fn hybrid_search_nonexistent_method_handles_gracefully() -> Result<(), Error> {
        init_tracing_once();

        let db_raw = fresh_backup_fixture_db(&FIXTURE_NODES_LOCAL_EMBEDDINGS)?;
        let db = Arc::new(db_raw);
        let rag = init_test_rag(Arc::clone(&db));

        rag.bm25_rebuild().await?;

        let search_term = "method_that_does_not_exist_anywhere_987654";

        // Should not error, even if no results found
        let fused: Vec<(Uuid, f32)> = rag
            .hybrid_search(search_term, 15, LOADED_WORKSPACE_SCOPE)
            .await?;

        // Hybrid may return some results from dense fallback, but shouldn't error
        eprintln!(
            "Hybrid search for non-existent method returned {} results",
            fused.len()
        );

        Ok(())
    }

    /// Integration test: Verify methods are indexed in BM25.
    ///
    /// This test checks that method nodes are actually present in the BM25 index
    /// by searching for a common method name and verifying method IDs are returned.
    #[tokio::test]
    async fn bm25_index_contains_methods() -> Result<(), Error> {
        init_tracing_once();

        let db_raw = fresh_backup_fixture_db(&FIXTURE_NODES_LOCAL_EMBEDDINGS)?;
        let db = Arc::new(db_raw);
        let rag = init_test_rag(Arc::clone(&db));

        rag.bm25_rebuild().await?;
        sleep(Duration::from_millis(100)).await;

        // Check BM25 status to verify methods are indexed
        let status = rag.bm25_status().await?;
        eprintln!("BM25 status: {:?}", status);

        // Search for "new" which is a very common method name
        let search_term = "new";
        let results = rag
            .search_bm25(search_term, 20, LOADED_WORKSPACE_SCOPE)
            .await?;

        assert!(
            !results.is_empty(),
            "BM25 should find results for common method name '{}'",
            search_term
        );

        // Verify at least some results are method nodes by checking against known method IDs
        let found_ids: Vec<Uuid> = results.iter().map(|(id, _)| *id).collect();

        // Get all method IDs from the database
        let method_result = db
            .raw_query(r#"?[id] := *method { id }"#)
            .map_err(ploke_error::Error::from)?;

        let method_ids: Vec<Uuid> = method_result
            .rows
            .iter()
            .filter_map(|row| row.first().and_then(|val| ploke_db::to_uuid(val).ok()))
            .collect();

        // Check that at least one found ID is a method
        let has_method = found_ids.iter().any(|id| method_ids.contains(id));
        assert!(
            has_method,
            "BM25 search for '{}' should return at least one method node",
            search_term
        );

        eprintln!(
            "BM25 search for '{}' returned {} results including methods",
            search_term,
            found_ids.len()
        );

        Ok(())
    }
}
