//! Serialization trait definitions
//!
//! This module defines the core serialization traits similar to serde::ser.

/// A data structure that can be serialized into any data format supported
/// by serde.
///
/// This trait is automatically implemented by the `#[derive(Serialize)]` macro.
pub trait Serialize {
    /// Serialize this value into the given Serde serializer.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer;
}

/// A data format that can serialize any data structure supported by Serde.
///
/// This trait represents the serializer side of the serialization process.
pub trait Serializer: Sized {
    /// The output type produced by this serializer during successful serialization.
    type Ok;

    /// The error type that can be produced during serialization.
    type Error: Error;

    /// Type returned from `serialize_seq` for serializing the content of the sequence.
    type SerializeSeq: SerializeSeq<Ok = Self::Ok, Error = Self::Error>;

    /// Type returned from `serialize_map` for serializing the content of the map.
    type SerializeMap: SerializeMap<Ok = Self::Ok, Error = Self::Error>;

    /// Serialize a bool value.
    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error>;

    /// Serialize an i8 value.
    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error>;

    /// Serialize an i16 value.
    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error>;

    /// Serialize an i32 value.
    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error>;

    /// Serialize an i64 value.
    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error>;

    /// Serialize a u8 value.
    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error>;

    /// Serialize a u16 value.
    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error>;

    /// Serialize a u32 value.
    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error>;

    /// Serialize a u64 value.
    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error>;

    /// Serialize an f32 value.
    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error>;

    /// Serialize an f64 value.
    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error>;

    /// Serialize a char value.
    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error>;

    /// Serialize a string slice.
    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error>;

    /// Serialize a byte slice.
    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok, Self::Error>;

    /// Serialize a None value.
    fn serialize_none(self) -> Result<Self::Ok, Self::Error>;

    /// Serialize a Some(T) value.
    fn serialize_some<T: ?Sized>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize;

    /// Serialize a unit value.
    fn serialize_unit(self) -> Result<Self::Ok, Self::Error>;

    /// Begin to serialize a sequence.
    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error>;

    /// Begin to serialize a map.
    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error>;
}

/// Trait returned from `Serializer::serialize_seq`.
pub trait SerializeSeq {
    /// Must match the `Ok` type of our serializer.
    type Ok;

    /// Must match the `Error` type of our serializer.
    type Error: Error;

    /// Serialize a sequence element.
    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize;

    /// Finish serializing the sequence.
    fn end(self) -> Result<Self::Ok, Self::Error>;
}

/// Trait returned from `Serializer::serialize_map`.
pub trait SerializeMap {
    /// Must match the `Ok` type of our serializer.
    type Ok;

    /// Must match the `Error` type of our serializer.
    type Error: Error;

    /// Serialize a map key.
    fn serialize_key<T: ?Sized>(&mut self, key: &T) -> Result<(), Self::Error>
    where
        T: Serialize;

    /// Serialize a map value.
    fn serialize_value<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize;

    /// Finish serializing the map.
    fn end(self) -> Result<Self::Ok, Self::Error>;
}

/// Trait representing errors during serialization.
pub trait Error: core::fmt::Display {
    /// Creates a new error with a custom message.
    fn custom<T: core::fmt::Display>(msg: T) -> Self;
}
