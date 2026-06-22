//! Edge schema definitions for the Cozo database
//!
//! This module contains schemas for edge relations in the graph, with a two-tiered approach:
//! 1. Syntactic relations - directly representing AST relationships
//! 2. Semantic relations - higher-level logical relationships derived from syntactic ones

use crate::define_schema;

use super::*;
use cozo::{Db, MemStorage};
use itertools::Itertools;
use std::collections::BTreeMap;
use syn_parser::parser::nodes::{
    AnyCallSiteId, CallBodyOwnerId, CallNode, MethodCallReceiver, ToCozoUuid,
};
use syn_parser::parser::relations::{
    CallRelation, CallResolutionKind, CallResolutionStatus, CallSiteRelation, SyntacticRelation,
    TypeRelation,
};
use syn_parser::resolve::Colorize;
use syn_parser::utils::{LogStyle, LogStyleDebug};

define_schema!(SyntacticRelationSchema {
    "syntax_edge",
    source_id: "Uuid",
    target_id: "Uuid",
    relation_kind: "String",
    source_kind: "String",
    target_kind: "String"
});

define_schema!(TypeRelationSchema {
    "type_relation",
    source_id: "Uuid",
    target_id: "Uuid",
    relation_kind: "String",
    source_kind: "String",
    target_kind: "String"
});

define_schema!(CallSiteRelationSchema {
    "call_site_edge",
    source_id: "Uuid",
    target_id: "Uuid",
    relation_kind: "String",
    source_kind: "String",
    target_kind: "String"
});

define_schema!(CallRelationSchema {
    "call_relation",
    source_id: "Uuid",
    target_id: "Uuid",
    relation_kind: "String",
    source_kind: "String",
    target_kind: "String"
});

define_schema!(CallResolutionStatusSchema {
    "call_resolution_status",
    source_id: "Uuid",
    source_kind: "String",
    status_kind: "String",
    resolution_kind: "String?"
});

pub struct CallSiteSchema;

impl CallSiteSchema {
    pub const RELATION: &'static str = "call_site";

    pub fn create_and_insert_schema(db: &Db<MemStorage>) -> Result<(), TransformError> {
        db.run_script(
            r#":create call_site {
                id: Uuid,
                at: Validity =>
                owner_id: Uuid,
                call_kind: String,
                span: [Int; 2],
                cfgs: [String],
                path: [String]?,
                method_name: String?,
                macro_name: String?,
                receiver_kind: String?,
                receiver_path: [String]?,
                arg_count: Int?,
                generic_arg_count: Int?
            }"#,
            BTreeMap::new(),
            cozo::ScriptMutability::Mutable,
        )?;
        Ok(())
    }

    pub fn insert_call_site(
        db: &Db<MemStorage>,
        call_site: &CallNode,
    ) -> Result<(), TransformError> {
        let params = call_site_to_params(call_site);
        db.run_script(
            r#"?[id, at, owner_id, call_kind, span, cfgs, path, method_name, macro_name, receiver_kind, receiver_path, arg_count, generic_arg_count] :=
                id = $id,
                owner_id = $owner_id,
                call_kind = $call_kind,
                span = $span,
                cfgs = $cfgs,
                path = $path,
                method_name = $method_name,
                macro_name = $macro_name,
                receiver_kind = $receiver_kind,
                receiver_path = $receiver_path,
                arg_count = $arg_count,
                generic_arg_count = $generic_arg_count,
                at = 'ASSERT'
            :put call_site { id, at => owner_id, call_kind, span, cfgs, path, method_name, macro_name, receiver_kind, receiver_path, arg_count, generic_arg_count }"#,
            params,
            cozo::ScriptMutability::Mutable,
        )?;
        Ok(())
    }
}

pub struct TypeUseSchema;

impl TypeUseSchema {
    pub const RELATION: &'static str = "type_use";

