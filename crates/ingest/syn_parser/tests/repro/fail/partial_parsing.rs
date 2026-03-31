//! TEST_NOTE:2026-03-30
//!
//! Provenance:
//! - Corpus run: `run-1774867607815`
//! - Target repo: `linera-io/linera-protocol`
//! - Target crate: `linera-indexer/plugins`
//! - Saved failing member: `plugins`
//! - Saved hotspot file: `linera-indexer/plugins/src/template.rs`
//!
//! The original corpus failure was a resolve-stage error:
//! `Partial parsing success: 2 succeeded, 1 failed`
//!
//! The underlying syn error was emitted for `template.rs` and pointed at the
//! placeholder syntax used in the template example (`...`, `..`). That file is
//! intentionally non-compilable, but the crate still ships alongside valid
//! modules. This repro captures the same "some files parse, one does not" shape
//! in a minimal temporary crate, without relying on `cargo check`.

use std::fs;

use syn_parser::error::SynParserError;
use syn_parser::try_run_phases_and_resolve;
use tempfile::tempdir;

fn write_file(path: &std::path::Path, contents: &str) {
    fs::write(path, contents).expect("write fixture file");
}

#[test]
fn repro_partial_parsing_with_template_placeholders() {
    let td = tempdir().expect("create tempdir");
    let src_root = td.path().join("src");
    fs::create_dir_all(&src_root).expect("create src dir");

    write_file(
        &td.path().join("Cargo.toml"),
        r#"[package]
name = "repro_partial_parse_template"
version = "0.1.0"
edition = "2024"
"#,
    );

    write_file(
        &src_root.join("lib.rs"),
        r#"mod ok_one;
mod template;
"#,
    );
    write_file(
        &src_root.join("ok_one.rs"),
        "pub fn ok_one() -> u32 { 1 }\n",
    );

    // Intentionally invalid Rust: placeholder `...` / `..` to trigger syn parse failure.
    write_file(
        &src_root.join("template.rs"),
        r#"pub struct Template;

impl Template {
    async fn helper1(&self, ...) -> Result<.., ()> {
        Ok(())
    }
}
"#,
    );

    let err = try_run_phases_and_resolve(td.path())
        .expect_err("fixture should fail with partial parsing");

    match err {
        SynParserError::PartialParsing { successes, errors } => {
            assert_eq!(
                successes.0.len(),
                2,
                "expected 2 successfully parsed files, got {}",
                successes.0.len()
            );
            assert_eq!(errors.len(), 1, "expected one parse error, got {errors:?}");

            match &errors[0] {
                SynParserError::Syn {
                    message,
                    source_path,
                    ..
                } => {
                    assert!(
                        message.contains("expected one of"),
                        "error should mention parse expectations, got: {message}"
                    );
                    assert!(
                        source_path.ends_with("template.rs"),
                        "error should point at template.rs, got: {}",
                        source_path.display()
                    );
                }
                other => panic!("unexpected child error for partial parsing: {other:?}"),
            }
        }
        other => panic!("unexpected error for partial parsing repro: {other:?}"),
    }
}
