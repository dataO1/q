//! LLM client abstraction layer
//!
//! Provides a unified interface for LLM interactions with streaming support
//! and Ollama compatibility through async-openai.

pub mod client;
pub mod streaming;
pub mod tools;

pub use client::{LLMClient, LLMClientConfig};
pub use streaming::{StreamingResponse, StreamingError};
pub use tools::{ToolCall, ToolCallResult, ToolDefinition};