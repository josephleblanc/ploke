use std::collections::{BTreeMap, BTreeSet};

use cozo::{DataValue, ScriptMutability, UuidWrapper};
use uuid::Uuid;

use crate::{Database, DbError, database::to_string};

use super::super::{
    CallContextRow, CallResolutionRow, CallSiteKind, CallSiteRow, CallStatusKind, CallTargetRow,
    decode::{decode_resolution, decode_site, decode_target, validate_owner_context_targets},
    families::{valid_call_owner_rules, valid_call_target, valid_call_target_rules},
};
use super::effective_cfgs::enrich_call_site_cfgs;

impl Database {
    pub fn call_sites_for_owner(&self, owner_id: Uuid) -> Result<Vec<CallSiteRow>, DbError> {
        let mut params = BTreeMap::new();
        params.insert(
            "owner_id".to_string(),
            DataValue::Uuid(UuidWrapper(owner_id)),
        );

        let mut script = valid_call_owner_rules();
        script.push_str(
            r#"
            ?[
                id,
                owner_id,
                call_kind,
                span,
                cfgs,
                unsafe_block,
                path,
                method_name,
                macro_name,
                receiver_kind,
                receiver_path,
                arg_count,
                generic_arg_count
            ] :=
                owner_id = $owner_id,
                valid_owner[owner_id, owner_kind],
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
                    unsafe_block,
                    path,
                    method_name,
                    macro_name,
                    receiver_kind,
                    receiver_path,
                    arg_count,
                    generic_arg_count @ 'NOW'
                }
            :sort span"#,
        );
        let rows = self.run_script(&script, params, ScriptMutability::Immutable)?;

