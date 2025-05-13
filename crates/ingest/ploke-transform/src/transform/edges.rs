// ------------------------------------------------------------
// ----------------- To Implement -----------------------------
// ------------------------------------------------------------
//
// These relations are needed to ensure we do not have any orphaned nodes:
// Ensure the following edges are added explicitly to the graph:
//  Contains:
//  Imports:
//  Attribute:
//  GenericParams (basic)
//  Reexports:
//  HasMethod:
//  HasField:
//  HasVariant:
//
// Requires Type Resolution:
//  Self Type
//  GenericParams (advanced, needs planning)
//  TraitImpl (Trait->Impl)
//  StructImpl (Struct->Impl)
//
// Abstracted Relations (Not strictly syntactic, composed of other relations)
//  ImplementsTrait (struct->method)
//      - Struct->Impl
//      - Impl->Trait
//  ImplementsMethod (struct->method)
//      - Struct->Impl
//      - Impl->Method
//  ImplementsTraitMethod (struct->method)
//      - Struct->Impl
//      - Impl->Trait   // These two previous are ImplementsTrait
//      - Trait->Method
