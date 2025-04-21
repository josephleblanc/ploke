//! Fixture crate designed to test various path resolution scenarios,
//! including modules, imports, re-exports, #[path], #[cfg], and dependencies.

// === Dependencies ===
use log::{debug, info}; // External dep (workspace = true)
use ploke_core::{IdTrait, ItemKind, NodeId, TypeId}; // Import IdTrait and ItemKind
use regex::Regex; // External dep (non-workspace)
use serde::Serialize; // External dep with feature
use std::fmt::Debug; // For generic bounds

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

// 6a. Crate-visible module
pub(crate) mod crate_mod {
    pub fn crate_internal_func() {}
}

// 6b. Module with restricted visibility
pub mod restricted_vis_mod {
    // Visible only within restricted_vis_mod and its descendants
    pub(in crate::restricted_vis_mod) fn restricted_func() {}

    pub mod inner {
        // Can access restricted_func because it's a descendant
        pub fn call_restricted() {
            super::restricted_func();
        }
    }
}

// 6c. Module using #[path] to point outside src/
#[path = "../common_file.rs"]
pub mod common_import_mod;

// === Imports (Private) ===
// Adding some private imports for completeness
#[allow(unused_imports)]
use crate::local_mod::nested as PrivateNestedAlias;
#[allow(unused_imports)]
use regex::Regex as PrivateRegexAlias;

// === Imports/Re-exports with CFG ===

#[cfg(feature = "feature_a")]
use crate::local_mod::func_using_dep as aliased_func_a;

#[cfg(not(feature = "feature_a"))]
use crate::inline_mod::inline_func as aliased_func_not_a;

#[cfg(feature = "feature_b")]
pub use crate::local_mod::local_func as pub_aliased_func_b;

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

// 14. Re-export a generic struct
pub use self::generics::GenStruct as PublicGenStruct;

// 15. Re-export a generic trait
pub use self::generics::GenTrait as PublicGenTrait;

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

// === Generics / Self / Traits ===
pub mod generics {
    use super::{IdTrait, NodeId, TypeId}; // Import necessary core types
    use std::fmt::Debug;

    // Generic Struct
    #[derive(Debug, Clone)]
    pub struct GenStruct<T: Debug + Clone> {
        pub data: T,
        id: NodeId, // Example usage
    }

    impl<T: Debug + Clone> GenStruct<T> {
        pub fn new(data: T) -> Self {
            Self {
                data,
                // Example ID generation - replace with actual logic if needed for tests
                id: NodeId::Synthetic(uuid::Uuid::new_v4()),
            }
        }

        // Method using Self
        pub fn get_id(&self) -> NodeId {
            self.id
        }

        // Method using generic type T
        pub fn process(&self) -> String {
            format!("Processed: {:?}", self.data)
        }
    }

    // Generic Trait with Associated Type
    pub trait GenTrait<Input> {
        type Output: Debug; // Associated Type

        fn transform(&self, input: Input) -> Self::Output;

        // Generic method within trait
        fn describe<D: Debug>(&self, detail: D) -> String;
    }

    // Implement trait for generic struct
    impl<T: Debug + Clone + Default + 'static> GenTrait<T> for GenStruct<T> {
        type Output = (T, T); // Example associated type implementation

        fn transform(&self, input: T) -> Self::Output {
            (self.data.clone(), input)
        }

        fn describe<D: Debug>(&self, detail: D) -> String {
            format!("GenStruct describing {:?} with detail {:?}", self.data, detail)
        }
    }

    // Generic Function
    pub fn gen_func<T: Debug>(param: T) {
        debug!("Generic function called with {:?}", param);
    }
}

// === Macro Definition ===
#[macro_export] // Export macro to make it usable externally if needed
macro_rules! simple_macro {
    () => {
        println!("Simple macro invoked!");
    };
    ($e:expr) => {
        println!("Simple macro invoked with: {}", $e);
    };
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
        let _gs: PublicGenStruct<i32> = PublicGenStruct::new(42); // Use re-exported generic
        let _ = common_import_mod::function_in_common_file(); // Use item from external #[path]

        // Call macro
        simple_macro!();
        simple_macro!("test");

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
