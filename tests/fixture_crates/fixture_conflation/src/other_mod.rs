//! Contains items similar to lib.rs but in a separate file
//! to test that file path context is correctly used in TypeId generation.

#![allow(dead_code, unused_variables)]

use std::fmt::Debug;

// 1a. Test FieldNode.type_id (generic T) - Should be different from lib.rs::TopLevelStruct's T
//     Test GenericParamKind::Type.bounds
pub struct OtherFileStruct<T: Debug> {
    field: T,
}

// 2a. Test ParamData.type_id (generic T) - Should be different from lib.rs::top_level_func's T
//     Test FunctionNode.return_type (generic T)
//     Test GenericParamKind::Type.bounds
pub fn other_file_func<T: Clone>(param: T) -> T {
    param.clone()
}

// 3a. Test ImplNode.self_type (generic T) - Different T and Self from lib.rs::TopLevelStruct
//     Test FunctionNode.return_type (Self)
//     Test ParamData.type_id (generic T)
impl<T: Debug> OtherFileStruct<T> {
    pub fn method(&self, input: T) -> Self {
        // Self here refers to OtherFileStruct<T>
        println!("OtherFileStruct::method: {:?}", input);
        OtherFileStruct { field: input }
    }
}

// 4a. Test TraitNode.super_traits - Different trait from lib.rs::TopLevelTrait
//     Test GenericParamKind::Type.bounds
pub trait OtherFileTrait<T: Default>: Debug {
    // 5a. Test FunctionNode.return_type (Self associated type)
    type OtherAssociated;
    // 6a. Test ParamData.type_id (Self)
    //     Test ParamData.type_id (generic T)
    fn other_trait_method(&self, input: T) -> Self::OtherAssociated;
}

// 7a. Test ImplNode.trait_type (generic T) - Different T and Trait from lib.rs impl
//     Test ImplNode.self_type (concrete)
//     Test GenericParamKind::Type.bounds (combined)
impl<T: Default + Debug + Clone> OtherFileTrait<T> for f64 {
    type OtherAssociated = T; // 8a. Test TypeAliasNode.type_id (implicitly, for Associated = T)
    fn other_trait_method(&self, input: T) -> Self::OtherAssociated {
        // Self here refers to f64
        println!("f64::other_trait_method: {}", self);
        input
    }
}

// 9a. Test ValueNode.type_id (concrete) - Same name, different file/scope
pub const TOP_CONST: i32 = 100;

// 11a. Test TypeAliasNode.type_id (generic T) - Different T and Alias from lib.rs
pub type OtherFileAlias<T> = std::collections::HashSet<T>;
