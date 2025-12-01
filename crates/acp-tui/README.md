# ACP TUI - Agent Communication Protocol Terminal Interface

A modern, high-performance terminal user interface for the Agent Communication Protocol (ACP), built with Rust and following React/Elm architecture principles.

## Features

🚀 **Modern Architecture**
- Elm/React-inspired Model-Update-View pattern
- Component-based UI with TUIRealm 3.2.0
- Smart dirty-flag rendering system for optimal performance
- Async/await throughout with tokio integration

⚡ **Performance Optimized**
- Conditional rendering with dirty component tracking
- Timeline viewport optimization with caching
- Animation frame limiting and message batching
- Optimized main loop polling (10ms intervals)

🔄 **Real-time Features**
- Live WebSocket connection for execution updates
- Real-time timeline visualization of agent workflows
- Human-in-the-Loop (HITL) approval system
- Status streaming with reconnection handling

🎨 **Rich UI Components**
- Interactive timeline tree with expand/collapse
- Query input with history and validation
- Status line with connection state indicators
- HITL review modal for decision making
- Context-sensitive help system

## Architecture Overview

### Elm Architecture Pattern

The application follows strict Elm architecture with clear separation of concerns:

```
┌─────────────────────────────────────────────────────────────┐
│                     APPLICATION LAYER                       │
├─────────────────────────────────────────────────────────────┤
│  Model (State)     │  Update (Logic)    │  View (Rendering) │
│  - AppModel        │  - update_app()    │  - render_app()   │
│  - Dirty flags     │  - Message routing │  - Layout mgmt    │
│  - Component state │  - Side effects    │  - Component sync │
└─────────────────────────────────────────────────────────────┘
│
├── SERVICES LAYER (Business Logic)
│   ├── ApiService (HTTP client operations)
│   ├── QueryExecutor (Query execution coordination)
│   └── WebSocketManager (Real-time communication)
│
├── COMPONENTS LAYER (UI Components)
│   ├── TimelineRealmComponent (Execution visualization)
│   ├── QueryInputRealmComponent (Query entry)
│   ├── StatusLineRealmComponent (Status display)
│   ├── HitlReviewRealmComponent (HITL decision UI)
│   └── HitlQueueRealmComponent (HITL queue)
│
└── INFRASTRUCTURE LAYER
    ├── Client (OpenAPI-generated ACP client)
    ├── Config (Configuration management)
    ├── Models (Data structures)
    └── Utils (Helper functions)
```

### Performance Optimizations

#### Smart Component Updates
- **Dirty Flag System**: Each component has a dirty flag tracking when it needs re-rendering
- **Selective Rendering**: Only dirty components are rendered each frame
- **Automatic Flag Management**: Flags are set based on message types and cleared after rendering

#### Timeline Optimization
- **Viewport Caching**: Rendered lines are cached until tree changes
- **Lazy Rendering**: Only visible items are rendered with scroll offsets
- **Animation Limiting**: Animation frames only generated when animations are active

#### Message Processing
- **Batching**: Multiple messages are processed in single batches (max 10)
- **Async Coordination**: Side effects handled separately from pure model updates
- **Graceful Degradation**: Connection failures don't crash the UI

## Quick Start

### Prerequisites