    pub fn create_and_insert_schema(db: &Db<MemStorage>) -> Result<(), TransformError> {
        db.run_script(
            r#":create type_use {
                id: Uuid,
                at: Validity =>
                owner_id: Uuid,
                root_type_id: Uuid,
                role: String
            }"#,
            BTreeMap::new(),
            cozo::ScriptMutability::Mutable,
        )?;
        create_type_use_coordinate_schema(db)?;
        Ok(())
    }

    pub fn insert_relation(
        db: &Db<MemStorage>,
        id: cozo::DataValue,
        owner_id: cozo::DataValue,
        root_type_id: cozo::DataValue,
        role: &'static str,
    ) -> Result<(), TransformError> {
        let mut params = BTreeMap::new();
        params.insert("id".to_string(), id);
        params.insert("owner_id".to_string(), owner_id);
        params.insert("root_type_id".to_string(), root_type_id);
        params.insert("role".to_string(), cozo::DataValue::from(role));

        db.run_script(
            r#"?[id, at, owner_id, root_type_id, role] :=
                id = $id,
                owner_id = $owner_id,
                root_type_id = $root_type_id,
                role = $role,
                at = 'ASSERT'
            :put type_use { id, at => owner_id, root_type_id, role }"#,
            params,
            cozo::ScriptMutability::Mutable,
        )?;
        Ok(())
    }

    pub fn insert_param_slot(
        db: &Db<MemStorage>,
        type_use_id: cozo::DataValue,
        param_index: usize,
    ) -> Result<(), TransformError> {
        insert_coordinate_relation(
            db,
            "type_use_param_slot",
            &["type_use_id", "at"],
            &[
                ("type_use_id", type_use_id),
                ("param_index", int(param_index)),
            ],
            "param_index",
        )
    }

    pub fn insert_field_slot(
        db: &Db<MemStorage>,
        type_use_id: cozo::DataValue,
        field_index: usize,
    ) -> Result<(), TransformError> {
        insert_coordinate_relation(
            db,
            "type_use_field_slot",
            &["type_use_id", "at"],
            &[
                ("type_use_id", type_use_id),
                ("field_index", int(field_index)),
            ],
            "field_index",
        )
    }

    pub fn insert_trait_super_slot(
        db: &Db<MemStorage>,
        type_use_id: cozo::DataValue,
        supertrait_index: usize,
    ) -> Result<(), TransformError> {
        insert_coordinate_relation(
            db,
            "type_use_trait_super_slot",
            &["type_use_id", "at"],
            &[
                ("type_use_id", type_use_id),
                ("supertrait_index", int(supertrait_index)),
            ],
            "supertrait_index",
        )
    }

    pub fn insert_generic_bound_slot(
        db: &Db<MemStorage>,
        type_use_id: cozo::DataValue,
        generic_param_index: usize,
        bound_index: usize,
    ) -> Result<(), TransformError> {
        insert_coordinate_relation(
            db,
            "type_use_generic_bound_slot",
            &["type_use_id", "at"],
            &[
                ("type_use_id", type_use_id),
                ("generic_param_index", int(generic_param_index)),
                ("bound_index", int(bound_index)),
            ],
            "generic_param_index, bound_index",
        )
    }

    pub fn insert_generic_param_bound_slot(
        db: &Db<MemStorage>,
        type_use_id: cozo::DataValue,
        containing_owner_id: cozo::DataValue,
        generic_param_index: usize,
        bound_index: usize,
    ) -> Result<(), TransformError> {
        insert_coordinate_relation(
            db,
            "type_use_generic_param_bound_slot",
            &["type_use_id", "at"],
            &[
                ("type_use_id", type_use_id),
                ("containing_owner_id", containing_owner_id),
                ("generic_param_index", int(generic_param_index)),
                ("bound_index", int(bound_index)),
            ],
            "containing_owner_id, generic_param_index, bound_index",
        )
    }

    pub fn insert_where_subject_slot(
        db: &Db<MemStorage>,
        type_use_id: cozo::DataValue,
        predicate_index: usize,
    ) -> Result<(), TransformError> {
        insert_coordinate_relation(
            db,
            "type_use_where_subject_slot",
            &["type_use_id", "at"],
            &[
                ("type_use_id", type_use_id),
                ("predicate_index", int(predicate_index)),
            ],
            "predicate_index",
        )
    }

    pub fn insert_where_bound_slot(
        db: &Db<MemStorage>,
        type_use_id: cozo::DataValue,
        predicate_index: usize,
        bound_index: usize,
    ) -> Result<(), TransformError> {
        insert_coordinate_relation(
            db,
            "type_use_where_bound_slot",
            &["type_use_id", "at"],
            &[
                ("type_use_id", type_use_id),
                ("predicate_index", int(predicate_index)),
                ("bound_index", int(bound_index)),
            ],
            "predicate_index, bound_index",
        )
    }

    pub fn insert_where_generic_param_bound_slot(
        db: &Db<MemStorage>,
        type_use_id: cozo::DataValue,
        containing_owner_id: cozo::DataValue,
        predicate_index: usize,
        bound_index: usize,
    ) -> Result<(), TransformError> {
        insert_coordinate_relation(
            db,
            "type_use_where_generic_param_bound_slot",
            &["type_use_id", "at"],
            &[
                ("type_use_id", type_use_id),
                ("containing_owner_id", containing_owner_id),
                ("predicate_index", int(predicate_index)),
                ("bound_index", int(bound_index)),
            ],
            "containing_owner_id, predicate_index, bound_index",
        )
    }

    pub fn insert_associated_type_bound_slot(
        db: &Db<MemStorage>,
        type_use_id: cozo::DataValue,
        associated_type_index: usize,
        associated_type_name: &str,
        bound_index: usize,
    ) -> Result<(), TransformError> {
        insert_coordinate_relation(
            db,
            "type_use_associated_type_bound_slot",
            &["type_use_id", "at"],
            &[
                ("type_use_id", type_use_id),
                ("associated_type_index", int(associated_type_index)),
                (
                    "associated_type_name",
                    cozo::DataValue::from(associated_type_name),
                ),
                ("bound_index", int(bound_index)),
            ],
            "associated_type_index, associated_type_name, bound_index",
        )
    }
}

