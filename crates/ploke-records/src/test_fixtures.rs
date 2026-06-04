use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn prototype1_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("prototype1")
}

pub(crate) fn protocol_artifacts_dir() -> PathBuf {
    prototype1_root().join("protocol-artifacts")
}

pub(crate) fn read_json_value(path: &Path) -> serde_json::Value {
    let text = fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("read prototype1 fixture {}: {err}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|err| panic!("parse prototype1 fixture {}: {err}", path.display()))
}
