#![allow(unused_macros)] // Allow unused macros for the fixture

//! Test fixture for parsing macro definitions (`macro_rules!` and procedural).

// --- Declarative Macros (`macro_rules!`) ---

/// A simple exported macro_rules macro.
#[macro_export]
macro_rules! exported_macro {
    () => {
        println!("Exported!")
    };
}

// A local (non-exported) macro_rules macro.
macro_rules! local_macro {
    ($x:expr) => {
        $x * 2
    };
}

/// A documented macro_rules macro.
#[macro_export]
macro_rules! documented_macro {
    ($($tts:tt)*) => {
        stringify!($($tts)*)
    };
}

/// A macro_rules macro with other attributes.
#[macro_export]
#[allow(clippy::empty_loop)]
macro_rules! attributed_macro {
    () => {
        loop {}
    };
}

// --- Procedural Macros ---
// Note: We only parse the function signature, not the actual proc macro logic.
// These require a proc-macro crate type, but we parse them syntactically here.

/// A function-like procedural macro signature.
#[proc_macro]
pub fn function_like_proc_macro(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    input // Dummy implementation
}

/// A derive procedural macro signature.
#[proc_macro_derive(MyTrait)]
pub fn derive_proc_macro(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    input // Dummy implementation
}

/// An attribute procedural macro signature.
#[proc_macro_attribute]
pub fn attribute_proc_macro(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    item // Dummy implementation
}

/// A documented procedural macro with attributes.
#[proc_macro_derive(AnotherTrait, attributes(helper))]
#[deprecated = "Use something else"]
pub fn documented_proc_macro(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    input // Dummy implementation
}

// --- Macros in Modules ---

mod inner_macros {
    /// An exported macro defined inside an inline module.
    #[macro_export]
    macro_rules! inner_exported_macro {
        () => {
            "Inner Exported"
        };
    }

    // A local macro defined inside an inline module.
    macro_rules! inner_local_macro {
        () => {
            "Inner Local"
        };
    }

    /// A proc macro defined inside an inline module.
    #[proc_macro]
    pub fn inner_proc_macro(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
        input
    }
}

// --- Usage (to avoid unused warnings if not exporting all) ---
pub fn use_macros() {
    exported_macro!();
    let _ = local_macro!(5);
    let _ = documented_macro!(a b c);
    attributed_macro!();

    // Note: Proc macros aren't "called" like declarative macros in regular code.
    // Their usage is implicit via derives/attributes.

    inner_macros::inner_exported_macro!();
    inner_macros::inner_local_macro!();
}
