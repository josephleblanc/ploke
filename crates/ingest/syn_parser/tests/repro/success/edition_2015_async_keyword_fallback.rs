//! Feature-gated success repro for the edition-2015 keyword rewrite fallback.

#![cfg(feature = "convert_keyword_2015")]

use std::fs;

use syn_parser::try_run_phases_and_resolve;
use tempfile::tempdir;

const EDITION_2015_ASYNC_METHODS: &str = r#"
pub struct Worker;

impl Worker {
    pub fn async(&self) -> u32 {
        7
    }

    pub fn sync(&self) -> u32 {
        self.async() + 1
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
name = "repro_edition_2015_async_keyword_fallback"
version = "0.1.0"
edition = "2015"
"#,
    );
    write_file(&src_root.join("lib.rs"), EDITION_2015_ASYNC_METHODS);

    td
}

#[test]
fn repro_try_run_phases_and_resolve_accepts_bare_async_identifier_crate_under_feature() {
    let td = create_edition_2015_async_ident_crate();
    let graphs = try_run_phases_and_resolve(td.path())
        .expect("feature-gated fallback should parse edition-2015 async identifier crate");

    assert_eq!(graphs.len(), 1, "expected one parsed file graph");
    let graph = &graphs[0].graph;
    assert_eq!(graph.impls.len(), 1, "expected one impl block");
    let methods = &graph.impls[0].methods;
    assert_eq!(methods.len(), 2, "expected async and sync methods");
    assert_eq!(methods[0].name, "async");
    assert_eq!(methods[1].name, "sync");

    let async_slice = &EDITION_2015_ASYNC_METHODS[methods[0].span.0..methods[0].span.1];
    assert!(
        async_slice.contains("fn async("),
        "async span should map to original source, got: {async_slice}"
    );

    let sync_slice = &EDITION_2015_ASYNC_METHODS[methods[1].span.0..methods[1].span.1];
    assert!(
        sync_slice.contains("fn sync("),
        "sync span should map to original source after rewrite compensation, got: {sync_slice}"
    );
}