fn create_type_use_coordinate_schema(db: &Db<MemStorage>) -> Result<(), TransformError> {
    for script in [
        r#":create type_use_param_slot {
            type_use_id: Uuid,
            at: Validity =>
            param_index: Int
        }"#,
        r#":create type_use_field_slot {
            type_use_id: Uuid,
            at: Validity =>
            field_index: Int
        }"#,
        r#":create type_use_trait_super_slot {
            type_use_id: Uuid,
            at: Validity =>
            supertrait_index: Int
        }"#,
        r#":create type_use_generic_bound_slot {
            type_use_id: Uuid,
            at: Validity =>
            generic_param_index: Int,
            bound_index: Int
        }"#,
        r#":create type_use_generic_param_bound_slot {
            type_use_id: Uuid,
            at: Validity =>
            containing_owner_id: Uuid,
            generic_param_index: Int,
            bound_index: Int
        }"#,
        r#":create type_use_where_subject_slot {
            type_use_id: Uuid,
            at: Validity =>
            predicate_index: Int
        }"#,
        r#":create type_use_where_bound_slot {
            type_use_id: Uuid,
            at: Validity =>
            predicate_index: Int,
            bound_index: Int
        }"#,
        r#":create type_use_where_generic_param_bound_slot {
            type_use_id: Uuid,
            at: Validity =>
            containing_owner_id: Uuid,
            predicate_index: Int,
            bound_index: Int
        }"#,
        r#":create type_use_associated_type_bound_slot {
            type_use_id: Uuid,
            at: Validity =>
            associated_type_index: Int,
            associated_type_name: String,
            bound_index: Int
        }"#,
    ] {
        db.run_script(script, BTreeMap::new(), cozo::ScriptMutability::Mutable)?;
    }
    Ok(())
}

fn insert_coordinate_relation(
    db: &Db<MemStorage>,
    relation: &str,
    key_fields: &[&str],
    values: &[(&str, cozo::DataValue)],
    value_fields: &str,
) -> Result<(), TransformError> {
    let mut params = BTreeMap::new();
    let mut assignments = Vec::with_capacity(values.len() + 1);
    for (name, value) in values {
        params.insert((*name).to_string(), value.clone());
        assignments.push(format!("{name} = ${name}"));
    }
    assignments.push("at = 'ASSERT'".to_string());

    let key_fields = key_fields.join(", ");
    let assignments = assignments.join(",\n                ");
    let script = format!(
        r#"?[{key_fields}, {value_fields}] :=
                {assignments}
            :put {relation} {{ {key_fields} => {value_fields} }}"#
    );

    db.run_script(&script, params, cozo::ScriptMutability::Mutable)?;
    Ok(())
}

