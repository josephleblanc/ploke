//! Test fixture for TypeId conflation.
//!
//! This fixture defines various items (structs, functions, traits, impls)
//! using generic parameters (often named `T`) and the `Self` type across
//! different scopes (top-level, modules, impls, different files).
//!
//! The goal is to create scenarios where `TypeId` generation *without* proper
//! contextual scoping (like `parent_scope_id`) would lead to identical TypeIds
//! for types that should be distinct (e.g., `T` defined in `StructA<T>` vs. `T`
//! defined in `fn func_b<T>`).
//!
//! Tests using this fixture should assert that the TypeIds generated for these
//! potentially conflated types are indeed *different* when contextual scoping
//! is correctly implemented in `TypeId::generate_synthetic`.
//!
//! TypeId fields tested by this fixture:
//! - FunctionNode.return_type
//! - ParamData.type_id
//! - FieldNode.type_id (in StructNode, EnumNode, UnionNode)
//! - TypeAliasNode.type_id
//! - ImplNode.self_type
//! - ImplNode.trait_type
//! - TraitNode.super_traits
//! - ValueNode.type_id (const/static)
//! - GenericParamKind::Type.bounds
//! - GenericParamKind::Type.default
//! - GenericParamKind::Const.type_id
//! - TypeNode.related_types (implicitly via generics, tuples, etc.)
//! - RelationKind (implicitly via structure)
//!
//! **Note:** This fixture aims for stable Rust compatibility. The `TraitWithSelfAlias`
//! example, which used `type AliasOfSelf = Self;`, was removed because it required
//! the unstable `associated_type_defaults` feature.

// Removed #![feature(associated_type_defaults)] as the unstable feature usage was removed.

#![allow(dead_code, unused_variables, unused_lifetimes)]

use std::fmt::{Debug, Display}; // Added Display import

// Include the contents of other_mod.rs as a module
mod other_mod;

// --- Top Level Definitions ---

// 1. Test FieldNode.type_id (generic T)
//    Test GenericParamKind::Type.bounds
#[derive(Debug)] // Added to satisfy trait bounds requiring Self: Debug
pub struct TopLevelStruct<T: Debug> {
    field: T,
}

// 2. Test ParamData.type_id (generic T)
//    Test FunctionNode.return_type (generic T)
//    Test GenericParamKind::Type.bounds
pub fn top_level_func<T: Clone>(param: T) -> T {
    param.clone()
}

// 3. Test ImplNode.self_type (generic T)
//    Test FunctionNode.return_type (Self)
//    Test ParamData.type_id (generic T)
impl<T: Debug> TopLevelStruct<T> {
    pub fn method(&self, input: T) -> Self {
        // Self here refers to TopLevelStruct<T>
        println!("TopLevelStruct::method: {:?}", input);
        TopLevelStruct { field: input } // Simplified return
    }
}

// 4. Test TraitNode.super_traits
//    Test GenericParamKind::Type.bounds
pub trait TopLevelTrait<T: Default>: Debug {
    // 5. Test FunctionNode.return_type (Self associated type)
    type Associated;
    // 6. Test ParamData.type_id (Self)
    //    Test ParamData.type_id (generic T)
    fn trait_method(&self, input: T) -> Self::Associated;
}

// 7. Test ImplNode.trait_type (generic T)
//    Test ImplNode.self_type (concrete)
//    Test GenericParamKind::Type.bounds (combined)
impl<T: Default + Debug + Clone> TopLevelTrait<T> for String {
    type Associated = T; // 8. Test TypeAliasNode.type_id (implicitly, for Associated = T)
    fn trait_method(&self, input: T) -> Self::Associated {
        // Self here refers to String
        println!("String::trait_method: {}", self);
        input // Return the generic type T
    }
}

// 9. Test ValueNode.type_id (concrete)
pub const TOP_CONST: i32 = 10;

// 10. Test ValueNode.type_id (generic T, via associated const)
//     Test GenericParamKind::Type.default
pub trait TraitWithConst<T = i32> {
    const ASSOC_CONST: T;
}

// 11. Test TypeAliasNode.type_id (generic T)
pub type TopLevelAlias<T> = Vec<T>;

// --- Nested Module Definitions ---

mod inner_mod {
    use super::TopLevelTrait; // Import trait for use
    use std::fmt::Display;

    // 12. Test FieldNode.type_id (generic T) - Should be different from TopLevelStruct's T
    //     Test GenericParamKind::Type.bounds
    #[derive(Debug)] // Added to satisfy trait bounds requiring Self: Debug
    pub struct InnerStruct<T: Display> {
        inner_field: T,
    }

