//! Transforms CodeGraph into CozoDB relations

// -- external
use cozo::{DataValue, Db, MemStorage, Num, ScriptMutability};
use std::collections::BTreeMap;
// -- from workspace
use syn_parser::parser::nodes::*;
use syn_parser::parser::types::TypeNode;
use syn_parser::parser::{graph::CodeGraph, nodes::TypeDefNode, types::VisibilityKind};
use syn_parser::resolve::module_tree::ModuleTree;
use syn_parser::resolve::RelationIndexer;
use syn_parser::utils::LogStyle;

mod fields;
mod secondary_nodes;
// -- primary nodes --
mod enums;
mod functions;
mod impls;
mod structs;

// -- primary node transforms
use functions::transform_functions;

// -- secondary node transformations
use secondary_nodes::{process_attributes, process_generic_params, process_params};

// -- schema
use crate::schema::secondary_nodes::AttributeNodeSchema;

/// Transforms a CodeGraph into CozoDB relations
pub fn transform_code_graph(
    db: &Db<MemStorage>,
    code_graph: CodeGraph,
    tree: &ModuleTree,
) -> Result<(), cozo::Error> {
    let relations = code_graph.relations;
    // Transform types
    // [ ] Refactored
    transform_types(db, code_graph.type_graph)?;

    // Transform functions
    // [✔] Refactored
    transform_functions(db, code_graph.functions, tree)?;

    // Transform defined types (structs, enums, etc.)
    // [ ] Refactored
    //  - [✔] Struct Refactored
    //  - [ ] Enum Refactored
    //  - [ ] Union Refactored
    //  - [ ] TypeAlias Refactored
    transform_defined_types(db, code_graph.defined_types)?;

    // Transform traits
    // [ ] Refactored
    transform_traits(db, code_graph.traits)?;

    // Transform impls
    // [ ] Refactored
    transform_impls(db, code_graph.impls)?;

    // Transform modules
    // [ ] Refactored
    transform_modules(db, code_graph.modules)?;

    // Transform consts
    // [ ] Refactored
    #[cfg(not(feature = "type_bearing_ids"))]
    transform_consts(db, code_graph)?;
    // Transoform statics
    // [ ] Refactored
    #[cfg(not(feature = "type_bearing_ids"))]
    transform_statics(db, code_graph)?;

    // Transform macros
    transform_macros(db, code_graph.macros)?;

    // Transform relations
    // [ ] Refactored
    #[cfg(not(feature = "type_bearing_ids"))]
    transform_relations(db, code_graph)?;

    Ok(())
}

/// Transforms trait nodes into the traits relation
fn transform_traits(db: &Db<MemStorage>, traits: Vec<TraitNode>) -> Result<(), cozo::Error> {
    // Process public traits
    for trait_node in traits.into_iter() {
        transform_single_trait(db, &trait_node)?;
    }

    Ok(())
}

