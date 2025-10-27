# Model Commands Data Flow Analysis

*Generated: 2025-09-06*

This document analyzes the complete data flow for model-related commands in Ploke TUI, from user input through command execution and state updates.

## 1. Command Input Processing

### Command Styles
**Location**: `crates/ploke-tui/src/app/commands/parser.rs:55-59`

**Style Handling**:
- **Slash Style** (default): `/model use gpt4` → stripped to `model use gpt4`
- **NeoVim Style**: `:model use gpt4` → stripped to `model use gpt4`

### Command Parsing Pipeline
**Trigger**: User presses Enter in Command mode (triggered by `/` or `:` prefix)

**Location**: `Action::ExecuteCommand` handling in app event loop

**Process**:
1. **Raw Input**: User types command with style prefix
2. **Normalization**: Style prefix stripped, whitespace trimmed  
3. **Pattern Matching**: Command matched against known patterns
4. **Structured Output**: Converted to `Command` enum variant

## 2. Model Command Variants

### 2.1 Model Information Commands

#### `model list`
**Purpose**: Display all available model configurations

**Parser**: Direct match `"model list"` → `Command::ModelList`

**Execution** (`list_models_async`):
```rust
// Async task spawned to avoid blocking UI
tokio::spawn(async move {
    let cfg = state.config.read().await;
    let available = cfg.model_registry.list_available();
    let active = cfg.model_registry.active_model_config.clone();
    
    // Format output with active model highlighted
    let formatted = available.iter()
        .map(|(id, display)| {
            if *id == active {
                format!("* {} ({})", display, id)  // Mark active with *
            } else {
                format!("  {} ({})", display, id)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
        
    // Send result to chat history
    cmd_tx.send(StateCommand::AddMessageImmediate {
        msg: format!("Available models:\n{}", formatted),
        kind: MessageKind::SysInfo,
        new_msg_id: Uuid::new_v4(),
    }).await;
});
```

#### `model info`
**Purpose**: Show detailed information about currently active model

**Parser**: Direct match `"model info"` → `Command::ModelInfo`

**Execution**:
- Displays active provider configuration
- Shows model capabilities (if cached from OpenRouter)
- Includes API key source, base URL, provider type

### 2.2 Model Selection Commands

#### `model use <alias_or_id>`
**Purpose**: Switch active model

**Parser**: 
```rust
s if s.starts_with("model use ") => {
    let alias = s.trim_start_matches("model use ").trim().to_string();
    Command::ModelUse(alias)
}
```

**Execution Flow**:
1. **Command Dispatch**: Delegates to `StateCommand::SwitchModel`
2. **State Manager Processing**: `handlers::model::switch_model()`
3. **Registry Update**: `ModelRegistry::set_active()` validates and switches
4. **Event Emission**: `SystemEvent::ModelSwitched` for UI updates

**Registry Validation**:
```rust
// Location: user_config.rs:388-431
pub fn set_active(&mut self, id_or_alias: &str) -> bool {
    // 1. Resolve alias to provider ID
    let provider_id = self.aliases.get(id_or_alias)
        .map(|s| s.as_str())
        .unwrap_or(id_or_alias);
    
    // 2. Check provider exists
    let provider = self.providers.iter().find(|p| p.id == *provider_id)?;
    
    // 3. Enforce strictness policy
    let allowed = match self.strictness {
        ModelRegistryStrictness::OpenRouterOnly => 
            matches!(provider.provider_type, ProviderType::OpenRouter),
        ModelRegistryStrictness::AllowCustom => 
            matches!(provider.provider_type, 
                ProviderType::OpenRouter | ProviderType::Custom),
        ModelRegistryStrictness::AllowAny => true,
    };
    
    if allowed {
        self.active_model_config = provider_id.to_string();
        true
    } else {
        false // Rejected by policy
    }
}
```

### 2.3 Model Discovery Commands

#### `model search <keyword>`
**Purpose**: Search OpenRouter model catalog

**Parser**:
```rust
s if s.starts_with("model search") => {
    let kw = s.trim_start_matches("model search").trim().to_string();
    Command::ModelSearch(kw)
}
```

**Execution Flow**:
1. **UI Preparation**: Opens model browser overlay immediately (prevents perceived delay)
2. **Async Search**: Spawned task queries OpenRouter API
3. **Result Processing**: Converts API response to UI-friendly format
4. **Event Publishing**: `AppEvent::ModelSearchResults` with results

**API Integration**:
```rust
// Query OpenRouter /models endpoint with keyword filter
let models = openrouter_catalog::fetch_models(&client, url, api_key).await?;
let filtered: Vec<ModelsEndpoint> = models
    .into_iter()
    .filter(|m| m.id.to_lowercase().contains(&keyword.to_lowercase()) ||
               m.name.to_lowercase().contains(&keyword.to_lowercase()))
    .map(|m| ModelsEndpoint::from(m))  // Convert to UI format
    .collect();

// Emit results to UI
emit_app_event(AppEvent::ModelSearchResults {
    keyword: keyword.clone(),
    items: filtered,
}).await;
```

#### `model providers <model_id>`
**Purpose**: List available providers for a specific model

**Parser**:
```rust
s if s.starts_with("model providers") => {
    let id = s.trim_start_matches("model providers ").trim().to_string();
    let model_id = if id.is_empty() {
        app.active_model_id.clone()  // Default to current model
    } else {
        id
    };
    Command::ModelProviders(model_id)
}
```

**Execution**: 
- Queries OpenRouter `/models/{model_id}/endpoints` API
- Returns provider options with pricing, context limits, tool support
- Results displayed in expandable model browser UI

### 2.4 Configuration Management Commands

#### `model refresh [--local]`
**Purpose**: Reload API keys and optionally refresh OpenRouter capabilities

