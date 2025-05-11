use super::*;
use ploke_core::IdTrait;
use std::fmt::Write;

impl PrintToCozo for FunctionNode {
    fn print_cozo_insert(&self, module_id: ModuleNodeId) -> String {
        let mut output = String::new();

        // Start the relation
        output.push_str(
            "?[id, name, module_id, visibility, return_type, generic_params,
attributes, docstring, span, body, tracking_hash, cfgs] <- [\n",
        );

        let generic_params = serde_json::to_string(&self.generic_params).unwrap();
        let attributes = self
            .attributes
            .iter()
            .map(|a| format!("'{}'", escape_str(format!("{:?}", a).as_str())))
            .collect::<Vec<_>>()
            .join(", ");

        write!(
            output,
            "  [\n    '{}',\n    '{}',\n    '{}',\n    '{}',\n    {},",
            self.id.to_uuid_string(),
            escape_str(&self.name),
            module_id.to_uuid_string(),
            self.visibility,
            self.return_type
                .as_ref()
                .map_or("null".into(), |rt| format!("'{}'", rt.uuid()))
        )
        .unwrap();

        write!(
            output,
            "\n    '{}',\n    [{}],\n    {},",
            generic_params,
            attributes,
            self.docstring
                .as_ref()
                .map_or("null".into(), |d| format!("'{}'", escape_str(d)))
        )
        .unwrap();

        write!(
            output,
            "\n    [{}, {}],\n    {},\n    {},\n    []\n  ]",
            self.span.0,
            self.span.1,
            self.body
                .as_ref()
                .map_or("null".into(), |b| format!("'{}'", escape_str(b))),
            self.tracking_hash
                .as_ref()
                .map_or("null".into(), |h| format!("'{}'", h.0))
        )
        .unwrap();

        // Close and add put command
        output.push_str("\n];\n");
        output.push_str(
            ":put function {id, name, module_id, visibility, return_type,
generic_params, attributes, docstring, span, body, tracking_hash, cfgs}\n",
        );

        output
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use cozo::{Db, MemStorage, NamedRows, ScriptMutability};
    use log::{debug, info};
    use ploke_test_utils::run_phases_and_collect;
    use syn_parser::{
        parser::{
            graph::GraphAccess,
            nodes::{FunctionNode, PrimaryNodeIdTrait},
        },
        utils::LogStyle,
    };

    use crate::printable::{CozoSchema, PrintToCozo};

    #[test]
    fn basic_print() {
        let _ = env_logger::builder()
            .is_test(true)
            .format_timestamp(None) // Disable timestamps
            .try_init();
        let successful_graphs = run_phases_and_collect("fixture_types");
        let target_graph = successful_graphs
            .iter()
            .find(|pg| pg.crate_context.is_some())
            .expect("root file not found"); // find root file

        for node in target_graph.functions() {
            let module_id = target_graph
                .find_containing_mod_id(node.id.to_pid())
                .expect("containing module not found");
            let printable = node.print_cozo_insert(module_id);
            eprintln!("{{ {} }}", printable);
        }
        eprintln!("The schema may be created using the following:");
        eprintln!("{}", FunctionNode::schema());
        eprintln!("Then each individual entry may simply be copied and pasted into the cozo script command");
    }

    #[test]
    fn basic_run_script() -> Result<(), cozo::Error> {
        // Enable logging
        let _ = env_logger::builder()
            .is_test(true)
            .format_timestamp(None) // Disable timestamps
            .try_init();

        // Setup printable nodes
        let successful_graphs = run_phases_and_collect("fixture_types");
        let target_graph = successful_graphs
            .iter()
            .find(|pg| pg.crate_context.is_some())
            .expect("root file not found"); // find root file

        let mut all_cozo_printable: Vec<String> = Vec::new();
        for node in target_graph.functions() {
            let module_id = target_graph
                .find_containing_mod_id(node.id.to_pid())
                .expect("containing module not found");
            let printable = node.print_cozo_insert(module_id);
            all_cozo_printable.push(printable);
        }

        // Initialize db
        let db = Db::new(MemStorage::default()).expect("Failed to create database");
        db.initialize().expect("Failed to initialize database");

        let function_schema = FunctionNode::schema();
        db.run_script(
            function_schema,
            BTreeMap::new(),
            cozo::ScriptMutability::Mutable,
        )?;

        let node_cozo = &all_cozo_printable[0];
        let named_row =
            db.run_script(node_cozo, BTreeMap::new(), cozo::ScriptMutability::Mutable)?;
        // Cozo returns the status after the insert,
        // e.g. named_row: NamedRows { headers: ["status"], rows: [["OK"]], next: None }
        info!(target: "basic", "named_row: {:?}", named_row);

        // Inserting the exact same row a second time does not result in an error.
        // NOTE: Not sure how exactly cozo handles this, it likely replaces the first entry with the
        // second?
        let dup_named_row =
            db.run_script(node_cozo, BTreeMap::new(), cozo::ScriptMutability::Mutable)?;
        println!("duplicate named_row: {:?}", dup_named_row);

        // Query all fields of the function. This will return a `cozo::NamedRows`
        let query_all_fields = FunctionNode::query_all_string();
        let db_return: NamedRows = db
            .run_script(
                &query_all_fields,
                BTreeMap::new(),
                cozo::ScriptMutability::Immutable, // can use Immutable access for query
            )
            .inspect_err(|_| {
                debug!(target: "basic",
                    "{}: {} | {} \n{}",
                    "Error".log_error(),
                    "basic script",
                    "Script dump follows:",
                    query_all_fields
                );
            })?;
        info!(target: "basic", "{}: {:?}", "Single Query Return:".log_step(), db_return);

        // Try entering the rest of the entries in individusal `run_script` calls
        info!(target: "basic", "{}", "Enter Remaining Entries".log_header());
        let all_rows_count = all_cozo_printable
            .iter()
            .map(|n| db.run_script(n, BTreeMap::new(), ScriptMutability::Mutable))
            .inspect(|named_row| {
                info!(target: "verbose", "  {}:\n{:?}", "Returned Name Row".log_step(), named_row);
            })
            .count();
        info!(target: "basic", "{}: {}", "  Total Named Rows".log_step(), all_rows_count.to_string().log_spring_green());

        // Print all rows again. This time there should be more entries
        let db_return: NamedRows = db
            .run_script(
                &query_all_fields,
                BTreeMap::new(),
                cozo::ScriptMutability::Immutable, // can use Immutable access for query
            )
            .inspect_err(|_| {
                debug!(target: "basic",
                    "{}: {} | {} \n{}",
                    "Error".log_error(),
                    "basic script",
                    "Script dump follows:",
                    query_all_fields
                );
            })?;
        info!(target: "verbose", "{}: {:#?}", "Query Return:".log_step(), db_return);

        Ok(())
    }
}
