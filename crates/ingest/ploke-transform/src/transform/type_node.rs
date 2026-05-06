#[cfg(not(feature = "typed_type_graph"))]
use ploke_core::TypeKind;
#[cfg(not(feature = "typed_type_graph"))]
use syn_parser::{resolve::Colorize, utils::LogStyleDebug};
use tracing::instrument;

use crate::schema::types::*;

use super::*;

#[cfg(feature = "typed_type_graph")]
#[instrument(skip_all)]
pub(super) fn transform_types(
    db: &Db<MemStorage>,
    type_nodes: Vec<TypeNode>,
) -> Result<(), TransformError> {
    for type_node in type_nodes {
        let (script, params) = match &type_node {
            TypeNode::Named(node) => {
                let schema = NamedTypeSchema::SCHEMA;
                let cozo_path = DataValue::List(
                    node.path
                        .iter()
                        .map(|segment| DataValue::Str(segment.into()))
                        .collect(),
                );
                let params = BTreeMap::from([
                    (schema.type_id().to_string(), type_node.id().to_cozo_uuid()),
                    (schema.path().to_string(), cozo_path),
                    (
                        schema.is_fully_qualified().to_string(),
                        DataValue::Bool(node.is_fully_qualified),
                    ),
                ]);
                (schema.script_put(&params), params)
            }
            TypeNode::Reference(node) => {
                let schema = ReferenceTypeSchema::SCHEMA;
                let cozo_lifetime = node
                    .lifetime
                    .as_ref()
                    .map(|lifetime| DataValue::Str(lifetime.into()))
                    .unwrap_or(DataValue::Null);
                let params = BTreeMap::from([
                    (schema.type_id().to_string(), type_node.id().to_cozo_uuid()),
                    (schema.lifetime().to_string(), cozo_lifetime),
                    (
                        schema.is_mutable().to_string(),
                        DataValue::Bool(node.is_mutable),
                    ),
                    (
                        schema.references_type().to_string(),
                        node.referenced.to_cozo_uuid(),
                    ),
                ]);
                (schema.script_put(&params), params)
            }
            TypeNode::Slice(node) => {
                let schema = SliceTypeSchema::SCHEMA;
                let params = BTreeMap::from([
                    (schema.type_id().to_string(), type_node.id().to_cozo_uuid()),
                    (
                        schema.element_type().to_string(),
                        node.element.to_cozo_uuid(),
                    ),
                ]);
                (schema.script_put(&params), params)
            }
            TypeNode::Array(node) => {
                let schema = ArrayTypeSchema::SCHEMA;
                let cozo_size = node.size.as_ref().map_or(DataValue::Null, |size| {
                    size.parse::<i64>()
                        .map_or(DataValue::Null, |parsed| DataValue::Num(Num::Int(parsed)))
                });
                let params = BTreeMap::from([
                    (schema.type_id().to_string(), type_node.id().to_cozo_uuid()),
                    (
                        schema.element_type().to_string(),
                        node.element.to_cozo_uuid(),
                    ),
                    (schema.size().to_string(), cozo_size),
                ]);
                (schema.script_put(&params), params)
            }
            TypeNode::Tuple(node) => {
                let schema = TupleTypeSchema::SCHEMA;
                let params = BTreeMap::from([
                    (schema.type_id().to_string(), type_node.id().to_cozo_uuid()),
                    (
                        schema.element_types().to_string(),
                        cozo_type_list(node.elements.iter().copied()),
                    ),
                ]);
                (schema.script_put(&params), params)
            }
            TypeNode::Function(node) => {
                let schema = FunctionTypeSchema::SCHEMA;
                let cozo_abi = node
                    .abi
                    .as_ref()
                    .map(|abi| DataValue::Str(abi.into()))
                    .unwrap_or(DataValue::Null);
                let params = BTreeMap::from([
                    (schema.type_id().to_string(), type_node.id().to_cozo_uuid()),
                    (
                        schema.is_unsafe().to_string(),
                        DataValue::Bool(node.is_unsafe),
                    ),
                    (
                        schema.is_extern().to_string(),
                        DataValue::Bool(node.is_extern),
                    ),
                    (schema.abi().to_string(), cozo_abi),
                ]);
                (schema.script_put(&params), params)
            }
            TypeNode::Never(_) => {
                let schema = NeverTypeSchema::SCHEMA;
                let params =
                    BTreeMap::from([(schema.type_id().to_string(), type_node.id().to_cozo_uuid())]);
                (schema.script_put(&params), params)
            }
            TypeNode::Inferred(_) => {
                let schema = InferredTypeSchema::SCHEMA;
                let params =
                    BTreeMap::from([(schema.type_id().to_string(), type_node.id().to_cozo_uuid())]);
                (schema.script_put(&params), params)
            }
            TypeNode::RawPointer(node) => {
                let schema = RawPointerTypeSchema::SCHEMA;
                let params = BTreeMap::from([
                    (schema.type_id().to_string(), type_node.id().to_cozo_uuid()),
                    (
                        schema.is_mutable().to_string(),
                        DataValue::Bool(node.is_mutable),
                    ),
                    (schema.points_to().to_string(), node.pointee.to_cozo_uuid()),
                ]);
                (schema.script_put(&params), params)
            }
            TypeNode::TraitObject(node) => {
                let schema = TraitObjectTypeSchema::SCHEMA;
                let params = BTreeMap::from([
                    (schema.type_id().to_string(), type_node.id().to_cozo_uuid()),
                    (
                        schema.dyn_token().to_string(),
                        DataValue::Bool(node.dyn_token),
                    ),
                    (
                        schema.trait_bounds().to_string(),
                        cozo_type_list(node.bounds.iter().copied()),
                    ),
                ]);
                (schema.script_put(&params), params)
            }
            TypeNode::ImplTrait(node) => {
                let schema = ImplTraitTypeSchema::SCHEMA;
                let params = BTreeMap::from([
                    (schema.type_id().to_string(), type_node.id().to_cozo_uuid()),
                    (
                        schema.trait_bounds().to_string(),
                        cozo_type_list(node.bounds.iter().copied()),
                    ),
                ]);
                (schema.script_put(&params), params)
            }
            TypeNode::TraitBound(node) => {
                let schema = TraitBoundTypeSchema::SCHEMA;
                let cozo_path = DataValue::List(
                    node.path
                        .iter()
                        .map(|segment| DataValue::Str(segment.into()))
                        .collect(),
                );
                let params = BTreeMap::from([
                    (schema.type_id().to_string(), type_node.id().to_cozo_uuid()),
                    (schema.path().to_string(), cozo_path),
                    (
                        schema.is_fully_qualified().to_string(),
                        DataValue::Bool(node.is_fully_qualified),
                    ),
                    (
                        schema.related_types().to_string(),
                        cozo_type_list(node.arguments.iter().copied()),
                    ),
                ]);
                (schema.script_put(&params), params)
            }
            TypeNode::Paren(node) => {
                let schema = ParenTypeSchema::SCHEMA;
                let params = BTreeMap::from([
                    (schema.type_id().to_string(), type_node.id().to_cozo_uuid()),
                    (schema.inner_type().to_string(), node.inner.to_cozo_uuid()),
                ]);
                (schema.script_put(&params), params)
            }
            TypeNode::Macro(node) => {
                let schema = MacroTypeSchema::SCHEMA;
                let params = BTreeMap::from([
                    (schema.type_id().to_string(), type_node.id().to_cozo_uuid()),
                    (
                        schema.name().to_string(),
                        DataValue::Str((&node.name).into()),
                    ),
                    (
                        schema.tokens().to_string(),
                        DataValue::Str((&node.tokens).into()),
                    ),
                ]);
                (schema.script_put(&params), params)
            }
            TypeNode::Unknown(node) => {
                let schema = UnknownTypeSchema::SCHEMA;
                let params = BTreeMap::from([
                    (schema.type_id().to_string(), type_node.id().to_cozo_uuid()),
                    (
                        schema.type_str().to_string(),
                        DataValue::Str((&node.type_str).into()),
                    ),
                ]);
                (schema.script_put(&params), params)
            }
        };

        db.run_script(&script, params, ScriptMutability::Mutable)?;
    }

    Ok(())
}

