//! Deserialization trait definitions
//!
//! This module defines the core deserialization traits similar to serde::de.

/// A data structure that can be deserialized from any data format supported
/// by serde.
///
/// This trait is automatically implemented by the `#[derive(Deserialize)]` macro.
pub trait Deserialize<'de>: Sized {
    /// Deserialize this value from the given Serde deserializer.
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>;
}

/// A data format that can deserialize any data structure supported by Serde.
///
/// This trait represents the deserializer side of the deserialization process.
pub trait Deserializer<'de>: Sized {
    /// The error type that can be produced during deserialization.
    type Error: Error;

    /// Deserialize any type.
    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>;

    /// Deserialize a bool.
    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>;

    /// Deserialize an i8.
    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>;

    /// Deserialize an i32.
    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>;

    /// Deserialize an i64.
    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>;

    /// Deserialize a u8.
    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>;

    /// Deserialize a u32.
    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>;

    /// Deserialize a u64.
    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>;

    /// Deserialize an f32.
    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>;

    /// Deserialize an f64.
    fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>;

    /// Deserialize a string.
    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>;

    /// Deserialize an option.
    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>;

    /// Deserialize a unit.
    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>;

    /// Begin deserializing a sequence.
    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>;

    /// Begin deserializing a map.
    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>;
}

/// Trait for visiting a type during deserialization.
///
/// This trait represents the visitor pattern for deserialization. Implementors
/// define how to construct their type from various input types.
pub trait Visitor<'de>: Sized {
    /// The value produced by this visitor.
    type Value;

    /// Format a message stating what data this visitor expects to receive.
    fn expecting(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result;

    /// The input contains a boolean.
    fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
    where
        E: Error,
    {
        let _ = v;
        Err(Error::custom("unexpected bool"))
    }

    /// The input contains an i64.
    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
    where
        E: Error,
    {
        let _ = v;
        Err(Error::custom("unexpected integer"))
    }

    /// The input contains a u64.
    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
    where
        E: Error,
    {
        let _ = v;
        Err(Error::custom("unexpected unsigned integer"))
    }

    /// The input contains an f64.
    fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
    where
        E: Error,
    {
        let _ = v;
        Err(Error::custom("unexpected float"))
    }

    /// The input contains a string.
    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: Error,
    {
        let _ = v;
        Err(Error::custom("unexpected string"))
    }

    /// The input contains a unit.
    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: Error,
    {
        Err(Error::custom("unexpected unit"))
    }
}

/// Trait representing errors during deserialization.
pub trait Error: core::fmt::Display {
    /// Creates a new error with a custom message.
    fn custom<T: core::fmt::Display>(msg: T) -> Self;
}

/// Simple error implementation for testing
#[derive(Debug)]
pub struct SimpleError {
    msg: &'static str,
}

impl core::fmt::Display for SimpleError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.msg)
    }
}

impl Error for SimpleError {
    fn custom<T: core::fmt::Display>(msg: T) -> Self {
        let _ = msg;
        SimpleError { msg: "custom error" }
    }
}
