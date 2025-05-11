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
