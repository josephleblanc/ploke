use super::*;

/// The `IoManager` is a central actor responsible for handling all file I/O operations
/// in a non-blocking manner. It runs in a dedicated thread and processes request
/// received through a message-passing channel.
///
/// ## Supported operations
///
/// - **Read snippets**: given a batch of `EmbeddingData`, return the requested byte
///   ranges from source files, verifying content hashes to detect concurrent edits.
/// - **Scan for changes**: given a batch of `FileData`, compute fresh tracking hashes
///   and return the paths whose contents no longer match the stored hash.
///
///
/// ## Architecture
///
/// The `IoManager` follows the actor model. It is spawned by an `IoManagerHandle`,
/// which provides a clean API for other parts of the application to send I/O requests.
/// All communication happens through asynchronous channels, preventing the main application
/// from blocking on file operations.
///
/// ## Concurrency
///
/// To avoid exhausting system resources, the `IoManager` uses a `Semaphore` to limit
/// the number of concurrently open files. The limit is dynamically set based on the
/// system's available file descriptors (via `rlimit`), ensuring robust performance
/// without overwhelming the OS.
///
/// ## Request Handling
///
/// When a batch of snippet requests arrives, the `IoManager` performs the following steps:
/// 1.  Groups requests by their file path to minimize the number of file open operations.
/// 2.  For each file, it spawns a new asynchronous task.
/// 3.  Before reading snippets, it verifies the file's content against a provided hash
///     to ensure data integrity and prevent reading from stale files.
/// 4.  It reads the requested byte ranges (snippets) from the file.
/// 5.  The results, including any errors, are collected and returned to the original
///     caller, preserving the order of the initial requests.
///
/// ### Change Scanning
/// When processing change scan requests:
/// 1.  Files are processed concurrently with bounded parallelism (limited by semaphore permits)
/// 2.  Each file is fully read and parsed to tokens
/// 3.  A fresh tracking hash is generated and compared against the stored reference
/// 4.  Changed files paths are returned while unchanged files are omitted
#[derive(Default, Debug)]
pub struct IoManagerBuilder {
    semaphore_permits: Option<usize>,
    fd_limit: Option<usize>,
    roots: Vec<PathBuf>,
}

impl IoManagerBuilder {
    /// Set an explicit semaphore permit count, overriding fd-limit derived values.
    pub fn with_semaphore_permits(mut self, permits: usize) -> Self {
        self.semaphore_permits = Some(permits);
        self
    }

    /// Set an explicit FD limit baseline; will be clamped to 4..=1024 and used
    /// if `with_semaphore_permits` is not provided. Env `PLOKE_IO_FD_LIMIT` still
    /// takes precedence when no explicit semaphore permits are set.
    pub fn with_fd_limit(mut self, fd_limit: usize) -> Self {
        self.fd_limit = Some(fd_limit);
        self
    }

    /// Configure allowed roots (stored for future path policy enforcement).
    pub fn with_roots<I, P>(mut self, roots: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        self.roots = roots.into_iter().map(Into::into).collect();
        self
    }

    /// Build an IoManagerHandle with the configured options.
    pub fn build(self) -> IoManagerHandle {
        let (tx, rx) = mpsc::channel(100);

        thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to build Tokio runtime");

            // Resolve effective permits:
            // precedence: explicit semaphore_permits > env/builder fd_limit > soft limit heuristic > default
            let soft_limit = rlimit::getrlimit(rlimit::Resource::NOFILE)
                .ok()
                .map(|(soft, _)| soft);
            let env_override = std::env::var("PLOKE_IO_FD_LIMIT")
                .ok()
                .and_then(|s| s.parse::<usize>().ok());

            let effective_permits = if let Some(p) = self.semaphore_permits {
                p
            } else {
                compute_fd_limit_from_inputs(soft_limit, env_override, self.fd_limit)
            };

            let roots_opt = if self.roots.is_empty() {
                None
            } else {
                Some(self.roots)
            };

            rt.block_on(async {
                let manager = IoManager::new_with(rx, effective_permits, roots_opt);
                manager.run().await;
            });
        });

        IoManagerHandle { request_sender: tx }
    }
}

/// Compute effective file-descriptor-based concurrency limit given optional sources.
/// Precedence: builder override > env override > OS soft limit heuristic > default (50).
pub(crate) fn compute_fd_limit_from_inputs(
    soft_nofile: Option<u64>,
    env_override: Option<usize>,
    builder_override: Option<usize>,
) -> usize {
    if let Some(n) = builder_override {
        return n.clamp(4, 1024);
    }
    if let Some(n) = env_override {
        return n.clamp(4, 1024);
    }
    if let Some(soft) = soft_nofile {
        return std::cmp::min(100, (soft / 3) as usize);
    }
    50
}
