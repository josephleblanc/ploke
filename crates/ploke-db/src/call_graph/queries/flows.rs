use std::collections::BTreeMap;

use cozo::{DataValue, ScriptMutability, UuidWrapper};
use uuid::Uuid;

use crate::{Database, DbError};

use super::super::{
    CallContextRow, CallReceiver, CallRelationKind, CallSiteKind, CallStatusKind, CallTargetKind,
    FuturePollFieldProducerFlow, LocalBindingRelationKind, SelfFieldAssignmentArgumentFlow,
    SelfFieldAssignmentFlow, SelfFieldParameterFlow,
    decode::{
        decode_future_poll_field_producer_flow, decode_local_binding_edge, decode_resolution,
        decode_self_field_assignment_flow, decode_self_field_parameter_flow, decode_site,
        decode_target,
    },
    families::valid_call_owner_rules,
};

fn uuid_value(value: &DataValue, label: &str) -> Result<Uuid, DbError> {
    match value {
        DataValue::Uuid(value) => Ok(value.0),
        other => Err(DbError::Cozo(format!(
            "expected {label} to be a uuid, found {other:?}"
        ))),
    }
}

fn string_value(value: &DataValue, label: &str) -> Result<String, DbError> {
    match value {
        DataValue::Str(value) => Ok(value.to_string()),
        other => Err(DbError::Cozo(format!(
            "expected {label} to be a string, found {other:?}"
        ))),
    }
}

impl Database {
    pub fn future_poll_field_producer_flows_for_owner(
        &self,
        owner_id: Uuid,
    ) -> Result<Vec<FuturePollFieldProducerFlow>, DbError> {
        let owner_types = self.call_owner_self_type_names(owner_id)?;
        if owner_types.is_empty() {
            return Ok(Vec::new());
        }

        let candidates = self
            .call_context_for_owner(owner_id)?
            .into_iter()
            .filter_map(|row| {
                if row.site.kind != CallSiteKind::Method
                    || row.site.method.as_deref() != Some("poll")
                    || row.status.status == CallStatusKind::Resolved
                {
                    return None;
                }
                match row.site.receiver.as_ref()? {
                    CallReceiver::MethodResultField { field_path, .. } => {
                        Some((row.site.id, field_path.clone()))
                    }
                    _ => None,
                }
            })
            .collect::<Vec<_>>();

        let mut flows = Vec::new();
        for (site_id, field_path) in candidates {
            for owner_type in &owner_types {
                flows.extend(self.future_poll_field_producer_flows_for_site(
                    site_id,
                    &field_path,
                    owner_type,
                )?);
            }
        }
        Ok(flows)
    }

    fn call_owner_self_type_names(&self, owner_id: Uuid) -> Result<Vec<String>, DbError> {
        let mut params = BTreeMap::new();
        params.insert(
            "owner_id".to_string(),
            DataValue::Uuid(UuidWrapper(owner_id)),
        );

        let rows = self.run_script(
            r#"
            ?[type_name] :=
                owner_id = $owner_id,
                *method { id: owner_id, owner_id: impl_id @ 'NOW' },
                *impl { id: impl_id, self_type: self_type_id @ 'NOW' },
                *type_relation {
                    source_id: self_type_id,
                    target_id: self_target_id,
                    relation_kind: "Ordinary" @ 'NOW'
                },
                self_type_target[self_target_id, type_name]

            ?[type_name] :=
                owner_id = $owner_id,
                *method { id: owner_id, owner_id: impl_id @ 'NOW' },
                *impl { id: impl_id, self_type: self_type_id @ 'NOW' },
                *named_type { type_id: self_type_id, path @ 'NOW' },
                type_name in path

            self_type_target[id, name] := *struct { id, name @ 'NOW' }
            self_type_target[id, name] := *enum { id, name @ 'NOW' }
            self_type_target[id, name] := *union { id, name @ 'NOW' }
            :sort type_name"#,
            params,
            ScriptMutability::Immutable,
        )?;

        rows.rows
            .iter()
            .map(|row| match &row[0] {
                DataValue::Str(value) => Ok(value.to_string()),
                other => Err(DbError::Cozo(format!(
                    "expected owner self type name, found {other:?}"
                ))),
            })
            .collect()
    }

