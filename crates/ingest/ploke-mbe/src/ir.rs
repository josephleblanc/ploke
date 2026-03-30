use proc_macro2::{Delimiter, TokenStream};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclarativeMacro {
    pub name: String,
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub matcher: MetaTemplate,
    pub transcriber: MetaTemplate,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MetaTemplate {
    pub ops: Vec<Op>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    Var {
        name: String,
        kind: Option<MetaVarKind>,
    },
    Repeat {
        tokens: MetaTemplate,
        separator: Option<Separator>,
        kind: RepeatKind,
    },
    Subtree {
        delimiter: Delimiter,
        tokens: MetaTemplate,
    },
    Literal(String),
    Punct(char),
    Ident(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepeatKind {
    ZeroOrMore,
    OneOrMore,
    ZeroOrOne,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Separator {
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetaVarKind {
    Block,
    Expr,
    Expr2021,
    Ident,
    Item,
    Lifetime,
    Literal,
    Meta,
    Pat,
    PatParam,
    Path,
    Stmt,
    Tt,
    Ty,
    Vis,
}

impl MetaVarKind {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "block" => Some(Self::Block),
            "expr" => Some(Self::Expr),
            "expr_2021" => Some(Self::Expr2021),
            "ident" => Some(Self::Ident),
            "item" => Some(Self::Item),
            "lifetime" => Some(Self::Lifetime),
            "literal" => Some(Self::Literal),
            "meta" => Some(Self::Meta),
            "pat" => Some(Self::Pat),
            "pat_param" => Some(Self::PatParam),
            "path" => Some(Self::Path),
            "stmt" => Some(Self::Stmt),
            "tt" => Some(Self::Tt),
            "ty" => Some(Self::Ty),
            "vis" => Some(Self::Vis),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MacroInvocation {
    pub path: String,
    pub tokens: TokenStream,
}
