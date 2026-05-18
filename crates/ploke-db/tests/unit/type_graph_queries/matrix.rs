//! Corpus-backed TypeNode coverage matrix.
//!
//! The shared matrix in `ploke_test_utils::type_shape_matrix` is the source of
//! truth for cross-pipeline type-shape coverage. These DB tests own the strict
//! structural assertions: exact root role, source coordinate, terminal target,
//! relation family, and containment depth. RAG and TUI tests consume the same
//! cases but only for matrix rows whose owners and terminals are materializable
//! through those consumer APIs.

use cozo::DataValue;
use ploke_db::{Database, DbError, TypeUseCoordinate, TypeUseRole, TypeUseRoot, to_uuid};
use ploke_test_utils::{
    ContainingOwnerSelector, CoordinateSpec, OwnerSelector, TargetSelector, TypeShapeCase,
    TypeShapeNoTargetCase, absent_type_shape_cases, no_target_type_shape_cases,
    positive_type_shape_cases,
};
use uuid::Uuid;

use super::common::{
    enum_id_by_name, exactly_one_uuid, field_id_by_owner_index, function_id_by_name_in_file_suffix,
    function_id_by_name_in_module, generic_type_param_id_by_owner_name,
    method_id_by_impl_self_type_name, method_id_by_impl_trait_and_self_type_names,
    setup_typed_backup_db, struct_id_by_name, struct_id_by_name_in_module,
    trait_id_by_name_in_module, type_alias_row_by_name,
};

#[test]
fn corpus_matrix_positive_cases_have_exact_roots_and_terminals() -> Result<(), DbError> {
    for case in positive_type_shape_cases() {
        let db = setup_typed_backup_db(case.fixture.fixture())?;
        assert_positive_case(&db, case)?;
    }
    Ok(())
}

#[test]
fn corpus_matrix_no_target_cases_retain_roots_without_bogus_targets() -> Result<(), DbError> {
    for case in no_target_type_shape_cases() {
        let db = setup_typed_backup_db(case.fixture.fixture())?;
        assert_no_target_case(&db, case)?;
    }
    Ok(())
}

#[test]
fn corpus_matrix_absent_fallback_relations_remain_absent_in_registered_backups()
-> Result<(), DbError> {
    for fixture in [
        ploke_test_utils::CorpusFixture::Semver,
        ploke_test_utils::CorpusFixture::Memchr,
        ploke_test_utils::CorpusFixture::GenericArray,
        ploke_test_utils::CorpusFixture::Chrono,
        ploke_test_utils::CorpusFixture::Axum,
    ] {
        let fixture = fixture.fixture();
        let db = setup_typed_backup_db(fixture)?;
        for (kind, relation) in absent_type_shape_cases() {
            let rows = db.raw_query(&format!(
                r#"?[type_id] := *{relation} {{ type_id @ 'NOW' }}"#
            ))?;
            assert!(
                rows.rows.is_empty(),
                "{kind:?} relation {relation} should be absent in fixture {}; rows: {:#?}",
                fixture.id,
                rows.rows
            );
        }
    }
    Ok(())
}

fn assert_positive_case(db: &Database, case: &TypeShapeCase) -> Result<(), DbError> {
    let owner_id = resolve_owner(db, case.owner)?;
    let coordinate = resolve_coordinate(db, case.coordinate)?;
    let root = exactly_one_root(db, owner_id, case.role, &coordinate, case.name)?;
    let target_id = resolve_target(db, case.terminal, owner_id, &root, case)?;

    let reachable = db.type_targets_reachable_from_owner(owner_id)?;
    assert!(
        reachable.iter().any(|path| {
            path.owner_id == owner_id
                && path.type_use_id == root.id
                && path.root_type_id == root.root_type_id
                && path.target_id == target_id
                && path.relation_kind == case.relation_kind
                && path.depth == case.depth
        }),
        "{} should reach exact terminal target; source: {}; root: {root:#?}; target_id: {target_id}; reachable: {reachable:#?}",
        case.name,
        case.source
    );
    Ok(())
}