    fn future_poll_field_producer_flows_for_site(
        &self,
        site_id: Uuid,
        field_path: &[String],
        owner_type: &str,
    ) -> Result<Vec<FuturePollFieldProducerFlow>, DbError> {
        let field_name = format!("return.{}", field_path.join("."));
        let owner_type_path = DataValue::List(vec![DataValue::from(owner_type)]);
        let future_owner_type_path =
            DataValue::List(vec![DataValue::from("future"), DataValue::from(owner_type)]);

        let Some(poll_segment) = self.future_poll_site_segment(site_id)? else {
            return Ok(Vec::new());
        };

        let mut flows = Vec::new();
        for return_path in [owner_type_path, future_owner_type_path] {
            let candidates =
                self.future_poll_return_field_candidates(&field_name, owner_type, return_path)?;
            for candidate in candidates {
                let producer_id = uuid_value(&candidate[1], "future poll producer_id")?;
                let source_id = uuid_value(&candidate[23], "future poll field source_id")?;
                let source_kind =
                    string_value(&candidate[24], "future poll field source_call_kind")?;
                let Some(source_segment) =
                    self.future_poll_source_site_segment(source_id, producer_id, &source_kind)?
                else {
                    continue;
                };
                let Some(edge_segment) =
                    self.future_poll_source_edge_segment(&candidate[15], source_id, &source_kind)?
                else {
                    continue;
                };

                let mut row = poll_segment.clone();
                row.extend(candidate);
                row.extend(source_segment);
                row.extend(edge_segment);
                flows.push(decode_future_poll_field_producer_flow(&row)?);
            }
        }

        Ok(flows)
    }

    fn future_poll_site_segment(&self, site_id: Uuid) -> Result<Option<Vec<DataValue>>, DbError> {
        let mut params = BTreeMap::new();
        params.insert("site_id".to_string(), DataValue::Uuid(UuidWrapper(site_id)));

        let rows = self.run_script(
            r#"
            ?[
                site_id,
                site_owner_id,
                site_kind,
                site_span,
                site_cfgs,
                site_unsafe_block,
                site_path,
                site_method_name,
                site_macro_name,
                site_receiver_kind,
                site_receiver_path,
                site_arg_count,
                site_generic_arg_count,
                status_site_id,
                status_source_kind,
                status_kind,
                resolution_kind
            ] :=
                site_id = $site_id,
                *call_site {
                    id: site_id,
                    owner_id: site_owner_id,
                    call_kind: site_kind,
                    span: site_span,
                    cfgs: site_cfgs,
                    unsafe_block: site_unsafe_block,
                    path: site_path,
                    method_name: site_method_name,
                    macro_name: site_macro_name,
                    receiver_kind: site_receiver_kind,
                    receiver_path: site_receiver_path,
                    arg_count: site_arg_count,
                    generic_arg_count: site_generic_arg_count @ 'NOW'
                },
                site_kind = "Method",
                site_method_name = "poll",
                site_receiver_kind = "MethodResultField",
                *call_resolution_status {
                    source_id: status_site_id,
                    source_kind: status_source_kind,
                    status_kind,
                    resolution_kind @ 'NOW'
                },
                status_site_id = site_id,
                status_source_kind = "Method",
                status_kind != "Resolved""#,
            params,
            ScriptMutability::Immutable,
        )?;

        match rows.rows.len() {
            0 => Ok(None),
            1 => Ok(rows.rows.into_iter().next()),
            count => Err(DbError::Cozo(format!(
                "expected at most one future poll site segment for {site_id}, found {count}"
            ))),
        }
    }

