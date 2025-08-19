use super::*;
// Target module for change scanning and bounded concurrency

// async fn handle_scan_batch(requests: Vec<FileData>, semaphore: Arc<Semaphore>) -> Result<Vec<Option<ChangedFileData>>, PlokeError> { ... }
// async fn handle_scan_batch_with_roots(requests: Vec<FileData>, semaphore: Arc<Semaphore>, roots: Option<Arc<Vec<PathBuf>>>) -> Result<Vec<Option<ChangedFileData>>, PlokeError> { ... }
// async fn check_file_hash(file_data: FileData, semaphore: Arc<Semaphore>) -> Result<Option<ChangedFileData>, PlokeError> { ... }
// async fn check_file_hash_with_roots(file_data: FileData, semaphore: Arc<Semaphore>, roots: Option<Arc<Vec<PathBuf>>>) -> Result<Option<ChangedFileData>, PlokeError> { ... }

#[cfg(test)]
pub(crate) mod test_instrumentation {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::time::Duration;
    use std::sync::Mutex;
    use uuid::Uuid;

    static ENABLED: AtomicBool = AtomicBool::new(false);
    static CURRENT: AtomicUsize = AtomicUsize::new(0);
    static MAX: AtomicUsize = AtomicUsize::new(0);
    static DELAY_MS: AtomicUsize = AtomicUsize::new(0);
    static FILTER_NS: Mutex<Option<Uuid>> = Mutex::new(None);

    pub struct Guard(bool);

    impl Drop for Guard {
        fn drop(&mut self) {
            if self.0 {
                CURRENT.fetch_sub(1, Ordering::SeqCst);
            }
        }
    }

    pub fn reset() {
        ENABLED.store(false, Ordering::SeqCst);
        CURRENT.store(0, Ordering::SeqCst);
        MAX.store(0, Ordering::SeqCst);
        DELAY_MS.store(0, Ordering::SeqCst);
        *FILTER_NS.lock().unwrap() = None;
    }

    pub fn enable() {
        ENABLED.store(true, Ordering::SeqCst);
    }

    pub fn set_delay_ms(ms: usize) {
        DELAY_MS.store(ms, Ordering::SeqCst);
    }

    pub fn set_filter_namespace(ns: Uuid) {
        let mut guard = FILTER_NS.lock().unwrap();
        *guard = Some(ns);
    }

    pub fn clear_filter_namespace() {
        let mut guard = FILTER_NS.lock().unwrap();
        *guard = None;
    }

    pub fn max() -> usize {
        MAX.load(Ordering::SeqCst)
    }

