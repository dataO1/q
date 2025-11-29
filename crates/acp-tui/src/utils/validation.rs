//! Input validation utilities

use crate::error::{Error, Result};
use url::Url;

/// Validate a server URL
pub fn validate_server_url(url: &str) -> Result<()> {
    let parsed = Url::parse(url)
        .map_err(|_e| Error::Config(crate::error::ConfigError::InvalidValue {
            field: "server_url".to_string(),
            value: url.to_string(),
        }))?;
    
    // Check that scheme is http or https
    match parsed.scheme() {
        "http" | "https" => Ok(()),
        scheme => Err(Error::Config(crate::error::ConfigError::InvalidValue {
            field: "server_url".to_string(),
            value: format!("Unsupported scheme: {}", scheme),
        })),
    }
}

/// Validate a query string
pub fn validate_query(query: &str) -> Result<()> {
    if query.trim().is_empty() {
        return Err(Error::Generic {
            message: "Query cannot be empty".to_string(),
            source: None,
        });
    }
    
    if query.len() > 10_000 {
        return Err(Error::Generic {
            message: "Query is too long (max 10,000 characters)".to_string(),
            source: None,
        });
    }
    
    Ok(())
}

/// Validate a conversation ID
pub fn validate_conversation_id(id: &str) -> Result<()> {
    if id.is_empty() {
        return Err(Error::Generic {
            message: "Conversation ID cannot be empty".to_string(),
            source: None,
        });
    }
    
    // Basic UUID format validation (loose)
    if id.len() < 32 || id.len() > 36 {
        return Err(Error::Generic {
            message: "Invalid conversation ID format".to_string(),
            source: None,
        });
    }
    
    // Check that it only contains valid UUID characters
    let valid_chars = id.chars().all(|c| c.is_ascii_hexdigit() || c == '-');
    if !valid_chars {
        return Err(Error::Generic {
            message: "Conversation ID contains invalid characters".to_string(),
            source: None,
        });
    }
    
    Ok(())
}

/// Validate a file path
pub fn validate_file_path(path: &str) -> Result<()> {
    if path.is_empty() {
        return Err(Error::Generic {
            message: "File path cannot be empty".to_string(),
            source: None,
        });
    }
    
    // Check for null bytes (security issue in some contexts)
    if path.contains('\0') {
        return Err(Error::Generic {
            message: "File path cannot contain null bytes".to_string(),
            source: None,
        });
    }
    
    // Check for extremely long paths
    if path.len() > 4096 {
        return Err(Error::Generic {
            message: "File path is too long".to_string(),
            source: None,
        });
    }
    
    Ok(())
}

/// Validate a port number
pub fn validate_port(port: u16) -> Result<()> {
    if port == 0 {
        return Err(Error::Config(crate::error::ConfigError::InvalidValue {
            field: "port".to_string(),
            value: "0".to_string(),
        }));
    }
    
    if port < 1024 && port != 80 && port != 443 {
        return Err(Error::Config(crate::error::ConfigError::InvalidValue {
            field: "port".to_string(),
            value: format!("Port {} may require elevated privileges", port),
        }));
    }
    
    Ok(())
}

/// Validate a timeout value in seconds
pub fn validate_timeout_secs(timeout: u64) -> Result<()> {
    if timeout == 0 {
        return Err(Error::Config(crate::error::ConfigError::InvalidValue {
            field: "timeout".to_string(),
            value: "0".to_string(),
        }));
    }
    
    if timeout > 3600 {
        return Err(Error::Config(crate::error::ConfigError::InvalidValue {
            field: "timeout".to_string(),
            value: format!("Timeout of {} seconds is too long (max 1 hour)", timeout),
        }));
    }
    
    Ok(())
}

/// Validate a log level string
pub fn validate_log_level(level: &str) -> Result<()> {
    match level.to_lowercase().as_str() {
        "trace" | "debug" | "info" | "warn" | "error" | "off" => Ok(()),
        _ => Err(Error::Config(crate::error::ConfigError::InvalidValue {
            field: "log_level".to_string(),
            value: format!("Unknown log level: {}", level),
        })),
    }
}

