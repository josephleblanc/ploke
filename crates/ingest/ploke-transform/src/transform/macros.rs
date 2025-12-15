use crate::schema::primary_nodes_multi::MacroNodeSchema;

use crate::macro_traits::CommonFields;

use super::*;

pub(super) fn transform_macros(
    db: &Db<MemStorage>,
    macros: Vec<MacroNode>,
) -> Result<(), TransformError> {
    for macro_node in macros.into_iter() {
        // let schema = &FUNCTION_NODE_SCHEMA;
        let schema = &MacroNodeSchema::SCHEMA;
        let mut macro_node_params = macro_node.cozo_btree();

        let (kind, proc_kind) = match &macro_node.kind {
            syn_parser::parser::nodes::MacroKind::DeclarativeMacro => ("Declarative", None),
            syn_parser::parser::nodes::MacroKind::ProcedureMacro { kind } => {
                let proc = match kind {
                    syn_parser::parser::nodes::ProcMacroKind::Derive => "Derive",
                    syn_parser::parser::nodes::ProcMacroKind::Attribute => "Attribute",
                    syn_parser::parser::nodes::ProcMacroKind::Function => "Function",
                };
                ("Procedural", Some(proc))
            }
        };
        let cozo_proc_kind = proc_kind.map_or(DataValue::Null, |k| k.into());

        let cozo_body = macro_node.body.map_or(DataValue::Null, |b| b.into());

        macro_node_params.insert(schema.kind().to_string(), kind.into());
        macro_node_params.insert(schema.proc_kind().to_string(), cozo_proc_kind);
        macro_node_params.insert(schema.body().to_string(), cozo_body);

        let script = schema.script_put(&macro_node_params);
        db.run_script(&script, macro_node_params, ScriptMutability::Mutable)?;

        // Add attributes
        let attr_schema = AttributeNodeSchema::SCHEMA;
        for (i, attr) in macro_node.attributes.iter().enumerate() {
            let attr_params = process_attributes(macro_node.id.as_any(), i, attr);

            let script = attr_schema.script_put(&attr_params);
            db.run_script(&script, attr_params, ScriptMutability::Mutable)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {

    use std::collections::BTreeMap;

    use cozo::{Db, MemStorage};
    use ploke_test_utils::test_run_phases_and_collect;
    use syn_parser::parser::ParsedCodeGraph;

    use crate::schema::{
        log_db_result, primary_nodes::MacroNodeSchema, secondary_nodes::AttributeNodeSchema,
    };

    use super::transform_macros;
    #[test]
    fn test_transform_macros() -> Result<(), Box<dyn std::error::Error>> {
        let _ = env_logger::builder()
            .is_test(true)
            .format_timestamp(None) // Disable timestamps
            .try_init();

        // Setup printable nodes
        let successful_graphs = test_run_phases_and_collect("fixture_nodes");
        let merged = ParsedCodeGraph::merge_new(successful_graphs).expect("Failed to merge graph");

        let db = Db::new(MemStorage::default()).expect("Failed to create database");
        db.initialize().expect("Failed to initialize database");

        pub(crate) fn create_macro_schema(
            db: &Db<MemStorage>,
        ) -> Result<(), Box<dyn std::error::Error>> {
            let macro_schema = MacroNodeSchema::SCHEMA;
            let db_result = db.run_script(
                &macro_schema.script_create(),
                BTreeMap::new(),
                cozo::ScriptMutability::Mutable,
            )?;
            log_db_result(db_result);
            Ok(())
        }
        // create and insert attribute schema
        AttributeNodeSchema::create_and_insert_schema(&db)?;
        // create and insert macro schema
        create_macro_schema(&db)?;

        // transform and insert impls into cozo
        transform_macros(&db, merged.graph.macros)?;

        Ok(())
    }
}