    fn future_poll_return_field_candidates(
        &self,
        field_name: &str,
        owner_type: &str,
        return_path: DataValue,
    ) -> Result<Vec<Vec<DataValue>>, DbError> {
        let mut params = BTreeMap::new();
        params.insert("field_name".to_string(), DataValue::from(field_name));
        params.insert("owner_type".to_string(), DataValue::from(owner_type));
        params.insert("return_path".to_string(), return_path);

        let rows = self.run_script(
            r#"
            ?[
                owner_type,
                producer_id,
                return_id,
                return_owner_id,
                return_owner_kind,
                return_kind,
                return_name,
                return_span,
                return_cfgs,
                return_source_kind,
                return_source_id,
                return_source_call_kind,
                return_source_path,
                return_callee_kind,
                return_callee_path,
                field_id,
                field_owner_id,
                field_owner_kind,
                field_kind,
                field_name,
                field_span,
                field_cfgs,
                field_source_kind,
                field_source_id,
                field_source_call_kind,
                field_source_path,
                field_callee_kind,
                field_callee_path
            ] :=
                owner_type = $owner_type,
                *local_binding {
                    id: field_id,
                    owner_id: field_owner_id,
                    owner_kind: field_owner_kind,
                    binding_kind: field_kind,
                    name: field_name,
                    span: field_span,
                    cfgs: field_cfgs,
                    source_kind: field_source_kind,
                    source_id: field_source_id,
                    source_call_kind: field_source_call_kind,
                    source_path: field_source_path,
                    callee_kind: field_callee_kind,
                    callee_path: field_callee_path @ 'NOW'
                },
                field_name = $field_name,
                field_kind = "LetBinding",
                field_source_kind = "PathCallResult",
                producer_id = field_owner_id,
                *local_binding {
                    id: return_id,
                    owner_id: return_owner_id,
                    owner_kind: return_owner_kind,
                    binding_kind: return_kind,
                    name: return_name,
                    span: return_span,
                    cfgs: return_cfgs,
                    source_kind: return_source_kind,
                    source_id: return_source_id,
                    source_call_kind: return_source_call_kind,
                    source_path: return_source_path,
                    callee_kind: return_callee_kind,
                    callee_path: return_callee_path @ 'NOW'
                },
                return_owner_id = producer_id,
                return_kind = "ReturnExpression",
                return_name = "return",
                return_source_kind = "Constructed",
                return_source_path = $return_path
            :sort return_span, field_span"#,
            params,
            ScriptMutability::Immutable,
        )?;

        Ok(rows.rows)
    }

    fn future_poll_source_site_segment(
        &self,
        source_id: Uuid,
        producer_id: Uuid,
        source_kind: &str,
    ) -> Result<Option<Vec<DataValue>>, DbError> {
        let mut params = BTreeMap::new();
        params.insert(
            "source_id".to_string(),
            DataValue::Uuid(UuidWrapper(source_id)),
        );
        params.insert(
            "producer_id".to_string(),
            DataValue::Uuid(UuidWrapper(producer_id)),
        );
        params.insert("source_kind".to_string(), DataValue::from(source_kind));

        let rows = self.run_script(
            r#"
            ?[
                source_status_id,
                source_status_kind,
                source_status_state,
                source_resolution_kind,
                source_id,
                source_owner_id,
                source_kind,
                source_span,
                source_cfgs,
                source_unsafe_block,
                source_path,
                source_method_name,
                source_macro_name,
                source_receiver_kind,
                source_receiver_path,
                source_arg_count,
                source_generic_arg_count
            ] :=
                source_id = $source_id,
                producer_id = $producer_id,
                expected_source_kind = $source_kind,
                *call_site {
                    id: source_id,
                    owner_id: source_owner_id,
                    call_kind: source_kind,
                    span: source_span,
                    cfgs: source_cfgs,
                    unsafe_block: source_unsafe_block,
                    path: source_path,
                    method_name: source_method_name,
                    macro_name: source_macro_name,
                    receiver_kind: source_receiver_kind,
                    receiver_path: source_receiver_path,
                    arg_count: source_arg_count,
                    generic_arg_count: source_generic_arg_count @ 'NOW'
                },
                source_owner_id = producer_id,
                source_kind = expected_source_kind,
                *call_resolution_status {
                    source_id: source_status_id,
                    source_kind: source_status_kind,
                    status_kind: source_status_state,
                    resolution_kind: source_resolution_kind @ 'NOW'
                },
                source_status_id = source_id,
                source_status_kind = source_kind"#,
            params,
            ScriptMutability::Immutable,
        )?;

        match rows.rows.len() {
            0 => Ok(None),
            1 => Ok(rows.rows.into_iter().next()),
            count => Err(DbError::Cozo(format!(
                "expected at most one future poll source site segment for {source_id}, found {count}"
            ))),
        }
    }

