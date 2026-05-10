use cozo::{DataValue, Db, MemStorage};
use ploke_db::{Database, DbError, TypeRelationKind, to_uuid};
use ploke_transform::{schema::create_schema_all, transform::transform_parsed_graph};
use uuid::Uuid;

pub(super) fn setup_typed_fixture_db(fixture: &'static str) -> Result<Database, DbError> {
    setup_typed_db_from_parser_output(
        syn_parser::ParserOutput {
            merged_graph: Some(
                syn_parser::parser::ParsedCodeGraph::merge_new(
                    ploke_test_utils::test_run_phases_and_collect(fixture),
                )
                .map_err(|err| DbError::QueryExecution(err.to_string()))?,
            ),
            module_tree: None,
            parsed_graphs_for_masks: None,
            compilation_units: None,
        },
        Some(fixture),
    )
}

pub(super) fn setup_typed_corpus_db(corpus_dir: &'static str) -> Result<Database, DbError> {
    let root = ploke_common::fixture_github_clones_dir().join(corpus_dir);
    let output = syn_parser::try_run_phases_and_merge(&root)
        .map_err(|err| DbError::QueryExecution(err.to_string()))?;
    setup_typed_db_from_parser_output(output, None)
}

pub(super) fn setup_typed_backup_db(
    fixture: &'static ploke_test_utils::FixtureDb,
) -> Result<Database, DbError> {
    ploke_test_utils::fresh_backup_fixture_db(fixture)
        .map_err(|err| DbError::QueryExecution(err.to_string()))
}

fn setup_typed_db_from_parser_output(
    mut output: syn_parser::ParserOutput,
    fixture_name_for_tree: Option<&'static str>,
) -> Result<Database, DbError> {
    let db = Db::new(MemStorage::default()).expect("in-memory cozo db");
    db.initialize().expect("initialize cozo db");
    create_schema_all(&db).map_err(|err| DbError::QueryExecution(err.to_string()))?;

    let mut merged = output
        .extract_merged_graph()
        .ok_or_else(|| DbError::QueryExecution("parser output missing merged graph".into()))?;
    let tree = match output.extract_module_tree() {
        Some(tree) => tree,
        None => match fixture_name_for_tree {
            Some(_) => merged
                .build_tree_and_prune()
                .map_err(|err| DbError::QueryExecution(err.to_string()))?,
            None => {
                return Err(DbError::QueryExecution(
                    "parser output missing module tree".into(),
                ));
            }
        },
    };
    transform_parsed_graph(&db, merged, &tree)
        .map_err(|err| DbError::QueryExecution(err.to_string()))?;

    Ok(Database::new(db))
}

pub(super) fn exactly_one_uuid(
    db: &Database,
    script: &str,
    column: usize,
) -> Result<Uuid, DbError> {
    let rows = db.raw_query(script)?;
    assert_eq!(
        rows.rows.len(),
        1,
        "expected exactly one row for query:\n{script}\nrows: {:#?}",
        rows.rows
    );
    to_uuid(&rows.rows[0][column])
}

pub(super) fn function_id_by_name(db: &Database, name: &str) -> Result<Uuid, DbError> {
    exactly_one_uuid(
        db,
        &format!(
            r#"?[id] :=
                *function {{ id, name: "{name}" @ 'NOW' }}"#
        ),
        0,
    )
}

pub(super) fn function_id_by_name_in_module(
    db: &Database,
    module_path: &[&str],
    name: &str,
) -> Result<Uuid, DbError> {
    let module_path = cozo_string_list(module_path);
    exactly_one_uuid(
        db,
        &format!(
            r#"?[id] :=
                *function {{ id, name: "{name}", module_id @ 'NOW' }},
                *module {{ id: module_id, path: {module_path} @ 'NOW' }}"#
        ),
        0,
    )
}

pub(super) fn function_id_by_name_in_file_suffix(
    db: &Database,
    file_suffix: &str,
    name: &str,
) -> Result<Uuid, DbError> {
    let rows = db.raw_query(&format!(
        r#"?[id, file_path] :=
            *function {{ id, name: "{name}", module_id @ 'NOW' }},
            *file_mod {{ owner_id: module_id, file_path @ 'NOW' }}"#
    ))?;
    let matching: Vec<_> = rows
        .rows
        .iter()
        .filter_map(|row| {
            let file_path = match &row[1] {
                DataValue::Str(path) => path.as_str(),
                _ => return None,
            };
            file_path.ends_with(file_suffix).then(|| row[0].clone())
        })
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "expected exactly one function named {name} in file suffix {file_suffix}; rows: {:#?}",
        rows.rows
    );
    to_uuid(&matching[0])
}

