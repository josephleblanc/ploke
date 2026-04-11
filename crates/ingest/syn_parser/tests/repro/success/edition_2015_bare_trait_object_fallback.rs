//! Feature-gated success repro for the edition-2015 bare trait object rewrite fallback.
//!
//! When `convert_keyword_2015` feature is enabled, the parser should rewrite
//! bare trait objects (e.g., `Arc<Fn()>`) to use `dyn` (e.g., `Arc<dyn Fn()>`)
//! and successfully parse Rust 2015 code.

#![cfg(feature = "convert_keyword_2015")]

use std::fs;

use syn_parser::try_run_phases_and_resolve;
use syn_parser::GraphAccess;
use tempfile::tempdir;

const EDITION_2015_BARE_TRAIT_OBJECTS: &str = r#"
use std::sync::Arc;
use std::ffi::OsStr;
use std::cmp;

#[derive(Clone)]
pub enum Sorter {
    ByName(Arc<Fn(&OsStr, &OsStr) -> cmp::Ordering + Send + Sync + 'static>),
    ByPath(Arc<Fn(&str, &str) -> cmp::Ordering + Send + Sync + 'static>),
}

impl Sorter {
    pub fn sort_names(&self, a: &OsStr, b: &OsStr) -> cmp::Ordering {
        match self {
            Sorter::ByName(f) => f(a, b),
            Sorter::ByPath(f) => cmp::Ordering::Equal,
        }
    }
}
"#;

fn write_file(path: &std::path::Path, contents: &str) {
    fs::write(path, contents).expect("write fixture file");
}

fn create_edition_2015_bare_trait_object_crate() -> tempfile::TempDir {
    let td = tempdir().expect("create tempdir");
    let src_root = td.path().join("src");
    fs::create_dir_all(&src_root).expect("create src dir");

    write_file(
        &td.path().join("Cargo.toml"),
        r#"[package]
name = "repro_edition_2015_bare_trait_object_fallback"
version = "0.1.0"
edition = "2015"
"#,
    );
    write_file(&src_root.join("lib.rs"), EDITION_2015_BARE_TRAIT_OBJECTS);

    td
}

#[test]
fn repro_try_run_phases_and_resolve_accepts_bare_trait_object_crate_under_feature() {
    let td = create_edition_2015_bare_trait_object_crate();
    let graphs = try_run_phases_and_resolve(td.path())
        .expect("feature-gated fallback should parse edition-2015 bare trait object crate");

    assert_eq!(graphs.len(), 1, "expected one parsed file graph");
    let graph = &graphs[0].graph;
    
    // Verify the enum was parsed (enums are in defined_types as TypeDefNode::Enum)
    let enum_count = graph.defined_types().iter()
        .filter(|t| matches!(t, syn_parser::parser::nodes::TypeDefNode::Enum(_)))
        .count();
    assert_eq!(enum_count, 1, "expected one enum (Sorter)");
    
    // Verify the impl block was parsed
    assert_eq!(graph.impls().len(), 1, "expected one impl block");
    let methods = &graph.impls()[0].methods;
    assert_eq!(methods.len(), 1, "expected one method");
    assert_eq!(methods[0].name, "sort_names");
}
