use std::collections::BTreeMap;

use cozo::{DataValue, ScriptMutability, UuidWrapper};
use uuid::Uuid;

use crate::{Database, DbError, database::to_string};

use super::super::{
    CallContextRow, CallResolutionRow, CallSiteKind, CallSiteRow, CallStatusKind,
    decode::{decode_resolution, decode_site, validate_owner_context_targets},
};

impl Database {
    pub fn call_sites_for_owner(&self, owner_id: Uuid) -> Result<Vec<CallSiteRow>, DbError> {
        let mut params = BTreeMap::new();
        params.insert(
            "owner_id".to_string(),
            DataValue::Uuid(UuidWrapper(owner_id)),
        );

        let rows = self.run_script(
            r#"?[
                id,
                owner_id,
                call_kind,
                span,
                cfgs,
                path,
                method_name,
                macro_name,
                receiver_kind,
                receiver_path,
                arg_count,
                generic_arg_count
            ] :=
                owner_id = $owner_id,
                (
                    *function { id: owner_id @ 'NOW' },
                    owner_kind = "Function"
                ) or (
                    *method { id: owner_id @ 'NOW' },
                    owner_kind = "Method"
                ) or (
                    *const { id: owner_id @ 'NOW' },
                    owner_kind = "Const"
                ) or (
                    *static { id: owner_id @ 'NOW' },
                    owner_kind = "Static"
                ),
                *call_site_edge {
                    source_id: owner_id,
                    target_id: id,
                    relation_kind: "BodyContainsCall",
                    source_kind: owner_kind,
                    target_kind: call_kind @ 'NOW'
                },
                *call_site {
                    id,
                    owner_id,
                    call_kind,
                    span,
                    cfgs,
                    path,
                    method_name,
                    macro_name,
                    receiver_kind,
                    receiver_path,
                    arg_count,
                    generic_arg_count @ 'NOW'
                }
            :sort span"#,
            params,
            ScriptMutability::Immutable,
        )?;

        rows.rows.iter().map(|row| decode_site(row)).collect()
    }

    pub fn call_resolution_for_site(
        &self,
        site_id: Uuid,
    ) -> Result<Option<CallResolutionRow>, DbError> {
        let mut params = BTreeMap::new();
        params.insert("site_id".to_string(), DataValue::Uuid(UuidWrapper(site_id)));

        let rows = self.run_script(
            r#"?[site_id, source_kind, status_kind, resolution_kind, call_kind] :=
                site_id = $site_id,
                *call_resolution_status {
                    source_id: site_id,
                    source_kind,
                    status_kind,
                    resolution_kind @ 'NOW'
                },
                *call_site {
                    id: site_id,
                    call_kind @ 'NOW'
                }"#,
            params,
            ScriptMutability::Immutable,
        )?;

        match rows.rows.as_slice() {
            [] => Ok(None),
            [row] => {
                let status = decode_resolution(&row[..4])?;
                let site_kind = CallSiteKind::from_str(&to_string(&row[4])?)?;
                if status.site_kind != site_kind {
                    return Err(DbError::Cozo(format!(
                        "call_resolution_status source_kind {:?} does not match call site {} kind {:?}",
                        status.site_kind, site_id, site_kind
                    )));
                }
                Ok(Some(status))
            }
            rows => Err(DbError::Cozo(format!(
                "expected at most one call_resolution_status for call site {site_id}, found {}",
                rows.len()
            ))),
        }
    }

    pub fn call_context_for_owner(&self, owner_id: Uuid) -> Result<Vec<CallContextRow>, DbError> {
        self.call_sites_for_owner(owner_id)?
            .into_iter()
            .map(|site| {
                let status = self.call_resolution_for_site(site.id)?.ok_or_else(|| {
                    DbError::Cozo(format!(
                        "missing call_resolution_status for call site {} owned by {}",
                        site.id, site.owner_id
                    ))
                })?;
                let targets = self.call_targets_for_site(site.id)?;
                let row = CallContextRow {
                    site,
                    status,
                    targets,
                };
                validate_owner_context_targets(&row)?;
                Ok(row)
            })
            .collect()
    }
}