/// Helper function to transform a single trait
fn transform_single_trait(
    db: &Db<MemStorage>,
    trait_node: &syn_parser::parser::nodes::TraitNode,
) -> Result<(), cozo::Error> {
    let (vis_kind, vis_path) = match &trait_node.visibility {
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

    let docstring = trait_node
        .docstring
        .as_ref()
        .map(|s| DataValue::from(s.as_str()))
        .unwrap_or(DataValue::Null);

    // Insert into traits table
    let trait_params = BTreeMap::from([
        ("id".to_string(), trait_node.id.into()),
        (
            "name".to_string(),
            DataValue::from(trait_node.name.as_str()),
        ),
        ("docstring".to_string(), docstring),
    ]);

    db.run_script(
        "?[id, name, docstring] <- [[$id, $name, $docstring]] :put traits",
        trait_params,
        ScriptMutability::Mutable,
    )?;

    // Insert into visibility table
    let vis_params = BTreeMap::from([
        ("node_id".to_string(), trait_node.id.into()),
        ("kind".to_string(), vis_kind),
        ("path".to_string(), vis_path.unwrap_or(DataValue::Null)),
    ]);

    db.run_script(
        "?[node_id, kind, path] <- [[$node_id, $kind, $path]] :put visibility",
        vis_params,
        ScriptMutability::Mutable,
    )?;

    // Add trait methods (they're already in the functions table)
    // We just need to add relations between the trait and its methods

    // Add super traits
    for super_trait_id in trait_node.super_traits.iter() {
        let relation_params = BTreeMap::from([
            ("source_id".to_string(), trait_node.id.into()),
            ("target_id".to_string(), super_trait_id.into()),
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
fn transform_impls(db: &Db<MemStorage>, impls: Vec<ImplNode>) -> Result<(), cozo::Error> {
    for impl_node in impls.into_iter() {
        let trait_type_id = impl_node
            .trait_type
            .map(|id| id.into())
            .unwrap_or(DataValue::Null);

        let params: BTreeMap<String, DataValue> = BTreeMap::from([
            ("id".to_string(), impl_node.id.into()),
            ("self_type_id".to_string(), impl_node.self_type.into()),
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
fn transform_modules(db: &Db<MemStorage>, modules: Vec<ModuleNode>) -> Result<(), cozo::Error> {
    for module in modules {
        let (vis_kind, vis_path) = match &module.visibility {
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

        let docstring = module
            .docstring
            .as_ref()
            .map(|s| DataValue::from(s.as_str()))
            .unwrap_or(DataValue::Null);

        // Insert into modules table
        let module_params = BTreeMap::from([
            ("id".to_string(), module.id.into()),
            ("name".to_string(), DataValue::from(module.name.as_str())),
            ("docstring".to_string(), docstring),
        ]);

        db.run_script(
            "?[id, name, docstring] <- [[$id, $name, $docstring]] :put modules",
            module_params,
            ScriptMutability::Mutable,
        )?;

        // Insert into visibility table
        let vis_params = BTreeMap::from([
            ("node_id".to_string(), module.id.into()),
            ("kind".to_string(), vis_kind),
            ("path".to_string(), vis_path.unwrap_or(DataValue::Null)),
        ]);

        db.run_script(
            "?[node_id, kind, path] <- [[$node_id, $kind, $path]] :put visibility",
            vis_params,
            ScriptMutability::Mutable,
        )?;

        // Add submodule relationships

        // Add item relationships
        if let Some(module_items) = module.items() {
            for item_id in module_items {
                let relation_params = BTreeMap::from([
                    ("module_id".to_string(), module.id.into()),
                    ("related_id".to_string(), item_id.to_cozo_uuid()),
                    ("kind".to_string(), DataValue::from("Contains")),
                ]);

                db.run_script(
                "?[module_id, related_id, kind] <- [[$module_id, $related_id, $kind]] :put module_relationships",
                relation_params,
                ScriptMutability::Mutable,
            )?;
            }
        }

        // Add export relationships
        for export_id in &module.exports {
            let relation_params = BTreeMap::from([
                ("module_id".to_string(), module.id.into()),
                ("related_id".to_string(), export_id.into()),
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
#[cfg(not(feature = "type_bearing_ids"))]
fn transform_values(db: &Db<MemStorage>, consts: Vec<ConstNode>) -> Result<(), cozo::Error> {
    for value in consts.into_iter() {
        let (vis_kind, vis_path) = match &value.visibility {
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

        // Insert into values table
        let value_params = BTreeMap::from([
            ("id".to_string(), value.id.into()),
            ("name".to_string(), DataValue::from(value.name.as_str())),
            ("type_id".to_string(), value.type_id.into()),
            ("kind".to_string(), DataValue::from(kind)),
            ("value".to_string(), value_str),
            ("docstring".to_string(), docstring),
        ]);

        db.run_script(
            "?[id, name, type_id, kind, value, docstring] <- [[$id, $name, $type_id, $kind, $value, $docstring]] :put values",
            value_params,
            ScriptMutability::Mutable,
        )?;

        // Insert into visibility table
        let vis_params = BTreeMap::from([
            ("node_id".to_string(), value.id.into()),
            ("kind".to_string(), vis_kind),
            ("path".to_string(), vis_path.unwrap_or(DataValue::Null)),
        ]);

        db.run_script(
            "?[node_id, kind, path] <- [[$node_id, $kind, $path]] :put visibility",
            vis_params,
            ScriptMutability::Mutable,
        )?;
    }

    Ok(())
}

/// Transforms macro nodes into the macros relation
fn transform_macros(db: &Db<MemStorage>, macros: Vec<MacroNode>) -> Result<(), cozo::Error> {
    for macro_node in macros {
        let visibility = match macro_node.visibility {
            VisibilityKind::Public => "Public",
            VisibilityKind::Crate => "Crate",
            VisibilityKind::Restricted(_) => "Restricted",
            VisibilityKind::Inherited => "Inherited",
        };

        let kind = match &macro_node.kind {
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
            ("id".to_string(), macro_node.id.into()),
            (
                "name".to_string(),
                DataValue::from(macro_node.name.as_str()),
            ),
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
fn transform_types(db: &Db<MemStorage>, type_graph: Vec<TypeNode>) -> Result<(), cozo::Error> {
    for type_node in type_graph {
        let kind = match &type_node.kind {
            ploke_core::TypeKind::Named { .. } => "Named",
            ploke_core::TypeKind::Reference { .. } => "Reference",
            ploke_core::TypeKind::Slice { .. } => "Slice",
            ploke_core::TypeKind::Array { .. } => "Array",
            ploke_core::TypeKind::Tuple { .. } => "Tuple",
            ploke_core::TypeKind::Never => "Never",
            ploke_core::TypeKind::Inferred => "Inferred",
            ploke_core::TypeKind::RawPointer { .. } => "RawPointer",
            ploke_core::TypeKind::ImplTrait { .. } => "ImplTrait",
            ploke_core::TypeKind::TraitObject { .. } => "TraitObject",
            ploke_core::TypeKind::Macro { .. } => "Macro",
            ploke_core::TypeKind::Unknown { .. } => "Unknown",
            ploke_core::TypeKind::Function {
                is_unsafe: _,
                is_extern: _,
                abi: _,
            } => "Function",
            ploke_core::TypeKind::Paren { .. } => "Paren",
        };

        // Create a simplified string representation of the type
        let type_str = format!("{:?}", type_node.kind);

        let params = BTreeMap::from([
            ("id".to_string(), type_node.id.into()),
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
                ("type_id".to_string(), type_node.id.into()),
                ("related_index".to_string(), DataValue::from(i as i64)),
                ("related_type_id".to_string(), related_id.into()),
            ]);

            db.run_script(
                "?[type_id, related_index, related_type_id] <- [[$type_id, $related_index, $related_type_id]] :put type_relations",
                relation_params,
                ScriptMutability::Mutable,
            )?;
        }

        // Add type details
        match &type_node.kind {
            ploke_core::TypeKind::Reference {
                lifetime,
                is_mutable,
                ..
            } => {
                let lifetime_value = lifetime
                    .as_ref()
                    .map(|s| DataValue::from(s.as_str()))
                    .unwrap_or(DataValue::Null);

                let details_params = BTreeMap::from([
                    ("type_id".to_string(), type_node.id.into()),
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
            ploke_core::TypeKind::RawPointer { is_mutable, .. } => {
                let details_params = BTreeMap::from([
                    ("type_id".to_string(), type_node.id.into()),
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
            ploke_core::TypeKind::Function {
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
                    ("type_id".to_string(), type_node.id.into()),
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
            ploke_core::TypeKind::TraitObject { dyn_token, .. } => {
                let details_params = BTreeMap::from([
                    ("type_id".to_string(), type_node.id.into()),
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

/// Transforms defined types (structs, enums, etc.) into their respective relations
fn transform_defined_types(
    db: &Db<MemStorage>,
    defined_types: Vec<TypeDefNode>,
) -> Result<(), cozo::Error> {
    for type_def in defined_types.into_iter() {
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
                    ("id".to_string(), struct_node.id.into()),
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
                        ("struct_id".to_string(), struct_node.id.into()),
                        ("field_index".to_string(), DataValue::from(i as i64)),
                        ("field_name".to_string(), field_name),
                        ("type_id".to_string(), field.type_id.into()),
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
                    ("id".to_string(), enum_node.id.into()),
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
                        ("enum_id".to_string(), enum_node.id.into()),
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
                    ("id".to_string(), type_alias.id.into()),
                    (
                        "name".to_string(),
                        DataValue::from(type_alias.name.as_str()),
                    ),
                    ("visibility".to_string(), DataValue::from(visibility)),
                    ("type_id".to_string(), type_alias.type_id.into()),
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
                    ("id".to_string(), union_node.id.into()),
                    (
                        "name".to_string(),
                        DataValue::from(union_node.name.as_str()),
                    ),
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
                        ("struct_id".to_string(), union_node.id.into()),
                        ("field_index".to_string(), DataValue::from(i as i64)),
                        ("field_name".to_string(), field_name),
                        ("type_id".to_string(), field.type_id.into()),
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
#[cfg(not(feature = "type_bearing_ids"))]
fn transform_relations(
    db: &Db<MemStorage>,
    relations: Vec<SyntacticRelation>,
) -> Result<(), cozo::Error> {
    use syn_parser::parser::relations::SyntacticRelation;

    for relation in &code_graph.relations {
        let kind = match relation.kind {
            SyntacticRelation::FunctionParameter => "FunctionParameter",
            SyntacticRelation::FunctionReturn => "FunctionReturn",
            SyntacticRelation::StructField => "StructField",
            SyntacticRelation::EnumVariant => "EnumVariant",
            SyntacticRelation::ImplementsFor => "ImplementsFor",
            SyntacticRelation::ImplementsTrait => "ImplementsTrait",
            SyntacticRelation::Inherits => "Inherits",
            SyntacticRelation::References => "References",
            SyntacticRelation::Contains => "Contains",
            SyntacticRelation::Uses => "Uses",
            SyntacticRelation::ValueType => "ValueType",
            SyntacticRelation::MacroUse => "MacroUse",
            SyntacticRelation::Method => "Method",
            SyntacticRelation::ModuleImports => "ModuleImports",
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
