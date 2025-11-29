//! # Agent Communication Protocol (ACP) API Server
//!
//! This crate implements a REST API server following the Agent Communication Protocol (ACP) 
//! standard for multi-agent orchestration and coordination.
//!
//! ## Features
//!
//! - **Asynchronous Execution**: Start background agent workflows and return immediately
//! - **Real-time Streaming**: WebSocket-based status updates during execution
//! - **Conversation Management**: Group related queries in persistent conversations
//! - **Multi-agent Coordination**: Automatic task routing and agent orchestration
//! - **OpenAPI Documentation**: Comprehensive API documentation with Swagger UI
//! - **Project-aware Processing**: Context-aware execution based on project characteristics
//!
//! ## Architecture
//!
//! The API server follows a layered architecture:
//!
//! ```text
//! ┌─────────────────────┐
//! │   REST Endpoints    │ <- /query, /capabilities, /health
//! ├─────────────────────┤
//! │   WebSocket Stream  │ <- /stream/{conversation_id}
//! ├─────────────────────┤
//! │  Execution Manager  │ <- Stateful stream management
//! ├─────────────────────┤
//! │    Orchestrator     │ <- Stateless workflow execution
//! ├─────────────────────┤
//! │   Agent Network     │ <- Multi-agent coordination
//! └─────────────────────┘
//! ```
//!
//! ## Usage Patterns
//!
//! ### Basic Query Execution
//!
//! 1. **POST** `/query` - Start async execution with project context
//! 2. Connect to **WebSocket** `/stream/{conversation_id}` for live updates
//! 3. Receive **StatusEvent** messages as agents work
//! 4. Get final results from **ExecutionCompleted** event
//!
//! ### Agent Discovery
//!
//! - **GET** `/capabilities` - Discover available agents and features
//! - **GET** `/health` - Check API server health and status
//!
//! ### Documentation Access
//!
//! - **GET** `/api-doc/openapi.json` - Raw OpenAPI specification
//! - Browse `/docs` - Interactive Swagger UI documentation
//!
//! ## Project Context
//!
//! Clients must provide `ProjectScope` information including:
//!
//! - **Root directory**: Absolute path to project root
//! - **Languages**: Detected programming languages and frameworks
//! - **Key files**: Important files with their purposes
//! - **Development areas**: Active areas of development focus
//!
//! This context enables agents to:
//! - Choose appropriate tools for the project type
//! - Understand codebase structure and patterns
//! - Provide relevant assistance and suggestions
//!
//! ## Error Handling
//!
//! The API uses structured error responses with:
//! - **HTTP status codes** for immediate API errors
//! - **ErrorResponse** JSON for detailed error information
//! - **WebSocket events** for execution-time errors
//! - **Machine-readable codes** for programmatic handling
//!
//! ## Standards Compliance
//!
//! This implementation follows the [Agent Communication Protocol](https://agentcommunicationprotocol.dev)
//! specification for agent interoperability and standardized communication patterns.

pub mod server;
pub mod routes;
pub mod middleware;
pub mod types;
pub mod openapi;

pub use server::AcpServer;
pub use types::*;