        let mut sites = rows
            .rows
            .iter()
            .map(|row| decode_site(row))
            .collect::<Result<Vec<_>, DbError>>()?;
        enrich_call_site_cfgs(self, &mut sites)?;
        Ok(sites)
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
        self.call_context_for_owners(&BTreeSet::from([owner_id]))
    }

    pub(crate) fn call_context_for_owners(
        &self,
        owner_ids: &BTreeSet<Uuid>,
    ) -> Result<Vec<CallContextRow>, DbError> {
        if owner_ids.is_empty() {
            return Ok(Vec::new());
        }

        let sites = self.call_sites_for_owners(owner_ids)?;
        let resolutions = self.call_resolutions_for_sites(&sites)?;
        let targets = self.call_targets_for_sites(&sites)?;

        sites
            .into_iter()
            .map(|site| {
                let status = resolutions.get(&site.id).cloned().ok_or_else(|| {
                    DbError::Cozo(format!(
                        "missing call_resolution_status for call site {} owned by {}",
                        site.id, site.owner_id
                    ))
                })?;
                let targets = targets.get(&site.id).cloned().unwrap_or_default();
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

    fn call_sites_for_owners(
        &self,
        owner_ids: &BTreeSet<Uuid>,
    ) -> Result<Vec<CallSiteRow>, DbError> {
        if owner_ids.is_empty() {
            return Ok(Vec::new());
        }

        let input_rows = owner_ids
            .iter()
            .map(|owner| format!("[to_uuid(\"{owner}\")]"))
            .collect::<Vec<_>>()
            .join(",\n");

        let mut script = valid_call_owner_rules();
        script.push_str(&format!(
            r#"
            input_owner[owner_id] <- [
{input_rows}
            ]

            ?[
                id,
                owner_id,
                call_kind,
                span,
                cfgs,
                unsafe_block,
                path,
                method_name,
                macro_name,
                receiver_kind,
                receiver_path,
                arg_count,
                generic_arg_count
            ] :=
                input_owner[owner_id],
                valid_owner[owner_id, owner_kind],
                *call_site_edge {{
                    source_id: owner_id,
                    target_id: id,
                    relation_kind: "BodyContainsCall",
                    source_kind: owner_kind,
                    target_kind: call_kind @ 'NOW'
                }},
                *call_site {{
                    id,
                    owner_id,
                    call_kind,
                    span,
                    cfgs,
                    unsafe_block,
                    path,
                    method_name,
                    macro_name,
                    receiver_kind,
                    receiver_path,
                    arg_count,
                    generic_arg_count @ 'NOW'
                }}
            :sort owner_id, span"#
        ));
        let rows = self.run_script(&script, BTreeMap::new(), ScriptMutability::Immutable)?;

        let mut sites = rows
            .rows
            .iter()
            .map(|row| decode_site(row))
            .collect::<Result<Vec<_>, DbError>>()?;
        enrich_call_site_cfgs(self, &mut sites)?;
        Ok(sites)
    }

    fn call_resolutions_for_sites(
        &self,
        sites: &[CallSiteRow],
    ) -> Result<BTreeMap<Uuid, CallResolutionRow>, DbError> {
        if sites.is_empty() {
            return Ok(BTreeMap::new());
        }

        let input_rows = sites
            .iter()
            .map(|site| format!("[to_uuid(\"{}\")]", site.id))
            .collect::<Vec<_>>()
            .join(",\n");

        let script = format!(
            r#"
            input_site[site_id] <- [
{input_rows}
            ]

            ?[site_id, source_kind, status_kind, resolution_kind, call_kind] :=
                input_site[site_id],
                *call_resolution_status {{
                    source_id: site_id,
                    source_kind,
                    status_kind,
                    resolution_kind @ 'NOW'
                }},
                *call_site {{
                    id: site_id,
                    call_kind @ 'NOW'
                }}
            :sort site_id"#
        );
        let rows = self.run_script(&script, BTreeMap::new(), ScriptMutability::Immutable)?;

        let mut statuses = BTreeMap::new();
        for row in &rows.rows {
            let status = decode_resolution(&row[..4])?;
            let site_kind = CallSiteKind::from_str(&to_string(&row[4])?)?;
            if status.site_kind != site_kind {
                return Err(DbError::Cozo(format!(
                    "call_resolution_status source_kind {:?} does not match call site {} kind {:?}",
                    status.site_kind, status.site_id, site_kind
                )));
            }
            if statuses.insert(status.site_id, status).is_some() {
                return Err(DbError::Cozo(format!(
                    "expected at most one call_resolution_status for call site {}, found duplicate",
                    to_string(&row[0])?
                )));
            }
        }
        Ok(statuses)
    }

    fn call_targets_for_sites(
        &self,
        sites: &[CallSiteRow],
    ) -> Result<BTreeMap<Uuid, Vec<CallTargetRow>>, DbError> {
        if sites.is_empty() {
            return Ok(BTreeMap::new());
        }

        let input_rows = sites
            .iter()
            .map(|site| format!("[to_uuid(\"{}\")]", site.id))
            .collect::<Vec<_>>()
            .join(",\n");

        let mut script = valid_call_target_rules();
        script.push_str(&format!(
            r#"
            input_site[site_id] <- [
{input_rows}
            ]

            ?[site_id, target_id, relation_kind, source_kind, target_kind] :=
                input_site[site_id],
                *call_site {{
                    id: site_id,
                    call_kind: source_kind @ 'NOW'
                }},
                *call_relation {{
                    source_id: site_id,
                    target_id,
                    relation_kind,
                    source_kind,
                    target_kind @ 'NOW'
                }},
                valid_target[target_id, relation_kind, source_kind, target_kind]
            :sort site_id, target_id"#
        ));

        let rows = self.run_script(&script, BTreeMap::new(), ScriptMutability::Immutable)?;
        let mut targets = BTreeMap::<Uuid, Vec<CallTargetRow>>::new();
        for row in &rows.rows {
            let target = decode_target(row)?;
            if valid_call_target(&target) {
                targets.entry(target.site_id).or_default().push(target);
            }
        }
        Ok(targets)
    }
}
