use std::collections::BTreeMap;

use cozo::{DataValue, ScriptMutability, UuidWrapper};
use uuid::Uuid;

use crate::{
    Database, DbError,
    database::{to_string, to_string_list, to_uuid},
    multi_embedding::db_ext::{
        ANCESTOR_RULES_NOW, METHOD_NODE_ANCESTOR_RULE, VARIANT_ANCESTOR_RULE,
    },
};

use super::super::{CallNodeInfo, CallNodeKind};

impl Database {
    /// Returns stable source metadata for a node that can participate in call-graph queries.
    ///
    /// `is_public` is intentionally the direct stored node visibility predicate
    /// (`vis_kind == "public"`). It does not infer effective exported API
    /// visibility through public traits, re-exports, or module visibility.
    pub fn call_node_info(&self, node_id: Uuid) -> Result<Option<CallNodeInfo>, DbError> {
        let mut params = BTreeMap::new();
        params.insert("node_id".to_string(), DataValue::Uuid(UuidWrapper(node_id)));

        let script = format!(
            r#"
ancestor[desc, desc] := *module{{ id: desc @ 'NOW' }}
{ANCESTOR_RULES_NOW}
{METHOD_NODE_ANCESTOR_RULE}
{VARIANT_ANCESTOR_RULE}

module_has_file_mod[mid] := *file_mod{{ owner_id: mid @ 'NOW' }}
file_owner_for_module[mod_id, file_owner_id] := module_has_file_mod[mod_id], file_owner_id = mod_id
file_owner_for_module[mod_id, file_owner_id] := ancestor[mod_id, parent], module_has_file_mod[parent], file_owner_id = parent
owner_anchor[id, vis_kind, mod_id] := *function{{ id, vis_kind @ 'NOW' }}, ancestor[id, mod_id]
owner_anchor[id, vis_kind, mod_id] := *macro{{ id, vis_kind @ 'NOW' }}, ancestor[id, mod_id]
owner_anchor[id, vis_kind, mod_id] := *method{{ id, vis_kind @ 'NOW' }}, ancestor[id, mod_id]
owner_anchor[id, vis_kind, mod_id] := *const{{ id, vis_kind @ 'NOW' }}, ancestor[id, mod_id]
owner_anchor[id, vis_kind, mod_id] := *static{{ id, vis_kind @ 'NOW' }}, ancestor[id, mod_id]
owner_anchor[id, vis_kind, mod_id] := *call_body_owner{{ id, parent_id @ 'NOW' }}, owner_anchor[parent_id, vis_kind, mod_id]

node_info[id, kind, name, vis_kind, module_path, file_path] :=
  id = $node_id,
  *function{{ id, name, vis_kind @ 'NOW' }},
  kind = "Function",
  ancestor[id, mod_id],
  *module{{ id: mod_id, path: module_path @ 'NOW' }},
  file_owner_for_module[mod_id, file_owner_id],
  *file_mod{{ owner_id: file_owner_id, file_path @ 'NOW' }}

node_info[id, kind, name, vis_kind, module_path, file_path] :=
  id = $node_id,
  *macro{{ id, name, vis_kind @ 'NOW' }},
  kind = "Macro",
  ancestor[id, mod_id],
  *module{{ id: mod_id, path: module_path @ 'NOW' }},
  file_owner_for_module[mod_id, file_owner_id],
  *file_mod{{ owner_id: file_owner_id, file_path @ 'NOW' }}

node_info[id, kind, name, vis_kind, module_path, file_path] :=
  id = $node_id,
  *method{{ id, name, vis_kind @ 'NOW' }},
  kind = "Method",
  ancestor[id, mod_id],
  *module{{ id: mod_id, path: module_path @ 'NOW' }},
  file_owner_for_module[mod_id, file_owner_id],
  *file_mod{{ owner_id: file_owner_id, file_path @ 'NOW' }}

node_info[id, kind, name, vis_kind, module_path, file_path] :=
  id = $node_id,
  *const{{ id, name, vis_kind @ 'NOW' }},
  kind = "Const",
  ancestor[id, mod_id],
  *module{{ id: mod_id, path: module_path @ 'NOW' }},
  file_owner_for_module[mod_id, file_owner_id],
  *file_mod{{ owner_id: file_owner_id, file_path @ 'NOW' }}

node_info[id, kind, name, vis_kind, module_path, file_path] :=
  id = $node_id,
  *static{{ id, name, vis_kind @ 'NOW' }},
  kind = "Static",
  ancestor[id, mod_id],
  *module{{ id: mod_id, path: module_path @ 'NOW' }},
  file_owner_for_module[mod_id, file_owner_id],
  *file_mod{{ owner_id: file_owner_id, file_path @ 'NOW' }}

node_info[id, kind, name, vis_kind, module_path, file_path] :=
  id = $node_id,
  *call_body_owner{{ id, owner_kind: kind, label: name @ 'NOW' }},
  owner_anchor[id, vis_kind, mod_id],
  *module{{ id: mod_id, path: module_path @ 'NOW' }},
  file_owner_for_module[mod_id, file_owner_id],
  *file_mod{{ owner_id: file_owner_id, file_path @ 'NOW' }}

node_info[id, kind, name, vis_kind, module_path, file_path] :=
  id = $node_id,
  *struct{{ id, name, vis_kind @ 'NOW' }},
  kind = "Struct",
  ancestor[id, mod_id],
  *module{{ id: mod_id, path: module_path @ 'NOW' }},
  file_owner_for_module[mod_id, file_owner_id],
  *file_mod{{ owner_id: file_owner_id, file_path @ 'NOW' }}

node_info[id, kind, name, vis_kind, module_path, file_path] :=
  id = $node_id,
  *variant{{ id, name, owner_id: enum_id @ 'NOW' }},
  *enum{{ id: enum_id, vis_kind @ 'NOW' }},
  kind = "Variant",
  ancestor[id, mod_id],
  *module{{ id: mod_id, path: module_path @ 'NOW' }},
  file_owner_for_module[mod_id, file_owner_id],
  *file_mod{{ owner_id: file_owner_id, file_path @ 'NOW' }}

?[id, kind, name, vis_kind, module_path, file_path] :=
  node_info[id, kind, name, vis_kind, module_path, file_path]
"#
        );

        let rows = self.run_script(&script, params, ScriptMutability::Immutable)?;
        let mut infos = rows
            .rows
            .iter()
            .map(|row| decode_call_node_info(row))
            .collect::<Result<Vec<_>, DbError>>()?;
        // Exact node identity is authoritative here; module/file path are
        // descriptive metadata. Some associated items can reach nested test
        // modules through ancestor rules, so choose a stable shortest path
        // instead of failing an otherwise valid call-graph query.
        infos.sort_by_key(call_node_info_rank);
        infos.dedup();
        Ok(infos.into_iter().next())
    }
}

pub(super) fn call_node_info_rank(
    row: &CallNodeInfo,
) -> (usize, u128, CallNodeKind, String, String, String) {
    (
        row.module_path.len(),
        row.id.as_u128(),
        row.kind,
        row.name.clone(),
        row.visibility.clone(),
        row.file_path.clone(),
    )
}

pub(super) fn decode_call_node_info(row: &[DataValue]) -> Result<CallNodeInfo, DbError> {
    let visibility = to_string(&row[3])?;
    Ok(CallNodeInfo {
        id: to_uuid(&row[0])?,
        kind: CallNodeKind::from_str(&to_string(&row[1])?)?,
        name: to_string(&row[2])?,
        is_public: visibility == "public",
        visibility,
        module_path: to_string_list(&row[4])?,
        file_path: to_string(&row[5])?,
    })
}
