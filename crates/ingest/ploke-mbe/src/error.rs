use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum MbeError {
    #[error("expected `macro_rules!` item, found path `{found}`")]
    NotMacroRules { found: String },

    #[error("macro definition must have a name")]
    MissingMacroName,

    #[error("invalid rule syntax: {message}")]
    InvalidRuleSyntax { message: String },

    #[error("unexpected end of tokens while parsing {context}")]
    UnexpectedEnd { context: &'static str },

    #[error("unsupported macro syntax: {message}")]
    UnsupportedSyntax { message: String },

    #[error("invalid fragment specifier `{fragment}`")]
    InvalidFragment { fragment: String },

    #[error("failed to parse expanded items: {message}")]
    StructuralParse { message: String },
}
