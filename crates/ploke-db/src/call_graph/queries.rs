use std::collections::{BTreeMap, HashSet};

use cozo::{DataValue, ScriptMutability, UuidWrapper};
use uuid::Uuid;

use crate::{Database, DbError, database::to_string};

use super::{
    CallCallerRow, CallContextCandidate, CallContextOptions, CallContextRelation, CallContextRow,
    CallContextSeed, CallResolutionRow, CallSiteKind, CallSiteRow, CallStatusKind, CallTargetRow,
    decode::{decode_resolution, decode_site, decode_target, validate_owner_context_targets},
    families::{valid_call_target, valid_call_target_rules},
};

impl Database {
    pub fn has_call_graph_relations(&self) -> Result<bool, DbError> {
        const REQUIRED: [&str; 4] = [
            "call_site",
            "call_site_edge",
            "call_relation",
            "call_resolution_status",
        ];
        let rows = self.raw_query("::relations")?;
        let registered = rows
            .rows
            .iter()
            .filter_map(|row| row.first().and_then(|value| value.get_str()))
            .collect::<HashSet<_>>();
        if !REQUIRED.iter().all(|name| registered.contains(name)) {
            return Ok(false);
        }

        let populated = self.raw_query(
            r#"?[site_id] :=
                *call_site { id: site_id @ 'NOW' },
                *call_site_edge {
                    target_id: site_id,
                    relation_kind: "BodyContainsCall" @ 'NOW'
                },
                *call_resolution_status { source_id: site_id @ 'NOW' }
            :limit 1"#,
        )?;
        Ok(!populated.rows.is_empty())
    }

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

    /// Expands an owner or target seed into graphRAG call-context candidates.
    ///
    /// This is the application-facing layer over the lower-level call-site and
    /// target-centered helpers. It only promotes resolved call edges as context
    /// candidates; unsupported, external, unresolved, or ambiguous rows remain
    /// visible through [`Self::call_context_for_owner`] but do not become
    /// traversal targets.
    pub fn expand_call_context(
        &self,
        seed: CallContextSeed,
        options: CallContextOptions,
    ) -> Result<Vec<CallContextCandidate>, DbError> {
        if options.max_candidates == 0 {
            return Ok(Vec::new());
        }

        let mut candidates = Vec::new();

        match seed {
            CallContextSeed::Owner(owner_id) if options.include_outgoing_targets => {
                for row in self.call_context_for_owner(owner_id)? {
                    if row.status.status != CallStatusKind::Resolved {
                        continue;
                    }
                    candidates.extend(row.targets.into_iter().map(|target| CallContextCandidate {
                        node_id: target.target_id,
                        relation: CallContextRelation::OutgoingTarget,
                        call_site_id: row.site.id,
                        target_id: target.target_id,
                        distance: 1,
                    }));
                }
            }
            CallContextSeed::Target(target_id) if options.include_incoming_callers => {
                for caller in self.callers_for_target(target_id)? {
                    if caller.status.status != CallStatusKind::Resolved {
                        continue;
                    }
                    candidates.push(CallContextCandidate {
                        node_id: caller.site.owner_id,
                        relation: CallContextRelation::IncomingCaller,
                        call_site_id: caller.site.id,
                        target_id: caller.target.target_id,
                        distance: 1,
                    });
                }
            }
            CallContextSeed::Owner(_) | CallContextSeed::Target(_) => {}
        }

        candidates.sort_by_key(|candidate| {
            (
                candidate.distance,
                candidate.relation,
                candidate.node_id.as_u128(),
                candidate.call_site_id.as_u128(),
            )
        });
        candidates.truncate(options.max_candidates);
        Ok(candidates)
    }
}
