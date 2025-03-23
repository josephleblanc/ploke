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

    // Transform traits
    transform_traits(db, code_graph)?;

    // Transform impls
    transform_impls(db, code_graph)?;

    // Transform modules
    transform_modules(db, code_graph)?;

    // Transform values
    transform_values(db, code_graph)?;

    // Transform macros
    transform_macros(db, code_graph)?;

    // Transform relations
    transform_relations(db, code_graph)?;

    Ok(())
}

/// Transforms trait nodes into the traits relation
fn transform_traits(db: &Db<MemStorage>, code_graph: &CodeGraph) -> Result<(), cozo::Error> {
    // Process public traits
    for trait_node in &code_graph.traits {
        transform_single_trait(db, trait_node)?;
    }

    // Process private traits
    for trait_node in &code_graph.private_traits {
        transform_single_trait(db, trait_node)?;
    }

    Ok(())
}

/// Helper function to transform a single trait
fn transform_single_trait(
    db: &Db<MemStorage>,
    trait_node: &syn_parser::parser::nodes::TraitNode,
) -> Result<(), cozo::Error> {
    let visibility = match trait_node.visibility {
        VisibilityKind::Public => "Public",
        VisibilityKind::Crate => "Crate",
        VisibilityKind::Restricted(_) => "Restricted",
        VisibilityKind::Inherited => "Inherited",
    };

    let docstring = trait_node
        .docstring
        .as_ref()
        .map(|s| DataValue::from(s.as_str()))
        .unwrap_or(DataValue::Null);

    let params = BTreeMap::from([
        ("id".to_string(), DataValue::from(trait_node.id as i64)),
        ("name".to_string(), DataValue::from(trait_node.name.as_str())),
        ("visibility".to_string(), DataValue::from(visibility)),
        ("docstring".to_string(), docstring),
    ]);

    db.run_script(
        "?[id, name, visibility, docstring] <- [[$id, $name, $visibility, $docstring]] :put traits",
        params,
        ScriptMutability::Mutable,
    )?;

    // Add trait methods (they're already in the functions table)
    // We just need to add relations between the trait and its methods

    // Add super traits
    for (i, super_trait_id) in trait_node.super_traits.iter().enumerate() {
        let relation_params = BTreeMap::from([
            ("source_id".to_string(), DataValue::from(trait_node.id as i64)),
            ("target_id".to_string(), DataValue::from(*super_trait_id as i64)),
            ("kind".to_string(), DataValue::from("Inherits")),
        ]);

        db.run_script(
            "?[source_id, target_id, kind] <- [[$source_id, $target_id, $kind]] :put relations",
            relation_params,
            ScriptMutability::Mutable,
        )?;
    }

    Ok(())
}

/// Transforms impl nodes into the impls relation
fn transform_impls(db: &Db<MemStorage>, code_graph: &CodeGraph) -> Result<(), cozo::Error> {
    for impl_node in &code_graph.impls {
        let trait_type_id = impl_node
            .trait_type
            .map(|id| DataValue::from(id as i64))
            .unwrap_or(DataValue::Null);

        let params = BTreeMap::from([
            ("id".to_string(), DataValue::from(impl_node.id as i64)),
            (
                "self_type_id".to_string(),
                DataValue::from(impl_node.self_type as i64),
            ),
            ("trait_type_id".to_string(), trait_type_id),
        ]);

        db.run_script(
            "?[id, self_type_id, trait_type_id] <- [[$id, $self_type_id, $trait_type_id]] :put impls",
            params,
            ScriptMutability::Mutable,
        )?;

        // Add impl methods (they're already in the functions table)
        // We just need to add relations between the impl and its methods
    }

    Ok(())
}

