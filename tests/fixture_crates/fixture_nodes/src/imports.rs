#![allow(unused_imports)]
#![allow(clippy::single_component_path_imports)] // Allow `use crate;`

//! Test fixture for parsing import (`use` and `extern crate`) statements.

// --- Basic Imports ---
use crate::structs::TupleStruct;
use std::collections::HashMap; // Simple path
use std::fmt;
use std::sync::Arc; // Importing a module

// --- Renaming ---
use crate::structs::SampleStruct as MySimpleStruct;
use std::io::Result as IoResult; // Simple rename // Rename local item

// --- Grouped Imports ---
use crate::{
    enums::{EnumWithData, SampleEnum1}, // Multiple local items
    traits::{GenericTrait as MyGenTrait, SimpleTrait}, // Local items with rename
};
use std::{
    fs::{self, File},      // Module and item from same subpath
    path::{Path, PathBuf}, // Multiple items from same subpath
};

// --- Glob Imports ---
use std::env::*; // Glob import

// --- Relative Path Imports ---
use self::sub_imports::SubItem; // `self` import
use super::structs::AttributedStruct; // `super` import
use crate::type_alias::SimpleId; // `crate` import

// --- Absolute Path Import ---
// Note: `::` prefix is handled by `syn`'s `ItemUse.leading_colon`
use ::std::time::Duration;

// --- Extern Crate ---
extern crate serde; // Basic extern crate
extern crate serde as SerdeAlias; // Renamed extern crate

// --- Nested Module Imports ---
pub mod sub_imports {
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

    pub mod nested_sub {
        pub struct NestedItem;
    }
}

// --- Items used by imports to ensure fixture compiles ---
pub fn use_imported_items() {
    let _map = HashMap::<String, i32>::new();
    let _fmt_res: fmt::Result = Ok(());
    let _io_res: IoResult<()> = Ok(());
    let _local_struct = MySimpleStruct {
        field: "example".to_string(),
    };
    let _fs_res = fs::read_to_string("dummy");
    let _file: File;
    let _path: &Path;
    let _path_buf = PathBuf::new();
    let _enum1 = SampleEnum1::Variant1;
    let _enum_data = EnumWithData::Variant1(1);
    struct DummyTraitUser;
    impl SimpleTrait for DummyTraitUser {
        fn required_method(&self) -> i32 {
            5
        }
    }
    let _trait_user = DummyTraitUser;
    // MyGenTrait usage requires type annotation
    struct GenTraitImpl;
    impl<T> MyGenTrait<T> for GenTraitImpl {
        fn process(&self, item: T) -> T {
            item
        }
    }
    let _gen_trait_user = GenTraitImpl;

    // Glob import usage (e.g., current_dir)
    let _cwd = current_dir();

    // Relative path usage
    let _sub_item = SubItem;
    let _super_item = AttributedStruct {
        field: "x".to_string(),
    };
    let _crate_item: SimpleId = 123;

    // Absolute path usage
    let _duration = Duration::from_secs(1);

    // Extern crate usage (implicitly via types/macros if used)
    // let _serde_val: serde::Value;
    // let _serde_alias_val: SerdeAlias::Value;

    // Nested module usage
    let _arc = Arc::new(1);
    let _nested_item = sub_imports::nested_sub::NestedItem;
    let _tuple_struct = TupleStruct(1, 2);
}
