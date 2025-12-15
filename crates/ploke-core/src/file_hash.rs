use std::{
    fs::File,
    io::{self, BufReader, Read},
    path::Path,
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct FileHash(pub [u8; 32]);

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub enum LargeFilePolicy {
    #[default]
    Skip,
    Stream,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum HashOutcome {
    Hashed {
        size_bytes: u64,
        hash: FileHash,
    },
    SkippedTooLarge {
        size_bytes: u64,
        max_in_memory_bytes: u64,
    },
    NotARegularFile,
}

fn hash_bytes(bytes: &[u8]) -> FileHash {
    let mut hasher = blake3::Hasher::new();

    #[cfg(feature = "parallel-hash")]
    {
        hasher.update_rayon(bytes);
    }

    #[cfg(not(feature = "parallel-hash"))]
    {
        hasher.update(bytes);
    }

    FileHash(*hasher.finalize().as_bytes())
}

impl FileHash {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        hash_bytes(bytes)
    }
}

pub fn hash_file_blake3_bounded(
    path: &Path,
    max_in_memory_bytes: u64,
    large_policy: LargeFilePolicy,
) -> io::Result<HashOutcome> {
    let file = File::open(path)?;
    let meta = file.metadata()?;
    if !meta.is_file() {
        return Ok(HashOutcome::NotARegularFile);
    }

    let size = meta.len();
    let usize_limit = usize::MAX as u64;
    let in_memory_ok = size <= max_in_memory_bytes && size <= usize_limit;

    if in_memory_ok {
        let mut reader = BufReader::new(file);
        let mut bytes = Vec::with_capacity(size as usize);
        reader.read_to_end(&mut bytes)?;

        return Ok(HashOutcome::Hashed {
            size_bytes: size,
            hash: FileHash::from_bytes(&bytes),
        });
    }

    match large_policy {
        LargeFilePolicy::Skip => Ok(HashOutcome::SkippedTooLarge {
            size_bytes: size,
            max_in_memory_bytes,
        }),
        LargeFilePolicy::Stream => {
            let mut reader = BufReader::new(file);
            let mut hasher = blake3::Hasher::new();
            let mut buf = [0u8; 64 * 1024];

            loop {
                match reader.read(&mut buf)? {
                    0 => break,
                    n => hasher.update(&buf[..n]),
                };
            }

            Ok(HashOutcome::Hashed {
                size_bytes: size,
                hash: FileHash(*hasher.finalize().as_bytes()),
            })
        }
    }
}

#[cfg(feature = "read-tokio")]
mod read_tokio {
    use super::*;
    use std::{io, path::Path};
    use tokio::io::AsyncReadExt;

    use crate::file_hash::{FileHash, HashOutcome, LargeFilePolicy};

    #[allow(dead_code)]
    pub async fn hash_file_bounded_async(
        path: &Path,
        max_in_memory_bytes: u64,
        large_policy: LargeFilePolicy,
    ) -> io::Result<HashOutcome> {
        let file = tokio::fs::File::open(path).await?;
        let meta = file.metadata().await?;
        if !meta.is_file() {
            return Ok(HashOutcome::NotARegularFile);
        }

        let size = meta.len();
        let usize_limit = usize::MAX as u64;
        let in_memory_ok = size <= max_in_memory_bytes && size <= usize_limit;

        if in_memory_ok {
            let mut reader = tokio::io::BufReader::new(file);
            let mut bytes = Vec::with_capacity(size as usize);
            reader.read_to_end(&mut bytes).await?;
            return Ok(HashOutcome::Hashed {
                size_bytes: size,
                hash: hash_bytes(&bytes),
            });
        }

        match large_policy {
            LargeFilePolicy::Skip => Ok(HashOutcome::SkippedTooLarge {
                size_bytes: size,
                max_in_memory_bytes,
            }),

            LargeFilePolicy::Stream => {
                // Safe Rust fallback for big files: stream + incremental hash (no mmap)
                let mut reader = tokio::io::BufReader::new(file);
                let mut hasher = blake3::Hasher::new();
                let mut buf = vec![0u8; 64 * 1024];

                loop {
                    let n = reader.read(&mut buf).await?;
                    if n == 0 {
                        break;
                    }
                    hasher.update(&buf[..n]);
                }

                Ok(HashOutcome::Hashed {
                    size_bytes: size,
                    hash: FileHash(*hasher.finalize().as_bytes()),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, io::Write};

    fn write_temp_file(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let dir = std::env::temp_dir();
        let path = dir.join(format!(
            "ploke_filehash_test_{}_{}_{}",
            name,
            std::process::id(),
            // cheap uniqueness
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let mut f = fs::File::create(&path).unwrap();
        f.write_all(bytes).unwrap();
        f.flush().unwrap();

        path
    }

    #[test]
    fn from_bytes_matches_blake3_hash() {
        let data = b"hello\nworld\n";
        let got = FileHash::from_bytes(data);
        let expected = blake3::hash(data);
        assert_eq!(got.0, *expected.as_bytes());
    }

    #[test]
    fn bounded_hash_small_file_uses_in_memory_and_matches_from_bytes() {
        let data = b"small file content";
        let path = write_temp_file("small", data);

        let outcome =
            hash_file_blake3_bounded(&path, /*max_in_memory*/ 1024, LargeFilePolicy::Skip).unwrap();

        match outcome {
            HashOutcome::Hashed { size_bytes, hash } => {
                assert_eq!(size_bytes, data.len() as u64);
                assert_eq!(hash, FileHash::from_bytes(data));
            }
            other => panic!("expected Hashed, got {other:?}"),
        }

        let _ = fs::remove_file(path);
    }

    #[test]
    fn bounded_hash_large_file_skip_returns_skipped() {
        let data = vec![0u8; 4096];
        let path = write_temp_file("large_skip", &data);

        let outcome =
            hash_file_blake3_bounded(&path, /*max_in_memory*/ 1024, LargeFilePolicy::Skip).unwrap();

        match outcome {
            HashOutcome::SkippedTooLarge {
                size_bytes,
                max_in_memory_bytes,
            } => {
                assert_eq!(size_bytes, data.len() as u64);
                assert_eq!(max_in_memory_bytes, 1024);
            }
            other => panic!("expected SkippedTooLarge, got {other:?}"),
        }

        let _ = fs::remove_file(path);
    }

    #[test]
    fn bounded_hash_large_file_stream_matches_in_memory_reference() {
        // Big enough to trip the max_in_memory gate, small enough to keep the test fast.
        let data = (0..200_000u32)
            .flat_map(|x| x.to_le_bytes())
            .collect::<Vec<u8>>();

        let path = write_temp_file("large_stream", &data);

        let outcome =
            hash_file_blake3_bounded(&path, /*max_in_memory*/ 1024, LargeFilePolicy::Stream)
                .unwrap();

        match outcome {
            HashOutcome::Hashed { size_bytes, hash } => {
                assert_eq!(size_bytes, data.len() as u64);
                // reference hash computed from bytes (this will use rayon or not depending on feature)
                let expected = FileHash::from_bytes(&data);
                assert_eq!(hash, expected);
            }
            other => panic!("expected Hashed, got {other:?}"),
        }

        let _ = fs::remove_file(path);
    }

    #[test]
    fn different_contents_produce_different_hashes() {
        let a = b"aaa";
        let b = b"aab";
        assert_ne!(FileHash::from_bytes(a), FileHash::from_bytes(b));
    }
}
