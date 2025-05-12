pub mod primary_nodes;

use cozo::{MemStorage, ScriptMutability};
use std::collections::BTreeMap;

// TODO: use lazy_static and SmartString
// &str for now,
// find a better solution using lazy_static and maybe SmartString, since that is what cozo uses
// anyawys
pub struct CozoField {
    st: &'static str,
    dv: &'static str,
}
impl CozoField {
    fn schema_str(&self) -> impl Iterator<Item = char> {
        self.st.chars().chain(": ".chars()).chain(self.dv.chars())
    }
}

impl CozoField {
    pub fn st(&self) -> &str {
        self.st
    }

    pub fn dv(&self) -> &str {
        self.dv
    }
}