fn assert_no_target_case(db: &Database, case: &TypeShapeNoTargetCase) -> Result<(), DbError> {
    let owner_id = resolve_owner(db, case.owner)?;
    let coordinate = resolve_coordinate(db, case.coordinate)?;
    let root = exactly_one_root(db, owner_id, case.role, &coordinate, case.name)?;

    let rows = db.raw_query(&format!(
        r#"?[type_id] :=
            type_id = to_uuid("{}"),
            *{} {{ type_id @ 'NOW' }}"#,
        root.root_type_id, case.root_relation
    ))?;
    assert_eq!(
        rows.rows.len(),
        1,
        "{} should retain its {} root; source: {}; rows: {:#?}",
        case.name,
        case.root_relation,
        case.source,
        rows.rows
    );

    let no_target_type_id = if let Some(nested) = case.nested_relation {
        let mut candidates = vec![root.root_type_id];
        for kind in nested.containment_path {
            let mut next = Vec::new();
            for parent in &candidates {
                let rows = db.raw_query(&format!(
                    r#"?[child_type_id] :=
                        *type_contains {{
                            parent_type_id: to_uuid("{parent}"),
                            child_type_id,
                            kind: "{kind}" @ 'NOW'
                        }}"#
                ))?;
                for row in rows.rows {
                    next.push(to_uuid(&row[0])?);
                }
            }
            next.sort();
            next.dedup();
            assert!(
                !next.is_empty(),
                "{} should retain containment step {kind} from candidates {candidates:#?}; source: {}",
                case.name,
                case.source
            );
            candidates = next;
        }

        let mut matching = Vec::new();
        for candidate in &candidates {
            let rows = db.raw_query(&format!(
                r#"?[type_id] :=
                    type_id = to_uuid("{candidate}"),
                    *{} {{ type_id @ 'NOW' }}"#,
                nested.relation
            ))?;
            assert!(
                rows.rows.len() <= 1,
                "{} should have at most one nested {} row for {}; source: {}; rows: {:#?}",
                case.name,
                nested.relation,
                candidate,
                case.source,
                rows.rows
            );
            if !rows.rows.is_empty() {
                matching.push(*candidate);
            }
        }
        assert_eq!(
            matching.len(),
            1,
            "{} should retain exactly one nested {} at containment path {:?}; source: {}; candidates: {candidates:#?}",
            case.name,
            nested.relation,
            nested.containment_path,
            case.source
        );
        matching[0]
    } else {
        root.root_type_id
    };

    let bogus_relations = db.raw_query(&format!(
        r#"?[target_id, relation_kind] :=
            *type_relation {{
                source_id: to_uuid("{no_target_type_id}"),
                target_id,
                relation_kind @ 'NOW'
            }}"#
    ))?;
    assert!(
        bogus_relations.rows.is_empty(),
        "{} must not fabricate a terminal type_relation for {}; rows: {:#?}",
        case.name,
        no_target_type_id,
        bogus_relations.rows
    );

    if case.nested_relation.is_none() {
        let reachable = db.type_targets_reachable_from_owner(owner_id)?;
        assert!(
            reachable
                .iter()
                .all(|path| path.type_use_id != root.id && path.root_type_id != root.root_type_id),
            "{} must not fabricate a terminal target for root {}; reachable: {reachable:#?}",
            case.name,
            root.root_type_id
        );
    }
    Ok(())
}

fn exactly_one_root(
    db: &Database,
    owner_id: Uuid,
    role: TypeUseRole,
    coordinate: &TypeUseCoordinate,
    case_name: &str,
) -> Result<TypeUseRoot, DbError> {
    let roots = db.type_uses_for_owner(owner_id)?;
    let matching = roots
        .iter()
        .filter(|root| root.role == role && &root.coordinate == coordinate)
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "{case_name} expected exactly one root for owner {owner_id}, role {role:?}, coordinate {coordinate:?}; roots: {roots:#?}"
    );
    Ok(matching[0].clone())
}

