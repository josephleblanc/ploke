#[derive(Debug, Clone)]
pub struct FunctionNode {
    pub name: String,
    pub visibility: VisibilityKind,
    pub parameters: Vec<ParameterNode>,
    pub return_type: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ParameterNode {
    pub name: String,
    pub type_name: String,
}

#[derive(Debug, Clone)]
pub struct StructNode {
    pub name: String,
    pub visibility: VisibilityKind,
    pub fields: Vec<FieldNode>,
}

#[derive(Debug, Clone)]
pub struct EnumNode {
    pub name: String,
    pub visibility: VisibilityKind,
    pub variants: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct FieldNode {
    pub name: String,
    pub type_name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VisibilityKind {
    Public,
    Inherited,
}