fn int(value: usize) -> cozo::DataValue {
    cozo::DataValue::from(value as i64)
}

fn optional_int(value: Option<usize>) -> cozo::DataValue {
    value
        .map(|value| cozo::DataValue::from(value as i64))
        .unwrap_or(cozo::DataValue::Null)
}

fn string_list(values: &[String]) -> cozo::DataValue {
    cozo::DataValue::List(
        values
            .iter()
            .map(|value| cozo::DataValue::from(value.as_str()))
            .collect(),
    )
}

fn span_to_cozo(span: (usize, usize)) -> cozo::DataValue {
    cozo::DataValue::List(vec![
        cozo::DataValue::from(span.0 as i64),
        cozo::DataValue::from(span.1 as i64),
    ])
}

fn call_site_id_to_cozo(id: AnyCallSiteId) -> cozo::DataValue {
    match id {
        AnyCallSiteId::Path(id) => id.to_cozo_uuid(),
        AnyCallSiteId::Method(id) => id.to_cozo_uuid(),
        AnyCallSiteId::Dynamic(id) => id.to_cozo_uuid(),
        AnyCallSiteId::Macro(id) => id.to_cozo_uuid(),
    }
}

fn call_body_owner_to_cozo(owner: CallBodyOwnerId) -> cozo::DataValue {
    match owner {
        CallBodyOwnerId::Function(id) => id.into(),
        CallBodyOwnerId::Method(id) => id.into(),
    }
}

fn call_body_owner_kind(owner: CallBodyOwnerId) -> &'static str {
    match owner {
        CallBodyOwnerId::Function(_) => "Function",
        CallBodyOwnerId::Method(_) => "Method",
    }
}

fn call_site_kind(id: AnyCallSiteId) -> &'static str {
    match id {
        AnyCallSiteId::Path(_) => "Path",
        AnyCallSiteId::Method(_) => "Method",
        AnyCallSiteId::Dynamic(_) => "Dynamic",
        AnyCallSiteId::Macro(_) => "Macro",
    }
}

fn call_resolution_kind_str(kind: CallResolutionKind) -> &'static str {
    match kind {
        CallResolutionKind::LocalExact => "LocalExact",
    }
}

fn method_receiver_to_cozo(receiver: &MethodCallReceiver) -> (cozo::DataValue, cozo::DataValue) {
    match receiver {
        MethodCallReceiver::SelfValue => {
            (cozo::DataValue::from("SelfValue"), cozo::DataValue::Null)
        }
        MethodCallReceiver::SelfField { field_path } => {
            (cozo::DataValue::from("SelfField"), string_list(field_path))
        }
    }
}