/// Transforms module nodes into the modules relation
fn transform_modules(db: &Db<MemStorage>, code_graph: &CodeGraph) -> Result<(), cozo::Error> {
    for module in &code_graph.modules {
        let visibility = match module.visibility {
            VisibilityKind::Public => "Public",
            VisibilityKind::Crate => "Crate",
            VisibilityKind::Restricted(_) => "Restricted",
            VisibilityKind::Inherited => "Inherited",
        };

        let docstring = module
            .docstring
            .as_ref()
            .map(|s| DataValue::from(s.as_str()))
            .unwrap_or(DataValue::Null);

        let params = BTreeMap::from([
            ("id".to_string(), DataValue::from(module.id as i64)),
            ("name".to_string(), DataValue::from(module.name.as_str())),
            ("visibility".to_string(), DataValue::from(visibility)),
            ("docstring".to_string(), docstring),
        ]);

        db.run_script(
            "?[id, name, visibility, docstring] <- [[$id, $name, $visibility, $docstring]] :put modules",
            params,
            ScriptMutability::Mutable,
        )?;

        // Add submodule relationships
        for submodule_id in &module.submodules {
            let relation_params = BTreeMap::from([
                ("module_id".to_string(), DataValue::from(module.id as i64)),
                ("related_id".to_string(), DataValue::from(*submodule_id as i64)),
                ("kind".to_string(), DataValue::from("Contains")),
            ]);

            db.run_script(
                "?[module_id, related_id, kind] <- [[$module_id, $related_id, $kind]] :put module_relationships",
                relation_params,
                ScriptMutability::Mutable,
            )?;
        }

        // Add item relationships
        for item_id in &module.items {
            let relation_params = BTreeMap::from([
                ("module_id".to_string(), DataValue::from(module.id as i64)),
                ("related_id".to_string(), DataValue::from(*item_id as i64)),
                ("kind".to_string(), DataValue::from("Contains")),
            ]);

            db.run_script(
                "?[module_id, related_id, kind] <- [[$module_id, $related_id, $kind]] :put module_relationships",
                relation_params,
                ScriptMutability::Mutable,
            )?;
        }

        // Add export relationships
        for export_id in &module.exports {
            let relation_params = BTreeMap::from([
                ("module_id".to_string(), DataValue::from(module.id as i64)),
                ("related_id".to_string(), DataValue::from(*export_id as i64)),
                ("kind".to_string(), DataValue::from("Exports")),
            ]);

            db.run_script(
                "?[module_id, related_id, kind] <- [[$module_id, $related_id, $kind]] :put module_relationships",
                relation_params,
                ScriptMutability::Mutable,
            )?;
        }
    }

    Ok(())
}

/// Transforms value nodes into the values relation
fn transform_values(db: &Db<MemStorage>, code_graph: &CodeGraph) -> Result<(), cozo::Error> {
    for value in &code_graph.values {
        let visibility = match value.visibility {
            VisibilityKind::Public => "Public",
            VisibilityKind::Crate => "Crate",
            VisibilityKind::Restricted(_) => "Restricted",
            VisibilityKind::Inherited => "Inherited",
        };

        let kind = match value.kind {
            syn_parser::parser::nodes::ValueKind::Constant => "Constant",
            syn_parser::parser::nodes::ValueKind::Static { is_mutable } => {
                if is_mutable {
                    "MutableStatic"
                } else {
                    "Static"
                }
            }
        };

        let docstring = value
            .docstring
            .as_ref()
            .map(|s| DataValue::from(s.as_str()))
            .unwrap_or(DataValue::Null);

        let value_str = value
            .value
            .as_ref()
            .map(|s| DataValue::from(s.as_str()))
            .unwrap_or(DataValue::Null);

        let params = BTreeMap::from([
            ("id".to_string(), DataValue::from(value.id as i64)),
            ("name".to_string(), DataValue::from(value.name.as_str())),
            ("visibility".to_string(), DataValue::from(visibility)),
            ("type_id".to_string(), DataValue::from(value.type_id as i64)),
            ("kind".to_string(), DataValue::from(kind)),
            ("value".to_string(), value_str),
            ("docstring".to_string(), docstring),
        ]);

        db.run_script(
            "?[id, name, visibility, type_id, kind, value, docstring] <- [[$id, $name, $visibility, $type_id, $kind, $value, $docstring]] :put values",
            params,
            ScriptMutability::Mutable,
        )?;
    }

    Ok(())
}