    fn future_poll_source_edge_segment(
        &self,
        field_id: &DataValue,
        source_id: Uuid,
        source_kind: &str,
    ) -> Result<Option<Vec<DataValue>>, DbError> {
        let mut params = BTreeMap::new();
        params.insert("field_id".to_string(), field_id.clone());
        params.insert(
            "source_id".to_string(),
            DataValue::Uuid(UuidWrapper(source_id)),
        );
        params.insert("source_kind".to_string(), DataValue::from(source_kind));

        let rows = self.run_script(
            r#"
            ?[
                edge_source_id,
                edge_target_id,
                edge_relation,
                edge_source_kind,
                edge_target_kind
            ] :=
                field_id = $field_id,
                source_id = $source_id,
                source_kind = $source_kind,
                *local_binding_edge {
                    source_id: edge_source_id,
                    target_id: edge_target_id,
                    relation_kind: edge_relation,
                    source_kind: edge_source_kind,
                    target_kind: edge_target_kind @ 'NOW'
                },
                edge_source_id = field_id,
                edge_target_id = source_id,
                edge_relation = "BindingSourceCallResult",
                edge_source_kind = "LocalBinding",
                edge_target_kind = source_kind"#,
            params,
            ScriptMutability::Immutable,
        )?;

        match rows.rows.len() {
            0 => Ok(None),
            1 => Ok(rows.rows.into_iter().next()),
            count => Err(DbError::Cozo(format!(
                "expected at most one future poll source edge for {source_id}, found {count}"
            ))),
        }
    }

    pub fn self_field_parameter_flows_for_owner(
        &self,
        owner_id: Uuid,
    ) -> Result<Vec<SelfFieldParameterFlow>, DbError> {
        let candidates = self
            .call_context_for_owner(owner_id)?
            .into_iter()
            .filter_map(|row| {
                if row.status.status == CallStatusKind::Resolved {
                    return None;
                }
                let path = row.site.path.as_ref()?;
                match row.site.kind {
                    CallSiteKind::Dynamic if path.len() == 2 && path[0] == "self" => {
                        Some((row.site.id, path[1].clone()))
                    }
                    CallSiteKind::Path if path.len() == 1 => Some((row.site.id, path[0].clone())),
                    _ => None,
                }
            })
            .collect::<Vec<_>>();

        let mut flows = Vec::new();
        for (site_id, field) in candidates {
            flows.extend(self.self_field_parameter_flows_for_site(site_id, &field)?);
        }
        Ok(flows)
    }

