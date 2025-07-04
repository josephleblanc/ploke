
Based on the files you've added and the overall architecture, here's a detailed implementation plan for the indexing process:

### 1. Indexing Process Characteristics
- **Long-running process** (minutes to hours for large codebases)
- **Requires progress updates** (essential for UX)
- **Resource-intensive** (CPU-bound embedding generation + I/O operations)
- **Should be interruptible** (cancellation support)

### 2. Key Files Needed for Implementation
1. `crates/ploke-tui/src/main.rs` (event system)
2. `crates/ploke-tui/src/app_state.rs` (state management)
3. `crates/ingest/ploke-embed/src/indexer.rs` (core logic)
4. `crates/ploke-db/src/result/mod.rs` (DB interactions)
5. `crates/ploke-db/src/database.rs` (DB access)

### 3. Current Implementation Analysis
**Strengths:**
- Batch processing is well-structured
- Cancellation support via `CancellationToken`
- Async I/O handling
- Separation of concerns

**Gaps:**
- No progress reporting
- No error propagation to UI
- No state tracking
- No pause/resume capability
- No resource throttling

### 4. Required Data Structures
```rust
// New state to track indexing
#[derive(Debug, Clone)]
pub struct IndexingState {
    pub status: IndexStatus,
    pub processed: usize,
    pub total: usize,
    pub current_file: Option<PathBuf>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IndexStatus {
    Idle,
    Running,
    Paused,
    Completed,
    Failed(String),
    Cancelled,
}

// New events
pub enum AppEvent {
    // ... existing events ...
    IndexingStarted,
    IndexingProgress(IndexingState),
    IndexingCompleted,
    IndexingFailed(String),
}
```

### 5. Channel & Event Strategy
| Channel Type         | Purpose                          | Why                                                                 |
|----------------------|----------------------------------|---------------------------------------------------------------------|
| **Broadcast Channel** | Progress updates                 | Allows multiple subscribers (UI, logging) with backpressure         |
| **MPSC Channel**     | Control commands                 | Dedicated channel for pause/resume/cancel commands                 |
| **Oneshot Channel**  | Status queries                   | For synchronous state checks                                        |

### 6. Implementation Plan

**Step 1: Enhance IndexerTask (ploke-embed/src/indexer.rs)**
```rust
impl IndexerTask {
    pub async fn run(
        &self,
        progress_tx: broadcast::Sender<IndexingState>,
        mut control_rx: mpsc::Receiver<IndexerCommand>
    ) -> Result<(), EmbedError> {
        let total = self.db.count_pending_embeddings()?;
        let mut state = IndexingState {
            status: IndexStatus::Running,
            processed: 0,
            total,
            current_file: None,
            errors: Vec::new(),
        };

        progress_tx.send(state.clone())?;
        
        while let Some(batch) = self.next_batch().await? {
            // Check for control commands
            if let Ok(cmd) = control_rx.try_recv() {
                match cmd {
                    IndexerCommand::Pause => state.status = IndexStatus::Paused,
                    IndexerCommand::Resume => state.status = IndexStatus::Running,
                    IndexerCommand::Cancel => {
                        state.status = IndexStatus::Cancelled;
                        break;
                    }
                }
                progress_tx.send(state.clone())?;
            }
            
            if state.status != IndexStatus::Running {
                continue;
            }
            
            state.current_file = batch.first().map(|n| n.path.clone());
            progress_tx.send(state.clone())?;
            
            match process_batch(/* ... */).await {
                Ok(_) => state.processed += batch.len(),
                Err(e) => state.errors.push(e.to_string()),
            }
            
            progress_tx.send(state.clone())?;
        }
        
        state.status = if state.processed >= state.total {
            IndexStatus::Completed
        } else {
            IndexStatus::Cancelled
        };
        progress_tx.send(state)?;
        Ok(())
    }
}
```

**Step 2: DB Enhancements (ploke-db/src/database.rs)**
```rust
impl Database {
    pub fn count_pending_embeddings(&self) -> Result<usize, DbError> {
        let query = r#"
        ?[count(id)] := *embedding_nodes{id, embedding},
        embedding = null"#;
        let result = self.db.run_ro(query, Default::default())?;
        result.into_usize(0, "count(id)")
    }
    
    pub fn into_usize(named_rows: NamedRows, col: &str) -> Result<usize, DbError> {
        named_rows
            .rows
            .first()
            .and_then(|row| row.first())
            .and_then(|v| v.as_int())
            .map(|n| n as usize)
            .ok_or(DbError::NotFound)
    }
}
```

**Step 3: State Manager Integration (app_state.rs)**
```rust
match StateCommand::IndexWorkspace => {
    let (control_tx, control_rx) = mpsc::channel(4);
    let progress_tx = event_bus.index_tx.clone(); // New dedicated channel
    
    state.indexing_control = Some(control_tx); // Store control handle
    
    tokio::spawn(async move {
        event_bus.send(AppEvent::IndexingStarted);
        
        if let Err(e) = indexer_task.run(progress_tx, control_rx).await {
            event_bus.send(AppEvent::IndexingFailed(e.to_string()));
        } else {
            event_bus.send(AppEvent::IndexingCompleted);
        }
    });
}
```

**Step 4: Event Bus Extension (main.rs)**
```rust
pub struct EventBus {
    // ... existing channels ...
    index_tx: broadcast::Sender<IndexingState>, // Dedicated for indexing
}

impl EventBus {
    pub fn new(/* ... */) -> Self {
        // ...
        index_tx: broadcast::channel(100).0,
    }
    
    pub fn index_subscriber(&self) -> broadcast::Receiver<IndexingState> {
        self.index_tx.subscribe()
    }
}
```

**Step 5: UI Integration**
```rust
// In main app loop
match event {
    AppEvent::IndexingProgress(state) => {
        app.indexing_state = state;
        // Update progress bar in status line
    }
    // ... other events ...
}

// In rendering
fn render_progress(f: &mut Frame) {
    let progress = app.indexing_state.processed as f32 / app.indexing_state.total as f32;
    let gauge = Gauge::default()
        .block(Block::new().title("Indexing"))
        .gauge_style(Style::new().light_blue())
        .percent((progress * 100.0) as u16);
    f.render_widget(gauge, area);
}
```

### 7. Why This Approach?
1. **Fits Existing Architecture**:
   - Extends Tokio worker model from README diagram
   - Uses layered channels matching concurrency model
   - Maintains separation of concerns

2. **Solves Current Limitations**:
   - Adds progress reporting missing in README
   - Enables proper cancellation
   - Provides UI feedback
   - Allows operational control (pause/resume)

3. **Performance Considerations**:
   - Batched updates prevent UI flooding
   - Dedicated channel avoids priority inversion
   - Resource monitoring via progress events

4. **Error Resilience**:
   - Per-batch error collection
   - State machine for recoverable states
   - Graceful cancellation

### 8. Additional Recommendations
1. Add checkpointing for resume capability
2. Implement batch size auto-tuning
3. Add disk space monitoring
4. Create throttling mechanism
5. Add estimated time remaining (ETA) calculations

This implementation provides a robust, observable indexing process that maintains the application's responsiveness while giving users proper feedback during long-running operations. The design stays true to the existing architecture while addressing the specific needs of vector embedding workloads.
