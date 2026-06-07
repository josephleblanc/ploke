use serde::{Deserialize, Serialize};
/// Graph node type label carried on semantic code-edit tool arguments.
///
/// Wire-compatible with `ploke_db::NodeType` without pulling database crates into wasm builds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    Function,
    Struct,
    Enum,
    Trait,
    Module,
    Const,
    Impl,
    Import,
    Macro,
    Static,
    TypeAlias,
    Union,
    Method,
    Param,
    Variant,
    Field,
    Attribute,
    GenericType,
    GenericLifetime,
    GenericConst,
    NamedType,
    ReferenceType,
    SliceType,
    ArrayType,
    TupleType,
    FunctionType,
    NeverType,
    InferredType,
    RawPointerType,
    TraitObjectType,
    ImplTraitType,
    ParenType,
    MacroType,
    UnknownType,
    SyntaxEdge,
}
