//! Extreme edge cases for use statement parsing
//! Should not contain any invalid code.

// Deeply nested
use a::b::c::d::e::f;

// Multiple renames
use x::y as z;

// Empty segments
use self::self::module;

// UTF-8 paths
use 模块::子模块 as 类型;

// Raw identifiers
use r#mod::r#type as r#var;