**Parser**:
```rust
s if s.starts_with("model refresh") => {
    let remote = !s.contains("--local");  // Default to remote unless --local
    Command::ModelRefresh { remote }
}
```

**Execution Process**:
```rust
tokio::spawn(async move {
    let mut cfg = state.config.write().await;
    
    // 1. Always reload API keys from environment
    cfg.model_registry.load_api_keys();
    
    // 2. Optionally refresh from OpenRouter
    if remote {
        match cfg.model_registry.refresh_from_openrouter().await {
            Ok(_) => { /* Success message */ },
            Err(e) => { /* Error message with details */ }
        }
    }
});
```

#### `model load [path]` / `model save [path] [--with-keys]`
**Purpose**: Persist model configuration to/from TOML files

**Load Process**:
1. **Path Resolution**: Defaults to `~/.config/ploke/config.toml`
2. **Configuration Merge**: Loaded config merged with curated defaults
3. **Key Resolution**: API keys resolved from environment variables
4. **Capability Refresh**: OpenRouter capabilities updated if API key available
5. **Embedding Detection**: Warns if embedding backend changed (restart recommended)
6. **State Update**: Runtime configuration atomically replaced

**Save Process**:
1. **Key Redaction**: API keys optionally stripped (default: redacted)
2. **Atomic Write**: Uses temporary file + rename for consistency
3. **TOML Formatting**: Pretty-printed for human readability

## 3. Provider Commands Integration

### Provider Selection Commands
**Purpose**: Select specific OpenRouter provider for a model

#### `provider select <model_id> <provider_slug>`
**Command Flow**:
```rust
StateCommand::SelectModelProvider { model_id, provider_id } => {
    let mut cfg = state.config.write().await;
    let reg = &mut cfg.model_registry;
    
    // Update existing provider or create new one
    if let Some(p) = reg.providers.iter_mut().find(|p| p.id == model_id) {
        p.model = model_id.clone();
        p.provider_slug = ProviderSlug::from_str(&provider_id).ok();
        // ... update other fields
    } else {
        // Create new ModelConfig entry
        reg.providers.push(ModelConfig { /* ... */ });
    }
    
    // Activate this provider
    reg.active_model_config = model_id.clone();
}
```

### Provider Policy Commands

#### `provider strictness <policy>`
**Purpose**: Control which provider types are allowed

**Policies**:
- `openrouter-only`: Only OpenRouter providers
- `allow-custom`: OpenRouter + Custom providers (default)
- `allow-any`: No restrictions

#### `provider tools-only <on|off>`
**Purpose**: Filter models by tool calling capability

## 4. Event Flow Coordination

### State Update Pattern
**Model Switch Event Flow**:
1. **Command**: `StateCommand::SwitchModel`
2. **Validation**: Registry policy enforcement
3. **State Update**: Active model configuration changed
4. **Event Emission**: `AppEvent::System(SystemEvent::ModelSwitched)`
5. **UI Update**: Status line and indicators updated
6. **Chat Notification**: Success/failure message added to conversation

### Event Priority Routing
**ModelSwitched Priority**: `EventPriority::Realtime` (immediate UI update)

```rust
AppEvent::System(SystemEvent::ModelSwitched(_)) => EventPriority::Realtime,
```

### Error Handling
**Failure Modes**:
1. **Unknown Model**: Model ID not found in registry
2. **Policy Violation**: Model type blocked by strictness setting  
3. **API Failures**: OpenRouter API unavailable during refresh
4. **File I/O Errors**: Config load/save operations

**Error Communication**:
- Errors communicated via `AppEvent::Error(ErrorEvent)`
- User-friendly messages added to chat history
- Tracing logs for debugging

## 5. UI Integration Points

### Model Browser Overlay
**Trigger**: `model search` command opens interactive overlay

**Features**:
- **Async Loading**: Results populate as they arrive
- **Expandable Items**: Provider details loaded on-demand
- **Selection Interface**: Keyboard navigation (j/k, Enter, s)
- **Provider Comparison**: Side-by-side pricing and capability info

### Status Line Updates
**Model Indicator**: Top-right corner shows current model
- Updated immediately on `ModelSwitched` events
- Shows brief change notification (fades after delay)

### Chat History Integration
**System Messages**: All command results appear in conversation
- Success confirmations
- Error messages with context
- Configuration change summaries

## 6. Performance Characteristics

### Async Command Execution
**Non-Blocking Design**: All commands execute in spawned tasks
- UI remains responsive during API calls
- Background tasks communicate via event bus
- Bounded channels prevent memory growth

### Caching Strategy
**OpenRouter Capabilities**: Model metadata cached in memory
- Refreshed on startup and explicit refresh
- Used for tool support validation
- Persisted across sessions in configuration

### API Rate Management
**OpenRouter Integration**:
- Single client instance reused across requests
- No explicit rate limiting (relies on OpenRouter limits)
- Graceful degradation on API failures

## 7. Configuration State Management

### Thread Safety
**Concurrent Access**: Configuration protected by `RwLock<RuntimeConfig>`
- Multiple readers allowed simultaneously
- Exclusive writer access for updates
- Lock held minimally to prevent contention

### Consistency Guarantees
**Atomic Updates**: Configuration changes applied atomically
- Registry updates complete before event emission
- No partial state visible to other subsystems
- Rollback not implemented (restart required for corruption recovery)

### Default Management
**Curated Defaults**: Hardcoded model configurations merged at runtime
- User configs override defaults by provider ID
- Missing fields don't inherit from defaults (security)
- Defaults updated through code changes only

This analysis shows how model commands flow through a sophisticated parsing, validation, and execution pipeline that maintains configuration consistency while providing responsive user feedback.