//! Error handling for the ACP TUI application
//!
//! This module provides a comprehensive error type that covers all possible
//! error conditions in the application, with proper error chaining and
//! context information for debugging.

use thiserror::Error;

/// Result type alias using the application's error type
pub type Result<T> = std::result::Result<T, Error>;

/// Main error type for the ACP TUI application
#[derive(Error, Debug)]
pub enum Error {
    /// Configuration-related errors
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    /// Network and API client errors
    #[error("Client error: {0}")]
    Client(#[from] ClientError),

    /// WebSocket connection errors
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] WebSocketError),

    /// UI and terminal errors
    #[error("UI error: {0}")]
    Ui(#[from] UiError),

    /// I/O errors (file operations, etc.)
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization/deserialization errors
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    /// Generic error with context
    #[error("Application error: {message}")]
    Generic {
        /// Error message
        message: String,
        /// Optional source error
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}

/// Configuration-specific errors
#[derive(Error, Debug)]
pub enum ConfigError {
    /// Invalid configuration value
    #[error("Invalid configuration value for '{field}': {value}")]
    InvalidValue {
        /// Configuration field name
        field: String,
        /// Invalid value
        value: String,
    },

    /// Missing required configuration
    #[error("Missing required configuration: {field}")]
    MissingRequired {
        /// Required field name
        field: String,
    },

    /// Configuration file parsing error
    #[error("Failed to parse configuration file '{path}'")]
    ParseError {
        /// Configuration file path
        path: String,
        /// Parse error source
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// Configuration validation error
    #[error("Configuration validation failed: {message}")]
    ValidationError {
        /// Validation error message
        message: String,
    },
}

/// Client and API-specific errors
#[derive(Error, Debug)]
pub enum ClientError {
    /// Server connection failure
    #[error("Failed to connect to ACP server at '{url}'")]
    ConnectionFailed {
        /// Server URL
        url: String,
        /// Connection error source
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// HTTP request failed
    #[error("HTTP request failed: {method} {url} -> {status}")]
    HttpError {
        /// HTTP method
        method: String,
        /// Request URL
        url: String,
        /// HTTP status code
        status: u16,
        /// Response body (if available)
        body: Option<String>,
    },

    /// API response parsing error
    #[error("Failed to parse API response from '{endpoint}'")]
    ParseError {
        /// API endpoint
        endpoint: String,
        /// Parse error source
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// Authentication error
    #[error("Authentication failed: {message}")]
    AuthError {
        /// Auth error message
        message: String,
    },

    /// Rate limiting error
    #[error("Rate limited by server, retry after {retry_after}s")]
    RateLimited {
        /// Retry after seconds
        retry_after: u64,
    },
}

/// WebSocket-specific errors
#[derive(Error, Debug)]
pub enum WebSocketError {
    /// Connection failure
    #[error("WebSocket connection failed to '{url}'")]
    ConnectionFailed {
        /// WebSocket URL
        url: String,
        /// Connection error source
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// Message parsing error
    #[error("Failed to parse WebSocket message")]
    MessageParseError {
        /// Parse error source
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// Connection dropped unexpectedly
    #[error("WebSocket connection dropped: {reason}")]
    ConnectionDropped {
        /// Drop reason
        reason: String,
    },

    /// Reconnection failed
    #[error("WebSocket reconnection failed after {attempts} attempts")]
    ReconnectFailed {
        /// Number of failed attempts
        attempts: usize,
    },
}

/// UI and terminal-specific errors
#[derive(Error, Debug)]
pub enum UiError {
    /// Terminal setup error
    #[error("Failed to initialize terminal")]
    TerminalInit {
        /// Terminal init error source
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// Rendering error
    #[error("Failed to render UI component '{component}'")]
    RenderError {
        /// Component name
        component: String,
        /// Render error source
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Event handling error
    #[error("Failed to handle UI event: {event}")]
    EventError {
        /// Event description
        event: String,
        /// Event error source
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Component state error
    #[error("Component state error in '{component}': {message}")]
    StateError {
        /// Component name
        component: String,
        /// Error message
        message: String,
    },
}

impl Error {
    /// Create a generic error with a message
    pub fn generic(message: impl Into<String>) -> Self {
        Self::Generic {
            message: message.into(),
            source: None,
        }
    }

    /// Create a generic error with a message and source
    pub fn generic_with_source(
        message: impl Into<String>,
        source: impl Into<Box<dyn std::error::Error + Send + Sync>>,
    ) -> Self {
        Self::Generic {
            message: message.into(),
            source: Some(source.into()),
        }
    }

    /// Check if this error is recoverable (e.g., network issues)
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Error::Client(ClientError::ConnectionFailed { .. })
                | Error::Client(ClientError::RateLimited { .. })
                | Error::WebSocket(WebSocketError::ConnectionFailed { .. })
                | Error::WebSocket(WebSocketError::ConnectionDropped { .. })
        )
    }

    /// Get a user-friendly error message
    pub fn user_message(&self) -> String {
        match self {
            Error::Client(ClientError::ConnectionFailed { url, .. }) => {
                format!("Unable to connect to ACP server at {}", url)
            }
            Error::WebSocket(WebSocketError::ConnectionFailed { .. }) => {
                "Lost connection to server. Attempting to reconnect...".to_string()
            }
            Error::Config(ConfigError::InvalidValue { field, .. }) => {
                format!("Invalid configuration for {}", field)
            }
            _ => "An unexpected error occurred".to_string(),
        }
    }
}

/// Helper macro for creating context-aware errors
#[macro_export]
macro_rules! context_error {
    ($context:expr, $err:expr) => {
        $crate::Error::generic_with_source($context, $err)
    };
}

/// Helper macro for creating simple generic errors
#[macro_export]
macro_rules! app_error {
    ($msg:expr) => {
        $crate::Error::generic($msg)
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::Error::generic(format!($fmt, $($arg)*))
    };
}

impl From<reqwest::Error> for ClientError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_connect() {
            ClientError::ConnectionFailed {
                url: err.url().map(|u| u.to_string()).unwrap_or_default(),
                source: Box::new(err),
            }
        } else if err.is_status() {
            ClientError::HttpError {
                method: "Unknown".to_string(),
                url: err.url().map(|u| u.to_string()).unwrap_or_default(),
                status: err.status().map(|s| s.as_u16()).unwrap_or(0),
                body: None,
            }
        } else {
            ClientError::ParseError {
                endpoint: err.url().map(|u| u.to_string()).unwrap_or_default(),
                source: Box::new(err),
            }
        }
    }
}

impl From<url::ParseError> for ConfigError {
    fn from(err: url::ParseError) -> Self {
        ConfigError::InvalidValue {
            field: "url".to_string(),
            value: err.to_string(),
        }
    }
}