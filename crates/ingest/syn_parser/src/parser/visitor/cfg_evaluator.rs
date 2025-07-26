use std::collections::HashSet;

/// Everything we can encounter inside a `#[cfg(…)]`
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum CfgAtom {
    Feature(String),
    TargetOs(String),
    TargetArch(String),
    TargetFamily(String),
    // Add more as needed
}

/// Parsed cfg expression tree
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum CfgExpr {
    All(Vec<CfgExpr>),
    Any(Vec<CfgExpr>),
    Not(Box<CfgExpr>),
    Atom(CfgAtom),
}

/// Active flags as seen by Cargo
pub struct ActiveCfg {
    pub features: HashSet<String>,
    pub target_os: String,
    pub target_arch: String,
    pub target_family: String,
}

impl ActiveCfg {
    pub fn eval(&self, expr: &CfgExpr) -> bool {
        match expr {
            CfgExpr::All(children) => children.iter().all(|c| self.eval(c)),
            CfgExpr::Any(children) => children.iter().any(|c| self.eval(c)),
            CfgExpr::Not(inner) => !self.eval(inner),
            CfgExpr::Atom(atom) => match atom {
                CfgAtom::Feature(f) => self.features.contains(f),
                CfgAtom::TargetOs(os) => self.target_os == *os,
                CfgAtom::TargetArch(arch) => self.target_arch == *arch,
                CfgAtom::TargetFamily(family) => self.target_family == *family,
            },
        }
    }
}

impl ActiveCfg {
    pub fn from_crate_context(context: &crate::discovery::CrateContext) -> Self {
        let features = context
            .features()
            .iter()
            .flat_map(|(_, enabled)| enabled.iter().cloned())
            .collect();

        // Parse target triple from environment or use defaults
        let target_triple =
            std::env::var("TARGET").unwrap_or_else(|_| "x86_64-unknown-linux-gnu".to_string());
        let parts: Vec<&str> = target_triple.split('-').collect();

        let target_arch = parts.first().unwrap_or(&"x86_64").to_string();
        let target_os = parts.get(2).unwrap_or(&"linux").to_string();
        let target_family = if target_os == "linux" || target_os == "macos" {
            "unix".to_string()
        } else if target_os == "windows" {
            "windows".to_string()
        } else {
            "unknown".to_string()
        };

        Self {
            features,
            target_os,
            target_arch,
            target_family,
        }
    }
}
