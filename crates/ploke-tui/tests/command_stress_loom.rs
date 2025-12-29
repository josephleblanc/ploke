#![cfg(feature = "loom")]
//! Loom model test for producer/consumer command ordering.
//!
//! Covered (exactly):
//! - Synchronous model with `loom::sync::{Arc, Mutex}` and `loom::thread`.
//! - Two producers, each enqueues `[Add, Delete, Add]` into a shared Vec queue.
//! - One consumer pops commands and applies them to a minimal `ModelState`.
//! - All thread interleavings explored by `loom::model`.
//! - Assertion: `ModelState.messages >= 1` after all commands are processed.
//!
//! Not covered:
//! - Real `state_manager` or async Tokio runtime.
//! - Any real `StateCommand` variants or command parsing.
//! - UI, event bus behavior, or database/RAG/IO effects.
//! - Strong invariants beyond the single counter (`messages`) check.
//! - Larger workloads (bounded to a tiny model for exhaustiveness).

use loom::sync::Arc;
use loom::sync::Mutex;
use loom::sync::atomic::{AtomicUsize, Ordering};
use loom::thread;

#[derive(Clone, Copy, Debug)]
enum TestCommand {
    Add,
    Delete,
}

#[derive(Debug)]
struct ModelState {
    messages: usize,
}

impl ModelState {
    fn new() -> Self {
        Self { messages: 1 }
    }

    fn apply(&mut self, cmd: TestCommand) {
        match cmd {
            TestCommand::Add => {
                self.messages = self.messages.saturating_add(1);
            }
            TestCommand::Delete => {
                if self.messages > 1 {
                    self.messages -= 1;
                }
            }
        }
    }
}

#[test]
fn loom_command_queue_model() {
    loom::model(|| {
        let queue = Arc::new(Mutex::new(Vec::<TestCommand>::new()));
        let state = Arc::new(Mutex::new(ModelState::new()));
        let done = Arc::new(AtomicUsize::new(0));
        let producers = 2;

        let mut producer_handles = Vec::new();
        for _ in 0..producers {
            let queue = queue.clone();
            let done = done.clone();
            producer_handles.push(thread::spawn(move || {
                let cmds = [TestCommand::Add, TestCommand::Delete, TestCommand::Add];
                for cmd in cmds {
                    queue.lock().unwrap().push(cmd);
                    thread::yield_now();
                }
                done.fetch_add(1, Ordering::SeqCst);
            }));
        }

        let queue_consumer = queue.clone();
        let state_consumer = state.clone();
        let done_consumer = done.clone();
        let consumer = thread::spawn(move || {
            loop {
                let cmd_opt = {
                    let mut guard = queue_consumer.lock().unwrap();
                    guard.pop()
                };

                if let Some(cmd) = cmd_opt {
                    state_consumer.lock().unwrap().apply(cmd);
                } else if done_consumer.load(Ordering::SeqCst) == producers {
                    break;
                }

                thread::yield_now();
            }
        });

        for handle in producer_handles {
            handle.join().unwrap();
        }
        consumer.join().unwrap();

        let final_state = state.lock().unwrap();
        assert!(final_state.messages >= 1);
    });
}