fn call_site_to_params(call_site: &CallNode) -> BTreeMap<String, cozo::DataValue> {
    let mut params = BTreeMap::new();
    params.insert("id".to_string(), call_site_id_to_cozo(call_site.id()));
    params.insert(
        "owner_id".to_string(),
        call_body_owner_to_cozo(call_site.owner()),
    );
    params.insert(
        "call_kind".to_string(),
        cozo::DataValue::from(call_site_kind(call_site.id())),
    );
    params.insert("span".to_string(), span_to_cozo(call_site.span()));
    params.insert("cfgs".to_string(), string_list(call_site.cfgs()));

    match call_site {
        CallNode::PathCall(call) => {
            params.insert("path".to_string(), string_list(&call.path));
            params.insert("method_name".to_string(), cozo::DataValue::Null);
            params.insert("macro_name".to_string(), cozo::DataValue::Null);
            params.insert("receiver_kind".to_string(), cozo::DataValue::Null);
            params.insert("receiver_path".to_string(), cozo::DataValue::Null);
            params.insert("arg_count".to_string(), optional_int(Some(call.arg_count)));
            params.insert(
                "generic_arg_count".to_string(),
                optional_int(Some(call.generic_arg_count)),
            );
        }
        CallNode::MethodCall(call) => {
            let (receiver_kind, receiver_path) = method_receiver_to_cozo(&call.receiver);
            params.insert("path".to_string(), cozo::DataValue::Null);
            params.insert(
                "method_name".to_string(),
                cozo::DataValue::from(call.method_name.as_str()),
            );
            params.insert("macro_name".to_string(), cozo::DataValue::Null);
            params.insert("receiver_kind".to_string(), receiver_kind);
            params.insert("receiver_path".to_string(), receiver_path);
            params.insert("arg_count".to_string(), optional_int(Some(call.arg_count)));
            params.insert(
                "generic_arg_count".to_string(),
                optional_int(Some(call.generic_arg_count)),
            );
        }
        CallNode::DynamicCall(call) => {
            params.insert("path".to_string(), cozo::DataValue::Null);
            params.insert("method_name".to_string(), cozo::DataValue::Null);
            params.insert("macro_name".to_string(), cozo::DataValue::Null);
            params.insert("receiver_kind".to_string(), cozo::DataValue::Null);
            params.insert("receiver_path".to_string(), cozo::DataValue::Null);
            params.insert("arg_count".to_string(), optional_int(Some(call.arg_count)));
            params.insert("generic_arg_count".to_string(), cozo::DataValue::Null);
        }
        CallNode::MacroCall(call) => {
            params.insert("path".to_string(), cozo::DataValue::Null);
            params.insert("method_name".to_string(), cozo::DataValue::Null);
            params.insert(
                "macro_name".to_string(),
                cozo::DataValue::from(call.macro_name.as_str()),
            );
            params.insert("receiver_kind".to_string(), cozo::DataValue::Null);
            params.insert("receiver_path".to_string(), cozo::DataValue::Null);
            params.insert("arg_count".to_string(), cozo::DataValue::Null);
            params.insert("generic_arg_count".to_string(), cozo::DataValue::Null);
        }
    }

    params
}

pub struct TypeContainsSchema;

impl TypeContainsSchema {
    pub const RELATION: &'static str = "type_contains";

    pub fn create_and_insert_schema(db: &Db<MemStorage>) -> Result<(), TransformError> {
        db.run_script(
            r#":create type_contains {
                parent_type_id: Uuid,
                child_type_id: Uuid,
                kind: String,
                position: Int,
                at: Validity
            }"#,
            BTreeMap::new(),
            cozo::ScriptMutability::Mutable,
        )?;
        Ok(())
    }

    pub fn insert_relation(
        db: &Db<MemStorage>,
        parent_type_id: cozo::DataValue,
        child_type_id: cozo::DataValue,
        kind: &'static str,
        position: Option<usize>,
    ) -> Result<(), TransformError> {
        let mut params = BTreeMap::new();
        params.insert("parent_type_id".to_string(), parent_type_id);
        params.insert("child_type_id".to_string(), child_type_id);
        params.insert("kind".to_string(), cozo::DataValue::from(kind));
        params.insert(
            "position".to_string(),
            cozo::DataValue::from(position.map_or(-1_i64, |idx| idx as i64)),
        );

        db.run_script(
            r#"?[parent_type_id, child_type_id, kind, position, at] :=
                parent_type_id = $parent_type_id,
                child_type_id = $child_type_id,
                kind = $kind,
                position = $position,
                at = 'ASSERT'
            :put type_contains { parent_type_id, child_type_id, kind, position, at }"#,
            params,
            cozo::ScriptMutability::Mutable,
        )?;
        Ok(())
    }
}

