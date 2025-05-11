use super::*;

#[derive(Debug)]
pub struct ErrorContext {
    pub span: Option<Span>,
    pub file_path: PathBuf,
    pub code_snippet: Option<String>,
    // Any other contextual info
}

// Implement for error types:
impl FatalError {
    pub fn with_context(self, ctx: ErrorContext) -> ContextualError {
        ContextualError::new(self, ctx)
    }
}
