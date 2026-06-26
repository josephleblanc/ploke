use std::collections::HashSet;

use crate::{Database, DbError};

impl Database {
    pub fn has_call_graph_relations(&self) -> Result<bool, DbError> {
        const REQUIRED: [&str; 4] = [
            "call_site",
            "call_site_edge",
            "call_relation",
            "call_resolution_status",
        ];
        let rows = self.raw_query("::relations")?;
        let registered = rows
            .rows
            .iter()
            .filter_map(|row| row.first().and_then(|value| value.get_str()))
            .collect::<HashSet<_>>();
        if !REQUIRED.iter().all(|name| registered.contains(name)) {
            return Ok(false);
        }

        let populated = self.raw_query(
            r#"?[site_id] :=
                *call_site { id: site_id @ 'NOW' },
                *call_site_edge {
                    target_id: site_id,
                    relation_kind: "BodyContainsCall" @ 'NOW'
                },
                *call_resolution_status { source_id: site_id @ 'NOW' }
            :limit 1"#,
        )?;
        Ok(!populated.rows.is_empty())
    }
}
