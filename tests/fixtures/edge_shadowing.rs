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
