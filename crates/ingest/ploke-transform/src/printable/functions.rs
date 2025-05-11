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
    use ploke_test_utils::run_phases_and_collect;

    #[test]
    fn basic() {
        let successful_graphs = run_phases_and_collect("fixture_types");
    }
}