impl SyntacticRelationSchema {
    /// Transforms a SyntacticRelation into parameters for database insertion
    pub fn relation_to_params(
        &self,
        relation: &SyntacticRelation,
    ) -> BTreeMap<String, cozo::DataValue> {
        let source_id = relation.source().to_cozo_uuid();
        let target_id = relation.target().to_cozo_uuid();
        let relation_kind = relation.kind_str();
        let schema = &SyntacticRelationSchema::SCHEMA;

        // ANCHOR: impl_trait_associated_edges
        // TODO(import-backlinks): Keep import-bearing relation kinds explicit here. `ModuleImports`,
        // `ReExports`, and `ImportedBy` now participate in the real graph contract, so if relation
        // kind folding or endpoint typing changes, verify the `syntax_edge` surface still preserves
        // import semantics cleanly for downstream queries/importers.
        // Extract the node type names from the relation variant
        let (source_kind, target_kind) = match relation {
            SyntacticRelation::Contains { .. } => ("Module", "Primary"),
            // NOTE: ResolvesToDefinition needs work, this ModuleNode->ModuleNode is ambiguous.
            SyntacticRelation::ResolvesToDefinition { .. } => ("Module", "Module"),
            SyntacticRelation::CustomPath { .. } => ("Module", "Module"),
            SyntacticRelation::Sibling { .. } => ("Module", "Module"),
            SyntacticRelation::ModuleImports { .. } => ("Module", "Import"),
            SyntacticRelation::ReExports { .. } => ("Import", "Primary"),
            SyntacticRelation::StructField { .. } => ("Struct", "Field"),
            SyntacticRelation::UnionField { .. } => ("Union", "Field"),
            SyntacticRelation::VariantField { .. } => ("Variant", "Field"),
            SyntacticRelation::EnumVariant { .. } => ("Enum", "Variant"),
            SyntacticRelation::ImplAssociatedItem { .. } => ("Impl", "AssociatedItem"),
            SyntacticRelation::TraitAssociatedItem { .. } => ("Trait", "AssociatedItem"),
            SyntacticRelation::ImportedBy { .. } => ("Primary", "Import"),
        };
        // ANCHOR_END: impl_trait_associated_edges

        BTreeMap::from([
            (schema.source_id().to_string(), source_id),
            (schema.target_id().to_string(), target_id),
            (
                schema.relation_kind().to_string(),
                cozo::DataValue::from(relation_kind),
            ),
            (
                schema.source_kind().to_string(),
                cozo::DataValue::from(source_kind),
            ),
            (
                schema.target_kind().to_string(),
                cozo::DataValue::from(target_kind),
            ),
        ])
    }

    /// Inserts a syntactic relation into the database
    pub fn insert_relation(
        &self,
        db: &Db<MemStorage>,
        relation: &SyntacticRelation,
    ) -> Result<(), TransformError> {
        let params = self.relation_to_params(relation);
        let script = &self.script_put(&params);
        db.run_script(script, params, cozo::ScriptMutability::Mutable)
            .inspect_err(|&_| {
                tracing::error!(target: "db", "{} {}\n{} {}",
                    "Error:".log_error().bold(),
                    format_args!("running script {}", script.log_orange()),
                    "type_node info:".log_foreground_primary_debug(),
                    format!("{:#?}", relation).log_orange()
                );
            })?;
        Ok(())
    }
}

impl TypeRelationSchema {
    pub fn insert_relation(
        &self,
        db: &Db<MemStorage>,
        relation: &TypeRelation,
    ) -> Result<(), TransformError> {
        let schema = &TypeRelationSchema::SCHEMA;
        let params = match relation {
            TypeRelation::Ordinary { source, target } => BTreeMap::from([
                (schema.source_id().to_string(), source.to_cozo_uuid()),
                (
                    schema.target_id().to_string(),
                    syn_parser::parser::nodes::AnyNodeId::from(*target).to_cozo_uuid(),
                ),
                (
                    schema.relation_kind().to_string(),
                    cozo::DataValue::from(relation.kind_str()),
                ),
                (
                    schema.source_kind().to_string(),
                    cozo::DataValue::from(source.kind_name()),
                ),
                (
                    schema.target_kind().to_string(),
                    cozo::DataValue::from(format!("{:?}", target.kind())),
                ),
            ]),
            TypeRelation::Trait { source, target } => BTreeMap::from([
                (schema.source_id().to_string(), source.to_cozo_uuid()),
                (
                    schema.target_id().to_string(),
                    syn_parser::parser::nodes::AnyNodeId::from(*target).to_cozo_uuid(),
                ),
                (
                    schema.relation_kind().to_string(),
                    cozo::DataValue::from(relation.kind_str()),
                ),
                (
                    schema.source_kind().to_string(),
                    cozo::DataValue::from(source.kind_name()),
                ),
                (
                    schema.target_kind().to_string(),
                    cozo::DataValue::from(format!("{:?}", target.kind())),
                ),
            ]),
        };

        let script = schema.script_put(&params);
        db.run_script(&script, params, cozo::ScriptMutability::Mutable)?;

        Ok(())
    }
}

