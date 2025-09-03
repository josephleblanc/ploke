#![cfg(feature = "test_harness")]

//! Comprehensive threaded test harness for deep integration testing
//! 
//! Provides a realistic test environment with:
//! - App running on dedicated thread with full subsystems
//! - Real database with parsed fixture codebase and vector embeddings  
//! - Complete message lifecycle: User → RAG → LLM → Tool → Response
//! - Multi-turn conversation support with context persistence
//! - Comprehensive tool validation and error testing

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, mpsc, oneshot, Notify, RwLock, Mutex};
use tokio::task::JoinHandle;
use uuid::Uuid;

use ploke_tui::{
    AppEvent, EventBus, 
    app_state::{AppState, StateCommand},
    chat_history::{ChatHistory, Message, MessageKind},
    tools::ToolCallRecord,
    user_config::UserConfig,
};

/// Tracks a user message through the complete processing lifecycle
#[derive(Debug)]
pub struct MessageTracker {
    pub user_message_id: Uuid,
    pub start_time: Instant,
    pub completion_notifier: Arc<Notify>,
    pub events: Arc<Mutex<Vec<AppEvent>>>,
}

/// User message with pre-assigned ID for tracking
#[derive(Debug)]
pub struct UserMessageInput {
    pub id: Uuid,
    pub content: String,
}

/// Complete assistant response with tool execution details
#[derive(Debug)]
pub struct AssistantResponse {
    pub id: Uuid,
    pub content: String,
    pub tool_calls_made: Vec<ToolCallRecord>,
    pub processing_time: Duration,
    pub events_observed: Vec<AppEvent>,
}

/// Comprehensive test harness running a real ploke-tui app instance
pub struct RealAppHarness {
    /// Handle to the app thread
    app_handle: JoinHandle<()>,
    
    /// Direct access to app state for verification
    pub state: Arc<AppState>,
    pub event_bus: Arc<EventBus>,
    
    /// Communication with running app
    user_input_tx: mpsc::Sender<UserMessageInput>,
    app_events_rx: broadcast::Receiver<AppEvent>,
    
    /// Control and synchronization
    shutdown_tx: Option<oneshot::Sender<()>>,
    message_trackers: Arc<RwLock<HashMap<Uuid, MessageTracker>>>,
}

