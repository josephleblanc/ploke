use syn_parser::utils::LogStyleDebug;

use crate::macro_traits::HasAnyNodeId;
use crate::schema::primary_nodes::ImportNodeSchema;
use crate::utils::log_db_error;

use super::*;

pub(super) fn transform_imports(
    db: &Db<MemStorage>,
    imports: Vec<ImportNode>,
) -> Result<(), TransformError> {
    for import in imports.into_iter() {
        // let schema = &FUNCTION_NODE_SCHEMA;
        let schema = &ImportNodeSchema::SCHEMA;
        let mut import_params = import.cozo_btree_import();

        let cozo_source_path = DataValue::List(
            import
                .source_path
                .into_iter()
                .map(DataValue::from)
                .collect::<Vec<DataValue>>(),
        );
        let cozo_import_kind = match import.kind {
            ImportKind::ExternCrate => "ExternCrate".to_string(),
            ImportKind::UseStatement(_) => "UseStatement".to_string(),
        };
        let cozo_original_name = import
            .original_name
            .map(|n| n.into())
            .unwrap_or(DataValue::Null);

        import_params.insert(schema.source_path().to_string(), cozo_source_path);
        import_params.insert(schema.import_kind().to_string(), cozo_import_kind.into());
        import_params.insert(schema.original_name().to_string(), cozo_original_name);
        import_params.insert(schema.is_glob().to_string(), import.is_glob.into());
        import_params.insert(schema.embedding().to_string(), DataValue::Null);

        import_params.insert(
            schema.is_self_import().to_string(),
            import.is_self_import.into(),
        );
        // NOTE: Clone for now, possibly remove this later.
        import_params.insert(
            schema.visible_name().to_string(),
            import.visible_name.clone().into(),
        );

        // TODO: Add attribute once attribute tracking added to ImportNode in `syn_parser`

        let script = schema.script_put(&import_params);
        // temporary clone() for logging

        db.run_script(&script, import_params.clone(), ScriptMutability::Mutable)
            .inspect_err(|_| {
                tracing::error!(target: "db", "{} {} {}\n  {} {}\n  {} {}\n{} {:#?}",
                    "Error processing import".log_error(),
                    import.visible_name.log_name(),
                    import.id.to_string().log_id(),
                    "Full ID:",
                    import.id.log_id_debug(),
                    "put script".log_step(),
                    &script,
                    "import_params:".log_step(),
                    import_params
                );

                schema.log_create_script();
            })
            .map_err(log_db_error)?;
    }

    Ok(())
}

trait CommonFieldsImport
where
    Self: HasAnyNodeId,
{
    fn cozo_id(&self) -> DataValue {
        self.any_id().to_cozo_uuid()
    }
    fn cozo_name(&self) -> DataValue;
    fn cozo_span(&self) -> DataValue;
    fn process_vis(&self) -> (DataValue, Option<DataValue>);
    fn cozo_cfgs(&self) -> DataValue;

    fn cozo_btree_import(&self) -> BTreeMap<String, DataValue>;
}

impl HasAnyNodeId for ImportNode {
    fn any_id(&self) -> AnyNodeId {
        self.id.as_any()
    }
}

impl CommonFieldsImport for ImportNode {
    fn cozo_name(&self) -> DataValue {
        DataValue::from(self.visible_name.as_str())
    }

    fn cozo_span(&self) -> DataValue {
        let span_start = DataValue::Num(Num::Int(self.span.0 as i64));
        let span_end = DataValue::Num(Num::Int(self.span.1 as i64));
        DataValue::List(Vec::from([span_start, span_end]))
    }

    fn process_vis(&self) -> (DataValue, Option<DataValue>) {
        match &self.kind {
            ImportKind::ExternCrate => (DataValue::from("public".to_string()), None),
            ImportKind::UseStatement(visibility_kind) => {
                let (vis_kind, vis_path) = match visibility_kind {
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
        }
    }

    fn cozo_cfgs(&self) -> DataValue {
        let cfgs: Vec<DataValue> = self
            .cfgs
            .iter()
            .map(|s| DataValue::from(s.as_str()))
            .collect();
        DataValue::List(cfgs)
    }

    fn cozo_btree_import(&self) -> BTreeMap<String, DataValue> {
        let schema = &ImportNodeSchema::SCHEMA;
        let (vis_kind, vis_path) = self.process_vis();

        BTreeMap::from([
            (schema.name().to_string(), self.cozo_name()),
            (schema.id().to_string(), self.cozo_id()),
            (schema.span().to_string(), self.cozo_span()),
            (schema.vis_kind().to_string(), vis_kind),
            (
                schema.vis_path().to_string(),
                vis_path.unwrap_or(DataValue::Null),
            ),
            (schema.cfgs().to_string(), self.cozo_cfgs()),
        ])
    }
}

#[cfg(test)]
mod tests {

    use std::collections::BTreeMap;

    use cozo::{Db, MemStorage};
    use ploke_test_utils::test_run_phases_and_collect;
    use syn_parser::parser::ParsedCodeGraph;

    use crate::{
        schema::{log_db_result, primary_nodes::ImportNodeSchema},
        transform::imports::transform_imports,
    };

    #[test]
    fn test_transform_imports() -> Result<(), Box<dyn std::error::Error>> {
        let _ = env_logger::builder()
            .is_test(true)
            .format_timestamp(None) // Disable timestamps
            .try_init();

        // Setup printable nodes
        let successful_graphs = test_run_phases_and_collect("fixture_nodes");
        let merged = ParsedCodeGraph::merge_new(successful_graphs).expect("Failed to merge graph");

        let db = Db::new(MemStorage::default()).expect("Failed to create database");
        db.initialize().expect("Failed to initialize database");

        // TODO: Add attribute tracking to `ImportNode`
        // create and insert attribute schema
        // create_attribute_schema(&db)?;

        pub(crate) fn create_import_schema(
            db: &Db<MemStorage>,
        ) -> Result<(), Box<dyn std::error::Error>> {
            let import_schema = ImportNodeSchema::SCHEMA;

            let db_result = db.run_script(
                &import_schema.script_create(),
                BTreeMap::new(),
                cozo::ScriptMutability::Mutable,
            )?;

            log_db_result(db_result);

            Ok(())
        }
        create_import_schema(&db)?;

        // transform and insert impls into cozo
        transform_imports(&db, merged.graph.use_statements)?;

        Ok(())
    }
}