fn resolve_owner(db: &Database, selector: OwnerSelector) -> Result<Uuid, DbError> {
    match selector {
        OwnerSelector::FunctionInModule { module_path, name } => {
            function_id_by_name_in_module(db, module_path, name)
        }
        OwnerSelector::FunctionInFile { file_suffix, name } => {
            function_id_by_name_in_file_suffix(db, file_suffix, name)
        }
        OwnerSelector::MethodByImplSelf { self_type, method } => {
            method_id_by_impl_self_type_name(db, self_type, method)
        }
        OwnerSelector::MethodByImplTraitAndSelf {
            trait_name,
            self_type,
            method,
        } => method_id_by_impl_trait_and_self_type_names(db, trait_name, self_type, method),
        OwnerSelector::MethodByRawPointerImpl {
            file_suffix,
            trait_name,
            mutable,
            method,
        } => method_id_by_raw_pointer_impl(db, file_suffix, trait_name, mutable, method),
        OwnerSelector::FieldByStructInModule {
            module_path,
            struct_name,
            field_index,
        } => {
            let struct_id = struct_id_by_name_in_module(db, module_path, struct_name)?;
            field_id_by_owner_index(db, struct_id, field_index)
        }
        OwnerSelector::FieldByStructInFile {
            file_suffix,
            struct_name,
            field_index,
        } => {
            let struct_id = struct_id_by_name_in_file_suffix(db, file_suffix, struct_name)?;
            field_id_by_owner_index(db, struct_id, field_index)
        }
        OwnerSelector::TypeAlias { name } => type_alias_row_by_name(db, name).map(|row| row.0),
        OwnerSelector::ConstInFile { file_suffix, name } => {
            super::common::const_id_by_name_in_file_suffix(db, file_suffix, name)
        }
        OwnerSelector::StaticInFile { file_suffix, name } => {
            super::common::static_id_by_name_in_file_suffix(db, file_suffix, name)
        }
        OwnerSelector::StructByName { name } => struct_id_by_name(db, name),
        OwnerSelector::TraitInModule { module_path, name } => {
            trait_id_by_name_in_module(db, module_path, name)
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
        } => impl_id_by_trait_name_in_file_suffix(db, file_suffix, trait_name),
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
    }
}

fn resolve_containing_owner(
    db: &Database,
    selector: ContainingOwnerSelector,
) -> Result<Uuid, DbError> {
    match selector {
        ContainingOwnerSelector::StructByName { name } => struct_id_by_name(db, name),
        ContainingOwnerSelector::StructInModule { module_path, name } => {
            struct_id_by_name_in_module(db, module_path, name)
        }
        ContainingOwnerSelector::MethodByImplSelf { self_type, method } => {
            method_id_by_impl_self_type_name(db, self_type, method)
        }
        ContainingOwnerSelector::MethodByImplTraitAndSelf {
            trait_name,
            self_type,
            method,
        } => method_id_by_impl_trait_and_self_type_names(db, trait_name, self_type, method),
        ContainingOwnerSelector::TraitInModule { module_path, name } => {
            trait_id_by_name_in_module(db, module_path, name)
        }
        ContainingOwnerSelector::ImplByTraitInFile {
            file_suffix,
            trait_name,
        } => impl_id_by_trait_name_in_file_suffix(db, file_suffix, trait_name),
    }
}

