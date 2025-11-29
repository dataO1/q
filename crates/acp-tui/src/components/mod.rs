//! UI Components for the ACP TUI application
//!
//! This module contains all UI components following React/Elm-style architecture
//! with message-driven state transitions.

pub mod status_line;
pub mod timeline;

pub use status_line::{StatusLine, StatusLineMessage, StatusMessage, StatusSeverity};
pub use timeline::{TimelineComponent, TimelineMessage};