impl RealAppHarness {
    /// Spawn a new app instance with the fixture database loaded
    pub async fn spawn_with_fixture() -> color_eyre::Result<Self> {
        // Create communication channels
        let (user_input_tx, mut user_input_rx) = mpsc::channel::<UserMessageInput>(32);
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();
        
        // Initialize app with fixture database (similar to test_harness.rs)
        let mut config = UserConfig::default();
        config.registry = config.registry.with_defaults();
        config.registry.load_api_keys();
        
        let runtime_cfg: ploke_tui::app_state::core::RuntimeConfig = config.clone().into();
        
        // Initialize database with fixture backup
        let db = ploke_db::Database::init_with_schema()?;
        let backup_path = {
            let mut p = ploke_test_utils::workspace_root();
            p.push("tests/backup_dbs/fixture_nodes_bfc25988-15c1-5e58-9aa8-3d33b5e58b92");
            p
        };
        
        if backup_path.exists() {
            let prior_rels_vec = db.relations_vec()
                .map_err(|e| color_eyre::eyre::eyre!("Failed to get relations: {}", e))?;
            db.import_from_backup(&backup_path, &prior_rels_vec)
                .map_err(|e| color_eyre::eyre::eyre!("Failed to import backup: {}", e))?;
            println!("✓ Loaded fixture database from: {}", backup_path.display());
        } else {
            return Err(color_eyre::eyre::eyre!(
                "Fixture database not found at: {}. Run database tests first to generate fixture.", 
                backup_path.display()
            ));
        }
        
        // Ensure primary index exists
        ploke_db::create_index_primary(&db)?;
        let db_handle = Arc::new(db);
        
        // Initialize all app subsystems
        let io_handle = ploke_io::IoManagerHandle::new();
        let event_bus = Arc::new(EventBus::new(ploke_tui::EventBusCaps::default()));
        
        // Embedder
        let processor = config.load_embedding_processor()?;
        let proc_arc = Arc::new(processor);
        
        // BM25 service
        let bm25_cmd = ploke_db::bm25_index::bm25_service::start(Arc::clone(&db_handle), 0.0)?;
        
        // Indexer task
        let indexer_task = ploke_embed::indexer::IndexerTask::new(
            db_handle.clone(),
            io_handle.clone(),
            Arc::clone(&proc_arc),
            ploke_embed::cancel_token::CancellationToken::new().0,
            8,
        ).with_bm25_tx(bm25_cmd);
        let indexer_task = Arc::new(indexer_task);
        
        // RAG service
        let rag = ploke_rag::RagService::new_full(
            db_handle.clone(),
            Arc::clone(&proc_arc),
            io_handle.clone(),
            ploke_rag::RagConfig::default(),
        ).ok().map(Arc::new);
        
        // Create app state
        let state = Arc::new(AppState {
            chat: ploke_tui::app_state::ChatState::new(ChatHistory::new()),
            config: ploke_tui::app_state::ConfigState::new(runtime_cfg),
            system: ploke_tui::app_state::SystemState::default(),
            indexing_state: RwLock::new(None),
            indexer_task: Some(Arc::clone(&indexer_task)),
            indexing_control: Arc::new(Mutex::new(None)),
            db: db_handle,
            embedder: Arc::clone(&proc_arc),
            io_handle: io_handle.clone(),
            proposals: RwLock::new(std::collections::HashMap::new()),
            rag,
            budget: ploke_rag::TokenBudget::default(),
        });
        
        // Setup command and event channels
        let (cmd_tx, cmd_rx) = mpsc::channel::<StateCommand>(1024);
        let (rag_event_tx, rag_event_rx) = mpsc::channel(10);
        
        // Create event subscription for monitoring
        let app_events_rx = event_bus.subscribe(ploke_tui::EventPriority::Realtime);
        
        // Message tracker storage
        let message_trackers = Arc::new(RwLock::new(HashMap::<Uuid, MessageTracker>::new()));
        
        // Clone references for the app thread
        let state_clone = state.clone();
        let event_bus_clone = event_bus.clone();
        let cmd_tx_clone = cmd_tx.clone();
        let trackers_clone = message_trackers.clone();
        
        // Spawn the app on a dedicated thread
        let app_handle = tokio::spawn(async move {
            // Start all app subsystems
            let _state_manager_handle = tokio::spawn(ploke_tui::app_state::state_manager(
                state_clone.clone(),
                cmd_rx,
                event_bus_clone.clone(),
                rag_event_tx,
            ));
            
            let _llm_manager_handle = tokio::spawn(ploke_tui::llm::llm_manager(
                event_bus_clone.subscribe(ploke_tui::EventPriority::Background),
                state_clone.clone(),
                cmd_tx_clone.clone(),
                event_bus_clone.clone(),
            ));
            
            let _event_bus_handle = tokio::spawn(ploke_tui::run_event_bus(Arc::clone(&event_bus_clone)));
            
            let _observability_handle = tokio::spawn(ploke_tui::observability::run_observability(
                event_bus_clone.clone(),
                state_clone.clone(),
            ));
            
            // Event monitoring task for tracking message lifecycle
            let trackers_clone_for_events = trackers_clone.clone();
            let event_bus_clone_for_events = event_bus_clone.clone();
            let _event_monitor_handle = tokio::spawn(async move {
                let mut event_rx = event_bus_clone_for_events.subscribe(ploke_tui::EventPriority::Realtime);
                
                while let Ok(event) = event_rx.recv().await {
                    // Update all active trackers with this event
                    let mut trackers = trackers_clone_for_events.write().await;
                    for tracker in trackers.values_mut() {
                        tracker.events.lock().await.push(event.clone());
                    }
                }
            });
            
            // Handle user input messages
            let mut user_input_interval = tokio::time::interval(Duration::from_millis(10));
            
            loop {
                tokio::select! {
                    // Handle shutdown signal
                    _ = &mut shutdown_rx => {
                        println!("🔄 App harness received shutdown signal");
                        break;
                    },
                    
                    // Process user input
                    Some(user_input) = user_input_rx.recv() => {
                        // Use the pre-assigned message ID from the input
                        let msg_id = user_input.id;
                        
                        // Create tracker for this message (if one doesn't already exist)
                        if !trackers_clone.read().await.contains_key(&msg_id) {
                            let tracker = MessageTracker {
                                user_message_id: msg_id,
                                start_time: Instant::now(),
                                completion_notifier: Arc::new(Notify::new()),
                                events: Arc::new(Mutex::new(Vec::new())),
                            };
                            
                            trackers_clone.write().await.insert(msg_id, tracker);
                        }
                        
                        // Send the complete message lifecycle commands
                        let (completion_tx, completion_rx) = oneshot::channel();
                        let (scan_tx, scan_rx) = oneshot::channel();
                        
                        // AddUserMessage
                        let _ = cmd_tx_clone.send(StateCommand::AddUserMessage {
                            content: user_input.content,
                            new_msg_id: msg_id,
                            completion_tx,
                        }).await;
                        
                        // ScanForChange  
                        let _ = cmd_tx_clone.send(StateCommand::ScanForChange { scan_tx }).await;
                        
                        // EmbedMessage
                        let _ = cmd_tx_clone.send(StateCommand::EmbedMessage {
                            new_msg_id: msg_id,
                            completion_rx,
                            scan_rx,
                        }).await;
                    },
                    
                    // Regular processing tick
                    _ = user_input_interval.tick() => {
                        // Allow other tasks to run
                        tokio::task::yield_now().await;
                    }
                }
            }
            
            println!("🏁 App harness thread exiting");
        });
        
        // Wait a moment for app initialization
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        Ok(Self {
            app_handle,
            state,
            event_bus,
            user_input_tx,
            app_events_rx,
            shutdown_tx: Some(shutdown_tx),
            message_trackers,
        })
    }
    