fn resolve_target(
    db: &Database,
    selector: TargetSelector,
    owner_id: Uuid,
    root: &TypeUseRoot,
    case: &TypeShapeCase,
) -> Result<Uuid, DbError> {
    match selector {
        TargetSelector::StructByName { name } => struct_id_by_name(db, name),
        TargetSelector::StructInModule { module_path, name } => {
            struct_id_by_name_in_module(db, module_path, name)
        }
        TargetSelector::EnumByName { name } => enum_id_by_name(db, name),
        TargetSelector::TraitInModule { module_path, name } => {
            trait_id_by_name_in_module(db, module_path, name)
        }
        TargetSelector::TraitInFile { file_suffix, name } => {
            trait_id_by_name_in_file_suffix(db, file_suffix, name)
        }
        TargetSelector::TypeAlias { name } => type_alias_row_by_name(db, name).map(|row| row.0),
        TargetSelector::Union { name } => union_id_by_name(db, name),
        TargetSelector::GenericParamReachableByName { name } => {
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

fn union_id_by_name(db: &Database, name: &str) -> Result<Uuid, DbError> {
    exactly_one_uuid(
        db,
        &format!(
            r#"?[id] :=
                *union {{ id, name: "{name}" @ 'NOW' }}"#
        ),
        0,
    )
}

fn resolve_coordinate(db: &Database, spec: CoordinateSpec) -> Result<TypeUseCoordinate, DbError> {
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

fn struct_id_by_name_in_file_suffix(
    db: &Database,
    file_suffix: &str,
    struct_name: &str,
) -> Result<Uuid, DbError> {
    let rows = db.raw_query(&format!(
        r#"?[struct_id, file_path] :=
            *struct {{ id: struct_id, name: "{struct_name}" @ 'NOW' }},
            *syntax_edge {{
                source_id: module_id,
                target_id: struct_id,
                relation_kind: "Contains" @ 'NOW'
            }},
            *file_mod {{ owner_id: module_id, file_path @ 'NOW' }}"#
    ))?;
    exactly_one_id_matching_file_suffix(rows.rows, "struct", struct_name, file_suffix)
}

fn trait_id_by_name_in_file_suffix(
    db: &Database,
    file_suffix: &str,
    trait_name: &str,
) -> Result<Uuid, DbError> {
    let rows = db.raw_query(&format!(
        r#"?[trait_id, file_path] :=
            *trait {{ id: trait_id, name: "{trait_name}" @ 'NOW' }},
            *syntax_edge {{
                source_id: module_id,
                target_id: trait_id,
                relation_kind: "Contains" @ 'NOW'
            }},
            *file_mod {{ owner_id: module_id, file_path @ 'NOW' }}"#
    ))?;
    exactly_one_id_matching_file_suffix(rows.rows, "trait", trait_name, file_suffix)
}

fn method_id_by_raw_pointer_impl(
    db: &Database,
    file_suffix: &str,
    trait_name: &str,
    mutable: bool,
    method_name: &str,
) -> Result<Uuid, DbError> {
    let rows = db.raw_query(&format!(
        r#"?[method_id, file_path] :=
            *method {{ id: method_id, name: "{method_name}", owner_id: impl_id @ 'NOW' }},
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
    ))?;
    exactly_one_id_matching_file_suffix(rows.rows, "method", method_name, file_suffix)
}

fn impl_id_by_trait_name_in_file_suffix(
    db: &Database,
    file_suffix: &str,
    trait_name: &str,
) -> Result<Uuid, DbError> {
    let rows = db.raw_query(&format!(
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
    ))?;
    exactly_one_id_matching_file_suffix(rows.rows, "impl", trait_name, file_suffix)
}

fn where_generic_param_bound_owner(
    db: &Database,
    containing_owner_id: Uuid,
    predicate_index: u32,
    bound_index: u32,
    target_trait: TargetSelector,
) -> Result<Uuid, DbError> {
    let target_id = match target_trait {
        TargetSelector::TraitInModule { module_path, name } => {
            trait_id_by_name_in_module(db, module_path, name)?
        }
        other => {
            return Err(DbError::QueryExecution(format!(
                "WhereGenericParamBoundOwner target must be a trait, got {other:?}"
            )));
        }
    };
    exactly_one_uuid(
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
        0,
    )
}

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

fn exactly_one_id_matching_file_suffix(
    rows: Vec<Vec<DataValue>>,
    item_kind: &str,
    item_name: &str,
    file_suffix: &str,
) -> Result<Uuid, DbError> {
    let matching = rows
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
        "expected exactly one {item_kind} for {item_name} in file suffix {file_suffix}; rows: {rows:#?}"
    );
    to_uuid(&matching[0])
}
