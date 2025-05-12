//! Defines conversion traits for CozoDB operations

use cozo::{DataValue, Num};
use itertools::Itertools;
use std::collections::BTreeMap;
use syn_parser::parser::nodes::{
    AnyNodeId, AsAnyNodeId, Attribute, ConstNode, FunctionNode, StructNode, ToCozoUuid,
};
use syn_parser::parser::types::VisibilityKind;

use crate::schema::primary_nodes::{ConstNodeSchema, FunctionNodeSchema, StructNodeSchema};
use crate::schema::secondary_nodes::AttributeNodeSchema; // For join() functionality
                                                         //
pub trait CommonFields
where
    Self: HasAnyNodeId,
{
    fn cozo_id(&self) -> DataValue {
        self.any_id().to_cozo_uuid()
    }
    fn cozo_name(&self) -> DataValue;
    fn cozo_docstring(&self) -> DataValue;
    fn cozo_span(&self) -> DataValue;
    fn cozo_tracking_hash(&self) -> DataValue;
    fn cozo_cfgs(&self) -> DataValue;

    fn process_vis(&self) -> (DataValue, Option<DataValue>);
    fn process_attribute(&self, i: i64, attr: &Attribute) -> BTreeMap<String, DataValue> {
        let schema = AttributeNodeSchema::SCHEMA;
        let value = attr
            .value
            .as_ref()
            .map(|s| DataValue::from(s.as_str()))
            .unwrap_or(DataValue::Null);
        let args: Vec<DataValue> = attr
            .args
            .iter()
            .map(|s| DataValue::from(s.as_str()))
            .collect();

        let attr_params = BTreeMap::from([
            (schema.owner_id().to_string(), self.any_id().to_cozo_uuid()),
            (schema.index().to_string(), DataValue::from(i)),
            (
                schema.name().to_string(),
                DataValue::from(attr.name.as_str()),
            ),
            (schema.value().to_string(), value),
            (schema.args().to_string(), DataValue::List(args)),
        ]);
        attr_params
    }

    fn cozo_btree(&self) -> BTreeMap<String, DataValue>;
}

// NOTE: WIP, turn this into macro for, e.g. FunctionNodeId, StructNodeId, etc
pub trait HasAnyNodeId {
    fn any_id(&self) -> AnyNodeId;
}