pub(super) fn function_param_type_by_name(
    db: &Database,
    name: &str,
    param_index: u32,
) -> Result<Uuid, DbError> {
    let owner_id = function_id_by_name(db, name)?;
    exactly_one_uuid(
        db,
        &format!(
            r#"?[type_id] :=
                *function {{ id: function_id, name: "{name}" @ 'NOW' }},
                *param {{
                    function_id,
                    param_index: {param_index},
                    type_id @ 'NOW'
                }},
                function_id = to_uuid("{owner_id}")"#
        ),
        0,
    )
}

pub(super) fn function_return_type_by_name_in_module(
    db: &Database,
    module_path: &[&str],
    name: &str,
) -> Result<Uuid, DbError> {
    let module_path = cozo_string_list(module_path);
    exactly_one_uuid(
        db,
        &format!(
            r#"?[type_id] :=
                *function {{ name: "{name}", return_type_id: type_id, module_id @ 'NOW' }},
                *module {{ id: module_id, path: {module_path} @ 'NOW' }}"#
        ),
        0,
    )
}

pub(super) fn type_alias_row_by_name(db: &Database, name: &str) -> Result<(Uuid, Uuid), DbError> {
    let rows = db.raw_query(&format!(
        r#"?[id, type_id] :=
            *type_alias {{ id, name: "{name}", ty_id: type_id @ 'NOW' }}"#
    ))?;
    assert_eq!(
        rows.rows.len(),
        1,
        "expected exactly one type alias named {name}, rows: {:#?}",
        rows.rows
    );
    Ok((to_uuid(&rows.rows[0][0])?, to_uuid(&rows.rows[0][1])?))
}

pub(super) fn struct_id_by_name(db: &Database, name: &str) -> Result<Uuid, DbError> {
    exactly_one_uuid(
        db,
        &format!(
            r#"?[id] :=
                *struct {{ id, name: "{name}" @ 'NOW' }}"#
        ),
        0,
    )
}

pub(super) fn struct_id_by_name_in_file_suffix(
    db: &Database,
    file_suffix: &str,
    name: &str,
) -> Result<Uuid, DbError> {
    let rows = db.raw_query(&format!(
        r#"?[id, file_path] :=
            *struct {{ id, name: "{name}" @ 'NOW' }},
            *syntax_edge {{
                source_id: module_id,
                target_id: id,
                relation_kind: "Contains" @ 'NOW'
            }},
            *file_mod {{ owner_id: module_id, file_path @ 'NOW' }}"#
    ))?;
    let matching: Vec<_> = rows
        .rows
        .iter()
        .filter_map(|row| {
            let file_path = match &row[1] {
                DataValue::Str(path) => path.as_str(),
                _ => return None,
            };
            file_path.ends_with(file_suffix).then(|| row[0].clone())
        })
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "expected exactly one struct named {name} in file suffix {file_suffix}; rows: {:#?}",
        rows.rows
    );
    to_uuid(&matching[0])
}

pub(super) fn struct_id_by_name_in_module(
    db: &Database,
    module_path: &[&str],
    name: &str,
) -> Result<Uuid, DbError> {
    let module_path = cozo_string_list(module_path);
    exactly_one_uuid(
        db,
        &format!(
            r#"?[id] :=
                *module {{ id: module_id, path: {module_path} @ 'NOW' }},
                *syntax_edge {{
                    source_id: module_id,
                    target_id: id,
                    relation_kind: "Contains" @ 'NOW'
                }},
                *struct {{ id, name: "{name}" @ 'NOW' }}"#
        ),
        0,
    )
}

pub(super) fn enum_id_by_name(db: &Database, name: &str) -> Result<Uuid, DbError> {
    exactly_one_uuid(
        db,
        &format!(
            r#"?[id] :=
                *enum {{ id, name: "{name}" @ 'NOW' }}"#
        ),
        0,
    )
}

pub(super) fn field_id_by_owner_index(
    db: &Database,
    owner_id: Uuid,
    index: u32,
) -> Result<Uuid, DbError> {
    exactly_one_uuid(
        db,
        &format!(
            r#"?[id] :=
                *field {{
                    id,
                    owner_id: to_uuid("{owner_id}"),
                    index: {index} @ 'NOW'
                }}"#
        ),
        0,
    )
}

pub(super) fn generic_type_param_id_by_owner_name(
    db: &Database,
    owner_id: Uuid,
    name: &str,
) -> Result<Uuid, DbError> {
    exactly_one_uuid(
        db,
        &format!(
            r#"?[generic_id] :=
                *generic_type {{
                    id: generic_id,
                    owner_id: to_uuid("{owner_id}"),
                    name: "{name}" @ 'NOW'
                }}"#
        ),
        0,
    )
}

