use proc_macro2::Span;
use std::backtrace::Backtrace;

use super::*;

#[derive(Debug)]
pub struct ErrorContext {
    pub span: Option<Span>,
    pub file_path: PathBuf,
    pub code_snippet: Option<String>,
    pub backtrace: Option<Backtrace>,
}

#[derive(Debug, thiserror::Error)]
pub enum ContextualError {
    #[error("{source}\nContext: {context:?}")]
    WithContext {
        #[source]
        source: Box<Error>,
        context: ErrorContext,
    },
}

impl ErrorContext {
    pub fn new(file_path: PathBuf) -> Self {
        Self {
            file_path,
            span: None,
            code_snippet: None,
            backtrace: Some(Backtrace::capture()),
        }
    }
}

impl<T: Into<Error>> From<T> for ContextualError {
    fn from(err: T) -> Self {
        ContextualError::WithContext {
            source: Box::new(err.into()),
            context: ErrorContext::new(PathBuf::new()),
        }
    }
}