    fn self_field_parameter_flows_for_site(
        &self,
        site_id: Uuid,
        field: &str,
    ) -> Result<Vec<SelfFieldParameterFlow>, DbError> {
        let mut params = BTreeMap::new();
        params.insert("site_id".to_string(), DataValue::Uuid(UuidWrapper(site_id)));
        params.insert("field_name".to_string(), DataValue::from(field));
        params.insert(
            "field_path".to_string(),
            DataValue::List(vec![DataValue::from(field)]),
        );

        let mut script = valid_call_owner_rules();
        script.push_str(
            r#"
            ?[
                site_id,
                site_owner_id,
                site_kind,
                site_span,
                site_cfgs,
                site_unsafe_block,
                site_path,
                site_method_name,
                site_macro_name,
                site_receiver_kind,
                site_receiver_path,
                site_arg_count,
                site_generic_arg_count,
                status_site_id,
                status_source_kind,
                status_kind,
                resolution_kind,
                constructor_id,
                return_id,
                return_owner_id,
                return_owner_kind,
                return_kind,
                return_name,
                return_span,
                return_cfgs,
                return_source_kind,
                return_source_id,
                return_source_call_kind,
                return_source_path,
                return_callee_kind,
                return_callee_path,
                field_id,
                field_owner_id,
                field_owner_kind,
                field_kind,
                field_name,
                field_span,
                field_cfgs,
                field_source_kind,
                field_source_id,
                field_source_call_kind,
                field_source_path,
                field_callee_kind,
                field_callee_path,
                parameter_id,
                parameter_owner_id,
                parameter_owner_kind,
                parameter_kind,
                parameter_name,
                parameter_span,
                parameter_cfgs,
                parameter_source_kind,
                parameter_source_id,
                parameter_source_call_kind,
                parameter_source_path,
                parameter_callee_kind,
                parameter_callee_path
            ] :=
                site_id = $site_id,
                *call_site {
                    id: site_id,
                    owner_id: site_owner_id,
                    call_kind: site_kind,
                    span: site_span,
                    cfgs: site_cfgs,
                    unsafe_block: site_unsafe_block,
                    path: site_path,
                    method_name: site_method_name,
                    macro_name: site_macro_name,
                    receiver_kind: site_receiver_kind,
                    receiver_path: site_receiver_path,
                    arg_count: site_arg_count,
                    generic_arg_count: site_generic_arg_count @ 'NOW'
                },
                valid_owner[site_owner_id, site_owner_kind],
                *call_resolution_status {
                    source_id: status_site_id,
                    source_kind: status_source_kind,
                    status_kind,
                    resolution_kind @ 'NOW'
                },
                status_site_id = site_id,
                status_source_kind = "Dynamic",
                *local_binding {
                    id: return_id,
                    owner_id: return_owner_id,
                    owner_kind: return_owner_kind,
                    binding_kind: return_kind,
                    name: return_name,
                    span: return_span,
                    cfgs: return_cfgs,
                    source_kind: return_source_kind,
                    source_id: return_source_id,
                    source_call_kind: return_source_call_kind,
                    source_path: return_source_path,
                    callee_kind: return_callee_kind,
                    callee_path: return_callee_path @ 'NOW'
                },
                constructor_id = return_owner_id,
                return_kind = "ReturnExpression",
                return_source_kind = "Constructed",
                *local_binding {
                    id: field_id,
                    owner_id: field_owner_id,
                    owner_kind: field_owner_kind,
                    binding_kind: field_kind,
                    name: field_name,
                    span: field_span,
                    cfgs: field_cfgs,
                    source_kind: field_source_kind,
                    source_id: field_source_id,
                    source_call_kind: field_source_call_kind,
                    source_path: field_source_path,
                    callee_kind: field_callee_kind,
                    callee_path: field_callee_path @ 'NOW'
                },
                field_owner_id = constructor_id,
                field_kind = "FieldProjection",
                field_source_kind = "FieldProjection",
                field_source_id = return_id,
                field_source_path == $field_path,
                field_callee_kind = "Path",
                field_callee_path == $field_path,
                *local_binding {
                    id: parameter_id,
                    owner_id: parameter_owner_id,
                    owner_kind: parameter_owner_kind,
                    binding_kind: parameter_kind,
                    name: parameter_name,
                    span: parameter_span,
                    cfgs: parameter_cfgs,
                    source_kind: parameter_source_kind,
                    source_id: parameter_source_id,
                    source_call_kind: parameter_source_call_kind,
                    source_path: parameter_source_path,
                    callee_kind: parameter_callee_kind,
                    callee_path: parameter_callee_path @ 'NOW'
                },
                parameter_owner_id = constructor_id,
                parameter_kind = "ParameterBinding",
                parameter_name == $field_name,
                parameter_source_kind = "Parameter",
                *local_binding_edge {
                    source_id: field_id,
                    target_id: return_id,
                    relation_kind: "BindingProjectsField",
                    source_kind: "LocalBinding",
                    target_kind: "LocalBinding" @ 'NOW'
                }
            :sort site_span, constructor_id, field_span"#,
        );
        let rows = self.run_script(&script, params, ScriptMutability::Immutable)?;

        rows.rows
            .iter()
            .map(|row| decode_self_field_parameter_flow(row))
            .collect::<Result<Vec<_>, DbError>>()
    }

