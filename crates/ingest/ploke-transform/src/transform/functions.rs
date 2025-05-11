use super::*;
/// Transforms function nodes into the functions relation
pub(super) fn transform_functions(
    db: &Db<MemStorage>,
    code_graph: &CodeGraph,
) -> Result<(), cozo::Error> {
    for function in &code_graph.functions {
        let (vis_kind, vis_path) = vis_to_dataval(function);

        let return_type_id = function
            .return_type
            .map(|id| id.into())
            .unwrap_or(DataValue::Null);

        let docstring = function
            .docstring
            .as_ref()
            .map(|s| DataValue::from(s.as_str()))
            .unwrap_or(DataValue::Null);

        let body = function
            .body
            .as_ref()
            .map(|s| DataValue::from(s.as_str()))
            .unwrap_or(DataValue::Null);

        // Insert into functions table
        let func_params = BTreeMap::from([
            ("id".to_string(), function.id.into()),
            ("name".to_string(), DataValue::from(function.name.as_str())),
            ("return_type_id".to_string(), return_type_id),
            ("docstring".to_string(), docstring),
            ("body".to_string(), body),
        ]);

        db.run_script(
            "?[id, name, return_type_id, docstring, body] <- [[$id, $name, $return_type_id, $docstring, $body]] :put functions",
            func_params,
            ScriptMutability::Mutable,
        )?;

        // Insert into visibility table
        let vis_params = BTreeMap::from([
            ("node_id".to_string(), function.id.into()),
            ("kind".to_string(), vis_kind),
            ("path".to_string(), vis_path.unwrap_or(DataValue::Null)),
        ]);

        db.run_script(
            "?[node_id, kind, path] <- [[$node_id, $kind, $path]] :put visibility",
            vis_params,
            ScriptMutability::Mutable,
        )?;

        // Add function parameters
        for (i, param) in function.parameters.iter().enumerate() {
            let param_name = param
                .name
                .as_ref()
                .map(|s| DataValue::from(s.as_str()))
                .unwrap_or(DataValue::Null);

            let param_params = BTreeMap::from([
                ("function_id".to_string(), function.id.into()),
                ("param_index".to_string(), DataValue::from(i as i64)),
                ("param_name".to_string(), param_name),
                ("type_id".to_string(), param.type_id.into()),
                ("is_mutable".to_string(), DataValue::from(param.is_mutable)),
                ("is_self".to_string(), DataValue::from(param.is_self)),
            ]);

            db.run_script(
                "?[function_id, param_index, param_name, type_id, is_mutable, is_self] <- [[$function_id, $param_index, $param_name, $type_id, $is_mutable, $is_self]] :put function_params",
                param_params,
                ScriptMutability::Mutable,
            )?;
        }

        // Add generic parameters
        for (i, generic_param) in function.generic_params.iter().enumerate() {
            let kind = match &generic_param.kind {
                syn_parser::parser::types::GenericParamKind::Type { .. } => "Type",
                syn_parser::parser::types::GenericParamKind::Lifetime { .. } => "Lifetime",
                syn_parser::parser::types::GenericParamKind::Const { .. } => "Const",
            };

            let name = match &generic_param.kind {
                syn_parser::parser::types::GenericParamKind::Type { name, .. } => name,
                syn_parser::parser::types::GenericParamKind::Lifetime { name, .. } => name,
                syn_parser::parser::types::GenericParamKind::Const { name, .. } => name,
            };

            let type_id = match &generic_param.kind {
                syn_parser::parser::types::GenericParamKind::Type { default, .. } => {
                    default.map(|id| id.into()).unwrap_or(DataValue::Null)
                }
                syn_parser::parser::types::GenericParamKind::Const { type_id, .. } => {
                    type_id.into()
                }
                _ => DataValue::Null,
            };

            let generic_params = BTreeMap::from([
                ("owner_id".to_string(), function.id.into()),
                ("param_index".to_string(), DataValue::from(i as i64)),
                ("kind".to_string(), DataValue::from(kind)),
                ("name".to_string(), DataValue::from(name.as_str())),
                ("type_id".to_string(), type_id),
            ]);

            db.run_script(
                "?[owner_id, param_index, kind, name, type_id] <- [[$owner_id, $param_index, $kind, $name, $type_id]] :put generic_params",
                generic_params,
                ScriptMutability::Mutable,
            )?;
        }

        // Add attributes
        for (i, attr) in function.attributes.iter().enumerate() {
            let value = attr
                .value
                .as_ref()
                .map(|s| DataValue::from(s.as_str()))
                .unwrap_or(DataValue::Null);

            let attr_params = BTreeMap::from([
                ("owner_id".to_string(), function.id.into()),
                ("attr_index".to_string(), DataValue::from(i as i64)),
                ("name".to_string(), DataValue::from(attr.name.as_str())),
                ("value".to_string(), value),
            ]);

            db.run_script(
                "?[owner_id, attr_index, name, value] <- [[$owner_id, $attr_index, $name, $value]] :put attributes",
                attr_params,
                ScriptMutability::Mutable,
            )?;
        }
    }

    Ok(())
}

fn vis_to_dataval(function: &FunctionNode) -> (DataValue, Option<DataValue>) {
    let (vis_kind, vis_path) = match &function.visibility {
        VisibilityKind::Public => (DataValue::from("public".to_string()), None),
        VisibilityKind::Crate => ("crate".into(), None),
        VisibilityKind::Restricted(path) => {
            let list = DataValue::List(
                path.iter()
                    .map(|p_string| DataValue::from(p_string.to_string()))
                    .collect(),
            );
            ("restricted".into(), Some(list))
        }
        VisibilityKind::Inherited => ("inherited".into(), None),
    };
    (vis_kind, vis_path)
}
