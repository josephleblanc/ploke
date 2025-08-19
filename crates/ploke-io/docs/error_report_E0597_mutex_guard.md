Title: E0597 borrow error from short-lived Mutex binding when holding a lock guard

Context
- Component: per-file write locks in ploke-io/src/write.rs
- Symptom: Rust compile error E0597: borrowed value does not live long enough when acquiring a tokio::sync::Mutex guard.

Error Summary
- Problematic pattern:
  - Acquiring a MutexGuard inside a temporary block while the Arc<Mutex<()>> binding goes out of scope immediately after:
    {
        let lock = get_file_lock(&path);
        lock.lock().await
    }
  - The guard borrows the Mutex via &self; dropping the lock binding at the end of the block makes the borrow checker reject the code because the guard’s lifetime outlives the borrowed reference.

Root Cause
- tokio::sync::Mutex::lock(&self) returns a guard tied to the lifetime of the referenced Mutex.
- If the Arc<Mutex<()>> is created in a temporary scope and dropped, the guard would hold a reference to a value that (logically) no longer lives in that stack frame, triggering E0597.

Resolution
- Keep the Arc<Mutex<()>> binding alive for at least as long as the guard:
    let lock = get_file_lock(&file_path);
    let _guard = lock.lock().await;

Prevention Guidelines
- Avoid returning a guard from a small scope that drops the underlying Mutex binding immediately.
- Prefer two-step lock acquisition (bind then lock) rather than inlining lock() in a transient expression or block.
- Consider naming the guard _guard to intentionally keep it alive for the critical section while suppressing unused warnings.

Impact
- This change is purely structural and does not alter runtime behavior. It ensures the guard’s lifetime is valid and the code compiles across Rust versions.

References
- Rust error E0597: https://doc.rust-lang.org/error-index.html#E0597
- tokio::sync::Mutex::lock docs: https://docs.rs/tokio/latest/tokio/sync/struct.Mutex.html#method.lock
