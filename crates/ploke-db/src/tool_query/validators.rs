use std::path::Path;

use itertools::Itertools;

use crate::{result::get_pos, Database, DbError};

pub trait Validator {
    fn validate_path_file(&self, file_path: &Path) -> Result<(), DbError>;
    fn validate_name(&self, name: &str) -> Result<(), DbError>;
    fn validate_path_module(&self, module_path: &[&str]) -> Result<(), DbError>;
}

impl Validator for Database {
    fn validate_path_file(&self, file_path: &Path) -> Result<(), DbError> {
        let fp = file_path.display().to_string();
        // let query = "?[count(files)] := *module {{id, name, module_kind, path @ 'NOW'";
        let query = r#"?[count(files)] := *module {{ id @ 'NOW' }}, *file_mod {{ owner_id: id, file_path @ 'NOW'}} "#;
        self.raw_query(query)?;
        Ok(())
    }

    // TODO: Test
    fn validate_name(&self, name: &str) -> Result<(), DbError> {
        let rels = self.raw_query("::relations")?;
        let name_index = get_pos(&rels.headers, "name")?;
        let columns_index = get_pos(&rels.headers, "columns")?;

        let join_query_part = rels
            .rows
            .iter()
            .filter_map(|r| {
                let has_name_col = r[columns_index]
                    .get_slice()?
                    .iter()
                    .filter_map(|s| s.get_str())
                    .any(|s| s.contains("name"));
                if has_name_col {
                    r[name_index].get_str()
                } else {
                    None
                }
            })
            .map(|rel_name| format!("*{rel_name} {{ id, name @ 'NOW' }}"))
            .join(" or ");

        let all_names_query = format!("?[count(name)] := {join_query_part}, name == {name}");
        let all_names_result = self.raw_query(&all_names_query)?;
        // for name in rels_with_name {
        // }
        Ok(())
    }

    fn validate_path_module(&self, module_path: &[&str]) -> Result<(), DbError> {
        todo!()
    }
}