- Rust 1.70+ with async/await support
- ACP server running (default: http://localhost:9999)
- Terminal with Unicode and color support

### Installation

```bash
# Clone the repository
git clone <repository-url>
cd q/crates/acp-tui

# Build the application
cargo build --release

# Run with default settings
cargo run --release

# Run with custom server
cargo run --release -- --server http://your-acp-server:8080
```

### Configuration

The application supports multiple configuration methods:

1. **Command Line Arguments**:
   ```bash
   cargo run -- --server http://localhost:9999 --log-level debug
   ```

2. **Configuration File**:
   ```toml
   # config.toml
   server_url = "http://localhost:9999"
   log_level = "info"
   
   [ui]
   animation_interval_ms = 100
   reconnect_delay_ms = 5000
   max_reconnect_attempts = 10
   ```

3. **Environment Variables**:
   ```bash
   export RUST_LOG=debug
   export ACP_SERVER_URL=http://localhost:9999
   ```

## Usage Guide

### Keyboard Navigation

| Key | Action |
|-----|--------|
| `Tab` | Focus next component |
| `Shift+Tab` | Focus previous component |
| `↑/↓` | Scroll timeline or navigate |
| `PgUp/PgDn` | Fast scroll timeline |
| `Enter` | Submit query or confirm action |
| `c` | Clear timeline |
| `?` | Toggle help overlay |
| `q` | Quit application |
| `Ctrl+C` | Force quit |

### Query Execution

1. Focus the query input (bottom panel)
2. Type your query (supports multi-line)
3. Press `Enter` to submit
4. Watch real-time execution in the timeline

### HITL (Human-in-the-Loop) Workflow

1. When agents request human approval, HITL requests appear in queue
2. Select a request to open the review modal
3. Choose `Approve`, `Reject`, or `Modify` with optional reason
4. Decision is sent to the agent for processing

### Timeline Navigation

- **Expand/Collapse**: Click or use arrow keys on tree nodes
- **Scroll**: Use arrow keys or Page Up/Down for large timelines
- **Status Indicators**:
  - `⟳` Running/In Progress
  - `✓` Completed Successfully
  - `✗` Failed/Error
  - `⚠` Warning

## Component Architecture

### Core Components

#### TimelineRealmComponent
- Visualizes execution workflow as an expandable tree
- Real-time updates from WebSocket events
- Cached rendering for performance
- Scroll viewport optimization

#### QueryInputRealmComponent  
- Multi-line text input with syntax highlighting
- Query validation and history
- Auto-completion support (future)

#### StatusLineRealmComponent
- Connection status indicators
- Real-time status messages
- Performance metrics display

#### HitlReviewRealmComponent
- Modal for HITL decision making
- Context display and decision options
- Reason input for modifications

### Message System

All communication uses a typed message system:

```rust
pub enum AppMsg {
    // System events
    Quit,
    TerminalResized(u16, u16),
    Tick,
    
    // Connection events  
    WebSocketConnected,
    WebSocketDisconnected,
    ConnectionFailed(String),
    
    // Query events
    QueryInputChanged(String),
    QuerySubmitted,
    QueryExecutionStarted(String),
    
    // Timeline events
    StatusEventReceived(StatusEvent),
    TimelineScrollUp,
    TimelineScrollDown,
    
    // HITL events
    HitlRequestReceived(HitlApprovalRequest),
    HitlDecisionMade(String, HitlDecisionRequest),
    
    // UI navigation
    FocusNext,
    FocusPrevious,
    HelpToggle,
}
```

## Development

### Building

```bash
# Debug build with full logging
cargo build

# Release build (optimized)
cargo build --release

# Run tests
cargo test

# Run with specific log level
RUST_LOG=debug cargo run
```

### Code Structure

```
src/
├── application/           # Core Elm architecture
│   ├── mod.rs            # Main application loop
│   ├── state.rs          # AppModel and state management
│   ├── update.rs         # Message processing and updates
│   └── view.rs           # UI rendering and layout
├── components/           # TUIRealm components
│   ├── realm/           # Realm-based components
│   └── mod.rs           # Component trait definitions
├── services/            # Business logic layer
│   ├── api.rs           # HTTP API operations
│   ├── query_executor.rs # Query execution coordination
│   └── websocket_manager.rs # WebSocket handling
├── client.rs            # Generated OpenAPI client wrapper
├── config.rs            # Configuration management
├── models/              # Data structures
├── utils/               # Helper functions
└── main.rs              # Application entry point
```

### Key Design Patterns

1. **Elm Architecture**: Unidirectional data flow with pure update functions
2. **Component Isolation**: Each component manages its own state and rendering
3. **Service Layer**: Business logic separated from UI concerns
4. **Message-Driven**: All state changes happen through typed messages
5. **Performance-First**: Optimizations built in from the ground up

### Contributing Guidelines

1. **Architecture Compliance**: Follow Elm pattern strictly
2. **Performance**: Consider dirty flags and caching for any new components
3. **Error Handling**: Use `anyhow::Result` and proper error propagation
4. **Testing**: Add unit tests for model updates and component logic
5. **Documentation**: Update this README for any architectural changes

## Troubleshooting

### Common Issues

**Connection Failed**
- Verify ACP server is running at the configured URL
- Check network connectivity and firewall settings
- Ensure server supports the required API version

**Performance Issues**
- Reduce animation interval in configuration
- Check terminal emulator performance
- Monitor log files for error patterns

**Display Problems**
- Ensure terminal supports Unicode and 256 colors
- Check terminal size (minimum 80x24 recommended)
- Update terminal emulator if rendering is corrupted

### Debug Mode

Enable detailed logging for troubleshooting:

```bash
RUST_LOG=debug cargo run 2>&1 | tee debug.log
```

Log files are automatically created in the current directory with daily rotation.

## Performance Benchmarks

The optimized implementation achieves:
- **< 10ms** UI update latency for most operations
- **< 1ms** component rendering for cached content  
- **< 100ms** WebSocket message processing
- **< 5MB** memory usage for typical workloads

## License

This project is part of the Agent Communication Protocol implementation suite.