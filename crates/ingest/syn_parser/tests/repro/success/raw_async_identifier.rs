//! Minimal success repro showing that rewriting a legacy bare identifier to a
//! raw identifier makes the file parseable by `syn`.
//!
//! This validates the narrow fallback hypothesis for the `fn async(...)` class
//! of Rust 2015 failures. It does not yet prove broader item-position coverage.

use std::fs;

use syn_parser::try_run_phases_and_resolve;
use tempfile::tempdir;

const RAW_ASYNC_IDENT_LIB_RS: &str = r#"
pub struct Worker;

impl Worker {
    pub fn r#async(&self) -> u32 {
        7
    }
}
"#;

fn write_file(path: &std::path::Path, contents: &str) {
    fs::write(path, contents).expect("write fixture file");
}

fn create_edition_2015_raw_async_ident_crate() -> tempfile::TempDir {
    let td = tempdir().expect("create tempdir");
    let src_root = td.path().join("src");
    fs::create_dir_all(&src_root).expect("create src dir");

    write_file(
        &td.path().join("Cargo.toml"),
        r#"[package]
name = "repro_edition_2015_raw_async_ident"
version = "0.1.0"
edition = "2015"
"#,
    );
    write_file(&src_root.join("lib.rs"), RAW_ASYNC_IDENT_LIB_RS);

    td
}

#[test]
fn repro_syn_parse_file_accepts_raw_async_identifier() {
    let file = syn::parse_file(RAW_ASYNC_IDENT_LIB_RS)
        .expect("raw identifier form should parse successfully in syn");

    let rendered = quote::quote!(#file).to_string();
    assert!(
        rendered.contains("r#async"),
        "rendered AST should preserve the raw identifier, got: {rendered}"
    );
}

#[test]
fn repro_try_run_phases_and_resolve_accepts_raw_async_identifier_crate() {
    let td = create_edition_2015_raw_async_ident_crate();
    let graphs = try_run_phases_and_resolve(td.path())
        .expect("raw identifier form should parse through the public parser entrypoint");

    assert_eq!(graphs.len(), 1, "expected a single parsed file graph");
}
