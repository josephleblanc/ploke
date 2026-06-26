use std::collections::{BTreeMap, BTreeSet};

use cozo::{DataValue, ScriptMutability};
use uuid::Uuid;

use crate::{
    Database, DbError,
    multi_embedding::db_ext::{ANCESTOR_RULES_NOW, METHOD_NODE_ANCESTOR_RULE},
};

impl Database {
    pub(super) fn source_file_for_owner(&self, owner_id: Uuid) -> Result<String, DbError> {
        let mut params = BTreeMap::new();
        params.insert(
            "owner_id".to_string(),
            DataValue::Uuid(cozo::UuidWrapper(owner_id)),
        );
        let script = format!(
            r#"
ancestor[desc, desc] := *module{{ id: desc @ 'NOW' }}
{ANCESTOR_RULES_NOW}
{METHOD_NODE_ANCESTOR_RULE}

module_has_file[mid] := *file_mod{{ owner_id: mid @ 'NOW' }}
file_owner_for_module[mod_id, file_id] := module_has_file[mod_id], file_id = mod_id
file_owner_for_module[mod_id, file_id] := ancestor[mod_id, parent], module_has_file[parent], file_id = parent

?[file_path] :=
    owner_id = $owner_id,
    ancestor[owner_id, mod_id],
    *module{{ id: mod_id @ 'NOW' }},
    file_owner_for_module[mod_id, file_id],
    *file_mod{{ owner_id: file_id, file_path @ 'NOW' }}
"#
        );
        let rows = self
            .run_script(&script, params, ScriptMutability::Immutable)
            .map_err(|error| DbError::Cozo(error.to_string()))?;
        let paths = rows
            .rows
            .iter()
            .map(|row| {
                row.first()
                    .and_then(DataValue::get_str)
                    .map(ToOwned::to_owned)
                    .ok_or_else(|| {
                        DbError::Cozo(format!(
                            "source-file query returned non-string row for owner {owner_id}: {row:?}"
                        ))
                    })
            })
            .collect::<Result<BTreeSet<_>, _>>()?;

        match paths.len() {
            0 => Err(DbError::Cozo(format!(
                "missing source file for call graph owner {owner_id}"
            ))),
            1 => Ok(paths.into_iter().next().expect("one path")),
            _ => Err(DbError::Cozo(format!(
                "ambiguous source files for call graph owner {owner_id}: {paths:?}"
            ))),
        }
    }
}