impl CallSiteRelationSchema {
    pub fn insert_relation(
        &self,
        db: &Db<MemStorage>,
        relation: &CallSiteRelation,
    ) -> Result<(), TransformError> {
        let schema = &CallSiteRelationSchema::SCHEMA;
        let params = match relation {
            CallSiteRelation::BodyContainsCall { source, target } => BTreeMap::from([
                (
                    schema.source_id().to_string(),
                    call_body_owner_to_cozo(*source),
                ),
                (
                    schema.target_id().to_string(),
                    call_site_id_to_cozo(*target),
                ),
                (
                    schema.relation_kind().to_string(),
                    cozo::DataValue::from(relation.kind_str()),
                ),
                (
                    schema.source_kind().to_string(),
                    cozo::DataValue::from(call_body_owner_kind(*source)),
                ),
                (
                    schema.target_kind().to_string(),
                    cozo::DataValue::from(call_site_kind(*target)),
                ),
            ]),
        };

        let script = schema.script_put(&params);
        db.run_script(&script, params, cozo::ScriptMutability::Mutable)?;
        Ok(())
    }
}

impl CallRelationSchema {
    pub fn insert_relation(
        &self,
        db: &Db<MemStorage>,
        relation: &CallRelation,
    ) -> Result<(), TransformError> {
        let schema = &CallRelationSchema::SCHEMA;
        let params = match relation {
            CallRelation::Function { source, target } => {
                let target_id: cozo::DataValue = (*target).into();
                BTreeMap::from([
                    (schema.source_id().to_string(), source.to_cozo_uuid()),
                    (schema.target_id().to_string(), target_id),
                    (
                        schema.relation_kind().to_string(),
                        cozo::DataValue::from(relation.kind_str()),
                    ),
                    (
                        schema.source_kind().to_string(),
                        cozo::DataValue::from("Path"),
                    ),
                    (
                        schema.target_kind().to_string(),
                        cozo::DataValue::from("Function"),
                    ),
                ])
            }
            CallRelation::Method { source, target } => {
                let target_id: cozo::DataValue = (*target).into();
                BTreeMap::from([
                    (schema.source_id().to_string(), source.to_cozo_uuid()),
                    (schema.target_id().to_string(), target_id),
                    (
                        schema.relation_kind().to_string(),
                        cozo::DataValue::from(relation.kind_str()),
                    ),
                    (
                        schema.source_kind().to_string(),
                        cozo::DataValue::from("Method"),
                    ),
                    (
                        schema.target_kind().to_string(),
                        cozo::DataValue::from("Method"),
                    ),
                ])
            }
        };

        let script = schema.script_put(&params);
        db.run_script(&script, params, cozo::ScriptMutability::Mutable)?;
        Ok(())
    }
}

impl CallResolutionStatusSchema {
    pub fn insert_status(
        &self,
        db: &Db<MemStorage>,
        status: &CallResolutionStatus,
    ) -> Result<(), TransformError> {
        let schema = &CallResolutionStatusSchema::SCHEMA;
        let resolution_kind = match status {
            CallResolutionStatus::Resolved { kind, .. } => {
                cozo::DataValue::from(call_resolution_kind_str(*kind))
            }
            CallResolutionStatus::Unresolved { .. }
            | CallResolutionStatus::Ambiguous { .. }
            | CallResolutionStatus::External { .. }
            | CallResolutionStatus::Unsupported { .. } => cozo::DataValue::Null,
        };
        let source = status.source();
        let params = BTreeMap::from([
            (schema.source_id().to_string(), call_site_id_to_cozo(source)),
            (
                schema.source_kind().to_string(),
                cozo::DataValue::from(call_site_kind(source)),
            ),
            (
                schema.status_kind().to_string(),
                cozo::DataValue::from(status.kind_str()),
            ),
            (schema.resolution_kind().to_string(), resolution_kind),
        ]);

        let script = schema.script_put(&params);
        db.run_script(&script, params, cozo::ScriptMutability::Mutable)?;
        Ok(())
    }
}

