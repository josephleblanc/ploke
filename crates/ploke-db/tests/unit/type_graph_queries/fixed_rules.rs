use cozo::DataValue;
use ploke_db::DbError;

use super::common::{function_id_by_name, setup_typed_fixture_db};

#[test]
fn type_target_paths_fixed_rule_matches_recursive_datalog_for_owner() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_type_resolution_v2")?;
    let owner_id = function_id_by_name(&db, "takes_vec_of_t")?;

    let recursive = db.raw_query(&target_paths_recursive_query(owner_id))?;
    let fixed = db.raw_query(&target_paths_fixed_rule_query(owner_id))?;

    assert_eq!(
        sorted_rows(recursive.rows),
        sorted_rows(fixed.rows),
        "fixed rule should preserve recursive type-target traversal rows"
    );
    Ok(())
}

fn target_paths_recursive_query(owner_id: uuid::Uuid) -> String {
    format!(
        r#"
        ordinary_target[target_id] := *struct {{ id: target_id @ 'NOW' }}
        ordinary_target[target_id] := *enum {{ id: target_id @ 'NOW' }}
        ordinary_target[target_id] := *union {{ id: target_id @ 'NOW' }}
        ordinary_target[target_id] := *type_alias {{ id: target_id @ 'NOW' }}
        ordinary_target[target_id] := *generic_type {{ id: target_id @ 'NOW' }}

        trait_source[source_id] := *named_type {{ type_id: source_id @ 'NOW' }}
        trait_source[source_id] := *trait_bound_type {{ type_id: source_id @ 'NOW' }}

        valid_type_relation[source_id, target_id, relation_kind] :=
            *type_relation {{ source_id, target_id, relation_kind @ 'NOW' }},
            relation_kind = "Ordinary",
            *named_type {{ type_id: source_id @ 'NOW' }},
            ordinary_target[target_id]

        valid_type_relation[source_id, target_id, relation_kind] :=
            *type_relation {{ source_id, target_id, relation_kind @ 'NOW' }},
            relation_kind = "Trait",
            trait_source[source_id],
            *trait {{ id: target_id @ 'NOW' }}

        roots[type_use_id, owner_id, root_type_id] :=
            owner_id = to_uuid("{owner_id}"),
            *type_use {{ id: type_use_id, owner_id, root_type_id, role @ 'NOW' }}

        reachable[type_use_id, owner_id, root_type_id, terminal_type_id, depth] :=
            roots[type_use_id, owner_id, root_type_id],
            terminal_type_id = root_type_id,
            depth = 0

        reachable[type_use_id, owner_id, root_type_id, terminal_type_id, depth] :=
            reachable[type_use_id, owner_id, root_type_id, parent_type_id, previous_depth],
            previous_depth < 32,
            *type_contains {{
                parent_type_id,
                child_type_id: terminal_type_id,
                kind,
                position @ 'NOW'
            }},
            depth = previous_depth + 1

        ?[
            type_use_id,
            owner_id,
            root_type_id,
            terminal_type_id,
            target_id,
            relation_kind,
            depth
        ] :=
            reachable[type_use_id, owner_id, root_type_id, terminal_type_id, depth],
            valid_type_relation[terminal_type_id, target_id, relation_kind]
        "#
    )
}

fn target_paths_fixed_rule_query(owner_id: uuid::Uuid) -> String {
    format!(
        r#"
        ordinary_target[target_id] := *struct {{ id: target_id @ 'NOW' }}
        ordinary_target[target_id] := *enum {{ id: target_id @ 'NOW' }}
        ordinary_target[target_id] := *union {{ id: target_id @ 'NOW' }}
        ordinary_target[target_id] := *type_alias {{ id: target_id @ 'NOW' }}
        ordinary_target[target_id] := *generic_type {{ id: target_id @ 'NOW' }}

        trait_source[source_id] := *named_type {{ type_id: source_id @ 'NOW' }}
        trait_source[source_id] := *trait_bound_type {{ type_id: source_id @ 'NOW' }}

        valid_type_relation[source_id, target_id, relation_kind] :=
            *type_relation {{ source_id, target_id, relation_kind @ 'NOW' }},
            relation_kind = "Ordinary",
            *named_type {{ type_id: source_id @ 'NOW' }},
            ordinary_target[target_id]

        valid_type_relation[source_id, target_id, relation_kind] :=
            *type_relation {{ source_id, target_id, relation_kind @ 'NOW' }},
            relation_kind = "Trait",
            trait_source[source_id],
            *trait {{ id: target_id @ 'NOW' }}

        roots[type_use_id, owner_id, root_type_id] :=
            owner_id = to_uuid("{owner_id}"),
            *type_use {{ id: type_use_id, owner_id, root_type_id, role @ 'NOW' }}

        contains[parent_type_id, child_type_id] :=
            *type_contains {{ parent_type_id, child_type_id, kind, position @ 'NOW' }}

        ?[
            type_use_id,
            owner_id,
            root_type_id,
            terminal_type_id,
            target_id,
            relation_kind,
            depth
        ] <~ ploke.TypeTargetPaths(roots[], contains[], valid_type_relation[])
        "#
    )
}

fn sorted_rows(mut rows: Vec<Vec<DataValue>>) -> Vec<Vec<DataValue>> {
    rows.sort_by(|left, right| format!("{left:?}").cmp(&format!("{right:?}")));
    rows
}

#[test]
fn fixed_rule_output_paths_keep_exact_type_use_id() -> Result<(), DbError> {
    let db = setup_typed_fixture_db("fixture_type_resolution_v2")?;
    let owner_id = function_id_by_name(&db, "takes_vec_of_t")?;
    let roots = db.type_uses_for_owner(owner_id)?;
    let paths = db.type_targets_reachable_from_owner(owner_id)?;

    assert!(
        paths
            .iter()
            .all(|path| roots.iter().any(|root| root.id == path.type_use_id)),
        "every reachable path should point back to an exact root type_use id; roots: {roots:#?}; paths: {paths:#?}"
    );
    Ok(())
}