    /// Send a user message and get a tracker for monitoring completion
    pub async fn send_user_message(&self, content: &str) -> MessageTracker {
        let msg_id = Uuid::new_v4();
        
        // Create tracker
        let tracker = MessageTracker {
            user_message_id: msg_id,
            start_time: Instant::now(),
            completion_notifier: Arc::new(Notify::new()),
            events: Arc::new(Mutex::new(Vec::new())),
        };
        
        // Store tracker for monitoring
        self.message_trackers.write().await.insert(msg_id, tracker.clone());
        
        // Send message to app with pre-assigned ID
        self.user_input_tx.send(UserMessageInput {
            id: msg_id,
            content: content.to_string(),
        }).await.expect("Failed to send user message to app");
        
        tracker
    }
    
    /// Wait for complete assistant response with tool execution
    pub async fn wait_for_assistant_response(&self, tracker: MessageTracker, timeout: Duration) -> color_eyre::Result<AssistantResponse> {
        let start_time = Instant::now();
        
        // Wait for processing to complete or timeout
        tokio::time::timeout(timeout, async {
            loop {
                // Check if we have an assistant response for this conversation
                {
                    let chat = self.state.chat.read().await;
                    
                    // Look for assistant message in the conversation path
                    for message in chat.iter_path() {
                        if message.kind == MessageKind::Assistant && 
                           message.parent.is_some() &&
                           !message.content.trim().is_empty() &&
                           message.content != "Pending..." {
                            
                            // Check if this assistant message follows our user message
                            let mut current = Some(message.id);
                            while let Some(msg_id) = current {
                                if let Some(msg) = chat.messages.get(&msg_id) {
                                    if msg.id == tracker.user_message_id {
                                        // Extract tool calls from events
                                        let events = tracker.events.lock().await;
                                        let tool_calls: Vec<ToolCallRecord> = events
                                            .iter()
                                            .filter_map(|event| {
                                                match event {
                                                    AppEvent::System(_system_event) => {
                                                        // Convert system tool events to ToolCallRecord if needed
                                                        // For now, return empty record since extraction is complex
                                                        None // TODO: Implement proper conversion from system tool event
                                                    },
                                                    _ => None,
                                                }
                                            })
                                            .collect();
                                        
                                        // Found the assistant response to our message
                                        return Ok(AssistantResponse {
                                            id: message.id,
                                            content: message.content.clone(),
                                            tool_calls_made: tool_calls,
                                            processing_time: start_time.elapsed(),
                                            events_observed: events.clone(),
                                        });
                                    }
                                    current = msg.parent;
                                } else {
                                    break;
                                }
                            }
                        }
                    }
                }
                
                // Brief pause before checking again
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }).await.map_err(|_| color_eyre::eyre::eyre!("Timeout waiting for assistant response"))?
    }
    
    /// Get current conversation history for debugging
    pub async fn get_conversation_history(&self) -> Vec<Message> {
        let chat = self.state.chat.read().await;
        chat.iter_path().cloned().collect()
    }
    
    /// Graceful shutdown of the app harness
    pub async fn shutdown(mut self) -> color_eyre::Result<()> {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
        
        // Wait for app thread to finish
        tokio::time::timeout(Duration::from_secs(5), self.app_handle).await
            .map_err(|_| color_eyre::eyre::eyre!("Timeout waiting for app thread to shutdown"))?
            .map_err(|e| color_eyre::eyre::eyre!("App thread panicked: {}", e))?;
        
        println!("✓ RealAppHarness shutdown complete");
        Ok(())
    }
}

// Helper trait for cloning MessageTracker
impl Clone for MessageTracker {
    fn clone(&self) -> Self {
        Self {
            user_message_id: self.user_message_id,
            start_time: self.start_time,
            completion_notifier: self.completion_notifier.clone(),
            events: self.events.clone(),
        }
    }
}
