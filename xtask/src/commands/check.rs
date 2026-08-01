//! Workspace checks that guard repository conventions.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use walkdir::WalkDir;

use super::{CommandContext, XtaskError};
use crate::executor::Command;

const DEFAULT_PATHS: &[&str] = &["crates/ploke-eval/src"];
const DEFAULT_THRESHOLD: usize = 3;
const DEFAULT_MAX_FINDINGS: usize = 50;
const ALLOW_MARKERS: &[&str] = &["structural-naming:allow", "structural_naming:allow"];
const TOKENS: &[&str] = &[
    "Published",
    "Checked",
    "Admitted",
    "Selected",
    "Hydrated",
    "Pending",
    "Awaiting",
    "Request",
    "Admission",
    "Binding",
    "Grant",
    "Coordinate",
    "Surface",
    "Harness",
    "Parent",
    "Child",
    "Evidence",
    "History",
    "Claim",
    "Witness",
    "Record",
    "Projection",
];

/// Repository check commands.
#[derive(Debug, Clone, clap::Subcommand)]
pub enum Check {
    /// Flag compound protocol and authority names that need structural carriers.
    #[command(name = "structural-naming")]
    Naming(Naming),
}

impl Check {
    /// Execute a repository check.
    pub fn execute(&self, ctx: &CommandContext) -> Result<Scan, XtaskError> {
        match self {
            Check::Naming(cmd) => cmd.execute(ctx),
        }
    }
}

/// Arguments for the structural naming tripwire.
#[derive(Debug, Clone, clap::Args)]
pub struct Naming {
    /// Files or directories to scan. Defaults to `crates/ploke-eval/src`.
    #[arg(value_name = "PATH")]
    pub paths: Vec<PathBuf>,

    /// Minimum number of semantic tokens in one identifier before it is flagged.
    #[arg(long, default_value_t = DEFAULT_THRESHOLD)]
    pub threshold: usize,

    /// Maximum findings included in command output and validation errors.
    #[arg(long, default_value_t = DEFAULT_MAX_FINDINGS)]
    pub max_findings: usize,

    /// Return a report with `passed=false` instead of exiting with a validation error.
    #[arg(long)]
    pub report_only: bool,
}

impl Command for Naming {
    type Output = Scan;
    type Error = XtaskError;

    fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        let root = ctx.workspace_root()?;
        let paths = if self.paths.is_empty() {
            DEFAULT_PATHS.iter().map(PathBuf::from).collect()
        } else {
            self.paths.clone()
        };
        let mut scan = Scan::new(self.threshold, self.max_findings);

        for path in paths {
            let resolved = resolve(&root, &path);
            scan_path(&resolved, &root, &mut scan)?;
        }

        if !scan.passed && !self.report_only {
            return Err(XtaskError::validation(scan.failure_message()).with_recovery(
                "Install a structural carrier/transition boundary or mark a durable record projection with `structural-naming:allow`.",
            ));
        }

        Ok(scan)
    }
}

/// Result of a structural naming scan.
#[derive(Debug, Clone, Serialize)]
pub struct Scan {
    /// Stable output discriminator for renderers and scripts.
    pub kind: &'static str,
    /// Whether no identifiers crossed the configured threshold.
    pub passed: bool,
    /// Threshold used for this scan.
    pub threshold: usize,
    /// Number of Rust source files inspected.
    pub checked_files: usize,
    /// Number of files skipped because they were not Rust source files.
    pub skipped_files: usize,
    /// Findings retained in the report.
    pub findings: Vec<Finding>,
    /// Count of findings beyond `findings` when output was capped.
    pub truncated_findings: usize,
    #[serde(skip)]
    max_findings: usize,
}

impl Scan {
    fn new(threshold: usize, max_findings: usize) -> Self {
        Self {
            kind: "structural_naming_check",
            passed: true,
            threshold,
            checked_files: 0,
            skipped_files: 0,
            findings: Vec::new(),
            truncated_findings: 0,
            max_findings,
        }
    }

    fn push(&mut self, finding: Finding) {
        self.passed = false;
        if self.findings.len() < self.max_findings {
            self.findings.push(finding);
        } else {
            self.truncated_findings += 1;
        }
    }

    fn failure_message(&self) -> String {
        let mut message = format!(
            "structural naming check found {} compound identifier(s)",
            self.findings.len() + self.truncated_findings
        );
        for finding in &self.findings {
            message.push_str(&format!(
                "\n{}:{}: {} `{}` contains {}",
                finding.path,
                finding.line,
                finding.kind,
                finding.identifier,
                finding.tokens.join(", ")
            ));
        }
        if self.truncated_findings > 0 {
            message.push_str(&format!(
                "\n... {} additional finding(s) omitted",
                self.truncated_findings
            ));
        }
        message
    }
}

/// One compound identifier flagged by the checker.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Finding {
    /// Path relative to the workspace root when possible.
    pub path: String,
    /// One-indexed source line.
    pub line: usize,
    /// Rust item kind that introduced the identifier.
    pub kind: &'static str,
    /// Identifier text.
    pub identifier: String,
    /// Semantic tokens found in the identifier.
    pub tokens: Vec<String>,
}

