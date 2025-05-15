use super::*;
use crate::define_schema;

pub fn create_and_insert_types(db: &Db<MemStorage>) -> Result<(), Box<cozo::Error>> {
    NamedTypeSchema::SCHEMA.create_and_insert(db)?;
    NamedTypeSchema::SCHEMA.log_create_script();

    ReferenceTypeSchema::SCHEMA.create_and_insert(db)?;
    ReferenceTypeSchema::SCHEMA.log_create_script();

    SliceTypeSchema::SCHEMA.create_and_insert(db)?;
    SliceTypeSchema::SCHEMA.log_create_script();

    ArrayTypeSchema::SCHEMA.create_and_insert(db)?;
    ArrayTypeSchema::SCHEMA.log_create_script();

    TupleTypeSchema::SCHEMA.create_and_insert(db)?;
    TupleTypeSchema::SCHEMA.log_create_script();

    FunctionTypeSchema::SCHEMA.create_and_insert(db)?;
    FunctionTypeSchema::SCHEMA.log_create_script();

    NeverTypeSchema::SCHEMA.create_and_insert(db)?;
    NeverTypeSchema::SCHEMA.log_create_script();

    InferredTypeSchema::SCHEMA.create_and_insert(db)?;
    InferredTypeSchema::SCHEMA.log_create_script();

    RawPointerTypeSchema::SCHEMA.create_and_insert(db)?;
    RawPointerTypeSchema::SCHEMA.log_create_script();

    TraitObjectTypeSchema::SCHEMA.create_and_insert(db)?;
    TraitObjectTypeSchema::SCHEMA.log_create_script();

    ImplTraitTypeSchema::SCHEMA.create_and_insert(db)?;
    ImplTraitTypeSchema::SCHEMA.log_create_script();

    ParenTypeSchema::SCHEMA.create_and_insert(db)?;
    ParenTypeSchema::SCHEMA.log_create_script();

    MacroTypeSchema::SCHEMA.create_and_insert(db)?;
    MacroTypeSchema::SCHEMA.log_create_script();

    UnknownTypeSchema::SCHEMA.create_and_insert(db)?;
    UnknownTypeSchema::SCHEMA.log_create_script();
    Ok(())
}

define_schema!(NamedTypeSchema {
    "named_type",
    type_id: "Uuid",
    path: "[String]",
    is_fully_qualified: "Bool",
});

define_schema!(ReferenceTypeSchema {
    "reference_type",
    type_id: "Uuid",
    lifetime: "String?",
    is_mutable: "Bool",
    references_type: "Uuid",
});

// TODO: Add length
define_schema!(SliceTypeSchema {
    "slice_type",
    type_id: "Uuid",
    element_type: "Uuid",
});

define_schema!(ArrayTypeSchema {
    "array_type",
    type_id: "Uuid",
    element_type: "Uuid",
    size: "Int?",
});

// Tuple type schema for the cozo database.
// The element_types are a list of possibly repeating `TypeIds` in order of their use in the
// tuple.
define_schema!(TupleTypeSchema {
    "tuple_type",
    type_id: "Uuid",
    element_types: "[Uuid]"
});

define_schema!(FunctionTypeSchema {
    "function_type",
    type_id: "Uuid",
    is_unsafe: "Bool",
    is_extern: "Bool",
    abi: "String?",
});

define_schema!(NeverTypeSchema {
    "never_type",
    type_id: "Uuid",
});

define_schema!(InferredTypeSchema {
    "inferred_type",
    type_id: "Uuid",
});

// points_to is in related_types[0]
define_schema!(RawPointerTypeSchema {
    "raw_pointer_type",
    type_id: "Uuid",
    is_mutable: "Bool",
    points_to: "Uuid"
});

define_schema!(TraitObjectTypeSchema {
    "trait_object_type",
    type_id: "Uuid",
    dyn_token: "Bool",
    trait_bounds: "[Uuid]?"
});

define_schema!(ImplTraitTypeSchema {
    "impl_trait_type",
    type_id: "Uuid",
    trait_bounds: "[Uuid]?",
});

define_schema!(ParenTypeSchema {
    "paren_type",
    type_id: "Uuid",
    inner_type: "Uuid",
});

// macro tokens are just the literal string from the macro for now.
define_schema!(MacroTypeSchema {
    "macro_type",
    type_id: "Uuid",
    name: "String",
    tokens: "String",
});

// fallback type, only temporary and as a fallback until more types are implemented, but it should
// be helpful for debugging and expanding the currently available types.
define_schema!(UnknownTypeSchema {
    "unknown_type",
    type_id: "Uuid",
    type_str: "String",
});
