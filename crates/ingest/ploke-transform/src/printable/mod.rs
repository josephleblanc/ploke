use syn_parser::parser::nodes::*;

mod functions;

pub(crate) trait PrintToCozo
where
    Self: HasAnyNodeId,
{
    fn print_cozo_insert(&self, module_id: ModuleNodeId) -> String;
}

fn escape_str(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\'', "\\'")
}

pub trait CozoSchema {
    // TODO: turn these into lazy statics to stay DRY
    fn schema() -> &'static str;
    fn query_all_rhs() -> &'static str;
    fn all_fields() -> &'static str;
    fn query_all_string() -> String {
        let mut query = String::from("?");
        query.push('[');
        query.push_str(Self::all_fields());
        query.push(']');
        query.push_str(" := ");
        query.push_str(Self::query_all_rhs());
        query
    }
}

impl CozoSchema for FunctionNode {
    fn schema() -> &'static str {
        // TODO: remove the `:create` part of the string here, make composable instead.
        r#"
:create function {
    id:             Uuid,
    name:           String,
    docstring:      String?,
    span:           [Int],
    visibility:     String,
    return_type:    String?,
    generic_params: Json,
    attributes:     [String],
    body:           String?,
    tracking_hash:  String,
    cfgs:           [String]?
    module_id:      Uuid,
}"#
    }
    /// Forms the right hand side of a query,
    /// e.g. `[id, name, ..] := *function { id, name .. }`
    ///
    /// Note that the `..` ellisions are not valid, merely for example. For a real query you would
    /// need to enter all the fields on both the lhs and rhs.
    fn query_all_rhs() -> &'static str {
        r#"
*function {
    id,
    name,
    docstring,
    span,
    visibility,
    return_type,
    generic_params,
    attributes,
    body,
    tracking_hash,
    cfgs
    module_id,
}
"#
    }
    fn all_fields() -> &'static str {
        r#"
    id,
    name,
    visibility,
    return_type,
    generic_params,
    attributes,
    docstring,
    span,
    body,
    tracking_hash,
    cfgs
    module_id,
"#
    }
}

#[cfg(test)]
mod test {
    use std::collections::BTreeMap;

    use cozo::{Db, MemStorage};
    use syn_parser::parser::nodes::FunctionNode;

    use super::CozoSchema;

    #[test]
    fn basic_function_schema() -> Result<(), Box<dyn std::error::Error>> {
        let db = Db::new(MemStorage::default()).expect("Failed to create database");
        db.initialize().expect("Failed to initialize database");

        let function_schema = FunctionNode::schema();
        db.run_script(
            function_schema,
            BTreeMap::new(),
            cozo::ScriptMutability::Mutable,
        )?;

        Ok(())
    }
}
