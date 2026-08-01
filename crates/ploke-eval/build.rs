use std::{
    env,
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};

include!("src/cli/prototype1_state/walk/source_guard_paths.rs");

fn main() {
    let manifest_dir = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR").expect("Cargo provides CARGO_MANIFEST_DIR"),
    );
    let workspace_root = manifest_dir
        .parent()
        .and_then(|path| path.parent())
        .expect("ploke-eval is nested two levels below the workspace root");
    let _runtime_guard_paths = SOURCE_GUARD_PATHS;

    let mut files = Vec::new();
    for relative in CONTRACT_FINGERPRINT_PATHS {
        let path = workspace_root.join(relative);
        println!("cargo:rerun-if-changed={}", path.display());
        collect_files(&path, &mut files);
    }
    files.sort_by(|left, right| {
        left.strip_prefix(workspace_root)
            .expect("guarded file belongs to workspace")
            .cmp(
                right
                    .strip_prefix(workspace_root)
                    .expect("guarded file belongs to workspace"),
            )
    });
    files.dedup();

    let mut hasher = Sha256::new();
    hash_part(
        &mut hasher,
        b"schema",
        b"ploke-walk-client-contract-fingerprint.v2",
    );
    for key in CONTRACT_FINGERPRINT_ENV {
        println!("cargo:rerun-if-env-changed={key}");
        if let Ok(value) = env::var(key) {
            hash_part(&mut hasher, key.as_bytes(), value.as_bytes());
        }
    }
    for path in files {
        println!("cargo:rerun-if-changed={}", path.display());
        let relative = path
            .strip_prefix(workspace_root)
            .expect("guarded file belongs to workspace")
            .to_str()
            .expect("guarded source paths must be valid UTF-8");
        let bytes = fs::read(&path).unwrap_or_else(|error| {
            panic!(
                "failed to read guarded source '{}': {error}",
                path.display()
            )
        });
        hash_part(&mut hasher, relative.as_bytes(), &bytes);
    }

    let mut fingerprint = String::with_capacity(64);
    for byte in hasher.finalize() {
        write!(&mut fingerprint, "{byte:02x}").expect("write fingerprint");
    }
    println!("cargo:rustc-env=PLOKE_WALK_BUILD_FINGERPRINT={fingerprint}");
}

fn collect_files(path: &Path, files: &mut Vec<PathBuf>) {
    let metadata = fs::symlink_metadata(path).unwrap_or_else(|error| {
        panic!(
            "failed to inspect guarded path '{}': {error}",
            path.display()
        )
    });
    if metadata.is_file() {
        files.push(path.to_path_buf());
        return;
    }
    assert!(
        metadata.is_dir(),
        "guarded path is neither a file nor a directory: '{}'",
        path.display()
    );

    let mut entries = fs::read_dir(path)
        .unwrap_or_else(|error| {
            panic!(
                "failed to read guarded directory '{}': {error}",
                path.display()
            )
        })
        .map(|entry| entry.expect("read guarded directory entry").path())
        .collect::<Vec<_>>();
    entries.sort();
    for entry in entries {
        collect_files(&entry, files);
    }
}

fn hash_part(hasher: &mut Sha256, label: &[u8], bytes: &[u8]) {
    hasher.update((label.len() as u64).to_le_bytes());
    hasher.update(label);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}
