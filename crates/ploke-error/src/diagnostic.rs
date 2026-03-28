use std::panic::Location;
use std::path::Path;

pub trait DiagnosticInfo {
    fn diagnostic_kind(&self) -> &'static str;
    fn diagnostic_summary(&self) -> String;
    fn diagnostic_detail(&self) -> Option<String> {
        None
    }
    fn diagnostic_source_path(&self) -> Option<&Path> {
        None
    }
    fn diagnostic_span(&self) -> Option<&dyn DiagnosticSpan> {
        None
    }
    fn diagnostic_context(&self) -> Vec<DiagnosticField> {
        Vec::new()
    }
    fn diagnostic_emission_site(&self) -> Option<&DiagnosticSite> {
        None
    }
}

pub trait DiagnosticSpan {
    fn line(&self) -> Option<usize>;
    fn column(&self) -> Option<usize>;
    fn end_line(&self) -> Option<usize> {
        None
    }
    fn end_column(&self) -> Option<usize> {
        None
    }
    fn start(&self) -> Option<usize> {
        None
    }
    fn end(&self) -> Option<usize> {
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticField {
    pub key: &'static str,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticSite {
    pub file: &'static str,
    pub line: u32,
    pub column: u32,
}

impl DiagnosticSite {
    pub fn from_location(location: &'static Location<'static>) -> Self {
        Self {
            file: location.file(),
            line: location.line(),
            column: location.column(),
        }
    }
}
