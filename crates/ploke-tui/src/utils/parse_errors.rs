use std::path::Path;

use syn_parser::error::SynParserError;

const PARSE_NEXT_STEPS: &str = "Next steps:\n\
- Run the `cargo` tool call in the crate root (e.g., `cargo check`) to surface syntax or manifest issues.\n\
- Use `read_file` to inspect the failing files, then `non_semantic_patch` to fix them.\n\
- Paths must be absolute or crate-root-relative (e.g., `src/lib.rs`).\n\
- Re-run parsing/indexing after fixes.";

pub fn format_parse_failure(target_dir: &Path, err: &SynParserError) -> String {
    format!(
        "Parse failed for crate: {target}\nReason: {error}\nSemantic tools (request_code_context/apply_code_edit) are unavailable until parsing succeeds.\n\n{next_steps}",
        target = target_dir.display(),
        error = err,
        next_steps = PARSE_NEXT_STEPS
    )
}
