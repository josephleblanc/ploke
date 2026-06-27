use std::collections::BTreeMap;

use cozo::{DataValue, ScriptMutability, UuidWrapper};
use uuid::Uuid;

use crate::{Database, DbError};

use super::super::{
    CallCallerRow, CallContextRow, CallSiteRow, CallTargetRow,
    decode::{decode_site, decode_target, validate_owner_context_targets},
    families::{valid_call_target, valid_call_target_rules},
};

impl Database {
    pub fn call_targets_for_site(&self, site_id: Uuid) -> Result<Vec<CallTargetRow>, DbError> {
        let mut params = BTreeMap::new();
        params.insert("site_id".to_string(), DataValue::Uuid(UuidWrapper(site_id)));

        let mut script = valid_call_target_rules();
        script.push_str(
            r#"
            ?[site_id, target_id, relation_kind, source_kind, target_kind] :=
                site_id = $site_id,
                *call_site {
                    id: site_id,
                    call_kind: source_kind @ 'NOW'
                },
                *call_relation {
                    source_id: site_id,
                    target_id,
                    relation_kind,
                    source_kind,
                    target_kind @ 'NOW'
                },
                valid_target[target_id, relation_kind, source_kind, target_kind]
            :sort target_id"#,
        );

        let rows = self.run_script(&script, params, ScriptMutability::Immutable)?;

        Ok(rows
            .rows
            .iter()
            .map(|row| decode_target(row))
            .collect::<Result<Vec<CallTargetRow>, DbError>>()?
            .into_iter()
            .filter(valid_call_target)
            .collect())
    }

    pub fn callers_for_target(&self, target_id: Uuid) -> Result<Vec<CallCallerRow>, DbError> {
        let mut params = BTreeMap::new();
        params.insert(
            "target_id".to_string(),
            DataValue::Uuid(UuidWrapper(target_id)),
        );

        let mut script = valid_call_target_rules();
        script.push_str(
            r#"
            ?[
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
                generic_arg_count,
                relation_site_id,
                relation_target_id,
                relation_kind,
                source_kind,
                target_kind
            ] :=
                relation_target_id = $target_id,
                *call_relation {
                    source_id: relation_site_id,
                    target_id: relation_target_id,
                    relation_kind,
                    source_kind,
                    target_kind @ 'NOW'
                },
                valid_target[relation_target_id, relation_kind, source_kind, target_kind],
                *call_site_edge {
                    source_id: owner_id,
                    target_id: relation_site_id,
                    relation_kind: "BodyContainsCall",
                    source_kind: owner_kind,
                    target_kind: call_kind @ 'NOW'
                },
                *call_site {
                    id: relation_site_id,
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
                },
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
                id = relation_site_id,
                call_kind = source_kind
            :sort owner_id, span, relation_kind"#,
        );

        let rows = self.run_script(&script, params, ScriptMutability::Immutable)?;

        let callers = rows
            .rows
            .iter()
            .map(|row| {
                let site = decode_site(&row[..12])?;
                let target = decode_target(&row[12..])?;
                let status = self.call_resolution_for_site(site.id)?.ok_or_else(|| {
                    DbError::Cozo(format!(
                        "missing call_resolution_status for call site {} targeting {}",
                        site.id, target_id
                    ))
                })?;
                Ok(CallCallerRow {
                    site,
                    status,
                    target,
                })
            })
            .collect::<Result<Vec<CallCallerRow>, DbError>>()?;

        let mut valid_callers = Vec::new();
        for caller in callers {
            if !valid_call_target(&caller.target) {
                continue;
            }
            self.validate_target_centered_caller_targets(&caller)?;
            valid_callers.push(caller);
        }

        Ok(valid_callers)
    }

    pub fn call_context_for_target(&self, target_id: Uuid) -> Result<Vec<CallContextRow>, DbError> {
        let callers = self.callers_for_target(target_id)?;
        let mut out = Vec::with_capacity(callers.len());
        for caller in callers {
            let targets = self.call_targets_for_site(caller.site.id)?;
            let row = CallContextRow {
                site: caller.site,
                status: caller.status,
                targets,
            };
            validate_owner_context_targets(&row)?;
            out.push(row);
        }
        Ok(out)
    }

    pub fn call_sites_for_target(&self, target_id: Uuid) -> Result<Vec<CallSiteRow>, DbError> {
        self.call_context_for_target(target_id)
            .map(|rows| rows.into_iter().map(|row| row.site).collect())
    }

    fn validate_target_centered_caller_targets(
        &self,
        caller: &CallCallerRow,
    ) -> Result<(), DbError> {
        let targets = self.call_targets_for_site(caller.site.id)?;
        let row = CallContextRow {
            site: caller.site.clone(),
            status: caller.status,
            targets,
        };
        validate_owner_context_targets(&row)
    }
}
