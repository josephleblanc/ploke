//! Defines conversion traits for CozoDB operations

use cozo::DataValue;
use itertools::Itertools;
use std::collections::BTreeMap;
use syn_parser::parser::nodes::FunctionNode;
use syn_parser::parser::types::VisibilityKind; // For join() functionality

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
        map.insert("id".into(), DataValue::from(self.id as i64));
        map.insert("name".into(), self.name.into());
        map.insert("visibility_kind".into(), vis_kind.into());
        map.insert(
            "visibility_path".into(),
            vis_path.unwrap_or(DataValue::Null),
        );
        map.insert(
            "return_type_id".into(),
            self.return_type
                .map_or(DataValue::Null, |id| DataValue::from(id as i64)),
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
