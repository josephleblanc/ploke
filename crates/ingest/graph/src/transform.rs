//! Transforms CodeGraph into CozoDB relations

use cozo::{DataValue, Db, MemStorage, ScriptMutability};
use std::collections::BTreeMap;
use syn_parser::parser::{
    graph::CodeGraph,
    nodes::TypeDefNode,
    relations::RelationKind,
    types::{TypeKind, VisibilityKind},
};

/// Transforms a CodeGraph into CozoDB relations
pub fn transform_code_graph(
    db: &Db<MemStorage>,
    code_graph: &CodeGraph,
) -> Result<(), cozo::Error> {
    // Transform types
    transform_types(db, code_graph)?;

    // Transform functions
    transform_functions(db, code_graph)?;

    // Transform defined types (structs, enums, etc.)
    transform_defined_types(db, code_graph)?;

    // Transform relations
    transform_relations(db, code_graph)?;

    Ok(())
}

/// Transforms type nodes into the types relation
fn transform_types(db: &Db<MemStorage>, code_graph: &CodeGraph) -> Result<(), cozo::Error> {
    for type_node in &code_graph.type_graph {
        let kind = match &type_node.kind {
            TypeKind::Named { .. } => "Named",
            TypeKind::Reference { .. } => "Reference",
            TypeKind::Slice { .. } => "Slice",
            TypeKind::Array { .. } => "Array",
            TypeKind::Tuple { .. } => "Tuple",
            TypeKind::Never => "Never",
            TypeKind::Inferred => "Inferred",
            TypeKind::RawPointer { .. } => "RawPointer",
            TypeKind::ImplTrait { .. } => "ImplTrait",
            TypeKind::TraitObject { .. } => "TraitObject",
            TypeKind::Macro { .. } => "Macro",
            TypeKind::Unknown { .. } => "Unknown",
            TypeKind::Function {
                is_unsafe: _,
                is_extern: _,
                abi: _,
            } => "Function",
            TypeKind::Paren { .. } => "Paren",
        };

        // Create a simplified string representation of the type
        let type_str = format!("{:?}", type_node.kind);

        let params = BTreeMap::from([
            ("id".to_string(), DataValue::from(type_node.id as i64)),
            ("kind".to_string(), DataValue::from(kind)),
            ("type_str".to_string(), DataValue::from(type_str)),
        ]);

        db.run_script(
            "?[id, kind, type_str] <- [[$id, $kind, $type_str]] :put types",
            params,
            ScriptMutability::Mutable,
        )?;

        // Add type relations for related types
        for (i, related_id) in type_node.related_types.iter().enumerate() {
            let relation_params = BTreeMap::from([
                ("type_id".to_string(), DataValue::from(type_node.id as i64)),
                ("related_index".to_string(), DataValue::from(i as i64)),
                (
                    "related_type_id".to_string(),
                    DataValue::from(*related_id as i64),
                ),
            ]);

            db.run_script(
                "?[type_id, related_index, related_type_id] <- [[$type_id, $related_index, $related_type_id]] :put type_relations",
                relation_params,
                ScriptMutability::Mutable,
            )?;
        }
    }

    Ok(())
}

