// --- Existing Enums (used for basic tests) ---

/// A simple public enum with unit-like variants.
pub enum SampleEnum1 {
    Variant1,
    Variant2,
}

/// An enum demonstrating variants with unnamed (tuple) data.
pub enum EnumWithData {
    Variant1(i32),
    Variant2(String),
}

/// An enum with a doc comment.
/// This is a documented enum
pub enum DocumentedEnum {
    Variant1,
    Variant2,
}

/// An enum demonstrating struct-like variants.
pub enum SampleEnum {
    Variant1,
    Variant2 { value: i32 },
    Variant3,
}

// --- New Enums for Comprehensive Testing ---

// 1. Enum-Level Visibility
// Default (private) visibility
enum PrivateEnum {
    A,
}

// Crate-level visibility
pub(crate) enum CrateEnum {
    B,
}

// pub(in path) visibility would require a module structure, e.g.:
// mod outer_mod {
//     pub mod inner_mod {
//         pub(in crate::outer_mod) enum RestrictedInPathEnum { X }
//     }
//     pub enum TestOuter { Y }
// }


// 2. Enum-Level Generic Parameters
/// Enum with lifetime, type, and const generic parameters, and a where clause.
#[derive(Debug, Clone)] // Attribute for testing
pub enum GenericEnum<'a, T: Default + Clone, const N: usize>
where
    T: Send, // Where clause
{
    /// Doc comment on a generic variant.
    GenericVariant(T),
    LifetimeVariant(&'a str),
    ConstGenericVariant([u8; N]),
    #[cfg(feature = "enum_feature_one")] // CFG on a variant
    ConditionalGeneric(Option<T>),
}

// 3. Enum-Level Attributes (including repr) and Variant Discriminants
/// Enum with `#[repr]` attribute and explicit discriminants.
#[repr(u16)]
pub enum EnumWithAttributesAndDiscriminants {
    Up = 1,
    #[allow(dead_code)] // Attribute on a variant
    Down, // Implicitly 2
    Left = 10,
    /// Doc comment on a variant with an implicit discriminant.
    Right, // Implicitly 11
}

// 4. Enum-Level CFG (and variant-level CFGs)
/// Enum that is conditionally compiled.
#[cfg(feature = "enum_main_feature")]
pub enum CfgEnum {
    AlwaysPresent,
    #[cfg(feature = "enum_variant_feature")]
    SometimesPresent,
}

// 5. Mixed Variant Kinds (more complex fields)
/// Enum with various variant kinds for detailed field parsing (though not fully checked by ExpectedEnumNode yet).
pub enum EnumWithMixedVariants {
    Simple,
    TupleMulti(i32, String, bool),
    StructMulti {
        id: u32,
        #[cfg(feature = "enum_field_feature")]
        name: String,
        active: bool,
    },
    /// Doc on a unit variant within a mixed enum.
    UnitWithDoc,
}

// Helper function to ensure features are linked if tests depend on them
#[cfg(test)]
fn ensure_features() {
    #[cfg(feature = "enum_feature_one")]
    let _ = "enum_feature_one_active";
    #[cfg(feature = "enum_main_feature")]
    let _ = "enum_main_feature_active";
    #[cfg(feature = "enum_variant_feature")]
    let _ = "enum_variant_feature_active";
    #[cfg(feature = "enum_field_feature")]
    let _ = "enum_field_feature_active";
}

// --- New Enums for Isolated Generic Property Testing ---

/// Enum solely for testing type generics.
pub enum JustTypeGeneric<A, B> {
    VariantA(A),
    VariantB(B),
}

/// Enum solely for testing lifetime generics.
pub enum JustLifetimeGeneric<'x, 'y> {
    VariantX(&'x i32),
    VariantY(&'y str),
}

/// Enum solely for testing const generics.
pub enum JustConstGeneric<const X: usize, const Y: usize> {
    VariantX([u8; X]),
    VariantY([u16; Y]),
}

/// Enum solely for testing a simple where clause.
pub enum JustWhereClause<T>
where
    T: Copy,
{
    Data(T),
}

// --- New Enums for Isolated Complex Field Testing ---

/// Enum solely for testing multi-field tuple variants.
pub enum OnlyTupleVariants {
    Point(i32, i32, i32),
    Color(u8, u8, u8, u8),
}

/// Enum solely for testing multi-field struct variants.
pub enum OnlyStructVariants {
    User { id: u64, username: String },
    Product { sku: String, price: f32, in_stock: bool },
}