    pub fn enter() -> Guard {
        if !ENABLED.load(Ordering::SeqCst) {
            return Guard(false);
        }
        let cur = CURRENT.fetch_add(1, Ordering::SeqCst) + 1;
        // Update max if needed
        loop {
            let prev = MAX.load(Ordering::SeqCst);
            if cur > prev {
                if MAX
                    .compare_exchange(prev, cur, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok()
                {
                    break;
                } else {
                    continue;
                }
            } else {
                break;
            }
        }
        Guard(true)
    }

    pub fn enter_for_namespace(ns: Uuid) -> Guard {
        if !ENABLED.load(Ordering::SeqCst) {
            return Guard(false);
        }
        if let Some(filter) = *FILTER_NS.lock().unwrap() {
            if filter != ns {
                return Guard(false);
            }
        }
        let cur = CURRENT.fetch_add(1, Ordering::SeqCst) + 1;
        // Update max if needed
        loop {
            let prev = MAX.load(Ordering::SeqCst);
            if cur > prev {
                if MAX
                    .compare_exchange(prev, cur, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok()
                {
                    break;
                } else {
                    continue;
                }
            } else {
                break;
            }
        }
        Guard(true)
    }

    pub async fn maybe_sleep() {
        let ms = DELAY_MS.load(Ordering::SeqCst);
        if ms > 0 {
            tokio::time::sleep(Duration::from_millis(ms as u64)).await;
        }
    }
}


#[cfg(test)]
mod tests {
    use crate::{read::tests::tracking_hash_with_path_ns, scan::test_instrumentation};

    use super::*;
    use ploke_common::{fixtures_crates_dir, workspace_root};
    use ploke_test_utils::{setup_db_full, setup_db_full_embeddings};
    use std::fs;
    use syn_parser::discovery::run_discovery_phase;
    use tempfile::tempdir;
    use tracing_error::ErrorLayer;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_scan_changes_preserves_input_order() {
        let dir = tempfile::tempdir().unwrap();
        let namespace = uuid::Uuid::new_v4();

        // Helper to create an unchanged file (hash matches content)
        let make_unchanged = |name: &str, content: &str| {
            let path = dir.path().join(name);
            std::fs::write(&path, content).unwrap();
            let hash = tracking_hash_with_path_ns(content, &path, namespace);
            FileData {
                id: uuid::Uuid::new_v4(),
                namespace,
                file_tracking_hash: hash,
                file_path: path,
            }
        };

        // Helper to create a changed file (stored hash is from old content; file is then modified)
        let make_changed = |name: &str, old_content: &str, new_content: &str| {
            let path = dir.path().join(name);
            // Write old, compute hash, then overwrite with new
            std::fs::write(&path, old_content).unwrap();
            let old_hash = tracking_hash_with_path_ns(old_content, &path, namespace);
            std::fs::write(&path, new_content).unwrap();
            FileData {
                id: uuid::Uuid::new_v4(),
                namespace,
                file_tracking_hash: old_hash,
                file_path: path,
            }
        };

        // Build requests in specific order: U C U C U
        let reqs = vec![
            make_unchanged("f0.rs", "fn a() {}"),
            make_changed("f1.rs", "fn b_old() {}", "fn b_new() {}"),
            make_unchanged("f2.rs", "fn c() {}"),
            make_changed("f3.rs", "fn d_old() {}", "fn d_new() {}"),
            make_unchanged("f4.rs", "fn e() {}"),
        ];

        let handle = IoManagerHandle::new();
        let result = handle.scan_changes_batch(reqs).await.unwrap();
        let ordered = result.expect("scan should succeed");

        assert_eq!(ordered.len(), 5);
        assert!(ordered[0].is_none(), "index 0 should be unchanged");
        assert!(ordered[1].is_some(), "index 1 should be changed");
        assert!(ordered[2].is_none(), "index 2 should be unchanged");
        assert!(ordered[3].is_some(), "index 3 should be changed");
        assert!(ordered[4].is_none(), "index 4 should be unchanged");

        handle.shutdown().await;
    }

    #[tokio::test]
    async fn test_scan_changes_bounded_concurrency() {
        let permits = 4usize;

        // Enable test instrumentation to measure max concurrent scans
        test_instrumentation::reset();
        test_instrumentation::set_filter_namespace(namespace);
        test_instrumentation::enable();
        test_instrumentation::set_delay_ms(25);

        let handle = IoManagerHandle::builder()
            .with_semaphore_permits(permits)
            .build();

        let dir = tempfile::tempdir().unwrap();
        let namespace = uuid::Uuid::new_v4();

        let mut reqs = Vec::new();
        for i in 0..32 {
            let path = dir.path().join(format!("g{:02}.rs", i));
            let old = format!("fn f{}() {{}}", i);
            std::fs::write(&path, &old).unwrap();
            let old_hash = tracking_hash_with_path_ns(&old, &path, namespace);

            // Change file after computing old hash so it registers as "changed"
            let new = format!("fn f{}() {{ let _x: usize = {}; }}", i, i);
            std::fs::write(&path, &new).unwrap();

            reqs.push(FileData {
                id: uuid::Uuid::new_v4(),
                namespace,
                file_tracking_hash: old_hash,
                file_path: path,
            });
        }

        let result = handle.scan_changes_batch(reqs).await.unwrap();
        let ordered = result.expect("scan should succeed");

        // All files should be detected as changed
        assert!(ordered.iter().all(|o| o.is_some()));

        // Verify measured concurrency never exceeded permit count
        let max = test_instrumentation::max();
        assert!(
            max <= permits,
            "observed concurrent scans {} exceeds permits {}",
            max,
            permits
        );

        handle.shutdown().await;
    }
}
