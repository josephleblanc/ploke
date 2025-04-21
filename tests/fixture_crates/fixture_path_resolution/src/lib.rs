//! Fixture crate designed to test various path resolution scenarios,
//! including modules, imports, re-exports, #[path], #[cfg], and dependencies.

// === Dependencies ===
use log::{debug, info}; // External dep (workspace = true)
use ploke_core::{IdTrait, ItemKind, NodeId, TypeId}; // Import IdTrait and ItemKind
use regex::Regex; // External dep (non-workspace)
use serde::Serialize; // External dep with feature
use thiserror::Error; // External dep (workspace = true) // Workspace dep
use uuid::Uuid; // Import Uuid directly

// === Modules ===

// 1. Standard file-based public module
pub mod local_mod;

// 2. Inline public module
pub mod inline_mod {
    pub fn inline_func() -> u8 {
        1
    }

    // 2a. Re-export within inline module
    pub use super::local_mod::nested::deep_func as deep_reexport_inline;
}

// 3. Private inline module
mod private_inline_mod {
    fn private_inline_func() {}

    // 3a. Public item within private module (should not be accessible externally)
    pub fn pub_in_private_inline() {}
}

// 4. Module using #[path] attribute
#[path = "renamed_path/actual_file.rs"]
pub mod logical_path_mod;

// 5. Module gated by cfg attribute
#[cfg(feature = "feature_a")]
pub mod cfg_mod_a {
    pub fn func_a() -> String {
        "feature_a active".to_string()
    }
}

// 6. Another module gated by cfg attribute
#[cfg(feature = "feature_b")]
pub mod cfg_mod_b {
    pub fn func_b() -> String {
        "feature_b active".to_string()
    }
}

// === Re-exports at Crate Root ===

// 7. Re-export a local item
pub use local_mod::local_func; // Shortest path: crate::local_func

// 8. Re-export a nested local item
pub use local_mod::nested::deep_func; // Shortest path: crate::deep_func

// 9. Re-export an item from an external dependency (log::debug)
pub use log::debug as log_debug_reexport; // Shortest path: crate::log_debug_reexport

// Removed: Re-export of ploke_common::workspace_root

// 11. Re-export with rename
pub use local_mod::nested::deep_func as renamed_deep_func; // Shortest path: crate::renamed_deep_func

// 12. Re-export a module
pub use local_mod::nested as reexported_nested_mod; // Shortest path: crate::reexported_nested_mod

// 13. Re-export an item from the #[path] module
pub use logical_path_mod::item_in_actual_file; // Shortest path: crate::item_in_actual_file

// === Items at Crate Root ===
pub struct RootStruct {
    pub field: TypeId, // Use TypeId from workspace dep
}

#[derive(Error, Debug, Serialize)] // Use derive from external dep
pub enum RootError {
    #[error("An error occurred")]
    SomeError,
}

pub fn root_func() {
    info!("Calling root_func"); // Use external dep
                                // Removed: let _root = workspace_root();
    let _regex = Regex::new(r"^\d{4}$").unwrap(); // Use non-workspace dep
    let _s = RootStruct {
        field: TypeId::Synthetic(
            NodeId::generate_synthetic(
                uuid::Uuid::nil(),
                std::path::Path::new(""),
                &[],
                "dummy",
                ItemKind::Struct,
                None,
                None,
            )
            .uuid(),
        ),
    }; // Use workspace dep TypeId/NodeId
    debug!("Root func finished");
}

// === Items using CFG ===
#[cfg(feature = "feature_a")]
pub fn func_using_feature_a() -> String {
    cfg_mod_a::func_a()
}

#[cfg(not(feature = "feature_b"))]
pub fn func_without_feature_b() -> bool {
    true
}

// === Test function (optional, but good for verifying fixture compiles) ===
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_compilation_and_paths() {
        root_func();
        local_mod::local_func();
        inline_mod::inline_func();
        let _ = inline_mod::deep_reexport_inline();

        // Access via re-exports
        local_func();
        deep_func();
        log_debug_reexport!("Test re-export");
        // Removed: let _ = ws_root_reexport();
        renamed_deep_func();
        reexported_nested_mod::deep_func();
        item_in_actual_file();

        // Cfg checks (only one might compile depending on test features)
        #[cfg(feature = "feature_a")]
        {
            assert_eq!(func_using_feature_a(), "feature_a active");
            assert_eq!(cfg_mod_a::func_a(), "feature_a active");
        }
        #[cfg(feature = "feature_b")]
        {
            // assert!(func_without_feature_b()); // This line wouldn't compile if feature_b is active
            assert_eq!(cfg_mod_b::func_b(), "feature_b active");
        }
        #[cfg(not(feature = "feature_b"))]
        {
            assert!(func_without_feature_b());
        }
    }
}
