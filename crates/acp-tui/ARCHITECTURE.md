# ACP TUI Architecture Documentation

## Overview

The ACP TUI application implements a modern, performance-optimized terminal user interface using React/Elm architecture principles. This document provides detailed technical information about the implementation, design decisions, and performance optimizations.

## Architecture Principles

### 1. Elm Architecture Pattern

The application strictly follows the Elm architecture pattern with three distinct layers:

```rust
// Model: Pure data structures representing application state
pub struct AppModel {
    pub component_dirty_flags: ComponentDirtyFlags,
    pub timeline_tree: TimelineTree,
    pub hitl_requests: Vec<HitlApprovalRequest>,
    // ... other state
}

// Update: Pure functions that transform state based on messages  
pub fn update_app(model: &mut AppModel, msg: AppMsg) -> Result<Vec<AppMsg>> {
    // Pure state transformations with side effect messages returned
}

// View: Functions that render UI based on current state
pub fn render_app(model: &AppModel, app: &mut Application, frame: &mut Frame) {
    // Pure rendering functions that don't modify state
}
```

### 2. Unidirectional Data Flow

All state changes flow through a single update cycle:

```
User Input → Message → Update Function → New State → View Update → UI Refresh
     ↑                                                                    ↓
External Events ←─────────────────────────────────────────────────────────┘
```

### 3. Component Isolation

Each UI component is isolated with its own:
- **State Management**: Component-specific state in `AppModel`
- **Message Handling**: Typed messages for component interactions
- **Rendering Logic**: Independent view functions
- **Performance Optimization**: Individual dirty flags

## Performance Architecture

### Smart Component Updates System

The dirty flag system provides fine-grained control over rendering:

```rust
pub struct ComponentDirtyFlags {
    pub timeline: bool,        // Timeline needs re-rendering
    pub query_input: bool,     // Query input changed
    pub status_line: bool,     // Status/connection changed
    pub hitl_queue: bool,      // HITL queue modified
    pub hitl_review: bool,     // HITL review modal updated
    pub help: bool,            // Help overlay toggled
}
```

**Performance Benefits:**
- **Reduced CPU Usage**: Only dirty components are rendered
- **Improved Responsiveness**: Faster frame rates for unchanged content
- **Battery Life**: Lower power consumption on laptops
- **Scalability**: Performance scales with active components, not total components

### Timeline Viewport Optimization

The timeline component implements several optimization strategies:

```rust
pub struct TimelineRealmComponent {
    // Viewport caching
    cached_lines: Option<Vec<String>>,
    cached_stats: Option<TreeStats>,
    tree_generation: usize,
    
    // Scroll optimization  
    scroll_offset: usize,
    max_display_lines: usize,
    
    // Animation limiting
    animation_tick: usize,
}
```

**Implementation Details:**
1. **Lazy Rendering**: Only visible lines are computed and styled
2. **Cache Invalidation**: Cache cleared only when tree structure changes  
3. **Viewport Limiting**: Rendering limited to visible area + small buffer
4. **Animation Gating**: Animations only run when there are active operations

### Message Batching System

High-frequency events are batched to prevent UI blocking:

```rust
// Collect additional messages if available (batching)
let mut messages = vec![msg];
while let Ok(additional_msg) = self.receiver.try_recv() {
    messages.push(additional_msg);
    // Limit batch size to prevent blocking
    if messages.len() >= 10 {
        break;
    }
}
```

**Benefits:**
- **Smooth UI**: Prevents message flooding from blocking rendering
- **Efficient Processing**: Batch processing reduces context switching
- **Bounded Latency**: Maximum 10 messages per batch prevents unbounded delays

## Component Architecture

### Timeline Component (`TimelineRealmComponent`)

**Responsibilities:**
- Visualize agent execution workflows as an expandable tree
- Handle real-time updates from WebSocket events  
- Provide smooth scrolling and navigation
- Cache rendered content for performance

**Key Features:**
```rust
impl TimelineRealmComponent {
    // Event processing with cache invalidation
    pub fn add_status_event(&mut self, event: StatusEvent) {
        self.invalidate_cache();  // Clear cache when data changes
        // Process event...
    }
    
    // Optimized rendering with viewport limiting
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let visible_lines: Vec<Line> = lines
            .iter()
            .skip(self.scroll_offset)
            .take(self.max_display_lines)  // Only render visible lines
            .map(|line| Line::from(self.style_line(line)))
            .collect();
    }
}
```

### Query Input Component (`QueryInputRealmComponent`)