macro_rules! common_fields {
    ($node:path, $schema:path) => {

        impl HasAnyNodeId for $node {
            fn any_id(&self) -> AnyNodeId {
                    self.id.as_any()
            }
        }
        impl CommonFields for $node {
            fn cozo_name(&self) -> DataValue {
                DataValue::from(self.name.as_str())
            }

            fn cozo_docstring(&self) -> DataValue {
                self.docstring
                    .as_ref()
                    .map(|s| DataValue::from(s.as_str()))
                    .unwrap_or(DataValue::Null)
            }

            fn cozo_span(&self) -> DataValue {
                let span_start = DataValue::Num(Num::Int(self.span.0 as i64));
                let span_end = DataValue::Num(Num::Int(self.span.1 as i64));
                DataValue::List(Vec::from([span_start, span_end]))
            }

            fn cozo_tracking_hash(&self) -> DataValue {
                DataValue::Uuid(cozo::UuidWrapper(
                    self.tracking_hash
                        .as_ref()
                        .unwrap_or_else(|| {
                            panic!(
                                "Invariant Violated: Node must have TrackingHash upon database insertion"
                            )
                        })
                        .0,
                ))
            }

            fn cozo_cfgs(&self) -> DataValue {
                let cfgs: Vec<DataValue> = self
                    .cfgs
                    .iter()
                    .map(|s| DataValue::from(s.as_str()))
                    .collect();
                DataValue::List(cfgs)
            }

            fn process_vis(&self) -> (DataValue, Option<DataValue>) {
                let (vis_kind, vis_path) = match &self.visibility {
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
            fn cozo_btree(&self) -> BTreeMap<String, DataValue> {
                let schema = &($schema);
                let (vis_kind, vis_path) = self.process_vis();

                BTreeMap::from([
                    (schema.id().to_string(), self.cozo_id()),
                    (schema.name().to_string(), self.cozo_name()),
                    (schema.docstring().to_string(), self.cozo_docstring()),
                    (schema.span().to_string(), self.cozo_span()),
                    (schema.tracking_hash().to_string(), self.cozo_tracking_hash()),
                    (schema.cfgs().to_string(), self.cozo_cfgs()),
                    (schema.vis_kind().to_string(), vis_kind),
                    (schema.vis_path().to_string(), vis_path.unwrap_or(DataValue::Null),
                    ),
                ])
            }
        }
    }
}

common_fields!(ConstNode, ConstNodeSchema::SCHEMA);
common_fields!(FunctionNode, FunctionNodeSchema::SCHEMA);
common_fields!(StructNode, StructNodeSchema::SCHEMA);

/// Types that can be converted to/from CozoDB representation
pub trait IntoCozo {
    /// Returns the Cozo relation name this type maps to
    fn cozo_relation() -> &'static str;

    /// Converts the type into a CozoDB data map
    fn into_cozo_map(self) -> BTreeMap<String, DataValue>;

    /// Generates a CozoScript PUT operation for this item
    fn cozo_insert_script(&self) -> String
    where
        Self: Clone,
    {
        let map = self.clone().into_cozo_map();
        let columns: Vec<_> = map.keys().collect();
        let values: Vec<_> = map.values().map(|v| v.to_string()).collect();

        format!(
            "?[{}] <- [[{}]] :put {}",
            columns.iter().join(", "),
            values.iter().join(", "),
            Self::cozo_relation()
        )
    }
}

impl IntoCozo for FunctionNode {
    fn cozo_relation() -> &'static str {
        "functions"
    }

    fn into_cozo_map(self) -> BTreeMap<String, DataValue> {
        let mut map = BTreeMap::new();
        let (vis_kind, vis_path) = visibility_to_cozo(self.visibility);
        map.insert("id".into(), self.id.into());
        map.insert("name".into(), self.name.into());
        map.insert("visibility_kind".into(), vis_kind.into());
        map.insert(
            "visibility_path".into(),
            vis_path.unwrap_or(DataValue::Null),
        );
        map.insert(
            "return_type_id".into(),
            self.return_type.map_or(DataValue::Null, |id| id.into()),
        );
        map.insert(
            "docstring".into(),
            self.docstring.map_or(DataValue::Null, |s| s.into()),
        );
        map.insert(
            "body".into(),
            self.body.map_or(DataValue::Null, |s| s.into()),
        );
        map
    }
}

fn visibility_to_cozo(v: VisibilityKind) -> (String, Option<DataValue>) {
    match v {
        VisibilityKind::Public => ("public".into(), None),
        VisibilityKind::Crate => ("crate".into(), None),
        VisibilityKind::Restricted(path) => {
            let list = DataValue::List(path.into_iter().map(DataValue::from).collect());
            ("restricted".into(), Some(list))
        }
        VisibilityKind::Inherited => ("inherited".into(), None),
    }
}

/// Helper for batch operations
pub trait BatchIntoCozo: IntoCozo {
    fn cozo_batch_insert_script(items: &[Self]) -> String
    where
        Self: Clone,
    {
        if items.is_empty() {
            return String::new();
        }

        let sample = items[0].clone().into_cozo_map();
        let columns: Vec<_> = sample.keys().collect();

        let values = items
            .iter()
            .map(|item| {
                let map = item.clone().into_cozo_map();
                columns
                    .iter()
                    .map(|col| map[*col].to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .map(|vals| format!("[{vals}]"))
            .collect::<Vec<_>>()
            .join(", ");

        format!(
            "?[{}] <- [{}] :put {}",
            columns.into_iter().join(", "),
            values,
            Self::cozo_relation()
        )
    }
}

impl<T: IntoCozo> BatchIntoCozo for T {}
