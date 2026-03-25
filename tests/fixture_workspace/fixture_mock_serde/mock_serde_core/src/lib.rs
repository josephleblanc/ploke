//! # Mock Serde Core
//!
//! The `mock_serde_core` crate contains core trait definitions with **no support
//! for #\[derive()\]**.
//!
//! In crates that derive an implementation of `Serialize` or `Deserialize`, you
//! must depend on the [`mock_serde`] crate, not `mock_serde_core`.
//!
//! [`mock_serde`]: https://docs.rs/mock_serde

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

// Public modules for serialization and deserialization
pub mod ser;
pub mod de;
pub mod private;

// Re-export core types from the modules
pub use ser::{Serialize, Serializer, SerializeSeq, SerializeMap};
pub use de::{Deserialize, Deserializer, Visitor};

/// Macro for forwarding to deserialize_any
#[macro_export]
macro_rules! forward_to_deserialize_any {
    ($($func:ident)*) => {
        $(
            #[inline]
            fn $func<V>(self, visitor: V) -> Result<V::Value, Self::Error>
            where
                V: $crate::de::Visitor<'de>,
            {
                self.deserialize_any(visitor)
            }
        )*
    };
}

/// Internal macro to prevent direct serde_core usage with derive
#[macro_export]
#[doc(hidden)]
macro_rules! __require_mock_serde_not_mock_serde_core {
    () => {
        ::core::compile_error!(
            "MockSerde derive requires a dependency on the mock_serde crate, not mock_serde_core"
        );
    };
}