**Responsibilities:**
- Handle multi-line query input
- Provide syntax highlighting (future enhancement)
- Manage query history and validation
- Submit queries to execution system

**Architecture:**
```rust
impl QueryInputRealmComponent {
    // State management
    text_buffer: String,
    cursor_position: usize, 
    history: Vec<String>,
    
    // Validation and submission
    pub fn validate_query(&self) -> Result<()> {
        // Query validation logic
    }
    
    pub fn submit_query(&mut self) -> Option<AppMsg> {
        // Generate query submission message
    }
}
```

### Status Line Component (`StatusLineRealmComponent`)

**Responsibilities:**
- Display connection status with visual indicators
- Show real-time status messages
- Provide performance metrics and statistics
- Handle connection state transitions

**State Management:**
```rust
#[derive(Debug, Clone)]
pub enum ConnectionState {
    Disconnected,
    Connecting, 
    Connected,
    Reconnecting,
    Failed { error: String },
}
```

### HITL Components

The Human-in-the-Loop system consists of two coordinated components:

**HitlQueueRealmComponent:**
- Display pending HITL requests in a scrollable list
- Show request metadata and priority
- Handle request selection and queue management

**HitlReviewRealmComponent:**
- Modal interface for reviewing individual requests
- Context display and decision options
- Reason input for custom decisions
- Integration with API for decision submission

## Service Layer Architecture

### ApiService

Provides a clean abstraction over the OpenAPI-generated client:

```rust
pub struct ApiService {
    client: Arc<AcpClient>,
    retry_config: RetryConfig,
}

impl ApiService {
    // Resilient HTTP operations with retry logic
    async fn retry_with_backoff<T, F, Fut>(&self, operation: F) -> Result<T>
    where F: Fn() -> Fut, Fut: Future<Output = Result<T>>
    {
        // Exponential backoff retry implementation
    }
    
    // High-level API operations
    pub async fn submit_query(&self, query: &str) -> Result<QueryResponse> {
        self.retry_with_backoff(|| async {
            self.client.client().query_task(&QueryRequest {
                query: query.to_string(),
            }).await
        }, "submit_query").await
    }
}
```

### WebSocketManager

Manages real-time communication with the ACP server:

```rust
pub struct WebSocketManager {
    connection_state: ConnectionState,
    reconnect_attempts: u32,
    max_reconnect_attempts: u32,
    message_sender: mpsc::UnboundedSender<AppMsg>,
}

impl WebSocketManager {
    // Automatic reconnection with exponential backoff
    pub async fn reconnect(&mut self) -> Result<()> {
        let delay = Duration::from_millis(
            2u64.pow(self.reconnect_attempts) * 1000
        );
        tokio::time::sleep(delay).await;
        self.connect().await
    }
    
    // Message parsing and routing
    async fn handle_message(&self, message: WebSocketMessage) -> Result<()> {
        match message {
            WebSocketMessage::StatusEvent(event) => {
                self.message_sender.send(AppMsg::StatusEventReceived(event))?;
            }
            // ... other message types
        }
    }
}
```

### QueryExecutor

Coordinates query execution across multiple services:

```rust
pub struct QueryExecutor {
    api_service: ApiService,
    websocket_manager: WebSocketManager,
    message_sender: mpsc::UnboundedSender<AppMsg>,
}

impl QueryExecutor {
    pub async fn execute_query(&self, query: String) -> Result<()> {
        // 1. Submit query via HTTP API
        let response = self.api_service.submit_query(&query).await?;
        
        // 2. Subscribe to WebSocket updates  
        self.websocket_manager.subscribe_to_execution(
            &response.subscription_id
        ).await?;
        
        // 3. Send success message
        self.message_sender.send(
            AppMsg::QueryExecutionStarted(query)
        )?;
        
        Ok(())
    }
}
```

## Configuration System

### Hierarchical Configuration

The configuration system supports multiple sources with precedence:

```rust
impl Config {
    pub fn load(
        config_file: Option<&String>,
        server_url: &str,
        log_level: &str,
    ) -> Result<Self> {
        let mut config = Config::default();
        
        // 1. Load from file (lowest precedence)
        if let Some(file) = config_file {
            config = Config::from_file(file)?;
        }
        
        // 2. Apply environment variables (medium precedence)
        config = config.merge_from_env()?;
        
        // 3. Apply command line args (highest precedence)
        config.server_url = server_url.to_string();
        config.log_level = log_level.to_string();
        
        // 4. Validate final configuration
        config.validate()?;
        
        Ok(config)
    }
}
```

### Configuration Structure

