use std::collections::{BTreeMap, BTreeSet};

use cozo::ScriptMutability;
use uuid::Uuid;

use crate::{
    Database, DbError,
    database::{to_string_list, to_uuid},
    multi_embedding::db_ext::{ANCESTOR_RULES_NOW, METHOD_NODE_ANCESTOR_RULE},
};

use super::super::CallSiteRow;

pub(super) fn enrich_call_site_cfgs(
    db: &Database,
    sites: &mut [CallSiteRow],
) -> Result<(), DbError> {
    let owner_ids = sites
        .iter()
        .map(|site| site.owner_id)
        .collect::<BTreeSet<_>>();
    let inherited = inherited_file_module_cfgs_for_owners(db, &owner_ids)?;

    for site in sites {
        if let Some(cfgs) = inherited.get(&site.owner_id) {
            merge_cfgs(&mut site.cfgs, cfgs);
        }
    }

    Ok(())
}

fn inherited_file_module_cfgs_for_owners(
    db: &Database,
    owner_ids: &BTreeSet<Uuid>,
) -> Result<BTreeMap<Uuid, BTreeSet<String>>, DbError> {
    if owner_ids.is_empty() {
        return Ok(BTreeMap::new());
    }

    let input_rows = owner_ids
        .iter()
        .map(|id| format!("[to_uuid(\"{id}\")]"))
        .collect::<Vec<_>>()
        .join(",\n");
    let script = format!(
        r#"
input[id] <- [
{input_rows}
]

ancestor[desc, desc] := *module{{ id: desc @ 'NOW' }}
{ANCESTOR_RULES_NOW}
{METHOD_NODE_ANCESTOR_RULE}

module_has_file_mod[mid] := *file_mod{{ owner_id: mid @ 'NOW' }}
file_owner_for_module[mod_id, file_owner_id] := module_has_file_mod[mod_id], file_owner_id = mod_id
file_owner_for_module[mod_id, file_owner_id] := ancestor[mod_id, parent], module_has_file_mod[parent], file_owner_id = parent

owner_module[id, mod_id] := input[id], *function{{ id @ 'NOW' }}, ancestor[id, mod_id]
owner_module[id, mod_id] := input[id], *macro{{ id @ 'NOW' }}, ancestor[id, mod_id]
owner_module[id, mod_id] := input[id], *method{{ id @ 'NOW' }}, ancestor[id, mod_id]
owner_module[id, mod_id] := input[id], *const{{ id @ 'NOW' }}, ancestor[id, mod_id]
owner_module[id, mod_id] := input[id], *static{{ id @ 'NOW' }}, ancestor[id, mod_id]
owner_module[id, mod_id] := input[id], *call_body_owner{{ id, parent_id @ 'NOW' }}, owner_module[parent_id, mod_id]

owner_scope_path[id, scope_path, namespace] :=
  owner_module[id, mod_id],
  ancestor[mod_id, scope_mod],
  *module{{ id: scope_mod, path: scope_path @ 'NOW' }},
  file_owner_for_module[scope_mod, scope_file_id],
  *file_mod{{ owner_id: scope_file_id, namespace @ 'NOW' }}

?[id, cfgs] :=
  owner_scope_path[id, scope_path, namespace],
  *module{{ id: cfg_mod, path: scope_path, cfgs @ 'NOW' }},
  file_owner_for_module[cfg_mod, cfg_file_id],
  *file_mod{{ owner_id: cfg_file_id, namespace @ 'NOW' }}
"#
    );

    let rows = db.run_script(&script, BTreeMap::new(), ScriptMutability::Immutable)?;
    let mut out = BTreeMap::<Uuid, BTreeSet<String>>::new();
    for row in rows.rows {
        let owner_id = to_uuid(&row[0])?;
        let cfgs = to_string_list(&row[1])?;
        out.entry(owner_id).or_default().extend(cfgs);
    }

    Ok(out)
}

fn merge_cfgs(site_cfgs: &mut Vec<String>, inherited: &BTreeSet<String>) {
    if inherited.is_empty() {
        return;
    }

    let mut merged = site_cfgs.iter().cloned().collect::<BTreeSet<_>>();
    merged.extend(inherited.iter().cloned());
    *site_cfgs = merged.into_iter().collect();
}