    pub fn self_field_assignment_flows_for_owner(
        &self,
        owner_id: Uuid,
    ) -> Result<Vec<SelfFieldAssignmentFlow>, DbError> {
        let owner_types = self.call_owner_self_type_names(owner_id)?;
        if owner_types.is_empty() {
            return Ok(Vec::new());
        }

        let candidates = self
            .call_context_for_owner(owner_id)?
            .into_iter()
            .filter_map(|row| {
                if row.status.status == CallStatusKind::Resolved {
                    return None;
                }
                let path = row.site.path.as_ref()?;
                match row.site.kind {
                    CallSiteKind::Dynamic if path.len() == 2 && path[0] == "self" => {
                        Some((row.site.id, path[1].clone()))
                    }
                    CallSiteKind::Path if path.len() == 1 => Some((row.site.id, path[0].clone())),
                    _ => None,
                }
            })
            .collect::<Vec<_>>();

        let mut flows = Vec::new();
        for (site_id, field) in candidates {
            for owner_type in &owner_types {
                flows.extend(
                    self.self_field_assignment_flows_for_site(site_id, &field, owner_type)?,
                );
            }
        }
        Ok(flows)
    }

    fn self_field_assignment_flows_for_site(
        &self,
        site_id: Uuid,
        field: &str,
        owner_type: &str,
    ) -> Result<Vec<SelfFieldAssignmentFlow>, DbError> {
        let mut params = BTreeMap::new();
        params.insert("site_id".to_string(), DataValue::Uuid(UuidWrapper(site_id)));
        params.insert("owner_type".to_string(), DataValue::from(owner_type));
        params.insert(
            "field_path".to_string(),
            DataValue::List(vec![DataValue::from(field)]),
        );

        let mut script = valid_call_owner_rules();
        script.push_str(
            r#"
            ?[
                site_id,
                site_owner_id,
                site_kind,
                site_span,
                site_cfgs,
                site_unsafe_block,
                site_path,
                site_method_name,
                site_macro_name,
                site_receiver_kind,
                site_receiver_path,
                site_arg_count,
                site_generic_arg_count,
                status_site_id,
                status_source_kind,
                status_kind,
                resolution_kind,
                owner_type,
                setter_id,
                assignment_id,
                assignment_owner_id,
                assignment_owner_kind,
                assignment_kind,
                assignment_name,
                assignment_span,
                assignment_cfgs,
                assignment_source_kind,
                assignment_source_id,
                assignment_source_call_kind,
                assignment_source_path,
                assignment_callee_kind,
                assignment_callee_path,
                parameter_id,
                parameter_owner_id,
                parameter_owner_kind,
                parameter_kind,
                parameter_name,
                parameter_span,
                parameter_cfgs,
                parameter_source_kind,
                parameter_source_id,
                parameter_source_call_kind,
                parameter_source_path,
                parameter_callee_kind,
                parameter_callee_path,
                edge_source_id,
                edge_target_id,
                edge_relation,
                edge_source_kind,
                edge_target_kind
            ] :=
                site_id = $site_id,
                owner_type = $owner_type,
                *call_site {
                    id: site_id,
                    owner_id: site_owner_id,
                    call_kind: site_kind,
                    span: site_span,
                    cfgs: site_cfgs,
                    unsafe_block: site_unsafe_block,
                    path: site_path,
                    method_name: site_method_name,
                    macro_name: site_macro_name,
                    receiver_kind: site_receiver_kind,
                    receiver_path: site_receiver_path,
                    arg_count: site_arg_count,
                    generic_arg_count: site_generic_arg_count @ 'NOW'
                },
                valid_owner[site_owner_id, site_owner_kind],
                *call_resolution_status {
                    source_id: status_site_id,
                    source_kind: status_source_kind,
                    status_kind,
                    resolution_kind @ 'NOW'
                },
                status_site_id = site_id,
                status_source_kind = site_kind,
                status_kind != "Resolved",
                setter_self_type[setter_id, owner_type],
                *local_binding {
                    id: assignment_id,
                    owner_id: assignment_owner_id,
                    owner_kind: assignment_owner_kind,
                    binding_kind: assignment_kind,
                    name: assignment_name,
                    span: assignment_span,
                    cfgs: assignment_cfgs,
                    source_kind: assignment_source_kind,
                    source_id: assignment_source_id,
                    source_call_kind: assignment_source_call_kind,
                    source_path: assignment_source_path,
                    callee_kind: assignment_callee_kind,
                    callee_path: assignment_callee_path @ 'NOW'
                },
                assignment_owner_id = setter_id,
                assignment_kind = "FieldAssignment",
                assignment_source_kind = "SelfFieldAssignment",
                assignment_source_path == $field_path,
                assignment_callee_kind = "Path",
                *local_binding_edge {
                    source_id: edge_source_id,
                    target_id: edge_target_id,
                    relation_kind: edge_relation,
                    source_kind: edge_source_kind,
                    target_kind: edge_target_kind @ 'NOW'
                },
                edge_source_id = assignment_id,
                edge_relation = "BindingSourceParameter",
                edge_source_kind = "LocalBinding",
                edge_target_kind = "LocalBinding",
                *local_binding {
                    id: parameter_id,
                    owner_id: parameter_owner_id,
                    owner_kind: parameter_owner_kind,
                    binding_kind: parameter_kind,
                    name: parameter_name,
                    span: parameter_span,
                    cfgs: parameter_cfgs,
                    source_kind: parameter_source_kind,
                    source_id: parameter_source_id,
                    source_call_kind: parameter_source_call_kind,
                    source_path: parameter_source_path,
                    callee_kind: parameter_callee_kind,
                    callee_path: parameter_callee_path @ 'NOW'
                },
                parameter_id = edge_target_id,
                parameter_owner_id = setter_id,
                parameter_kind = "ParameterBinding",
                parameter_source_kind = "Parameter"

            setter_self_type[method_id, type_name] :=
                *method { id: method_id, owner_id: impl_id @ 'NOW' },
                *impl { id: impl_id, self_type: self_type_id @ 'NOW' },
                *type_relation {
                    source_id: self_type_id,
                    target_id: self_target_id,
                    relation_kind: "Ordinary" @ 'NOW'
                },
                self_type_target[self_target_id, type_name]

            setter_self_type[method_id, type_name] :=
                *method { id: method_id, owner_id: impl_id @ 'NOW' },
                *impl { id: impl_id, self_type: self_type_id @ 'NOW' },
                *named_type { type_id: self_type_id, path @ 'NOW' },
                type_name in path

            self_type_target[id, name] := *struct { id, name @ 'NOW' }
            self_type_target[id, name] := *enum { id, name @ 'NOW' }
            self_type_target[id, name] := *union { id, name @ 'NOW' }
            :sort site_span, owner_type, assignment_span"#,
        );
        let rows = self.run_script(&script, params, ScriptMutability::Immutable)?;
        rows.rows
            .iter()
            .map(|row| decode_self_field_assignment_flow(row))
            .collect::<Result<Vec<_>, DbError>>()
    }

