use std::sync::Arc;
use tokio::{sync::watch, time::sleep, time::Duration};

/// A token that can be used to signal cancellation across async tasks
#[derive(Debug)]
pub struct CancellationToken {
    receiver: watch::Receiver<bool>,
}

/// A clonable handle that can check for cancellation
#[derive(Clone)]
pub struct CancellationListener {
    receiver: Arc<watch::Receiver<bool>>,
}

/// Handle to trigger cancellation
pub struct CancellationHandle {
    sender: watch::Sender<bool>,
}

impl CancellationToken {
    /// Create a new cancellation token and its handle
    pub fn new() -> (Self, CancellationHandle) {
        let (tx, rx) = watch::channel(false);

        let token = Self { receiver: rx };

        let handle = CancellationHandle { sender: tx };

        (token, handle)
    }

    /// Check if cancellation has been requested
    pub fn is_cancelled(&self) -> bool {
        *self.receiver.borrow()
    }

    /// Wait asynchronously until cancellation is requested
    pub async fn cancelled(&mut self) {
        // Skip the current value and wait for changes
        while !*self.receiver.borrow() {
            if self.receiver.changed().await.is_err() {
                // Sender was dropped, treat as cancellation
                return;
            }
        }
    }

    /// Create a listener that can be cloned and shared
    pub fn listener(&self) -> CancellationListener {
        CancellationListener {
            receiver: Arc::new(self.receiver.clone()),
        }
    }
}

impl CancellationListener {
    /// Check if cancellation has been requested
    pub fn is_cancelled(&self) -> bool {
        *self.receiver.borrow()
    }

    /// Wait asynchronously until cancellation is requested
    pub async fn cancelled(&self) {
        let mut receiver = self.receiver.as_ref().clone();

        // Skip the current value and wait for changes
        while !*receiver.borrow() {
            if receiver.changed().await.is_err() {
                // Sender was dropped, treat as cancellation
                return;
            }
        }
    }
}

impl CancellationHandle {
    /// Signal cancellation to all associated tokens
    pub fn cancel(&self) {
        // Ignore the result - if receivers are gone, that's fine
        let _ = self.sender.send(true);
    }

    /// Check if this handle is still connected to tokens
    pub fn is_closed(&self) -> bool {
        self.sender.is_closed()
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new().0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, timeout, Duration};

    #[tokio::test]
    async fn test_cancellation_token_basic() {
        let (mut token, handle) = CancellationToken::new();

        // Initially not cancelled
        assert!(!token.is_cancelled());

        // Cancel the token
        handle.cancel();

        // Now it should be cancelled
        assert!(token.is_cancelled());

        // cancelled() should return immediately
        let result = timeout(Duration::from_millis(100), token.cancelled()).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_cancellation_listener() {
        let (token, handle) = CancellationToken::new();
        let listener1 = token.listener();
        let listener2 = listener1.clone();

        assert!(!listener1.is_cancelled());
        assert!(!listener2.is_cancelled());

        handle.cancel();

        assert!(listener1.is_cancelled());
        assert!(listener2.is_cancelled());

        // Both should complete immediately
        let result = timeout(Duration::from_millis(100), listener2.cancelled()).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_cancellation_token_async_wait() {
        let (mut token, handle) = CancellationToken::new();

        // Spawn a task that cancels after a delay
        tokio::spawn(async move {
            sleep(Duration::from_millis(50)).await;
            handle.cancel();
        });

        // Wait for cancellation
        token.cancelled().await;
        assert!(token.is_cancelled());
    }

    #[tokio::test]
    async fn test_handle_dropped() {
        let (mut token, handle) = CancellationToken::new();

        // Drop the handle
        drop(handle);

        // cancelled() should return immediately when sender is dropped
        let result = timeout(Duration::from_millis(100), token.cancelled()).await;
        assert!(result.is_ok());
    }
}

// Example usage
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (token, handle) = CancellationToken::new();

    // Create listeners for sharing across tasks
    let listener1 = token.listener();
    let listener2 = token.listener();

    // Spawn multiple long-running tasks
    let task1 = tokio::spawn(async move {
        let mut counter = 0;
        let mut interval = tokio::time::interval(Duration::from_millis(100));

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    counter += 1;
                    println!("Task 1 running... {}", counter);

                    if counter >= 20 {
                        println!("Task 1 completed normally");
                        break;
                    }
                }
                _ = listener1.cancelled() => {
                    println!("Task 1 cancelled gracefully");
                    break;
                }
            }
        }
    });

    let task2 = tokio::spawn(async move {
        let mut counter = 0;
        let mut interval = tokio::time::interval(Duration::from_millis(150));

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    counter += 1;
                    println!("Task 2 running... {}", counter);

                    if counter >= 15 {
                        println!("Task 2 completed normally");
                        break;
                    }
                }
                _ = listener2.cancelled() => {
                    println!("Task 2 cancelled gracefully");
                    break;
                }
            }
        }
    });

    // Let them run for a bit
    sleep(Duration::from_millis(500)).await;

    // Cancel all tasks
    println!("Requesting cancellation...");
    handle.cancel();

    // Wait for tasks to complete
    let _ = tokio::join!(task1, task2);

    // Verify cancellation state
    println!("Token is cancelled: {}", token.is_cancelled());

    Ok(())
}
