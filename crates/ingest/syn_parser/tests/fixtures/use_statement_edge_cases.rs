//! Extreme edge cases for use statement parsing

// Deeply nested
use a::b::c::d::e::f;

// Multiple renames
use x::y as z;

// Empty segments
use ::::module;

// UTF-8 paths
use 模块::子模块 as 类型;

// Raw identifiers
use r#mod::r#type as r#var;
