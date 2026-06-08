use tracing::instrument;

use crate::schema::types::*;

use super::*;

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

fn cozo_type_list<T>(ids: impl IntoIterator<Item = T>) -> DataValue
where
    T: ToCozoUuid,
{
    DataValue::List(ids.into_iter().map(ToCozoUuid::to_cozo_uuid).collect())
}
