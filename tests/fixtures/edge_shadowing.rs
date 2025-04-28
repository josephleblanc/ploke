//! Fixture for testing edge cases of name shadowing in Rust
//!
//! Covers scenarios where identical names appear in different scopes,
//! testing the parser's ability to correctly track visibility and resolve symbols.
//! Each test case is documented with the Rust Reference section it validates.

/// Case 1: Basic module hierarchy shadowing
/// Tests §3.3 (Name Resolution) - Simple shadowing in nested modules
mod basic_hierarchy {
    pub struct Shadowed;
    pub fn shadowed_fn() -> &'static str { "root" }
    
    pub mod nested {
        pub struct Shadowed;  // Shadows parent's struct
        pub fn shadowed_fn() -> &'static str { "nested" }
        
        pub mod inner {
            pub fn shadowed_fn() -> &'static str { "inner" }
        }
    }
}

/// Case 2: Re-export shadowing
/// Tests §3.3.1 (Use Declarations) - Re-exports creating shadowing
mod reexport_shadow {
    pub mod inner {
        pub fn shadowed() -> &'static str { "inner" }
    }
    pub fn shadowed() -> &'static str { "outer" }
    pub use inner::shadowed;  // Creates shadow of outer function
}

/// Case 3: pub(in path) restricted visibility
/// Tests RFC #3052 (scoped visibility) - Shadowing through restricted visibility
mod restricted_visibility {
    pub(in crate::restricted_visibility) struct Shadowed;
    pub struct Shadowed;  // Allowed since previous is scope-restricted
    
    pub mod nested {
        pub(in crate::restricted_visibility) fn shadowed_fn() -> &'static str { "restricted" }
        pub fn shadowed_fn() -> &'static str { "unrestricted" }
    }
}

/// Case 4: Comprehensive Generic Parameter Shadowing
/// Tests §17.1 (Generic Parameters) - Various shadowing scenarios with generics
mod generic_shadow {
    /// Base struct with generic parameter
    pub struct Outer<T: Clone>(pub T);
    
    /// Shadowing in impl block methods
    impl<T: Clone> Outer<T> {
        /// Shadows outer T with new generic parameter
        pub fn new<U: Clone + 'static>(t: U) -> Outer<U> {
            Outer(t)
        }
        
        /// Method with different bounds shadowing outer T
        pub fn convert<U>(&self) -> U 
        where
            T: Into<U>,
            U: Default,
        {
            self.0.clone().into()
        }
    }

    /// Shadowing in trait implementations
    pub trait Processor<T> {
        fn process(&self, input: T) -> T;
    }

    impl<T: Clone> Processor<T> for Outer<T> {
        fn process(&self, input: T) -> T {
            self.0.clone()
        }
    }

    /// Shadowing in function with complex bounds
    pub fn process_input<U, V>(input: U) -> V
    where
        U: Clone + Into<V>,
        V: Default + std::fmt::Debug,
    {
        input.into()
    }

    /// Shadowing with dynamic dispatch
    pub fn dynamic_dispatch<T: 'static + Clone + std::fmt::Debug>(item: Box<dyn std::any::Any>) -> T {
        *item.downcast::<T>().unwrap()
    }

    /// Nested generic shadowing
    pub struct Wrapper<A>(pub A);
    
    impl<A: Clone> Wrapper<A> {
        pub fn wrap<B: 'static>(&self, b: B) -> (A, B) {
            (self.0.clone(), b)
        }
    }

    /// Shadowing with associated types
    pub trait Transform {
        type Output;
        fn transform(&self) -> Self::Output;
    }

    impl<T: Clone> Transform for Outer<T> {
        type Output = T;
        fn transform(&self) -> T {
            self.0.clone()
        }
    }

    /// Shadowing in where clauses
    pub fn complex_shadow<X, Y>(x: X, y: Y) -> (X, Y)
    where
        X: Clone,
        Y: Clone + PartialEq<X>,
    {
        (x.clone(), y.clone())
    }

    /// Shadowing with lifetime parameters
    pub fn lifetime_shadow<'a, 'b: 'a>(s: &'b str) -> &'a str {
        s
    }
}

/// Case 5: Built-in Type Shadowing  
/// Tests §3.3 (Name Resolution) - Shadowing primitive types
mod primitive_shadow {
    /// Shadows Rust's bool type (valid but discouraged in practice)
    pub type bool = u8;
    
    /// Shadows i32 primitive
    pub type i32 = f64;
}

