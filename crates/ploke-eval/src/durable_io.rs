//! Atomic, fsynced replacement for small authority-bearing files.

use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let temp = stage(path, bytes)?;
    if let Err(source) = fs::rename(&temp, path) {
        let _ = fs::remove_file(&temp);
        return Err(source);
    }
    sync_parent(path)
}

pub(crate) fn create_atomic(path: &Path, bytes: &[u8]) -> io::Result<bool> {
    let temp = stage(path, bytes)?;
    match fs::hard_link(&temp, path) {
        Ok(()) => {
            sync_parent(path)?;
            fs::remove_file(&temp)?;
            sync_parent(path)?;
            Ok(true)
        }
        Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {
            let _ = fs::remove_file(&temp);
            Ok(false)
        }
        Err(source) => {
            let _ = fs::remove_file(&temp);
            Err(source)
        }
    }
}

/// Remove only staging files owned by this writer for one destination.
///
/// Callers must hold the destination's higher-level ownership lock. A matching
/// non-file entry is rejected instead of removed.
pub(crate) fn cleanup_staging(path: &Path) -> io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let entries = match fs::read_dir(parent) {
        Ok(entries) => entries,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(source) => return Err(source),
    };
    let prefix = staging_prefix(path)?;
    let mut removed = false;
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name();
        if !is_staging_name(&name.to_string_lossy(), &prefix) {
            continue;
        }
        if !entry.file_type()?.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "atomic staging path '{}' is not a regular file",
                    entry.path().display()
                ),
            ));
        }
        fs::remove_file(entry.path())?;
        removed = true;
    }
    if removed {
        sync_parent(path)?;
    }
    Ok(())
}

fn stage(path: &Path, bytes: &[u8]) -> io::Result<PathBuf> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let prefix = staging_prefix(path)?;
    let (temp, mut file) = loop {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temp = parent.join(format!("{prefix}{}-{sequence}", std::process::id()));
        match OpenOptions::new().create_new(true).write(true).open(&temp) {
            Ok(file) => break (temp, file),
            Err(source) if source.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(source) => return Err(source),
        }
    };
    if let Err(source) = file.write_all(bytes).and_then(|_| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(&temp);
        return Err(source);
    }
    Ok(temp)
}

fn staging_prefix(path: &Path) -> io::Result<String> {
    let file_name = path.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("durable path '{}' has no file name", path.display()),
        )
    })?;
    Ok(format!(".{}.tmp-", file_name.to_string_lossy()))
}

fn is_staging_name(name: &str, prefix: &str) -> bool {
    let Some(suffix) = name.strip_prefix(prefix) else {
        return false;
    };
    let Some((pid, sequence)) = suffix.split_once('-') else {
        return false;
    };
    let Ok(pid_value) = pid.parse::<u32>() else {
        return false;
    };
    let Ok(sequence_value) = sequence.parse::<u64>() else {
        return false;
    };
    pid_value.to_string() == pid && sequence_value.to_string() == sequence
}

fn sync_parent(path: &Path) -> io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    File::open(parent)?.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replacement_is_atomic_and_removes_staging_file() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("authority.json");
        fs::write(&path, b"old").expect("seed old value");

        write_atomic(&path, b"new").expect("replace authority");

        assert_eq!(fs::read(&path).expect("read replacement"), b"new");
        assert!(
            fs::read_dir(tmp.path())
                .expect("read tempdir")
                .all(|entry| !entry
                    .expect("directory entry")
                    .file_name()
                    .to_string_lossy()
                    .contains(".tmp-"))
        );
    }

    #[test]
    fn create_is_no_clobber() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("receipt.json");

        assert!(create_atomic(&path, b"first").expect("create receipt"));
        assert!(!create_atomic(&path, b"second").expect("preserve receipt"));
        assert_eq!(fs::read(path).expect("read receipt"), b"first");
    }

    #[test]
    fn cleanup_removes_only_matching_staging_files() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("receipt.json");
        let staging = tmp.path().join(".receipt.json.tmp-99-1");
        let lookalike = tmp.path().join(".receipt.json.tmp-note");
        let leading_zero = tmp.path().join(".receipt.json.tmp-099-1");
        let overflow = tmp
            .path()
            .join(".receipt.json.tmp-4294967296-18446744073709551616");
        let unrelated = tmp.path().join("other.json");
        fs::write(&staging, b"partial").expect("write staging remnant");
        fs::write(&lookalike, b"keep").expect("write staging lookalike");
        fs::write(&leading_zero, b"keep").expect("write leading-zero lookalike");
        fs::write(&overflow, b"keep").expect("write overflow lookalike");
        fs::write(&unrelated, b"keep").expect("write unrelated file");

        cleanup_staging(&path).expect("cleanup staging remnant");

        assert!(!staging.exists());
        assert_eq!(fs::read(lookalike).expect("read lookalike"), b"keep");
        assert_eq!(
            fs::read(leading_zero).expect("read leading-zero lookalike"),
            b"keep"
        );
        assert_eq!(
            fs::read(overflow).expect("read overflow lookalike"),
            b"keep"
        );
        assert_eq!(fs::read(unrelated).expect("read unrelated"), b"keep");
    }
}
