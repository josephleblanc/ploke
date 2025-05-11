#![allow(dead_code)]
// #![warn(unused_attributes)] // Example crate-level attribute
// NOTE: This crate contains attributes that cause the build of the workspace to fail.
// This means that we cannot keep this library within the test fixtures that are included in the
// cargo workspace for linting.
//
// WARNING: If there are issues with tests targeting this fixture, add the this crate to the cargo
// workspace to enable linting.

// Standard attributes
#[derive(Debug, Clone, Copy)] // derive attribute
pub struct StandardAttrs {
    #[allow(dead_code)] // allow attribute on field
    field: i32,
}

#[cfg(target_os = "linux")] // cfg attribute
fn linux_only_function() {}

#[cfg(not(target_os = "linux"))]
fn not_linux_function() {}

#[inline] // inline attribute
pub fn fast_function() {}

#[deprecated(since = "0.1.0", note = "Use new_function instead")] // deprecated attribute
pub fn old_function() {}

#[test] // test attribute
fn my_test() {
    assert!(true);
}

// Custom-looking attributes (syntax is valid, meaning depends on tools/macros)
// #[my_custom_tool::validate(strict)] // Commented out: Causes resolution error E0433
pub struct CustomAttrStruct {
    // #[my_custom_tool::field_marker] // Commented out: Causes resolution error E0433
    data: String,
}

// #[my_custom_attribute] // Commented out: Causes resolution error (cannot find attribute)
fn function_with_custom_attr() {}

// #[outer_attr(arg1, arg2 = "value")] // Commented out: Causes resolution error (cannot find attribute)
pub mod module_with_attrs {
    // #[inner_attr] // Commented out: Causes resolution error (cannot find attribute)
    pub fn inner_function() {}
}

// Attributes with values
#[repr(C)] // repr attribute with argument
pub struct ReprCStruct {
    a: i32,
    b: bool,
}

#[link(name = "mylib", kind = "static")] // link attribute with key-value pairs
extern "C" {}