#[cfg(not(feature = "typed_type_graph"))]
#[instrument(skip_all)]
pub(super) fn transform_types(
    db: &Db<MemStorage>,
    type_nodes: Vec<TypeNode>,
) -> Result<(), TransformError> {
    fn process_trait_bounds(type_node: &TypeNode) -> DataValue {
        let cozo_trait_bounds = DataValue::List(
            type_node
                .related_types
                .iter()
                .map(|t| t.to_cozo_uuid())
                .collect(),
        );
        cozo_trait_bounds
    }

    for type_node in type_nodes {
        let cozo_type_id = type_node.id.to_cozo_uuid();
        let (script, params) = match &type_node.kind {
            TypeKind::Named {
                path,
                is_fully_qualified,
            } => {
                let schema = NamedTypeSchema::SCHEMA;
                let cozo_path =
                    DataValue::List(path.iter().map(|s| DataValue::Str(s.into())).collect());

                let params = BTreeMap::from([
                    (schema.type_id().to_string(), cozo_type_id),
                    (schema.path().to_string(), cozo_path),
                    (
                        schema.is_fully_qualified().to_string(),
                        DataValue::Bool(*is_fully_qualified),
                    ),
                ]);
                let script = schema.script_put(&params);

                (script, params)
            }
            TypeKind::Reference {
                lifetime,
                is_mutable,
            } => {
                let schema = ReferenceTypeSchema::SCHEMA;
                let cozo_lifetime = lifetime.as_ref().map(|s| DataValue::Str(s.into()));
                let mut cozo_refs_types = type_node.related_types.iter().map(|t| t.to_cozo_uuid());
                let cozo_refs_type = cozo_refs_types.next().unwrap_or(DataValue::Null);
                let _ = cozo_refs_types.next().is_none_or(|_| {
                    // TODO: Better error handling
                    panic!("Invariant Violated: More than one value found inside parentheses.")
                });
                let params = BTreeMap::from([
                    (schema.type_id().to_string(), cozo_type_id),
                    (
                        schema.lifetime().to_string(),
                        cozo_lifetime.unwrap_or(DataValue::Null),
                    ),
                    (
                        schema.is_mutable().to_string(),
                        DataValue::Bool(*is_mutable),
                    ),
                    (schema.references_type().to_string(), cozo_refs_type),
                ]);
                let script = schema.script_put(&params);

                (script, params)
            }
            TypeKind::Slice {} => {
                let schema = SliceTypeSchema::SCHEMA;
                let cozo_element_type = process_element_type(&type_node);
                let params = BTreeMap::from([
                    (schema.type_id().to_string(), cozo_type_id),
                    (schema.element_type().to_string(), cozo_element_type),
                ]);
                let script = schema.script_put(&params);

                (script, params)
            }
            TypeKind::Array { size } => {
                let schema = ArrayTypeSchema::SCHEMA;
                let cozo_size = size.as_ref().map_or(DataValue::Null, |s| {
                    s.parse::<i64>()
                        .map_or(DataValue::Null, |i| DataValue::Num(Num::Int(i)))
                }); // Size may or may not exist.
                let cozo_element_type = process_element_type(&type_node);
                let params = BTreeMap::from([
                    (schema.type_id().to_string(), cozo_type_id),
                    (schema.size().to_string(), cozo_size),
                    (schema.element_type().to_string(), cozo_element_type),
                ]);
                let script = schema.script_put(&params);

                (script, params)
            }
            TypeKind::Tuple {} => {
                let schema = TupleTypeSchema::SCHEMA;
                let cozo_element_types = DataValue::List(
                    type_node
                        .related_types
                        .iter()
                        .map(|t| t.to_cozo_uuid())
                        .collect(),
                );
                let params = BTreeMap::from([
                    (schema.type_id().to_string(), cozo_type_id),
                    (schema.element_types().to_string(), cozo_element_types),
                ]);
                let script = schema.script_put(&params);

                (script, params)
            }
            TypeKind::Function {
                is_unsafe,
                is_extern,
                abi,
            } => {
                let schema = FunctionTypeSchema::SCHEMA;
                let cozo_abi = abi.as_ref().map(|s| DataValue::Str(s.into()));
                let params = BTreeMap::from([
                    (schema.type_id().to_string(), cozo_type_id),
                    (schema.is_unsafe().to_string(), DataValue::Bool(*is_unsafe)),
                    (schema.is_extern().to_string(), DataValue::Bool(*is_extern)),
                    (
                        schema.abi().to_string(),
                        cozo_abi.unwrap_or(DataValue::Null),
                    ),
                ]);
                let script = schema.script_put(&params);

                (script, params)
            }
            TypeKind::Never => {
                let schema = NeverTypeSchema::SCHEMA;
                let params = BTreeMap::from([(schema.type_id().to_string(), cozo_type_id)]);
                let script = schema.script_put(&params);

                (script, params)
            }
            TypeKind::Inferred => {
                let schema = InferredTypeSchema::SCHEMA;
                let params = BTreeMap::from([(schema.type_id().to_string(), cozo_type_id)]);
                let script = schema.script_put(&params);

                (script, params)
            }
            TypeKind::RawPointer { is_mutable } => {
                let schema = RawPointerTypeSchema::SCHEMA;
                let cozo_points_to = process_element_type(&type_node);
                let params = BTreeMap::from([
                    (schema.type_id().to_string(), cozo_type_id),
                    (
                        schema.is_mutable().to_string(),
                        DataValue::Bool(*is_mutable),
                    ),
                    (schema.points_to().to_string(), cozo_points_to),
                ]);
                let script = schema.script_put(&params);

                (script, params)
            }
            TypeKind::TraitObject { dyn_token } => {
                let schema = TraitObjectTypeSchema::SCHEMA;
                let cozo_trait_bounds = process_trait_bounds(&type_node);
                let params = BTreeMap::from([
                    (schema.type_id().to_string(), cozo_type_id),
                    (schema.dyn_token().to_string(), DataValue::Bool(*dyn_token)),
                    (schema.trait_bounds().to_string(), cozo_trait_bounds),
                ]);
                let script = schema.script_put(&params);

                (script, params)
            }
            TypeKind::ImplTrait {} => {
                let schema = ImplTraitTypeSchema::SCHEMA;
                let cozo_trait_bounds = process_trait_bounds(&type_node);
                let params = BTreeMap::from([
                    (schema.type_id().to_string(), cozo_type_id),
                    (schema.trait_bounds().to_string(), cozo_trait_bounds),
                ]);
                let script = schema.script_put(&params);

                (script, params)
            }
            TypeKind::TraitBound {
                path,
                is_fully_qualified,
            } => {
                let schema = TraitBoundTypeSchema::SCHEMA;
                let cozo_path =
                    DataValue::List(path.iter().map(|s| DataValue::Str(s.into())).collect());
                let cozo_related_types = DataValue::List(
                    type_node
                        .related_types
                        .iter()
                        .map(|t| t.to_cozo_uuid())
                        .collect(),
                );
                let params = BTreeMap::from([
                    (schema.type_id().to_string(), cozo_type_id),
                    (schema.path().to_string(), cozo_path),
                    (
                        schema.is_fully_qualified().to_string(),
                        DataValue::Bool(*is_fully_qualified),
                    ),
                    (schema.related_types().to_string(), cozo_related_types),
                ]);
                let script = schema.script_put(&params);

                (script, params)
            }
            TypeKind::Paren {} => {
                let schema = ParenTypeSchema::SCHEMA;
                let mut inner_types = type_node.related_types.iter().map(|t| t.to_cozo_uuid());
                let inner_type = inner_types.next().unwrap_or(DataValue::Null);
                let _ = inner_types.next().is_none_or(|_| {
                    // TODO: Better error handling
                    panic!("Invariant Violated: More than one value found inside parentheses.")
                });
                let params = BTreeMap::from([
                    (schema.type_id().to_string(), cozo_type_id),
                    (schema.inner_type().to_string(), inner_type),
                ]);
                let script = schema.script_put(&params);

                (script, params)
            }
            TypeKind::Macro { name, tokens } => {
                let schema = MacroTypeSchema::SCHEMA;
                let params = BTreeMap::from([
                    (schema.type_id().to_string(), cozo_type_id),
                    (schema.name().to_string(), DataValue::Str(name.into())),
                    (schema.tokens().to_string(), DataValue::Str(tokens.into())),
                ]);
                let script = schema.script_put(&params);

                (script, params)
            }
            TypeKind::Unknown { type_str } => {
                let schema = UnknownTypeSchema::SCHEMA;
                let params = BTreeMap::from([
                    (schema.type_id().to_string(), cozo_type_id),
                    (
                        schema.type_str().to_string(),
                        DataValue::Str(type_str.into()),
                    ),
                ]);
                let script = schema.script_put(&params);

                (script, params)
            }
        };

        db.run_script(&script, params, ScriptMutability::Mutable)
            .inspect_err(|&_| {
                tracing::error!(target: "db", "{} {}\n{} {}",
                    "Error:".log_error().bold(),
                    format_args!("running script {}", &script.log_path()),
                    "type_node info:".log_foreground_primary_debug(),
                    format!("{:#?}", type_node ).log_orange()
                );
            })?;
    }
    Ok(())
}

