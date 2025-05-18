use serde::{Deserialize, Serialize};


#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum Anchor {
    QueryCustom,
    QueryBuilder,
}

impl Anchor {
    fn all() -> Vec<Self> {
        vec![ Self::QueryCustom,
        Self::QueryBuilder, ]
    }

    fn from_str_case_insensitive(anchor: &str) -> Option<Self> {
        let anchor = anchor.to_lowercase();
        Self::all().into_iter().find(|x| x.to_string() == anchor)
    }
}

impl std::fmt::Display for Anchor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut name = format!("{self:?}");
        name.make_ascii_lowercase();
        f.write_str(&name)
    }
}
