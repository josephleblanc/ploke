
#![allow(unused_imports)]
#![allow(clippy::single_component_path_imports)] // Allow `use crate;`

//! Test fixture for parsing import (`use` and `extern crate`) statements.

// --- Basic Imports ---
use std::collections::HashMap; // Simple path
use std::fmt;                  // Importing a module

// --- Renaming ---
use std::io::Result as IoResult; // Simple rename
use crate::structs::SimpleStruct as MySimpleStruct; // Rename local item

// --- Grouped Imports ---
use std::{
    fs::{self, File}, // Module and item from same subpath
    path::{Path, PathBuf}, // Multiple items from same subpath
};
use crate::{
    enums::{EnumWithData, SampleEnum1}, // Multiple local items
    traits::{DefaultTrait, GenericTrait as MyGenTrait}, // Local items with rename
};

// --- Glob Imports ---
use std::env::*; // Glob import

// --- Relative Path Imports ---
use self::sub_imports::SubItem; // `self` import
use super::structs::AttributedStruct; // `super` import
use crate::type_alias::MyId; // `crate` import

// --- Absolute Path Import ---
// Note: `::` prefix is handled by `syn`'s `ItemUse.leading_colon`
use ::std::time::Duration;

// --- Extern Crate ---
extern crate serde; // Basic extern crate
extern crate serde as SerdeAlias; // Renamed extern crate

// --- Nested Module Imports ---
mod sub_imports {
    // Import from parent module
    use super::fmt;
    // Import from grandparent module (crate root)
    use crate::enums::DocumentedEnum;
    // Import from std
    use std::sync::Arc;
    // Import using self
    use self::nested_sub::NestedItem;
    // Import using super
    use super::super::structs::TupleStruct; // Goes up two levels

    pub struct SubItem;

    mod nested_sub {
        pub struct NestedItem;
    }
}

// --- Items used by imports to ensure fixture compiles ---
pub fn use_imported_items() {
    let _map = HashMap::<String, i32>::new();
    let _fmt_res: fmt::Result = Ok(());
    let _io_res: IoResult<()> = Ok(());
    let _local_struct = MySimpleStruct { x: 1 };
    let _fs_res = fs::read_to_string("dummy");
    let _file: File;
    let _path: &Path;
    let _path_buf = PathBuf::new();
    let _enum1 = SampleEnum1::Variant1;
    let _enum_data = EnumWithData::Variant1(1);
    struct DummyTraitUser;
    impl DefaultTrait for DummyTraitUser {}
    let _trait_user = DummyTraitUser;
    // MyGenTrait usage requires type annotation
    struct GenTraitImpl;
    impl<T> MyGenTrait<T> for GenTraitImpl {
        fn generic_method(&self, param: T) -> T {
            param
        }
    }
    let _gen_trait_user = GenTraitImpl;

    // Glob import usage (e.g., current_dir)
    let _cwd = current_dir();

    // Relative path usage
    let _sub_item = SubItem;
    let _super_item = AttributedStruct { field: true };
    let _crate_item: MyId = 123;

    // Absolute path usage
    let _duration = Duration::from_secs(1);

    // Extern crate usage (implicitly via types/macros if used)
    let _serde_val: serde::Value;
    let _serde_alias_val: SerdeAlias::Value;

    // Nested module usage
    let _arc = Arc::new(1);
    let _nested_item = sub_imports::nested_sub::NestedItem;
    let _tuple_struct = TupleStruct(1, 2);
}