    pub fn self_field_assignment_argument_flows_for_owner(
        &self,
        owner_id: Uuid,
    ) -> Result<Vec<SelfFieldAssignmentArgumentFlow>, DbError> {
        let field_flows = self.self_field_assignment_flows_for_owner(owner_id)?;
        let mut flows = Vec::new();

        for field_flow in field_flows {
            flows.extend(self.self_field_assignment_argument_flows_for_field(&field_flow)?);
        }

        flows.sort_by_key(|flow| {
            (
                flow.field_flow.site.span,
                flow.field_flow.setter_id,
                flow.setter_call.site.span,
                flow.argument_edge.source_id,
            )
        });
        Ok(flows)
    }

    fn self_field_assignment_argument_flows_for_field(
        &self,
        field_flow: &SelfFieldAssignmentFlow,
    ) -> Result<Vec<SelfFieldAssignmentArgumentFlow>, DbError> {
        let mut params = BTreeMap::new();
        params.insert(
            "setter_id".to_string(),
            DataValue::Uuid(UuidWrapper(field_flow.setter_id)),
        );
        params.insert(
            "parameter_id".to_string(),
            DataValue::Uuid(UuidWrapper(field_flow.parameter_binding.id)),
        );

        let mut script = valid_call_owner_rules();
        script.push_str(
            r#"
            ?[
                site_id,
                owner_id,
                site_kind,
                site_span,
                site_cfgs,
                site_unsafe_block,
                site_path,
                site_method_name,
                site_macro_name,
                site_receiver_kind,
                site_receiver_path,
                site_arg_count,
                site_generic_arg_count,
                status_site_id,
                status_source_kind,
                status_kind,
                resolution_kind,
                target_site_id,
                target_id,
                target_relation,
                target_source_kind,
                target_kind,
                edge_source_id,
                edge_target_id,
                edge_relation,
                edge_source_kind,
                edge_target_kind
            ] :=
                setter_id = $setter_id,
                parameter_id = $parameter_id,
                *local_binding_edge {
                    source_id: edge_source_id,
                    target_id: edge_target_id,
                    relation_kind: edge_relation,
                    source_kind: edge_source_kind,
                    target_kind: edge_target_kind @ 'NOW'
                },
                edge_target_id = parameter_id,
                edge_relation = "ArgumentSuppliesParameter",
                edge_target_kind = "LocalBinding",
                *call_relation {
                    source_id: target_site_id,
                    target_id,
                    relation_kind: target_relation,
                    source_kind: target_source_kind,
                    target_kind @ 'NOW'
                },
                target_site_id = edge_source_id,
                target_id = setter_id,
                target_kind = "Method",
                target_source_kind = edge_source_kind,
                *call_site {
                    id: site_id,
                    owner_id,
                    call_kind: site_kind,
                    span: site_span,
                    cfgs: site_cfgs,
                    unsafe_block: site_unsafe_block,
                    path: site_path,
                    method_name: site_method_name,
                    macro_name: site_macro_name,
                    receiver_kind: site_receiver_kind,
                    receiver_path: site_receiver_path,
                    arg_count: site_arg_count,
                    generic_arg_count: site_generic_arg_count @ 'NOW'
                },
                site_id = edge_source_id,
                site_kind = edge_source_kind,
                valid_owner[owner_id, _owner_kind],
                *call_resolution_status {
                    source_id: status_site_id,
                    source_kind: status_source_kind,
                    status_kind,
                    resolution_kind @ 'NOW'
                },
                status_site_id = site_id,
                status_source_kind = site_kind,
                status_kind = "Resolved"
            :sort site_span, site_id"#,
        );

        let rows = self.run_script(&script, params, ScriptMutability::Immutable)?;
        rows.rows
            .iter()
            .map(|row| {
                let site = decode_site(&row[0..13])?;
                let status = decode_resolution(&row[13..17])?;
                let target = decode_target(&row[17..22])?;
                let argument_edge = decode_local_binding_edge(&row[22..27])?;
                let flow = SelfFieldAssignmentArgumentFlow {
                    field_flow: field_flow.clone(),
                    setter_call: CallContextRow {
                        site,
                        status,
                        targets: vec![target],
                    },
                    argument_edge,
                };
                validate_self_field_assignment_argument_flow(&flow)?;
                Ok(flow)
            })
            .collect::<Result<Vec<_>, DbError>>()
    }
}

