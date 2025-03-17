// Sample with different visibility modifiers

// Public struct
pub struct PublicStruct {
    pub field: String,
}

// Crate-visible struct
crate struct CrateVisibleStruct {
    pub field: String,
}

// Restricted visibility struct
pub(super) struct RestrictedStruct {
    pub field: String,
}

// Inherited visibility struct
struct InheritedStruct {
    pub field: String,
}