#[cfg(feature = "level_up_relations")]
impl SemanticRelationSchema {
    /// Derives a semantic "contains" relation from syntactic relations
    pub fn derive_contains_relation<T, S>(
        &self,
        db: &Db<MemStorage>,
        source_id: T,
        target_id: S,
        // TODO: Consider seriously whether each edge should have its own hashed ID. This seems
        // wasteful since we will likely have an index for the edges generated by `cozo` as well,
        // but we would still like a way to refer to the edges more directly outside of cozo.
        // syntactic_ids: Vec<SyntacticRelationId>,
        //
        // This could work... as long as we had a strict rule that only one kind of relation
        // could exist between source->target. Not sure whether that is a good idea, or even
        // feasable.
        // syntactic_ids: Vec<(AnyNodeId, AnyNodeId)>,

        // This seems like it both makes the most sense but still needs to have some analog in
        // `cozo` that makes sense.
        // Need to think this over.
        syntactic_ids: Vec<SyntacticRelation>,
    ) -> Result<(), TransformError>
    where
        T: AsAnyNodeId + TypedId,
        S: AsAnyNodeId + TypedId,
    {
        let params = BTreeMap::from([
            (
                "source_id".to_string(),
                cozo::DataValue::Uuid(cozo::UuidWrapper(source_id)),
            ),
            (
                "target_id".to_string(),
                cozo::DataValue::Uuid(cozo::UuidWrapper(target_id)),
            ),
            (
                "relation_kind".to_string(),
                cozo::DataValue::from("Contains"),
            ),
            (
                "derived_from".to_string(),
                cozo::DataValue::List(
                    syntactic_ids
                        .into_iter()
                        .map(|id| cozo::DataValue::Uuid(cozo::UuidWrapper(id)))
                        .collect(),
                ),
            ),
        ]);

        db.run_script(
            &self.script_put(&params),
            params,
            cozo::ScriptMutability::Mutable,
        )?;
        Ok(())
    }

    /// Derives a semantic "defines" relation from syntactic relations
    pub fn derive_defines_relation(
        &self,
        db: &Db<MemStorage>,
        source_id: uuid::Uuid,
        target_id: uuid::Uuid,
        syntactic_ids: Vec<uuid::Uuid>,
    ) -> Result<(), TransformError> {
        let params = BTreeMap::from([
            (
                "source_id".to_string(),
                cozo::DataValue::Uuid(cozo::UuidWrapper(source_id)),
            ),
            (
                "target_id".to_string(),
                cozo::DataValue::Uuid(cozo::UuidWrapper(target_id)),
            ),
            (
                "relation_kind".to_string(),
                cozo::DataValue::from("Defines"),
            ),
            (
                "derived_from".to_string(),
                cozo::DataValue::List(
                    syntactic_ids
                        .into_iter()
                        .map(|id| cozo::DataValue::Uuid(cozo::UuidWrapper(id)))
                        .collect(),
                ),
            ),
        ]);

        db.run_script(
            &self.script_put(&params),
            params,
            cozo::ScriptMutability::Mutable,
        )?;
        Ok(())
    }

    /// Derives a semantic "uses" relation from syntactic relations
    pub fn derive_uses_relation(
        &self,
        db: &Db<MemStorage>,
        source_id: uuid::Uuid,
        target_id: uuid::Uuid,
        syntactic_ids: Vec<uuid::Uuid>,
    ) -> Result<(), TransformError> {
        let params = BTreeMap::from([
            (
                "source_id".to_string(),
                cozo::DataValue::Uuid(cozo::UuidWrapper(source_id)),
            ),
            (
                "target_id".to_string(),
                cozo::DataValue::Uuid(cozo::UuidWrapper(target_id)),
            ),
            ("relation_kind".to_string(), cozo::DataValue::from("Uses")),
            (
                "derived_from".to_string(),
                cozo::DataValue::List(
                    syntactic_ids
                        .into_iter()
                        .map(|id| cozo::DataValue::Uuid(cozo::UuidWrapper(id)))
                        .collect(),
                ),
            ),
        ]);

        db.run_script(
            &self.script_put(&params),
            params,
            cozo::ScriptMutability::Mutable,
        )?;
        Ok(())
    }
}
