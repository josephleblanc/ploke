use crate::schema::primary_nodes_multi::ModuleNodeSchema;

use std::path::PathBuf;

use cozo::UuidWrapper;
use syn_parser::utils::LogStyleDebug;
use uuid::Uuid;

use crate::{macro_traits::CommonFields, schema::subnode_variants::FileModuleNodeSchema};

use super::*;

/// Copy of the inner struct for a file-based module.
/// Used to keep the schema formation uniform by calling macro.
// NOTE: Consider replacing these three with a better macro instead.
pub struct FileModuleNode {
    /// PrimaryNodeIds of items directly contained within the module's file.
    /// Note: May be temporary; `Relation::Contains` is the primary source.
    items: Vec<PrimaryNodeId>,
    /// The absolute path to the file containing the module definition.
    file_path: PathBuf,
    /// Inner attributes (`#![...]`) found at the top of the module file.
    file_attrs: Vec<Attribute>,
    /// Inner documentation (`//! ...`) found at the top of the module file.
    file_docs: Option<String>,
    /// The namespace stored in the crate_context node in the database.
    name_space: Uuid,
}

pub(super) fn transform_modules(
    db: &Db<MemStorage>,
    modules: Vec<ModuleNode>,
    namespace: Uuid,
) -> Result<(), TransformError> {
    for module in modules.into_iter() {
        // let schema = &FUNCTION_NODE_SCHEMA;
        let schema = &ModuleNodeSchema::SCHEMA;
        let mut module_params = module.cozo_btree();

        let cozo_path = DataValue::List(
            module
                .path
                .into_iter()
                .map(DataValue::from)
                .collect::<Vec<DataValue>>(),
        );
        let cozo_module_kind = process_module_def(db, module.module_def, module.id, namespace)?;

        module_params.insert(schema.path().to_string(), cozo_path);
        module_params.insert(schema.module_kind().to_string(), cozo_module_kind.into());

        // Add attributes
        let attr_schema = AttributeNodeSchema::SCHEMA;
        for (i, attr) in module.attributes.iter().enumerate() {
            let attr_params = process_attributes(module.id.as_any(), i, attr);

            let script = attr_schema.script_put(&attr_params);
            db.run_script(&script, attr_params, ScriptMutability::Mutable)?;
        }

        let script = schema.script_put(&module_params);
        // temporary clone() for logging
        db.run_script(&script, module_params.clone(), ScriptMutability::Mutable)
            .inspect_err(|_| {
                tracing::error!(target: "db", "{} {} {}\n  {} {}\n  {} {}\n{} {:#?}",
                    "Error processing module".log_error(),
                    module.name.log_name(),
                    module.id.to_string().log_id(),
                    "Full ID:",
                    module.id.log_id_debug(),
                    "put script".log_step(),
                    &script,
                    "module_params:".log_step(),
                    module_params
                );
                schema.log_create_script();
            })?;
    }

    Ok(())
}

fn process_module_def(
    db: &Db<MemStorage>,
    module_def: ModuleKind,
    module_id: ModuleNodeId,
    namespace: Uuid,
) -> Result<String, TransformError> {
    let schema = &FileModuleNodeSchema::SCHEMA;

    match module_def {
        ModuleKind::FileBased {
            items,
            file_path,
            file_attrs,
            file_docs,
        } => {
            let cozo_owner = module_id.as_any().to_cozo_uuid();
            let cozo_file_path = file_path
                .into_os_string()
                .into_string()
                .unwrap_or_else(|f| {
                    tracing::error!(target: "db", "{} {} ({}) {:?}",
                        "Error in file path:".log_error(),
                        "ModuleNode ID",
                        module_id.to_string().log_name(),
                        f.to_string_lossy().log_foreground_primary()
                    );
                    panic!()
                });
            let cozo_file_docs = file_docs.map_or(DataValue::Null, |f| f.into());

            let cozo_items = DataValue::List(
                items
                    .into_iter()
                    .map(|id| id.to_cozo_uuid())
                    .collect::<Vec<DataValue>>(),
            );

            let attr_schema = AttributeNodeSchema::SCHEMA;
            for (i, attr) in file_attrs.iter().enumerate() {
                let attr_params = process_attributes(module_id.as_any(), i, attr);

                let script = attr_schema.script_put(&attr_params);
                db.run_script(&script, attr_params, ScriptMutability::Mutable)?;
            }
            let cozo_namespace = DataValue::Uuid(UuidWrapper(namespace));

            let params = BTreeMap::from([
                (schema.file_docs().to_string(), cozo_file_docs),
                (schema.owner_id().to_string(), cozo_owner),
                (schema.file_path().to_string(), cozo_file_path.into()),
                (schema.items().to_string(), cozo_items),
                (schema.namespace().to_string(), cozo_namespace),
            ]);
            let put_script = schema.script_put(&params);
            db.run_script(&put_script, params, ScriptMutability::Mutable)?;
            Ok("FileBased".to_string())
        }
        ModuleKind::Inline { .. } => Ok("Inline".to_string()),
        ModuleKind::Declaration { .. } => Ok("Declaration".to_string()),
    }
}

#[cfg(test)]
mod tests {

    use cozo::{Db, MemStorage};
    use ploke_test_utils::test_run_phases_and_collect;
    use syn_parser::parser::ParsedCodeGraph;

    use crate::{
        schema::{
            primary_nodes::ModuleNodeSchema, secondary_nodes::AttributeNodeSchema,
            subnode_variants::FileModuleNodeSchema,
        },
        transform::module::transform_modules,
    };

    #[test]
    fn test_transform_modules() -> Result<(), Box<dyn std::error::Error>> {
        let _ = env_logger::builder()
            .is_test(true)
            .format_timestamp(None) // Disable timestamps
            .try_init();

        // Setup printable nodes
        let successful_graphs = test_run_phases_and_collect("fixture_nodes");
        let merged = ParsedCodeGraph::merge_new(successful_graphs).expect("Failed to merge graph");

        let db = Db::new(MemStorage::default()).expect("Failed to create database");
        db.initialize().expect("Failed to initialize database");

        // create and insert attribute schema
        AttributeNodeSchema::create_and_insert_schema(&db)?;

        ModuleNodeSchema::create_and_insert_schema(&db)?;

        FileModuleNodeSchema::create_and_insert_schema(&db)?;

        // transform and insert impls into cozo
        transform_modules(&db, merged.graph.modules, merged.crate_namespace)?;

        Ok(())
    }
}