fn validate_self_field_assignment_argument_flow(
    flow: &SelfFieldAssignmentArgumentFlow,
) -> Result<(), DbError> {
    let Some(target) = flow.setter_call.targets.as_slice().first() else {
        return Err(DbError::Cozo(format!(
            "self-field assignment argument flow missing setter target for call site {}",
            flow.setter_call.site.id
        )));
    };
    let valid = flow.setter_call.targets.len() == 1
        && flow.setter_call.status.site_id == flow.setter_call.site.id
        && flow.setter_call.status.site_kind == flow.setter_call.site.kind
        && flow.setter_call.status.status == CallStatusKind::Resolved
        && flow.setter_call.status.resolution.is_some()
        && target.site_id == flow.setter_call.site.id
        && target.target_id == flow.field_flow.setter_id
        && target.relation == CallRelationKind::Method
        && target.source_kind == flow.setter_call.site.kind
        && target.target_kind == CallTargetKind::Method
        && flow.argument_edge.source_id == flow.setter_call.site.id
        && flow.argument_edge.target_id == flow.field_flow.parameter_binding.id
        && flow.argument_edge.relation == LocalBindingRelationKind::ArgumentSuppliesParameter
        && flow.argument_edge.source_kind == flow.setter_call.site.kind.as_str()
        && flow.argument_edge.target_kind == "LocalBinding";

    if valid {
        Ok(())
    } else {
        Err(DbError::Cozo(format!(
            "malformed self-field assignment argument flow for call site {}",
            flow.setter_call.site.id
        )))
    }
}
