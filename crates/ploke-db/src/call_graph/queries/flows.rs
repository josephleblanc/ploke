use std::collections::BTreeMap;

use cozo::{DataValue, ScriptMutability, UuidWrapper};
use uuid::Uuid;

use crate::{Database, DbError};

use super::super::{
    CallSiteKind, CallStatusKind, SelfFieldParameterFlow, decode::decode_self_field_parameter_flow,
    families::valid_call_owner_rules,
};

impl Database {
    pub fn self_field_parameter_flows_for_owner(
        &self,
        owner_id: Uuid,
    ) -> Result<Vec<SelfFieldParameterFlow>, DbError> {
        let candidates = self
            .call_context_for_owner(owner_id)?
            .into_iter()
            .filter_map(|row| {
                if row.site.kind != CallSiteKind::Dynamic
                    || row.status.status == CallStatusKind::Resolved
                {
                    return None;
                }
                let path = row.site.path.as_ref()?;
                if path.len() == 2 && path[0] == "self" {
                    Some((row.site.id, path[1].clone()))
                } else {
                    None
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
                site_kind = "Dynamic",
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
}