```rust
#[derive(Debug, Clone)]
pub struct Config {
    pub server_url: String,
    pub log_level: String,
    pub ui: UiConfig,
    pub network: NetworkConfig,
    pub performance: PerformanceConfig,
}

#[derive(Debug, Clone)]
pub struct UiConfig {
    pub animation_interval_ms: u64,
    pub scroll_buffer_lines: usize,
    pub max_timeline_nodes: usize,
}

#[derive(Debug, Clone)]  
pub struct NetworkConfig {
    pub connect_timeout_ms: u64,
    pub reconnect_delay_ms: u64,
    pub max_reconnect_attempts: u32,
}
```

## Error Handling Strategy

### Hierarchical Error Types

```rust
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),
    
    #[error("API client error: {0}")]
    Client(#[from] ClientError),
    
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] WebSocketError),
    
    #[error("UI error: {0}")]
    Ui(#[from] UiError),
}
```

### Error Recovery Patterns

1. **Graceful Degradation**: Failed connections don't crash the UI
2. **Automatic Retry**: Network operations retry with exponential backoff
3. **User Notification**: Errors are displayed in the status line
4. **State Preservation**: UI state is maintained across error conditions

## Threading and Concurrency

### Actor-Based Concurrency

The application uses an actor-like pattern with message passing:

```rust
// Main UI thread
async fn run(&mut self) -> Result<()> {
    loop {
        tokio::select! {
            // UI events (main thread)
            _ = tokio::time::sleep(Duration::from_millis(10)) => {
                self.handle_ui_events().await?;
            }
            
            // Background messages (service threads)
            msg = self.receiver.recv() => {
                self.handle_message(msg).await?;
            }
            
            // Periodic tasks
            _ = self.animation_timer.tick() => {
                self.handle_animation_tick().await?;
            }
        }
    }
}
```

### Thread Safety

- **Message Passing**: All cross-thread communication uses typed messages
- **Shared State**: Minimal shared state, mostly in `Arc<T>` containers
- **Lock-Free**: No explicit locking, relying on actor isolation

## Testing Strategy

### Unit Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_update_function_purity() {
        let mut model = AppModel::new("test".to_string());
        let initial_state = model.clone();
        
        // Update should be pure - same input, same output
        let effects1 = update_app(&mut model, AppMsg::Tick).unwrap();
        let state1 = model.clone();
        
        let mut model2 = initial_state;
        let effects2 = update_app(&mut model2, AppMsg::Tick).unwrap();
        let state2 = model2;
        
        assert_eq!(effects1, effects2);
        assert_eq!(state1, state2);
    }
}
```

### Integration Testing

```rust
#[tokio::test]
async fn test_full_query_cycle() {
    let mut app = Application::new(test_config()).await?;
    
    // Submit query
    app.handle_message(AppMsg::QuerySubmitted).await?;
    
    // Verify WebSocket connection attempted
    assert!(app.websocket_manager.is_connecting());
    
    // Simulate status events
    let status_event = StatusEvent { /* ... */ };
    app.handle_message(AppMsg::StatusEventReceived(status_event)).await?;
    
    // Verify timeline updated
    assert!(!app.model.timeline_tree.is_empty());
}
```

## Performance Monitoring

### Built-in Metrics

The application includes performance monitoring:

```rust
impl Application {
    fn collect_performance_metrics(&self) -> PerformanceMetrics {
        PerformanceMetrics {
            render_time_ms: self.last_render_duration.as_millis(),
            message_queue_size: self.receiver.len(),
            memory_usage_kb: self.estimate_memory_usage(),
            active_animations: self.model.timeline_tree.get_stats().running,
        }
    }
}
```

### Profiling Integration

For detailed profiling, the application supports:

```rust
#[cfg(feature = "profiling")]
use puffin;

#[cfg(feature = "profiling")]
fn profile_render() {
    puffin::profile_function!();
    // Rendering code...
}
```

## Future Enhancements

### Planned Optimizations

1. **Virtual Scrolling**: Implement virtual scrolling for extremely large timelines
2. **Background Parsing**: Move heavy parsing operations to background threads
3. **Incremental Updates**: Delta-based updates for large data structures
4. **GPU Acceleration**: Explore GPU-accelerated text rendering

### Architecture Evolution

1. **Plugin System**: Component-based plugin architecture for extensibility
2. **State Persistence**: Save/restore application state across sessions
3. **Multi-Window**: Support for multiple terminal windows/tabs
4. **Remote UI**: Web-based remote interface option

This architecture provides a solid foundation for a high-performance, maintainable terminal application while following established patterns from the React/Elm ecosystem.