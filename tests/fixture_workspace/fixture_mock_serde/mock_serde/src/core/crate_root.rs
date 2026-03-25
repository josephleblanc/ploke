//! Core crate root definitions for mock_serde
//!
//! This module demonstrates the pattern used in real serde for setting up
//! a facade around std/core/alloc types.

/// Macro that defines the core module structure
macro_rules! crate_root {
    () => {
        /// A facade around all the types we need from std, core, and alloc
        mod lib {
            pub mod core {
                #[cfg(not(feature = "std"))]
                pub use core::*;
                #[cfg(feature = "std")]
                pub use std::*;
            }

            // Core re-exports
            pub use self::core::fmt::{Debug, Display};
            pub use self::core::marker::PhantomData;
            pub use self::core::result::Result;

            // String/Vec re-exports based on feature flags
            #[cfg(all(feature = "alloc", not(feature = "std")))]
            pub use alloc::string::String;
            #[cfg(feature = "std")]
            pub use std::string::String;

            #[cfg(all(feature = "alloc", not(feature = "std")))]
            pub use alloc::vec::Vec;
            #[cfg(feature = "std")]
            pub use std::vec::Vec;
        }

        /// Simplified try macro for error handling
        macro_rules! tri {
            ($expr:expr) => {
                match $expr {
                    Ok(val) => val,
                    Err(err) => return Err(err),
                }
            };
        }

        // Re-export core traits from mock_serde_core
        pub use mock_serde_core::ser::{Serialize, Serializer, SerializeSeq, SerializeMap};
        pub use mock_serde_core::de::{Deserialize, Deserializer, Visitor};
        
        // Re-export the macro from mock_serde_core
        pub use mock_serde_core::forward_to_deserialize_any;
    };
}