/// Transforms function nodes into the functions relation
fn transform_functions(db: &Db<MemStorage>, code_graph: &CodeGraph) -> Result<(), cozo::Error> {
    for function in &code_graph.functions {
        let visibility = match function.visibility {
            VisibilityKind::Public => "Public",
            VisibilityKind::Crate => "Crate",
            VisibilityKind::Restricted(_) => "Restricted",
            VisibilityKind::Inherited => "Inherited",
        };

        let return_type_id = function
            .return_type
            .map(|id| DataValue::from(id as i64))
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

        let params = BTreeMap::from([
            ("id".to_string(), DataValue::from(function.id as i64)),
            ("name".to_string(), DataValue::from(function.name.as_str())),
            ("visibility".to_string(), DataValue::from(visibility)),
            ("return_type_id".to_string(), return_type_id),
            ("docstring".to_string(), docstring),
            ("body".to_string(), body),
        ]);

        db.run_script(
            "?[id, name, visibility, return_type_id, docstring, body] <- [[$id, $name, $visibility, $return_type_id, $docstring, $body]] :put functions",
            params,
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
                (
                    "function_id".to_string(),
                    DataValue::from(function.id as i64),
                ),
                ("param_index".to_string(), DataValue::from(i as i64)),
                ("param_name".to_string(), param_name),
                ("type_id".to_string(), DataValue::from(param.type_id as i64)),
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
                syn_parser::parser::types::GenericParamKind::Type { default, .. } => default
                    .map(|id| DataValue::from(id as i64))
                    .unwrap_or(DataValue::Null),
                syn_parser::parser::types::GenericParamKind::Const { type_id, .. } => {
                    DataValue::from(*type_id as i64)
                }
                _ => DataValue::Null,
            };

            let generic_params = BTreeMap::from([
                ("owner_id".to_string(), DataValue::from(function.id as i64)),
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
                ("owner_id".to_string(), DataValue::from(function.id as i64)),
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

/// Transforms defined types (structs, enums, etc.) into their respective relations
fn transform_defined_types(db: &Db<MemStorage>, code_graph: &CodeGraph) -> Result<(), cozo::Error> {
    for type_def in &code_graph.defined_types {
        match type_def {
            TypeDefNode::Struct(struct_node) => {
                let visibility = match struct_node.visibility {
                    VisibilityKind::Public => "Public",
                    VisibilityKind::Crate => "Crate",
                    VisibilityKind::Restricted(_) => "Restricted",
                    VisibilityKind::Inherited => "Inherited",
                };

                let docstring = struct_node
                    .docstring
                    .as_ref()
                    .map(|s| DataValue::from(s.as_str()))
                    .unwrap_or(DataValue::Null);

                let params = BTreeMap::from([
                    ("id".to_string(), DataValue::from(struct_node.id as i64)),
                    (
                        "name".to_string(),
                        DataValue::from(struct_node.name.as_str()),
                    ),
                    ("visibility".to_string(), DataValue::from(visibility)),
                    ("docstring".to_string(), docstring),
                ]);

                db.run_script(
                    "?[id, name, visibility, docstring] <- [[$id, $name, $visibility, $docstring]] :put structs",
                    params,
                    ScriptMutability::Mutable,
                )?;

                // Add struct fields
                for (i, field) in struct_node.fields.iter().enumerate() {
                    let field_name = field
                        .name
                        .as_ref()
                        .map(|s| DataValue::from(s.as_str()))
                        .unwrap_or(DataValue::Null);

                    let field_visibility = match field.visibility {
                        VisibilityKind::Public => "Public",
                        VisibilityKind::Crate => "Crate",
                        VisibilityKind::Restricted(_) => "Restricted",
                        VisibilityKind::Inherited => "Inherited",
                    };

                    let field_params = BTreeMap::from([
                        (
                            "struct_id".to_string(),
                            DataValue::from(struct_node.id as i64),
                        ),
                        ("field_index".to_string(), DataValue::from(i as i64)),
                        ("field_name".to_string(), field_name),
                        ("type_id".to_string(), DataValue::from(field.type_id as i64)),
                        ("visibility".to_string(), DataValue::from(field_visibility)),
                    ]);

                    db.run_script(
                        "?[struct_id, field_index, field_name, type_id, visibility] <- [[$struct_id, $field_index, $field_name, $type_id, $visibility]] :put struct_fields",
                        field_params,
                        ScriptMutability::Mutable,
                    )?;
                }
            }
            TypeDefNode::Enum(enum_node) => {
                let visibility = match enum_node.visibility {
                    VisibilityKind::Public => "Public",
                    VisibilityKind::Crate => "Crate",
                    VisibilityKind::Restricted(_) => "Restricted",
                    VisibilityKind::Inherited => "Inherited",
                };

                let docstring = enum_node
                    .docstring
                    .as_ref()
                    .map(|s| DataValue::from(s.as_str()))
                    .unwrap_or(DataValue::Null);

                let params = BTreeMap::from([
                    ("id".to_string(), DataValue::from(enum_node.id as i64)),
                    ("name".to_string(), DataValue::from(enum_node.name.as_str())),
                    ("visibility".to_string(), DataValue::from(visibility)),
                    ("docstring".to_string(), docstring),
                ]);

                db.run_script(
                    "?[id, name, visibility, docstring] <- [[$id, $name, $visibility, $docstring]] :put enums",
                    params,
                    ScriptMutability::Mutable,
                )?;

                // Add enum variants
                for (i, variant) in enum_node.variants.iter().enumerate() {
                    let discriminant = variant
                        .discriminant
                        .as_ref()
                        .map(|s| DataValue::from(s.as_str()))
                        .unwrap_or(DataValue::Null);

                    let variant_params = BTreeMap::from([
                        ("enum_id".to_string(), DataValue::from(enum_node.id as i64)),
                        ("variant_index".to_string(), DataValue::from(i as i64)),
                        (
                            "variant_name".to_string(),
                            DataValue::from(variant.name.as_str()),
                        ),
                        ("discriminant".to_string(), discriminant),
                    ]);

                    db.run_script(
                        "?[enum_id, variant_index, variant_name, discriminant] <- [[$enum_id, $variant_index, $variant_name, $discriminant]] :put enum_variants",
                        variant_params,
                        ScriptMutability::Mutable,
                    )?;
                }
            }
            // Handle other type definitions as needed
            _ => {}
        }
    }

    Ok(())
}

/// Transforms relations into the relations relation
fn transform_relations(db: &Db<MemStorage>, code_graph: &CodeGraph) -> Result<(), cozo::Error> {
    for relation in &code_graph.relations {
        let kind = match relation.kind {
            RelationKind::FunctionParameter => "FunctionParameter",
            RelationKind::FunctionReturn => "FunctionReturn",
            RelationKind::StructField => "StructField",
            RelationKind::EnumVariant => "EnumVariant",
            RelationKind::ImplementsFor => "ImplementsFor",
            RelationKind::ImplementsTrait => "ImplementsTrait",
            RelationKind::Inherits => "Inherits",
            RelationKind::References => "References",
            RelationKind::Contains => "Contains",
            RelationKind::Uses => "Uses",
            RelationKind::ValueType => "ValueType",
            RelationKind::MacroUse => "MacroUse",
            // These variants don't exist in the RelationKind enum
            // RelationKind::ModuleItem => "ModuleItem",
            // RelationKind::ModuleSubmodule => "ModuleSubmodule",
            // RelationKind::ModuleImport => "ModuleImport",
            // RelationKind::ModuleExport => "ModuleExport",
        };

        let params = BTreeMap::from([
            (
                "source_id".to_string(),
                DataValue::from(relation.source as i64),
            ),
            (
                "target_id".to_string(),
                DataValue::from(relation.target as i64),
            ),
            ("kind".to_string(), DataValue::from(kind)),
        ]);

        db.run_script(
            "?[source_id, target_id, kind] <- [[$source_id, $target_id, $kind]] :put relations",
            params,
            ScriptMutability::Mutable,
        )?;
    }

    Ok(())
}