fn resolve(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

fn scan_path(path: &Path, root: &Path, scan: &mut Scan) -> Result<(), XtaskError> {
    if path.is_file() {
        return scan_file(path, root, scan);
    }
    if !path.is_dir() {
        return Err(XtaskError::validation(format!(
            "Check path `{}` is not a file or directory",
            path.display()
        ))
        .with_recovery("Pass a Rust source file or a directory containing Rust source files."));
    }

    for entry in WalkDir::new(path).into_iter().filter_entry(|entry| {
        let name = entry.file_name().to_string_lossy();
        name != "target" && name != ".git"
    }) {
        let entry = entry.map_err(|err| XtaskError::Io(err.to_string()))?;
        if entry.file_type().is_file() {
            scan_file(entry.path(), root, scan)?;
        }
    }
    Ok(())
}

fn scan_file(path: &Path, root: &Path, scan: &mut Scan) -> Result<(), XtaskError> {
    if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
        scan.skipped_files += 1;
        return Ok(());
    }

    let content = fs::read_to_string(path)?;
    scan.checked_files += 1;
    let mut previous_line = "";
    for (idx, line) in content.lines().enumerate() {
        if let Some((kind, identifier, tokens)) = inspect_line(line, previous_line, scan.threshold)
        {
            scan.push(Finding {
                path: display_path(path, root),
                line: idx + 1,
                kind,
                identifier,
                tokens,
            });
        }
        previous_line = line;
    }
    Ok(())
}

fn inspect_line(
    line: &str,
    previous_line: &str,
    threshold: usize,
) -> Option<(&'static str, String, Vec<String>)> {
    if allowed(line) || allowed(previous_line) {
        return None;
    }
    let code = strip_line_comment(line).trim();
    if code.is_empty() {
        return None;
    }

    for (keyword, kind, public_only) in [
        ("struct", "struct", false),
        ("enum", "enum", false),
        ("trait", "trait", false),
        ("mod", "module", false),
        ("type", "type alias", false),
        ("fn", "function", true),
    ] {
        let Some((prefix, identifier)) = declaration_identifier(code, keyword) else {
            continue;
        };
        if public_only && !prefix.contains("pub") {
            continue;
        }
        let tokens = semantic_tokens(&identifier);
        if tokens.len() >= threshold {
            return Some((kind, identifier, tokens));
        }
    }

    None
}

fn declaration_identifier(line: &str, keyword: &str) -> Option<(String, String)> {
    for (idx, _) in line.match_indices(keyword) {
        if !is_keyword_boundary(line, idx, keyword.len()) {
            continue;
        }
        let prefix = line[..idx].to_string();
        if prefix.contains('"') {
            continue;
        }
        let rest = line[idx + keyword.len()..].trim_start();
        let identifier: String = rest
            .chars()
            .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
            .collect();
        if !identifier.is_empty() {
            return Some((prefix, identifier));
        }
    }
    None
}

fn is_keyword_boundary(line: &str, start: usize, len: usize) -> bool {
    let before = line[..start].chars().next_back();
    let after = line[start + len..].chars().next();
    !before.is_some_and(is_ident_char) && !after.is_some_and(is_ident_char)
}

fn strip_line_comment(line: &str) -> &str {
    line.split_once("//")
        .map(|(code, _comment)| code)
        .unwrap_or(line)
}

fn allowed(line: &str) -> bool {
    ALLOW_MARKERS.iter().any(|marker| line.contains(marker))
}

fn semantic_tokens(identifier: &str) -> Vec<String> {
    let words = identifier_words(identifier);
    TOKENS
        .iter()
        .filter(|token| words.iter().any(|word| word == **token))
        .map(|token| (*token).to_string())
        .collect()
}

fn identifier_words(identifier: &str) -> Vec<String> {
    let mut words = Vec::new();
    for part in identifier.split('_').filter(|part| !part.is_empty()) {
        let chars: Vec<char> = part.chars().collect();
        if chars.is_empty() {
            continue;
        }
        let mut start = 0;
        for idx in 1..chars.len() {
            let prev = chars[idx - 1];
            let curr = chars[idx];
            let next = chars.get(idx + 1).copied();
            let boundary = (curr.is_ascii_uppercase()
                && (prev.is_ascii_lowercase() || prev.is_ascii_digit()))
                || (prev.is_ascii_uppercase()
                    && curr.is_ascii_uppercase()
                    && next.is_some_and(|ch| ch.is_ascii_lowercase()));
            if boundary {
                words.push(chars[start..idx].iter().collect());
                start = idx;
            }
        }
        words.push(chars[start..].iter().collect());
    }
    words
}

fn is_ident_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

fn display_path(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_compound_structural_identifier() {
        let found = inspect_line(
            "pub(crate) struct PublishedBroadHarnessRequest {",
            "",
            DEFAULT_THRESHOLD,
        )
        .expect("compound identifier should be flagged");

        assert_eq!(found.0, "struct");
        assert_eq!(found.1, "PublishedBroadHarnessRequest");
        assert_eq!(
            found.2,
            vec![
                "Published".to_string(),
                "Request".to_string(),
                "Harness".to_string()
            ]
        );
    }

    #[test]
    fn ignores_allowed_record_projection() {
        let found = inspect_line(
            "pub(crate) struct PublishedBroadHarnessRequest {",
            "// structural-naming:allow record projection only",
            DEFAULT_THRESHOLD,
        );

        assert!(found.is_none());
    }

    #[test]
    fn private_helpers_are_not_flagged() {
        let found = inspect_line(
            "fn inspect_published_request_admission_binding() {}",
            "",
            DEFAULT_THRESHOLD,
        );

        assert!(found.is_none());
    }

    #[test]
    fn flags_compound_type_alias() {
        let found = inspect_line(
            "pub(crate) type PublishedBroadHarnessRequest = request::Request<request::Broad, request::Published>;",
            "",
            DEFAULT_THRESHOLD,
        )
        .expect("compound type alias should be flagged");

        assert_eq!(found.0, "type alias");
        assert_eq!(found.1, "PublishedBroadHarnessRequest");
    }
}
