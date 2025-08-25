use std::fmt::Debug;

// Basic public trait with a required method
pub trait SimpleTrait {
    fn required_method(&self) -> i32;
}

// Private trait (inherited visibility) with a default method
trait InternalTrait {
    fn default_method(&self) -> String {
        "Default implementation".to_string()
    }
}

// Crate-visible trait
pub(crate) trait CrateTrait {
    fn crate_method(&self);
}

/// Documented public trait
pub trait DocumentedTrait {
    /// Required method documentation
    fn documented_method(&self);
}

// Public trait with a generic type parameter
pub trait GenericTrait<T> {
    fn process(&self, item: T) -> T;
}

// Public trait with a generic lifetime parameter
pub trait LifetimeTrait<'a> {
    fn get_ref(&'a self) -> &'a str;
}

// Public trait with multiple generic parameters and bounds
pub trait ComplexGenericTrait<'a, T: Debug + Clone, S: Send + Sync>
where
    T: 'a,
{
    fn complex_process(&'a self, item: T, other: S) -> &'a T;
}

// Public trait with an associated type
pub trait AssocTypeTrait {
    type Output;
    fn generate(&self) -> Self::Output;
}

// Public trait with an associated type with bounds
pub trait AssocTypeWithBounds {
    type BoundedOutput: Debug + Clone;
    fn generate_bounded(&self) -> Self::BoundedOutput;
}

// Public trait with an associated constant
pub trait AssocConstTrait {
    const ID: u32;
    fn get_id(&self) -> u32 {
        Self::ID
    }
}

// Public trait inheriting from another trait (supertrait)
pub trait SuperTrait: SimpleTrait {
    fn super_method(&self);
}

// Public trait inheriting from multiple traits
pub trait MultiSuperTrait: SimpleTrait + InternalTrait + Debug {
    fn multi_super_method(&self);
}

// Public trait inheriting from a generic trait
pub trait GenericSuperTrait<T>: GenericTrait<T> {
    fn generic_super_method(&self, item: T);
}

// Public trait with an attribute
#[must_use = "Trait results should be used"]
pub trait AttributedTrait {
    fn calculate(&self) -> f64;
}

// Unsafe trait
pub unsafe trait UnsafeTrait {
    unsafe fn unsafe_method(&self);
}

// Auto trait (marker trait)
// pub auto trait MarkerAutoTrait {} // flagged by error checker as "experimental and buggy"
// ignore for now

// Module to test visibility interactions
mod inner {
    // Private trait inside private module
    trait InnerSecretTrait {
        fn secret_op(&self);
    }

    // Public trait inside private module
    pub trait InnerPublicTrait {
        fn public_inner_op(&self);
    }

    // Trait using super
    pub(super) trait SuperGraphNodeTrait: super::SimpleTrait {
        fn super_visible_op(&self);
    }
}

// Using an inner trait (effectively private)
impl dyn inner::InnerPublicTrait {} // Example usage to ensure it's referenced

// Trait with `Self` usage in method signature
pub trait SelfUsageTrait {
    fn returns_self(self) -> Self
    where
        Self: Sized;
    fn takes_self(&self, other: &Self);
}

// Trait with `Self` usage in associated type bound
pub trait SelfInAssocBound {
    type Related: SimpleTrait; // Related type must implement SimpleTrait
    fn get_related(&self) -> Self::Related;
}
