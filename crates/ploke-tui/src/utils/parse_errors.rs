use std::path::Path;

use ploke_error::DiagnosticInfo;
use serde::{Deserialize, Serialize};
use syn_parser::error::SynParserError;

const PARSE_NEXT_STEPS: &str = "Next steps:\n\
- Run the `cargo` tool call in the crate root (e.g., `cargo check`) to surface syntax or manifest issues.\n\
- Use `read_file` to inspect the failing files, then `non_semantic_patch` to fix them.\n\
- Paths must be absolute or workspace-root-relative (e.g., `crates/my-crate/src/lib.rs`).\n\
- Re-run parsing/indexing after fixes.";

pub fn format_parse_failure(target_dir: &Path, err: &SynParserError) -> String {
    format!(
        "Parse failed for crate: {target}\nReason: {error}\nSemantic tools (request_code_context/apply_code_edit) are unavailable until parsing succeeds.\n\n{next_steps}",
        target = target_dir.display(),
        error = err,
        next_steps = PARSE_NEXT_STEPS
    )
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParserDiagnosticField {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FlattenedParserDiagnostic {
    pub diagnostic_path: String,
    pub depth: usize,
    pub kind: String,
    pub summary: String,
    pub detail: Option<String>,
    pub source_path: Option<std::path::PathBuf>,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub end_line: Option<usize>,
    pub end_column: Option<usize>,
    pub start: Option<usize>,
    pub end: Option<usize>,
    pub context: Vec<ParserDiagnosticField>,
    pub emission_site_file: Option<String>,
    pub emission_site_line: Option<u32>,
    pub emission_site_column: Option<u32>,
    pub backtrace: Option<String>,
}

pub fn extract_nested_parser_diagnostics(err: &SynParserError) -> Vec<FlattenedParserDiagnostic> {
    let mut diagnostics = Vec::new();
    flatten_parser_diagnostics(err, "root".to_string(), 0, &mut diagnostics);
    diagnostics
}

fn flatten_parser_diagnostics(
    err: &SynParserError,
    diagnostic_path: String,
    depth: usize,
    out: &mut Vec<FlattenedParserDiagnostic>,
) {
    let span = err.diagnostic_span();
    let emission_site = err.diagnostic_emission_site();
    out.push(FlattenedParserDiagnostic {
        diagnostic_path: diagnostic_path.clone(),
        depth,
        kind: err.diagnostic_kind().to_string(),
        summary: err.diagnostic_summary(),
        detail: err.diagnostic_detail(),
        source_path: err.diagnostic_source_path().map(|path| path.to_path_buf()),
        line: span.and_then(|span| span.line()),
        column: span.and_then(|span| span.column()),
        end_line: span.and_then(|span| span.end_line()),
        end_column: span.and_then(|span| span.end_column()),
        start: span.and_then(|span| span.start()),
        end: span.and_then(|span| span.end()),
        context: err
            .diagnostic_context()
            .into_iter()
            .map(|field| ParserDiagnosticField {
                key: field.key.to_string(),
                value: field.value,
            })
            .collect(),
        emission_site_file: emission_site.map(|site| site.file.to_string()),
        emission_site_line: emission_site.map(|site| site.line),
        emission_site_column: emission_site.map(|site| site.column),
        backtrace: err.diagnostic_backtrace().map(ToString::to_string),
    });

    match err {
        SynParserError::MultipleErrors(errors) => {
            for (idx, child) in errors.iter().enumerate() {
                flatten_parser_diagnostics(
                    child,
                    format!("{diagnostic_path}.errors[{idx}]"),
                    depth + 1,
                    out,
                );
            }
        }
        SynParserError::PartialParsing { errors, .. } => {
            for (idx, child) in errors.iter().enumerate() {
                flatten_parser_diagnostics(
                    child,
                    format!("{diagnostic_path}.partial_errors[{idx}]"),
                    depth + 1,
                    out,
                );
            }
        }
        _ => {}
    }
}
