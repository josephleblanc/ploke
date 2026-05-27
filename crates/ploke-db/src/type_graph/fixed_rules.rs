use std::collections::{BTreeMap, BTreeSet, VecDeque};

use cozo::{DataValue, Db, MemStorage, NamedRows, SimpleFixedRule};

pub(crate) const TYPE_TARGET_PATHS_RULE: &str = "ploke.TypeTargetPaths";
const TYPE_TARGET_PATHS_ARITY: usize = 7;
const MAX_TYPE_DEPTH: u32 = 32;

pub(crate) fn register_ploke_fixed_rules(db: &Db<MemStorage>) -> Result<(), cozo::Error> {
    db.unregister_fixed_rule(TYPE_TARGET_PATHS_RULE)?;
    db.register_fixed_rule(TYPE_TARGET_PATHS_RULE.to_string(), type_target_paths_rule())
}

fn type_target_paths_rule() -> SimpleFixedRule {
    SimpleFixedRule::new(TYPE_TARGET_PATHS_ARITY, |inputs, _options| {
        let roots = inputs
            .first()
            .map(|rows| rows.rows.as_slice())
            .unwrap_or(&[]);
        let contains = inputs
            .get(1)
            .map(|rows| rows.rows.as_slice())
            .unwrap_or(&[]);
        let valid_rel = inputs
            .get(2)
            .map(|rows| rows.rows.as_slice())
            .unwrap_or(&[]);

        let mut children_by_parent: BTreeMap<DataValue, Vec<DataValue>> = BTreeMap::new();
        for row in contains {
            if row.len() >= 2 {
                children_by_parent
                    .entry(row[0].clone())
                    .or_default()
                    .push(row[1].clone());
            }
        }

        let mut targets_by_source: BTreeMap<DataValue, Vec<(DataValue, DataValue)>> =
            BTreeMap::new();
        for row in valid_rel {
            if row.len() >= 3 {
                targets_by_source
                    .entry(row[0].clone())
                    .or_default()
                    .push((row[1].clone(), row[2].clone()));
            }
        }

        let mut out_rows = Vec::new();
        for root in roots {
            if root.len() < 3 {
                continue;
            }

            let type_use_id = root[0].clone();
            let owner_id = root[1].clone();
            let root_type_id = root[2].clone();
            let mut visited = BTreeSet::new();
            let mut queue = VecDeque::from([(root_type_id.clone(), 0_u32)]);

            while let Some((terminal_type_id, depth)) = queue.pop_front() {
                if !visited.insert(terminal_type_id.clone()) {
                    continue;
                }

                if let Some(targets) = targets_by_source.get(&terminal_type_id) {
                    for (target_id, relation_kind) in targets {
                        out_rows.push(vec![
                            type_use_id.clone(),
                            owner_id.clone(),
                            root_type_id.clone(),
                            terminal_type_id.clone(),
                            target_id.clone(),
                            relation_kind.clone(),
                            DataValue::from(i64::from(depth)),
                        ]);
                    }
                }

                if depth >= MAX_TYPE_DEPTH {
                    continue;
                }

                if let Some(children) = children_by_parent.get(&terminal_type_id) {
                    for child in children {
                        queue.push_back((child.clone(), depth + 1));
                    }
                }
            }
        }

        Ok(NamedRows::new(
            vec![
                "type_use_id".to_string(),
                "owner_id".to_string(),
                "root_type_id".to_string(),
                "terminal_type_id".to_string(),
                "target_id".to_string(),
                "relation_kind".to_string(),
                "depth".to_string(),
            ],
            out_rows,
        ))
    })
}
