use std::backtrace::Backtrace;
use std::path::PathBuf;
use std::sync::Arc;

use super::*;

#[derive(Debug, Clone)]
pub struct SourceSpan {
    pub file: PathBuf,
    pub start: Option<usize>,
    pub end: Option<usize>,
    pub line: Option<u32>,
    pub col: Option<u32>,
}

impl SourceSpan {
    pub fn new(file: PathBuf) -> Self {
        Self {
            file,
            start: None,
            end: None,
            line: None,
            col: None,
        }
    }

    pub fn with_range(mut self, start: usize, end: usize) -> Self {
        self.start = Some(start);
        self.end = Some(end);
        self
    }

    pub fn with_line_col(mut self, line: u32, col: u32) -> Self {
        self.line = Some(line);
        self.col = Some(col);
        self
    }
}

#[derive(Debug, Clone)]
pub struct ErrorContext {
    pub span: Option<SourceSpan>,
    pub file_path: PathBuf,
    pub code_snippet: Option<String>,
    pub backtrace: Option<Arc<Backtrace>>,
}

#[cfg_attr(feature = "diagnostic", derive(miette::Diagnostic))]
#[derive(Debug, Clone, thiserror::Error)]
pub enum ContextualError {
    #[error("{source}\nContext: {context:?}")]
    #[cfg_attr(feature = "diagnostic", diagnostic(transparent))]
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
            backtrace: None,
        }
    }

    pub fn with_span(mut self, span: SourceSpan) -> Self {
        self.span = Some(span);
        self
    }

    pub fn with_snippet(mut self, snippet: String) -> Self {
        self.code_snippet = Some(snippet);
        self
    }

    pub fn with_backtrace(mut self, backtrace: Backtrace) -> Self {
        self.backtrace = Some(Arc::new(backtrace));
        self
    }
}

pub trait ContextExt<T> {
    fn with_path(self, path: impl Into<PathBuf>) -> Result<T>;
    fn with_span(self, span: SourceSpan) -> Result<T>;
    fn with_snippet<S: Into<String>>(self, snippet: S) -> Result<T>;
    fn with_backtrace(self) -> Result<T>;
}

impl<T> ContextExt<T> for Result<T, Error> {
    fn with_path(self, path: impl Into<PathBuf>) -> Result<T> {
        self.map_err(|e| {
            let context = ErrorContext::new(path.into());
            Error::from(ContextualError::WithContext {
                source: Box::new(e),
                context,
            })
        })
    }

    fn with_span(self, span: SourceSpan) -> Result<T> {
        self.map_err(|e| {
            let context = ErrorContext::new(span.file.clone()).with_span(span);
            Error::from(ContextualError::WithContext {
                source: Box::new(e),
                context,
            })
        })
    }

    fn with_snippet<S: Into<String>>(self, snippet: S) -> Result<T> {
        self.map_err(|e| {
            let context = ErrorContext::new(PathBuf::from("<unknown>")).with_snippet(snippet.into());
            Error::from(ContextualError::WithContext {
                source: Box::new(e),
                context,
            })
        })
    }

    fn with_backtrace(self) -> Result<T> {
        self.map_err(|e| {
            let context = ErrorContext::new(PathBuf::from("<unknown>")).with_backtrace(Backtrace::capture());
            Error::from(ContextualError::WithContext {
                source: Box::new(e),
                context,
            })
        })
    }
}