/// Validate a color string (for terminal colors)
pub fn validate_color(color: &str) -> Result<()> {
    match color.to_lowercase().as_str() {
        "black" | "red" | "green" | "yellow" | "blue" | "magenta" | "cyan" | "white" |
        "gray" | "darkgray" | "lightred" | "lightgreen" | "lightyellow" | 
        "lightblue" | "lightmagenta" | "lightcyan" | "lightwhite" => Ok(()),
        _ if color.starts_with('#') && color.len() == 7 => {
            // Validate hex color
            let hex_part = &color[1..];
            if hex_part.chars().all(|c| c.is_ascii_hexdigit()) {
                Ok(())
            } else {
                Err(Error::Config(crate::error::ConfigError::InvalidValue {
                    field: "color".to_string(),
                    value: format!("Invalid hex color: {}", color),
                }))
            }
        }
        _ if color.starts_with("rgb(") && color.ends_with(')') => {
            // Basic RGB validation
            Ok(())
        }
        _ => Err(Error::Config(crate::error::ConfigError::InvalidValue {
            field: "color".to_string(),
            value: format!("Unknown color: {}", color),
        })),
    }
}

/// Sanitize user input for display
pub fn sanitize_for_display(input: &str) -> String {
    input
        .chars()
        .filter(|c| c.is_ascii() && !c.is_control() || *c == '\n' || *c == '\t')
        .collect()
}

/// Validate that a string is a valid identifier (alphanumeric + underscore)
pub fn validate_identifier(id: &str) -> Result<()> {
    if id.is_empty() {
        return Err(Error::Generic {
            message: "Identifier cannot be empty".to_string(),
            source: None,
        });
    }
    
    if !id.chars().next().unwrap().is_ascii_alphabetic() && !id.starts_with('_') {
        return Err(Error::Generic {
            message: "Identifier must start with a letter or underscore".to_string(),
            source: None,
        });
    }
    
    if !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(Error::Generic {
            message: "Identifier can only contain letters, numbers, and underscores".to_string(),
            source: None,
        });
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_validate_server_url() {
        assert!(validate_server_url("http://localhost:8080").is_ok());
        assert!(validate_server_url("https://api.example.com").is_ok());
        assert!(validate_server_url("ftp://invalid.com").is_err());
        assert!(validate_server_url("not-a-url").is_err());
    }
    
    #[test]
    fn test_validate_query() {
        assert!(validate_query("Hello world").is_ok());
        assert!(validate_query("").is_err());
        assert!(validate_query("   ").is_err());
        assert!(validate_query(&"x".repeat(20000)).is_err());
    }
    
    #[test]
    fn test_validate_conversation_id() {
        assert!(validate_conversation_id("550e8400-e29b-41d4-a716-446655440000").is_ok());
        assert!(validate_conversation_id("550e8400e29b41d4a716446655440000").is_ok());
        assert!(validate_conversation_id("").is_err());
        assert!(validate_conversation_id("invalid-uuid").is_err());
        assert!(validate_conversation_id("550e8400-e29b-41d4-a716-44665544000z").is_err());
    }
    
    #[test]
    fn test_validate_log_level() {
        assert!(validate_log_level("info").is_ok());
        assert!(validate_log_level("DEBUG").is_ok());
        assert!(validate_log_level("invalid").is_err());
    }
    
    #[test]
    fn test_validate_color() {
        assert!(validate_color("red").is_ok());
        assert!(validate_color("lightblue").is_ok());
        assert!(validate_color("#FF0000").is_ok());
        assert!(validate_color("#ff0000").is_ok());
        assert!(validate_color("rgb(255,0,0)").is_ok());
        assert!(validate_color("invalid").is_err());
        assert!(validate_color("#GG0000").is_err());
    }
    
    #[test]
    fn test_sanitize_for_display() {
        assert_eq!(sanitize_for_display("Hello\x00World"), "HelloWorld");
        assert_eq!(sanitize_for_display("Normal text"), "Normal text");
        assert_eq!(sanitize_for_display("Text\nwith\nnewlines"), "Text\nwith\nnewlines");
    }
    
    #[test]
    fn test_validate_identifier() {
        assert!(validate_identifier("hello").is_ok());
        assert!(validate_identifier("_private").is_ok());
        assert!(validate_identifier("var123").is_ok());
        assert!(validate_identifier("").is_err());
        assert!(validate_identifier("123abc").is_err());
        assert!(validate_identifier("hello-world").is_err());
    }
}