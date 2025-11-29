//! Status line widget component for displaying user-actionable errors and warnings
//!
//! This widget shows the latest error or warning in a single line below the query input,
//! with color coding based on severity level.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use tracing::{debug, instrument};
use chrono::{DateTime, Utc};

/// Severity levels for status messages
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum StatusSeverity {
    /// Informational message (blue)
    Info,
    /// Warning that doesn't block operation (yellow)
    Warning,
    /// Error that blocks or fails operation (red)
    Error,
    /// Critical system error (bright red)
    Critical,
}

impl StatusSeverity {
    /// Get the color associated with this severity level
    pub fn color(&self) -> Color {
        match self {
            StatusSeverity::Info => Color::Blue,
            StatusSeverity::Warning => Color::Yellow,
            StatusSeverity::Error => Color::Red,
            StatusSeverity::Critical => Color::LightRed,
        }
    }

    /// Get the prefix symbol for this severity level
    pub fn symbol(&self) -> &'static str {
        match self {
            StatusSeverity::Info => "ℹ",
            StatusSeverity::Warning => "⚠",
            StatusSeverity::Error => "✗",
            StatusSeverity::Critical => "🔴",
        }
    }
}

/// A status message to display
#[derive(Debug, Clone)]
pub struct StatusMessage {
    /// The severity level of this message
    pub severity: StatusSeverity,
    /// The message text to display
    pub message: String,
    /// When this message was created
    pub timestamp: DateTime<Utc>,
    /// Optional error code for programmatic handling
    pub error_code: Option<String>,
}

impl StatusMessage {
    /// Create a new info message
    pub fn info(message: impl Into<String>) -> Self {
        Self {
            severity: StatusSeverity::Info,
            message: message.into(),
            timestamp: Utc::now(),
            error_code: None,
        }
    }

    /// Create a new warning message
    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            severity: StatusSeverity::Warning,
            message: message.into(),
            timestamp: Utc::now(),
            error_code: None,
        }
    }

    /// Create a new error message
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            severity: StatusSeverity::Error,
            message: message.into(),
            timestamp: Utc::now(),
            error_code: None,
        }
    }

    /// Create a new critical error message
    pub fn critical(message: impl Into<String>) -> Self {
        Self {
            severity: StatusSeverity::Critical,
            message: message.into(),
            timestamp: Utc::now(),
            error_code: None,
        }
    }

    /// Create an error message with an error code
    pub fn error_with_code(message: impl Into<String>, code: impl Into<String>) -> Self {
        Self {
            severity: StatusSeverity::Error,
            message: message.into(),
            timestamp: Utc::now(),
            error_code: Some(code.into()),
        }
    }

    /// Create a critical error message with an error code
    pub fn critical_with_code(message: impl Into<String>, code: impl Into<String>) -> Self {
        Self {
            severity: StatusSeverity::Critical,
            message: message.into(),
            timestamp: Utc::now(),
            error_code: Some(code.into()),
        }
    }

    /// Get the formatted display text for this message
    pub fn display_text(&self) -> String {
        if let Some(ref code) = self.error_code {
            format!("{} {} [{}]", self.severity.symbol(), self.message, code)
        } else {
            format!("{} {}", self.severity.symbol(), self.message)
        }
    }
}

/// Status line component for displaying the latest actionable message
#[derive(Debug)]
pub struct StatusLine {
    /// Current status message to display, if any
    current_message: Option<StatusMessage>,
    /// Whether to show the status line even when no message is present
    always_visible: bool,
}

impl Default for StatusLine {
    fn default() -> Self {
        Self::new()
    }
}

impl StatusLine {
    /// Create a new status line component
    pub fn new() -> Self {
        Self {
            current_message: None,
            always_visible: false,
        }
    }

    /// Create a new status line that's always visible
    pub fn always_visible() -> Self {
        Self {
            current_message: None,
            always_visible: true,
        }
    }

    /// Set the current status message
    #[instrument(skip(self))]
    pub fn set_message(&mut self, message: StatusMessage) {
        debug!(
            severity = ?message.severity,
            message = %message.message,
            code = ?message.error_code,
            "Setting status line message"
        );
        self.current_message = Some(message);
    }

    /// Clear the current status message
    #[instrument(skip(self))]
    pub fn clear(&mut self) {
        debug!("Clearing status line message");
        self.current_message = None;
    }

    /// Get the current message, if any
    pub fn current_message(&self) -> Option<&StatusMessage> {
        self.current_message.as_ref()
    }

    /// Check if the status line should be rendered
    pub fn should_render(&self) -> bool {
        self.always_visible || self.current_message.is_some()
    }

    /// Render the status line widget
    #[instrument(skip(self, f))]
    pub fn render(&self, f: &mut Frame, area: Rect) {
        if !self.should_render() {
            return;
        }

        let (text, style) = if let Some(ref message) = self.current_message {
            let text = message.display_text();
            let style = Style::default()
                .fg(message.severity.color())
                .add_modifier(Modifier::BOLD);
            (text, style)
        } else {
            // Show placeholder when always visible but no message
            let text = "Ready".to_string();
            let style = Style::default()
                .fg(Color::DarkGray);
            (text, style)
        };

        let paragraph = Paragraph::new(text)
            .style(style)
            .block(Block::default().borders(Borders::NONE));

        f.render_widget(paragraph, area);
    }
}

/// Messages for updating the status line component
#[derive(Debug, Clone)]
pub enum StatusLineMessage {
    /// Set a new status message
    SetMessage(StatusMessage),
    /// Clear the current message
    Clear,
    /// Set an info message
    Info(String),
    /// Set a warning message
    Warning(String),
    /// Set an error message
    Error(String),
    /// Set a critical error message
    Critical(String),
    /// Set an error with error code
    ErrorWithCode(String, String),
    /// Set a critical error with error code
    CriticalWithCode(String, String),
}

impl StatusLine {
    /// Handle a status line message
    #[instrument(skip(self))]
    pub fn handle_message(&mut self, message: StatusLineMessage) {
        match message {
            StatusLineMessage::SetMessage(msg) => self.set_message(msg),
            StatusLineMessage::Clear => self.clear(),
            StatusLineMessage::Info(text) => self.set_message(StatusMessage::info(text)),
            StatusLineMessage::Warning(text) => self.set_message(StatusMessage::warning(text)),
            StatusLineMessage::Error(text) => self.set_message(StatusMessage::error(text)),
            StatusLineMessage::Critical(text) => self.set_message(StatusMessage::critical(text)),
            StatusLineMessage::ErrorWithCode(text, code) => {
                self.set_message(StatusMessage::error_with_code(text, code))
            }
            StatusLineMessage::CriticalWithCode(text, code) => {
                self.set_message(StatusMessage::critical_with_code(text, code))
            }
        }
    }
}