//! Rust 2015 crates may legally use `async` as an identifier, but `syn::parse_file`
//! parses without edition context and rejects it as a keyword.
//!
//! TEST_NOTE:2026-04-08
//!
//! Provenance:
//! - Historical eval replay: `BurntSushi/ripgrep` setup failure
//! - Failing crate: `grep-cli` (effective edition 2015)
//! - Concrete failing file: `crates/cli/src/process.rs`
//! - Hotspot line shape: `fn async(...)`
//!
//! This repro isolates the edition-sensitive case into a single valid Rust 2015
//! crate so we can confirm whether the failure is in our pipeline or in `syn`
//! itself.

use std::fs;

use syn_parser::error::SynParserError;
use syn_parser::try_run_phases_and_resolve;
use tempfile::tempdir;

const ASYNC_IDENT_LIB_RS: &str = r#"
pub struct Worker;

impl Worker {
    pub fn async(&self) -> u32 {
        7
    }
}
"#;

fn write_file(path: &std::path::Path, contents: &str) {
    fs::write(path, contents).expect("write fixture file");
}

fn create_edition_2015_async_ident_crate() -> tempfile::TempDir {
    let td = tempdir().expect("create tempdir");
    let src_root = td.path().join("src");
    fs::create_dir_all(&src_root).expect("create src dir");

    write_file(
        &td.path().join("Cargo.toml"),
        r#"[package]
name = "repro_edition_2015_async_ident"
version = "0.1.0"
edition = "2015"
"#,
    );
    write_file(&src_root.join("lib.rs"), ASYNC_IDENT_LIB_RS);

    td
}

#[test]
fn repro_syn_parse_file_rejects_async_identifier_without_edition_context() {
    let err = match syn::parse_file(ASYNC_IDENT_LIB_RS) {
        Ok(_) => panic!("syn should reject bare `async` as an identifier without edition context"),
        Err(err) => err,
    };

    assert!(
        err.to_string()
            .contains("expected identifier, found keyword `async`"),
        "unexpected syn parse error: {err}"
    );
}

#[test]
#[cfg(not(feature = "convert_keyword_2015"))]
fn repro_try_run_phases_and_resolve_fails_on_valid_2015_async_identifier_crate() {
    let td = create_edition_2015_async_ident_crate();
    let err = try_run_phases_and_resolve(td.path())
        .expect_err("current parser should fail on edition-2015 async identifier fixture");

    match err {
        SynParserError::MultipleErrors(errors) => {
            assert_eq!(
                errors.len(),
                1,
                "expected one parse failure, got {errors:?}"
            );

            match &errors[0] {
                SynParserError::Syn {
                    message,
                    source_path,
                    ..
                } => {
                    assert!(
                        message.contains("expected identifier, found keyword `async`"),
                        "unexpected syn parse message: {message}"
                    );
                    assert!(
                        source_path.ends_with("src/lib.rs"),
                        "error should point at src/lib.rs, got: {}",
                        source_path.display()
                    );
                }
                other => panic!("unexpected child error kind: {other:?}"),
            }
        }
        other => panic!("unexpected error for edition-2015 async-ident repro: {other:?}"),
    }
}
