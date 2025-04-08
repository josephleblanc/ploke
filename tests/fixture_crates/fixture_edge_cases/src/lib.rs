#![allow(dead_code, unused_imports, unused_variables, unreachable_pub)]

// --- 1. Name Shadowing & Slight Variations ---

mod scope1 {
    pub struct Item { pub x: i32 }
    pub fn process() -> &'static str { "scope1::process" }
}

mod scope2 {
    pub struct Item { pub y: String } // Same name, different field
    pub fn process() -> &'static str { "scope2::process" }
}

// Slightly different names
pub struct Item { pub z: bool }
pub struct Item2 { pub z: bool } // Different name, same field

// --- 2. Generics Extravaganza ---

pub trait Processor<T> {
    type Output;
    fn process(&self, input: T) -> Self::Output;
}

pub struct GenericItem<'a, T: Default + Clone, const N: usize> {
    data: [T; N],
    _lifetime_marker: &'a (),
}

impl<'a, T: Default + Clone, const N: usize> GenericItem<'a, T, N> {
    // Inherent method with same name as trait method
    pub fn process(&self) -> T {
        self.data[0].clone()
    }

    pub fn new(marker: &'a ()) -> Self {
        Self {
            data: [T::default(); N],
            _lifetime_marker: marker,
        }
    }
}

impl<'a, T: Default + Clone + std::fmt::Debug, const N: usize> Processor<T> for GenericItem<'a, T, N>
where T: Send // Extra bound on impl
{
    type Output = String;
    // Trait method implementation with same name as inherent method
    fn process(&self, input: T) -> Self::Output {
        format!("Processed: {:?}, Data: {:?}", input, self.data[0])
    }
}

// --- 3. Super Traits ---

pub trait BaseTrait {
    fn base_method(&self);
}

pub trait DerivedTrait: BaseTrait + Send + Sync { // Inherits BaseTrait + marker traits
    fn derived_method(&self);
}

pub struct TraitImplStruct;

impl BaseTrait for TraitImplStruct {
    fn base_method(&self) {}
}

impl DerivedTrait for TraitImplStruct {
    fn derived_method(&self) {}
}

// --- 4. Complex Use / Exports / Visibility ---

mod internal {
    pub mod utils {
        pub struct Helper;
        impl Helper {
            pub fn help(&self) {}
        }
        pub(crate) fn internal_helper() {} // Crate visible
        pub(super) fn super_helper() {} // Module visible to parent (internal)
    }

    mod restricted {
        pub struct RestrictedItem;
        // Visible only within crate::internal module and its descendants
        pub(in crate::internal) fn restricted_func() {}
    }

    // Re-export with alias
    pub use utils::Helper as UtilityHelper;
    // Re-export specific item
    pub use restricted::RestrictedItem;

    fn test_visibility() {
        utils::internal_helper(); // OK
        utils::super_helper();    // OK
        restricted::restricted_func(); // OK
    }
}

// Use absolute path
use crate::internal::utils::Helper;
// Use re-exported alias
use crate::internal::UtilityHelper;
// Use glob import
use crate::scope1::*;
// Use alias for specific item
use crate::scope2::Item as ItemScope2;
// Use relative path with super
use self::internal::RestrictedItem;

// Use self to refer to current module's items (less common at top level)
use self::Item as RootItem;

pub fn use_imports() {
    let h = Helper;
    h.help();
    let uh = UtilityHelper;
    uh.help();
    let i1 = scope1::Item { x: 1 }; // scope1::Item is usable via glob
    let i2 = ItemScope2 { y: "hello".to_string() };
    let ri = RestrictedItem;
    let root_item = RootItem { z: true };

    // internal::utils::internal_helper(); // Error: private
    // internal::utils::super_helper(); // Error: private
    // internal::restricted::restricted_func(); // Error: private
}

// --- 5. Suspected Breakages / Edge Cases ---

// Macro defining an item (Parser might not see 'macro_generated_struct')
macro_rules! define_struct {
    ($name:ident) => {
        pub struct $name { pub val: i32 }
    };
}
define_struct!(MacroGeneratedStruct);

// Complex attributes
#[cfg_attr(feature = "some_feature", derive(Debug))]
#[outer_attr::group(#[inner_attr = "value"])]
pub struct ComplexAttributes {
    #[field_attr(arg = true)]
    field: i32,
}

// Type alias involving complex path/generics
pub type ComplexAlias<T> = Result<Vec<crate::internal::utils::Helper>, T>;

// Impl block with complex bounds
pub trait AnotherBase<T> {}
impl<T> AnotherBase<T> for GenericItem<'_, T, 5> where T: Default + Clone + Send + 'static {}

// Function with complex pattern matching in args (might affect ParamData extraction)
pub fn complex_args((x, y): (i32, i32), GenericItem { data, .. }: GenericItem<String, 3>) {}

// Extern crate with alias
// extern crate std as rust_std; // extern crate is Rust 2015, less common now

// Raw identifiers
pub fn r#match(r#in: i32) -> i32 {
    r#in
}
pub struct r#Struct { pub r#field: i32 }

// Empty enum
pub enum EmptyEnum {}

// Struct with no fields (Unit struct) - already covered somewhat, but ensure tracking hash works
pub struct UnitStruct;

// Function returning impl Trait with complex bounds
pub fn complex_impl_return() -> impl BaseTrait + DerivedTrait + Send {
    TraitImplStruct
}