    // 13. Test ParamData.type_id (generic T) - Should be different from top_level_func's T
    //     Test FunctionNode.return_type (generic T)
    //     Test GenericParamKind::Type.bounds
    pub fn inner_func<T: Send>(param: T) -> T {
        param
    }

    // 14. Test ImplNode.self_type (generic T) - Different T from TopLevelStruct<T>
    //     Test FunctionNode.return_type (Self) - Different Self from TopLevelStruct<T>::method
    //     Test ParamData.type_id (generic T)
    impl<T: Display> InnerStruct<T> {
        pub fn inner_method(&self, input: T) -> Self {
            // Self here refers to InnerStruct<T>
            println!("InnerStruct::inner_method: {}", input);
            InnerStruct { inner_field: input }
        }
    }

    // 15. Test ImplNode.trait_type (generic T) - Different T from impl for String
    //     Test ImplNode.self_type (generic T)
    //     Test GenericParamKind::Type.bounds (combined)
    //     Added `+ std::fmt::Debug` to T because InnerStruct derives Debug, which requires T: Debug,
    //     and TopLevelTrait requires Self: Debug (which InnerStruct satisfies via derive).
    //     Using full path `std::fmt::Debug` to avoid ambiguity with the derive macro.
    impl<T: Default + Display + Clone + std::fmt::Debug> TopLevelTrait<T> for InnerStruct<T> {
        type Associated = Vec<T>; // 16. Test TypeAliasNode.type_id (implicitly, for Associated = Vec<T>)
        fn trait_method(&self, input: T) -> Self::Associated {
            // Self here refers to InnerStruct<T>
            println!("InnerStruct::trait_method: {}", self.inner_field);
            vec![input]
        }
    }

    // 17. Test ValueNode.type_id (concrete) - Same name as top-level, different scope
    pub const INNER_CONST: i32 = 20;

    // 18. Test TypeAliasNode.type_id (generic T) - Different T from TopLevelAlias
    pub type InnerAlias<T> = Option<T>;

    // 19. Test GenericParamKind::Const.type_id
    pub struct InnerConstGeneric<const N: usize> {
        _data: [u8; N],
    }
}

// --- Using items from the other file ---

// 20. Test FieldNode.type_id (generic T from other_mod::OtherFileStruct)
//     Ensures file path context is used for TypeId generation.
//     Added `+ Debug` to T because OtherFileStruct requires it.
pub struct UsesOtherFile<T: Sync + Debug> {
    other_struct: other_mod::OtherFileStruct<T>,
}

// 21. Test ParamData.type_id (generic T from other_mod::other_file_func)
//     Test FunctionNode.return_type (generic T from other_mod::other_file_func)
//     Added `+ Clone` to T because other_file_func requires it.
pub fn calls_other_file_func<T: Send + Sync + Clone>(param: T) -> T {
    other_mod::other_file_func(param)
}

// 22. Test ImplNode.self_type (using type from other module)
//     Added `+ Debug` to T because the return type OtherFileStruct requires it.
impl<T: Sync + Debug> UsesOtherFile<T> {
    pub fn get_other(&self) -> &other_mod::OtherFileStruct<T> {
        &self.other_struct
    }
}

// 23. Test ImplNode.trait_type (using trait from other module)
//     Test ImplNode.self_type (using type from this module)
impl<T: Default + Sync + Debug> other_mod::OtherFileTrait<T> for TopLevelStruct<T> {
    type OtherAssociated = T;
    fn other_trait_method(&self, input: T) -> Self::OtherAssociated {
        input
    }
}

// --- Edge Cases ---

// Struct and function with the same name but different kinds
// (NodeId generation handles this via ItemKind, TypeId shouldn't be involved directly)
pub struct NameCollision;
pub fn name_collision() {}

// Nested generics
// 24. Test FieldNode.type_id (nested generic T, U)
//     Test TypeNode.related_types implicitly
//     Added `T: Debug` because TopLevelStruct requires it.
//     Added `U: Display` because InnerStruct requires it.
pub struct NestedGeneric<T: Debug, U: Display> {
    nested: TopLevelStruct<T>,
    other_nested: inner_mod::InnerStruct<U>,
}

// Removed TraitWithSelfAlias and its impl as it required an unstable feature
// (associated_type_defaults) which caused issues with `cargo clippy` on stable.
// This specific pattern wasn't essential for the primary goal of testing T/Self conflation.
