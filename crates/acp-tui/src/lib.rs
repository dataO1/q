//! # ACP TUI Library
//!
//! A terminal user interface library for the Agent Communication Protocol (ACP).
//! This library provides a modern, reactive TUI for interacting with ACP servers,
//! featuring real-time orchestration visualization and streaming updates.
//!
//! ## Architecture
//!
//! The library follows a component-based architecture inspired by React/Elm:
//!
//! - **Components**: Reusable UI widgets with their own state and props
//! - **State Management**: Centralized application state with message-driven updates
//! - **Event System**: Async event handling with proper error propagation
//! - **Client Layer**: Type-safe API client generated from OpenAPI specifications
//!
//! ## Example Usage
//!
//! ```rust,no_run
//! use acp_tui::{App, Config};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let config = Config::default();
//!     let mut app = App::new(config).await?;
//!     app.run().await?;
//!     Ok(())
//! }
//! ```

#![warn(missing_docs)]
#![warn(clippy::missing_docs_in_private_items)]
#![warn(clippy::unwrap_used)]
#![warn(clippy::expect_used)]

// pub mod app;
pub mod client;
// pub mod components;
pub mod config;
pub mod error;
// pub mod events;
pub mod models;
// pub mod ui;
pub mod utils;
// pub mod websocket;

// Re-export main types for convenience
// pub use app::App;
pub use config::Config;
pub use error::{Error, Result};
pub use models::AppState;