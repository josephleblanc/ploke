use std::collections::{BTreeMap, BTreeSet};

use cozo::{DataValue, ScriptMutability, UuidWrapper};
use uuid::Uuid;

use crate::{Database, DbError, database::to_string};

use super::super::{
    CallContextRow, CallResolutionRow, CallSiteKind, CallSiteRow, CallStatusKind, CallTargetRow,
    LocalBindingEdgeRow, LocalBindingRow, ReturnedCallBindingFlow, ReturnedFutureExecutionFlow,
    ReturnedFutureFlow,
    decode::{
        decode_local_binding, decode_local_binding_edge, decode_resolution,
        decode_returned_call_binding_flow, decode_returned_future_execution_flow,
        decode_returned_future_flow, decode_site, decode_target, validate_owner_context_targets,
    },
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

    pub fn awaited_call_sites_for_owner(
        &self,
        owner_id: Uuid,
    ) -> Result<Vec<CallSiteRow>, DbError> {
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
                    relation_kind: "CallResultAwaited",
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

    pub fn local_bindings_for_owner(
        &self,
        owner_id: Uuid,
    ) -> Result<Vec<LocalBindingRow>, DbError> {
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
                owner_kind,
                binding_kind,
                name,
                span,
                cfgs,
                source_kind,
                source_id,
                source_call_kind,
                source_path,
                callee_kind,
                callee_path
            ] :=
                owner_id = $owner_id,
                valid_owner[owner_id, owner_kind],
                *local_binding {
                    id,
                    owner_id,
                    owner_kind,
                    binding_kind,
                    name,
                    span,
                    cfgs,
                    source_kind,
                    source_id,
                    source_call_kind,
                    source_path,
                    callee_kind,
                    callee_path @ 'NOW'
                }
            :sort span"#,
        );
        let rows = self.run_script(&script, params, ScriptMutability::Immutable)?;

        rows.rows
            .iter()
            .map(|row| decode_local_binding(row))
            .collect::<Result<Vec<_>, DbError>>()
    }

    pub fn local_binding_edges_for_owner(
        &self,
        owner_id: Uuid,
    ) -> Result<Vec<LocalBindingEdgeRow>, DbError> {
        let mut params = BTreeMap::new();
        params.insert(
            "owner_id".to_string(),
            DataValue::Uuid(UuidWrapper(owner_id)),
        );

        let mut script = valid_call_owner_rules();
        script.push_str(
            r#"
            binding_for_owner[binding_id] :=
                owner_id = $owner_id,
                valid_owner[owner_id, owner_kind],
                *local_binding_edge {
                    source_id: owner_id,
                    target_id: binding_id,
                    relation_kind: "OwnerContainsBinding",
                    source_kind: owner_kind,
                    target_kind: "LocalBinding" @ 'NOW'
                }

            ?[source_id, target_id, relation_kind, source_kind, target_kind] :=
                owner_id = $owner_id,
                source_id = owner_id,
                relation_kind = "OwnerContainsBinding",
                target_kind = "LocalBinding",
                valid_owner[owner_id, owner_kind],
                source_kind = owner_kind,
                *local_binding_edge {
                    source_id,
                    target_id,
                    relation_kind,
                    source_kind,
                    target_kind @ 'NOW'
                }

            ?[source_id, target_id, relation_kind, source_kind, target_kind] :=
                binding_for_owner[binding_id],
                source_id = binding_id,
                *local_binding_edge {
                    source_id,
                    target_id,
                    relation_kind,
                    source_kind,
                    target_kind @ 'NOW'
                }

            ?[source_id, target_id, relation_kind, source_kind, target_kind] :=
                binding_for_owner[binding_id],
                target_id = binding_id,
                relation_kind = "ArgumentSuppliesParameter",
                target_kind = "LocalBinding",
                *local_binding_edge {
                    source_id,
                    target_id,
                    relation_kind,
                    source_kind,
                    target_kind @ 'NOW'
                }
            :sort relation_kind, target_kind, target_id"#,
        );
        let rows = self.run_script(&script, params, ScriptMutability::Immutable)?;

        rows.rows
            .iter()
            .map(|row| decode_local_binding_edge(row))
            .collect::<Result<Vec<_>, DbError>>()
    }

    pub fn returned_call_binding_flows_for_owner(
        &self,
        owner_id: Uuid,
    ) -> Result<Vec<ReturnedCallBindingFlow>, DbError> {
        let mut params = BTreeMap::new();
        params.insert(
            "owner_id".to_string(),
            DataValue::Uuid(UuidWrapper(owner_id)),
        );

        let mut script = valid_call_target_rules();
        script.push_str(
            r#"
            ?[
                caller_id,
                dynamic_id,
                dynamic_span,
                path,
                dynamic_target_id,
                dynamic_relation,
                dynamic_target_kind,
                producer_site_id,
                producer_span,
                producer_id,
                binding_id,
                source_id,
                source_relation,
                source_kind
            ] :=
                caller_id = $owner_id,
                *call_site {
                    id: dynamic_id,
                    owner_id: caller_id,
                    call_kind: "Dynamic",
                    span: dynamic_span,
                    path @ 'NOW'
                },
                *call_resolution_status {
                    source_id: dynamic_id,
                    source_kind: "Dynamic",
                    status_kind: "Resolved",
                    resolution_kind: "LocalExact" @ 'NOW'
                },
                *call_relation {
                    source_id: dynamic_id,
                    target_id: dynamic_target_id,
                    relation_kind: dynamic_relation,
                    source_kind: "Dynamic",
                    target_kind: dynamic_target_kind @ 'NOW'
                },
                valid_target[dynamic_target_id, dynamic_relation, "Dynamic", dynamic_target_kind],
                *call_site {
                    id: producer_site_id,
                    owner_id: caller_id,
                    call_kind: "Path",
                    span: producer_span,
                    path @ 'NOW'
                },
                *call_relation {
                    source_id: producer_site_id,
                    target_id: producer_id,
                    relation_kind: "Function",
                    source_kind: "Path",
                    target_kind: "Function" @ 'NOW'
                },
                valid_target[producer_id, "Function", "Path", "Function"],
                *local_binding {
                    id: binding_id,
                    owner_id: producer_id,
                    binding_kind: "ReturnExpression" @ 'NOW'
                },
                *local_binding_edge {
                    source_id: binding_id,
                    target_id: source_id,
                    relation_kind: source_relation,
                    source_kind: "LocalBinding",
                    target_kind: source_kind @ 'NOW'
                }
            :sort dynamic_span, producer_span, source_kind"#,
        );
        let rows = self.run_script(&script, params, ScriptMutability::Immutable)?;

        rows.rows
            .iter()
            .map(|row| decode_returned_call_binding_flow(row))
            .collect::<Result<Vec<_>, DbError>>()
    }

    pub fn returned_future_flows_for_owner(
        &self,
        owner_id: Uuid,
    ) -> Result<Vec<ReturnedFutureFlow>, DbError> {
        let mut params = BTreeMap::new();
        params.insert(
            "owner_id".to_string(),
            DataValue::Uuid(UuidWrapper(owner_id)),
        );

        let mut script = valid_call_target_rules();
        script.push_str(
            r#"
            ?[
                caller_id,
                producer_site_id,
                producer_span,
                producer_path,
                producer_id,
                binding_id,
                future_id,
                future_span,
                future_path,
                callee_kind,
                source_relation,
                source_kind
            ] :=
                caller_id = $owner_id,
                *call_site_edge {
                    source_id: caller_id,
                    target_id: producer_site_id,
                    relation_kind: "CallResultAwaited",
                    source_kind: caller_kind,
                    target_kind: "Path" @ 'NOW'
                },
                *call_site {
                    id: producer_site_id,
                    owner_id: caller_id,
                    call_kind: "Path",
                    span: producer_span,
                    path: producer_path @ 'NOW'
                },
                *call_relation {
                    source_id: producer_site_id,
                    target_id: producer_id,
                    relation_kind: "Function",
                    source_kind: "Path",
                    target_kind: "Function" @ 'NOW'
                },
                valid_target[producer_id, "Function", "Path", "Function"],
                *local_binding {
                    id: binding_id,
                    owner_id: producer_id,
                    binding_kind: "ReturnExpression",
                    source_kind: "DynamicCallResult",
                    source_id: future_id,
                    source_call_kind: "Dynamic",
                    callee_kind,
                    callee_path: future_path @ 'NOW'
                },
                callee_kind = "ReturnedPathCall",
                *local_binding_edge {
                    source_id: binding_id,
                    target_id: future_id,
                    relation_kind: source_relation,
                    source_kind: "LocalBinding",
                    target_kind: source_kind @ 'NOW'
                },
                source_relation = "BindingSourceCallResult",
                source_kind = "Dynamic",
                *call_site {
                    id: future_id,
                    owner_id: producer_id,
                    call_kind: "Dynamic",
                    span: future_span,
                    path: future_path @ 'NOW'
                }
            :sort producer_span, future_span"#,
        );
        let rows = self.run_script(&script, params, ScriptMutability::Immutable)?;

        rows.rows
            .iter()
            .map(|row| decode_returned_future_flow(row))
            .collect::<Result<Vec<_>, DbError>>()
    }

    pub fn returned_future_execution_flows_for_owner(
        &self,
        owner_id: Uuid,
    ) -> Result<Vec<ReturnedFutureExecutionFlow>, DbError> {
        let mut params = BTreeMap::new();
        params.insert(
            "owner_id".to_string(),
            DataValue::Uuid(UuidWrapper(owner_id)),
        );

        let mut script = valid_call_target_rules();
        script.push_str(
            r#"
            ?[
                caller_id,
                producer_site_id,
                producer_span,
                producer_path,
                producer_id,
                producer_binding_id,
                future_id,
                future_span,
                future_path,
                callee_kind,
                producer_source_relation,
                producer_source_kind,
                maker_site_id,
                maker_span,
                maker_id,
                callable_binding_id,
                callable_id,
                callable_source_relation,
                callable_source_kind,
                body_site_id,
                body_span,
                body_path,
                body_target_id,
                body_relation,
                body_source_kind,
                body_target_kind
            ] :=
                caller_id = $owner_id,
                *call_site_edge {
                    source_id: caller_id,
                    target_id: producer_site_id,
                    relation_kind: "CallResultAwaited",
                    source_kind: caller_kind,
                    target_kind: "Path" @ 'NOW'
                },
                *call_site {
                    id: producer_site_id,
                    owner_id: caller_id,
                    call_kind: "Path",
                    span: producer_span,
                    path: producer_path @ 'NOW'
                },
                *call_relation {
                    source_id: producer_site_id,
                    target_id: producer_id,
                    relation_kind: "Function",
                    source_kind: "Path",
                    target_kind: "Function" @ 'NOW'
                },
                valid_target[producer_id, "Function", "Path", "Function"],
                *local_binding {
                    id: producer_binding_id,
                    owner_id: producer_id,
                    binding_kind: "ReturnExpression",
                    source_kind: "DynamicCallResult",
                    source_id: future_id,
                    source_call_kind: "Dynamic",
                    callee_kind,
                    callee_path: future_path @ 'NOW'
                },
                callee_kind = "ReturnedPathCall",
                *local_binding_edge {
                    source_id: producer_binding_id,
                    target_id: future_id,
                    relation_kind: producer_source_relation,
                    source_kind: "LocalBinding",
                    target_kind: producer_source_kind @ 'NOW'
                },
                producer_source_relation = "BindingSourceCallResult",
                producer_source_kind = "Dynamic",
                *call_site {
                    id: future_id,
                    owner_id: producer_id,
                    call_kind: "Dynamic",
                    span: future_span,
                    path: future_path @ 'NOW'
                },
                *call_site {
                    id: maker_site_id,
                    owner_id: producer_id,
                    call_kind: "Path",
                    span: maker_span,
                    path: future_path @ 'NOW'
                },
                *call_relation {
                    source_id: maker_site_id,
                    target_id: maker_id,
                    relation_kind: "Function",
                    source_kind: "Path",
                    target_kind: "Function" @ 'NOW'
                },
                valid_target[maker_id, "Function", "Path", "Function"],
                *local_binding {
                    id: callable_binding_id,
                    owner_id: maker_id,
                    binding_kind: "ReturnExpression",
                    source_kind: "AsyncClosure",
                    source_id: callable_id @ 'NOW'
                },
                *local_binding_edge {
                    source_id: callable_binding_id,
                    target_id: callable_id,
                    relation_kind: callable_source_relation,
                    source_kind: "LocalBinding",
                    target_kind: callable_source_kind @ 'NOW'
                },
                callable_source_relation = "BindingSourceClosure",
                callable_source_kind = "Closure",
                *call_site {
                    id: body_site_id,
                    owner_id: callable_id,
                    span: body_span,
                    path: body_path @ 'NOW'
                },
                *call_relation {
                    source_id: body_site_id,
                    target_id: body_target_id,
                    relation_kind: body_relation,
                    source_kind: body_source_kind,
                    target_kind: body_target_kind @ 'NOW'
                },
                valid_target[body_target_id, body_relation, body_source_kind, body_target_kind]
            :sort producer_span, future_span, maker_span, body_span"#,
        );
        let rows = self.run_script(&script, params, ScriptMutability::Immutable)?;

        rows.rows
            .iter()
            .map(|row| decode_returned_future_execution_flow(row))
            .collect::<Result<Vec<_>, DbError>>()
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
