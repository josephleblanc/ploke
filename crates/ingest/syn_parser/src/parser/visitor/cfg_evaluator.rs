//! **WORK-IN-PROGRESS**
//!
//! This module provides basic evaluation of `#[cfg(...)]` attributes during
//! ingestion.  The implementation is intentionally minimal and will be extended
//! as new configuration atoms and target triples are required.
//!
//! # ⚠️ Known Limitations
//!
//! The evaluator **silently rejects** the following commonly-used atoms:
//!
//! * `target_pointer_width = "64"` / `"32"`  
//! * `target_endian = "little"` / `"big"`  
//! * `target_vendor = "unknown"` / `"apple"` / `"pc"`  
//! * `target_env = "gnu"` / `"musl"` / `"msvc"`  
//! * `windows`, `unix`, `test`, `debug_assertions`, `doc`, `proc_macro`  
//! * `panic = "unwind"` / `"abort"`  
//! * `target_has_atomic = "…"`, `target_feature = "…"`  
//!
//! Any item guarded by an unsupported atom is **dropped from the graph**.
//!
//! # Fallback Target Triple
//!
//! When the `TARGET` environment variable is missing, the evaluator falls back
//! to `"x86_64-unknown-linux-gnu"`.  This is **not neutral**:
//!
//! * It biases the corpus toward Linux/x86-64 code paths.  
//! * It breaks determinism across machines.  
//!
//! **Do not rely on this default for production ingestion.**
//!
//! TODO: Replace the fallback with an explicit CLI flag or error.

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
#[derive(Debug, Clone)]
pub struct ActiveCfg {
    pub features: HashSet<String>,
    pub target_os: String,
    pub target_arch: String,
    pub target_family: String,
}

impl ActiveCfg {
    /// Evaluate a parsed `#[cfg(...)]` expression against the active configuration.
    ///
    /// Returns `true` if the expression is satisfied under the current feature set
    /// and target platform, `false` otherwise.
    ///
    /// Supported atoms: `feature`, `target_os`, `target_arch`, `target_family`.
    /// Unsupported atoms are **not** evaluated and always yield `false`.
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
    /// Build an `ActiveCfg` from the discovered crate context.
    ///
    /// Combines:
    /// * enabled features defined in `Cargo.toml`,
    /// * the target triple taken from the `$TARGET` environment variable (or a
    ///   default fallback),
    /// * derived `target_arch`, `target_os`, and `target_family` strings.
    ///
    /// # Panics
    /// Never panics; unknown or malformed triples fall back to
    /// `"x86_64-unknown-linux-gnu"` semantics.
    pub fn from_crate_context(context: &crate::discovery::CrateContext) -> Self {
        let features = context
            .features()
            .iter()
            .flat_map(|(_, enabled)| enabled.iter().cloned())
            .collect();

        // Parse target triple from environment or use defaults
        // TODO: Replace the fallback triple with a more complete target-triple parser.
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

// impl ActiveCfg {
// }