/// Transforms macro nodes into the macros relation
fn transform_macros(db: &Db<MemStorage>, code_graph: &CodeGraph) -> Result<(), cozo::Error> {
    for macro_node in &code_graph.macros {
        let visibility = match macro_node.visibility {
            VisibilityKind::Public => "Public",
            VisibilityKind::Crate => "Crate",
            VisibilityKind::Restricted(_) => "Restricted",
            VisibilityKind::Inherited => "Inherited",
        };

        let kind = match macro_node.kind {
            syn_parser::parser::nodes::MacroKind::DeclarativeMacro => "DeclarativeMacro",
            syn_parser::parser::nodes::MacroKind::ProcedureMacro { kind } => match kind {
                syn_parser::parser::nodes::ProcMacroKind::Derive => "DeriveProcMacro",
                syn_parser::parser::nodes::ProcMacroKind::Attribute => "AttributeProcMacro",
                syn_parser::parser::nodes::ProcMacroKind::Function => "FunctionProcMacro",
            },
        };

        let docstring = macro_node
            .docstring
            .as_ref()
            .map(|s| DataValue::from(s.as_str()))
            .unwrap_or(DataValue::Null);

        let body = macro_node
            .body
            .as_ref()
            .map(|s| DataValue::from(s.as_str()))
            .unwrap_or(DataValue::Null);

        let params = BTreeMap::from([
            ("id".to_string(), DataValue::from(macro_node.id as i64)),
            ("name".to_string(), DataValue::from(macro_node.name.as_str())),
            ("visibility".to_string(), DataValue::from(visibility)),
            ("kind".to_string(), DataValue::from(kind)),
            ("docstring".to_string(), docstring),
            ("body".to_string(), body),
        ]);

        db.run_script(
            "?[id, name, visibility, kind, docstring, body] <- [[$id, $name, $visibility, $kind, $docstring, $body]] :put macros",
            params,
            ScriptMutability::Mutable,
        )?;
    }

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

        // Add type details
        match &type_node.kind {
            TypeKind::Reference {
                lifetime,
                is_mutable,
                ..
            } => {
                let lifetime_value = lifetime
                    .as_ref()
                    .map(|s| DataValue::from(s.as_str()))
                    .unwrap_or(DataValue::Null);

                let details_params = BTreeMap::from([
                    ("type_id".to_string(), DataValue::from(type_node.id as i64)),
                    ("is_mutable".to_string(), DataValue::from(*is_mutable)),
                    ("lifetime".to_string(), lifetime_value),
                    ("abi".to_string(), DataValue::Null),
                    ("is_unsafe".to_string(), DataValue::from(false)),
                    ("is_extern".to_string(), DataValue::from(false)),
                    ("dyn_token".to_string(), DataValue::from(false)),
                ]);

                db.run_script(
                    "?[type_id, is_mutable, lifetime, abi, is_unsafe, is_extern, dyn_token] <- [[$type_id, $is_mutable, $lifetime, $abi, $is_unsafe, $is_extern, $dyn_token]] :put type_details",
                    details_params,
                    ScriptMutability::Mutable,
                )?;
            }
            TypeKind::RawPointer { is_mutable, .. } => {
                let details_params = BTreeMap::from([
                    ("type_id".to_string(), DataValue::from(type_node.id as i64)),
                    ("is_mutable".to_string(), DataValue::from(*is_mutable)),
                    ("lifetime".to_string(), DataValue::Null),
                    ("abi".to_string(), DataValue::Null),
                    ("is_unsafe".to_string(), DataValue::from(false)),
                    ("is_extern".to_string(), DataValue::from(false)),
                    ("dyn_token".to_string(), DataValue::from(false)),
                ]);

                db.run_script(
                    "?[type_id, is_mutable, lifetime, abi, is_unsafe, is_extern, dyn_token] <- [[$type_id, $is_mutable, $lifetime, $abi, $is_unsafe, $is_extern, $dyn_token]] :put type_details",
                    details_params,
                    ScriptMutability::Mutable,
                )?;
            }
            TypeKind::Function {
                is_unsafe,
                is_extern,
                abi,
                ..
            } => {
                let abi_value = abi
                    .as_ref()
                    .map(|s| DataValue::from(s.as_str()))
                    .unwrap_or(DataValue::Null);

                let details_params = BTreeMap::from([
                    ("type_id".to_string(), DataValue::from(type_node.id as i64)),
                    ("is_mutable".to_string(), DataValue::from(false)),
                    ("lifetime".to_string(), DataValue::Null),
                    ("abi".to_string(), abi_value),
                    ("is_unsafe".to_string(), DataValue::from(*is_unsafe)),
                    ("is_extern".to_string(), DataValue::from(*is_extern)),
                    ("dyn_token".to_string(), DataValue::from(false)),
                ]);

                db.run_script(
                    "?[type_id, is_mutable, lifetime, abi, is_unsafe, is_extern, dyn_token] <- [[$type_id, $is_mutable, $lifetime, $abi, $is_unsafe, $is_extern, $dyn_token]] :put type_details",
                    details_params,
                    ScriptMutability::Mutable,
                )?;
            }
            TypeKind::TraitObject { dyn_token, .. } => {
                let details_params = BTreeMap::from([
                    ("type_id".to_string(), DataValue::from(type_node.id as i64)),
                    ("is_mutable".to_string(), DataValue::from(false)),
                    ("lifetime".to_string(), DataValue::Null),
                    ("abi".to_string(), DataValue::Null),
                    ("is_unsafe".to_string(), DataValue::from(false)),
                    ("is_extern".to_string(), DataValue::from(false)),
                    ("dyn_token".to_string(), DataValue::from(*dyn_token)),
                ]);

                db.run_script(
                    "?[type_id, is_mutable, lifetime, abi, is_unsafe, is_extern, dyn_token] <- [[$type_id, $is_mutable, $lifetime, $abi, $is_unsafe, $is_extern, $dyn_token]] :put type_details",
                    details_params,
                    ScriptMutability::Mutable,
                )?;
            }
            _ => {}
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
            TypeDefNode::TypeAlias(type_alias) => {
                let visibility = match type_alias.visibility {
                    VisibilityKind::Public => "Public",
                    VisibilityKind::Crate => "Crate",
                    VisibilityKind::Restricted(_) => "Restricted",
                    VisibilityKind::Inherited => "Inherited",
                };

                let docstring = type_alias
                    .docstring
                    .as_ref()
                    .map(|s| DataValue::from(s.as_str()))
                    .unwrap_or(DataValue::Null);

                let params = BTreeMap::from([
                    ("id".to_string(), DataValue::from(type_alias.id as i64)),
                    ("name".to_string(), DataValue::from(type_alias.name.as_str())),
                    ("visibility".to_string(), DataValue::from(visibility)),
                    ("type_id".to_string(), DataValue::from(type_alias.type_id as i64)),
                    ("docstring".to_string(), docstring),
                ]);

                db.run_script(
                    "?[id, name, visibility, type_id, docstring] <- [[$id, $name, $visibility, $type_id, $docstring]] :put type_aliases",
                    params,
                    ScriptMutability::Mutable,
                )?;
            }
            TypeDefNode::Union(union_node) => {
                let visibility = match union_node.visibility {
                    VisibilityKind::Public => "Public",
                    VisibilityKind::Crate => "Crate",
                    VisibilityKind::Restricted(_) => "Restricted",
                    VisibilityKind::Inherited => "Inherited",
                };

                let docstring = union_node
                    .docstring
                    .as_ref()
                    .map(|s| DataValue::from(s.as_str()))
                    .unwrap_or(DataValue::Null);

                let params = BTreeMap::from([
                    ("id".to_string(), DataValue::from(union_node.id as i64)),
                    ("name".to_string(), DataValue::from(union_node.name.as_str())),
                    ("visibility".to_string(), DataValue::from(visibility)),
                    ("docstring".to_string(), docstring),
                ]);

                db.run_script(
                    "?[id, name, visibility, docstring] <- [[$id, $name, $visibility, $docstring]] :put unions",
                    params,
                    ScriptMutability::Mutable,
                )?;

                // Add union fields (similar to struct fields)
                for (i, field) in union_node.fields.iter().enumerate() {
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
                            DataValue::from(union_node.id as i64),
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