/// Case 6: Macro-Generated Shadowing
/// Tests §19.6 (Macros) - Shadowing from macro expansions
mod macro_shadow {
    macro_rules! make_shadow {
        ($name:ident) => {
            pub fn $name() -> &'static str {
                stringify!($name)
            }
        };
    }
    
    make_shadow!(shadowed);  // First generation
    make_shadow!(shadowed);  // Shadowing allowed
    
    /// Explicit non-shadowed function for comparison
    pub fn non_shadowed() -> &'static str {
        "explicit"
    }
}

/// Case 7: Trait Method Shadowing
/// Tests §10.2 (Trait Items) - Method name shadowing between traits and impls
mod trait_method_shadow {
    pub trait Foo {
        fn name() -> &'static str;
    }
    
    pub struct Bar;
    
    impl Foo for Bar {
        fn name() -> &'static str { "Foo" }
    }
    
    impl Bar {
        /// Shadows trait method
        pub fn name() -> &'static str { "Bar" }
    }
}

/// Case 8: Enum Variant Shadowing  
/// Tests §8.1.3 (Enum Variants) - Shadowing across enum definitions
mod enum_variant_shadow {
    pub enum Status {
        Active,
        Inactive,
    }
    
    pub mod nested {
        pub enum Status {
            Active,  // Shadows parent variant
            Pending,
        }
    }
}

/// Case 9: Const/Static Shadowing
/// Tests §6.1 (Constants) - Shadowing of compile-time values
mod const_shadow {
    pub const VALUE: i32 = 42;
    pub mod inner {
        pub const VALUE: i32 = 24;  // Shadows parent
        pub static SHARED: i32 = VALUE;  // Uses shadowed value
    }
}

/// Case 10: Attribute Shadowing
/// Tests §15 (Attributes) - Shadowing with different attributes
mod attr_shadow {
    #[derive(Debug)]
    pub struct Base;
    
    #[derive(Clone)]
    pub struct Base;  // Shadows with different attributes
    
    pub fn shadowed() {}
    
    #[cfg(test)]
    pub fn shadowed() {}  // Conditional shadowing
}

/// Case 11: Pattern Matching Shadowing
/// Tests §18.2 (Patterns) - Shadowing in match arms
mod pattern_shadow {
    pub fn match_shadow(x: Option<i32>) {
        match x {
            Some(x) => {  // Shadows parameter
                let x = x + 1;  // Shadows again
                println!("{}", x);
            }
            None => {}
        }
    }
}

/// Case 12: Closure Parameter Shadowing
/// Tests §13.2 (Closures) - Shadowing in closure parameters
mod closure_shadow {
    pub fn test() {
        let x = 10;
        let closure = |x: i32| {  // Shadows outer x
            println!("Closure: {}", x);
            x * 2
        };
        println!("Outer: {}, Closure: {}", x, closure(20));
    }
}

/// Case 13: Macro Hygiene Shadowing
/// Tests §19.6 (Macros) - Hygiene in macro expansions
mod macro_hygiene {
    macro_rules! hygienic {
        ($x:ident) => {
            let $x = 42;
            println!("Macro: {}", $x);
        };
    }
    
    pub fn test() {
        let x = 10;
        hygienic!(x);  // Doesn't shadow outer x due to hygiene
        println!("Outer: {}", x);
    }
}

/// Case 14: Crate Root Shadowing
/// Tests §3.3 (Name Resolution) - Shadowing std library paths
mod crate_root_shadow {
    pub mod std {
        pub mod io {
            pub struct MockReader;
            pub fn mock_read() -> &'static str {
                "mock data"
            }
        }
    }
    
    pub fn test() -> &'static str {
        // Uses our shadowed std::io
        crate_root_shadow::std::io::mock_read()
    }
}

/// Case 15: Lifetime Shadowing in Impls
/// Tests §10.1 (Implementations) - Lifetime parameter shadowing
mod lifetime_impl_shadow {
    pub struct Wrapper<'a>(pub &'a str);
    
    impl<'b> Wrapper<'b> {  // Shadows 'a with 'b
        pub fn new(s: &'b str) -> Self {
            Wrapper(s)
        }
        
        pub fn get(&self) -> &'b str {
            self.0
        }
    }
}

/// Case 16: Feature-Gated Shadowing
/// Tests §7.1 (Attributes) - Conditional compilation shadowing
mod feature_shadow {
    #[cfg(feature = "alt")]
    pub fn special() -> &'static str { "alternative" }
    
    #[cfg(not(feature = "alt"))]
    pub fn special() -> &'static str { "default" }
    
    #[cfg(feature = "alt")]
    pub struct Config;
    
    #[cfg(not(feature = "alt"))]
    pub struct Config {  // Shadows alt version
        pub mode: &'static str,
    }
}