#[cfg(not(feature = "typed_type_graph"))]
fn process_element_type(type_node: &TypeNode) -> DataValue {
    let cozo_element_type = type_node
        .related_types
        .first()
        .expect("Invariant Violated: All slices must have an inner type.")
        .to_cozo_uuid();
    cozo_element_type
}

#[cfg(feature = "typed_type_graph")]
fn cozo_type_list<T>(ids: impl IntoIterator<Item = T>) -> DataValue
where
    T: ToCozoUuid,
{
    DataValue::List(ids.into_iter().map(ToCozoUuid::to_cozo_uuid).collect())
}

#[cfg(all(test, not(feature = "typed_type_graph")))]
mod tests {

    use cozo::{Db, MemStorage};
    use ploke_test_utils::test_run_phases_and_collect;
    use syn_parser::parser::ParsedCodeGraph;

    use crate::{error::TransformError, schema::types::create_and_insert_types};

    use super::transform_types;
    #[test]
    fn test_transform_types() -> Result<(), Box<TransformError>> {
        let _ = env_logger::builder()
            .is_test(true)
            .format_timestamp(None) // Disable timestamps
            .try_init();

        // Setup printable nodes
        let successful_graphs = test_run_phases_and_collect("fixture_nodes");
        let merged = ParsedCodeGraph::merge_new(successful_graphs).expect("Failed to merge graph");

        let db = Db::new(MemStorage::default()).expect("Failed to create database");
        db.initialize().expect("Failed to initialize database");
        create_and_insert_types(&db)?;

        // transform and insert impls into cozo
        transform_types(&db, merged.graph.type_graph)?;

        Ok(())
    }
}