pub(super) fn method_id_by_impl_self_type_name(
    db: &Database,
    self_type_name: &str,
    method_name: &str,
) -> Result<Uuid, DbError> {
    exactly_one_uuid(
        db,
        &format!(
            r#"?[method_id] :=
                *method {{ id: method_id, name: "{method_name}", owner_id: impl_id @ 'NOW' }},
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
                *struct {{ id: self_target_id, name: "{self_type_name}" @ 'NOW' }}"#
        ),
        0,
    )
}

pub(super) fn impl_id_by_trait_and_self_type_names(
    db: &Database,
    trait_name: &str,
    self_type_name: &str,
) -> Result<Uuid, DbError> {
    exactly_one_uuid(
        db,
        &format!(
            r#"?[impl_id] :=
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
                *struct {{ id: self_target_id, name: "{self_type_name}" @ 'NOW' }},
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
        ),
        0,
    )
}

pub(super) fn method_id_by_impl_trait_and_self_type_names(
    db: &Database,
    trait_name: &str,
    self_type_name: &str,
    method_name: &str,
) -> Result<Uuid, DbError> {
    exactly_one_uuid(
        db,
        &format!(
            r#"?[method_id] :=
                *method {{ id: method_id, name: "{method_name}", owner_id: impl_id @ 'NOW' }},
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
                *struct {{ id: self_target_id, name: "{self_type_name}" @ 'NOW' }},
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
        ),
        0,
    )
}

pub(super) fn const_id_by_name_in_file_suffix(
    db: &Database,
    file_suffix: &str,
    name: &str,
) -> Result<Uuid, DbError> {
    item_id_by_name_in_file_suffix(db, "const", file_suffix, name)
}

pub(super) fn static_id_by_name_in_file_suffix(
    db: &Database,
    file_suffix: &str,
    name: &str,
) -> Result<Uuid, DbError> {
    item_id_by_name_in_file_suffix(db, "static", file_suffix, name)
}

fn item_id_by_name_in_file_suffix(
    db: &Database,
    relation: &str,
    file_suffix: &str,
    name: &str,
) -> Result<Uuid, DbError> {
    let rows = db.raw_query(&format!(
        r#"?[id, file_path] :=
            *{relation} {{ id, name: "{name}" @ 'NOW' }},
            *syntax_edge {{
                source_id: module_id,
                target_id: id,
                relation_kind: "Contains" @ 'NOW'
            }},
            *file_mod {{ owner_id: module_id, file_path @ 'NOW' }}"#
    ))?;
    let matching: Vec<_> = rows
        .rows
        .iter()
        .filter_map(|row| {
            let file_path = match &row[1] {
                DataValue::Str(path) => path.as_str(),
                _ => return None,
            };
            file_path.ends_with(file_suffix).then(|| row[0].clone())
        })
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "expected exactly one {relation} named {name} in file suffix {file_suffix}; rows: {:#?}",
        rows.rows
    );
    to_uuid(&matching[0])
}

pub(super) fn assert_owner_reaches_target(
    db: &Database,
    owner_id: Uuid,
    target_id: Uuid,
    relation_kind: TypeRelationKind,
    min_depth: u32,
    message: &str,
) -> Result<(), DbError> {
    let reachable = db.type_targets_reachable_from_owner(owner_id)?;
    assert!(
        reachable.iter().any(|path| {
            path.owner_id == owner_id
                && path.target_id == target_id
                && path.relation_kind == relation_kind
                && path.depth >= min_depth
        }),
        "{message}; owner_id: {owner_id}; target_id: {target_id}; reachable: {reachable:#?}"
    );
    Ok(())
}

pub(super) fn trait_id_by_name_in_module(
    db: &Database,
    module_path: &[&str],
    name: &str,
) -> Result<Uuid, DbError> {
    let module_path = cozo_string_list(module_path);
    exactly_one_uuid(
        db,
        &format!(
            r#"?[id] :=
                *module {{ id: module_id, path: {module_path} @ 'NOW' }},
                *syntax_edge {{
                    source_id: module_id,
                    target_id: id,
                    relation_kind: "Contains" @ 'NOW'
                }},
                *trait {{ id, name: "{name}" @ 'NOW' }}"#
        ),
        0,
    )
}

pub(super) fn cozo_string_list(items: &[&str]) -> String {
    format!(
        "[{}]",
        items
            .iter()
            .map(|item| format!("\"{item}\""))
            .collect::<Vec<_>>()
            .join(", ")
    )
}
