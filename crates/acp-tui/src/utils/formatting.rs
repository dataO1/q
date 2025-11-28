//! Text and data formatting utilities

use chrono::{DateTime, Utc};
use std::time::Duration;

/// Format a timestamp for display in the UI
pub fn format_timestamp(timestamp: &DateTime<Utc>) -> String {
    timestamp.format("%H:%M:%S").to_string()
}

/// Format a timestamp with date for detailed view
pub fn format_timestamp_detailed(timestamp: &DateTime<Utc>) -> String {
    timestamp.format("%Y-%m-%d %H:%M:%S").to_string()
}

/// Format a duration for human-readable display
pub fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    let millis = duration.subsec_millis();
    
    if secs >= 60 {
        let mins = secs / 60;
        let remaining_secs = secs % 60;
        format!("{}m {}s", mins, remaining_secs)
    } else if secs > 0 {
        format!("{}.{}s", secs, millis / 100)
    } else {
        format!("{}ms", millis)
    }
}

/// Truncate text to fit within a given width, adding ellipsis if needed
pub fn truncate_text(text: &str, max_width: usize) -> String {
    if text.len() <= max_width {
        text.to_string()
    } else if max_width <= 3 {
        "...".to_string()
    } else {
        format!("{}...", &text[..max_width - 3])
    }
}

/// Wrap text to fit within a given width, preserving word boundaries
pub fn wrap_text(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![String::new()];
    }
    
    let mut lines = Vec::new();
    let mut current_line = String::new();
    
    for word in text.split_whitespace() {
        if current_line.is_empty() {
            current_line = word.to_string();
        } else if current_line.len() + 1 + word.len() <= width {
            current_line.push(' ');
            current_line.push_str(word);
        } else {
            lines.push(current_line);
            current_line = word.to_string();
        }
    }
    
    if !current_line.is_empty() {
        lines.push(current_line);
    }
    
    if lines.is_empty() {
        lines.push(String::new());
    }
    
    lines
}

/// Format a percentage value for display
pub fn format_percentage(value: f32) -> String {
    format!("{:.1}%", value * 100.0)
}

/// Format a file size in human-readable format
pub fn format_file_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;
    
    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }
    
    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else {
        format!("{:.1} {}", size, UNITS[unit_index])
    }
}

/// Pluralize a noun based on count
pub fn pluralize(count: usize, singular: &str, plural: Option<&str>) -> String {
    let plural_form = format!("{}s", singular);
    let noun = if count == 1 {
        singular
    } else {
        plural.unwrap_or(&plural_form)
    };
    format!("{} {}", count, noun)
}

/// Format a status message with appropriate styling hints
pub fn format_status_message(status: &str, is_success: bool) -> String {
    let prefix = if is_success { "✓" } else { "✗" };
    format!("{} {}", prefix, status)
}

/// Center text within a given width
pub fn center_text(text: &str, width: usize) -> String {
    let text_len = text.len();
    if text_len >= width {
        text.to_string()
    } else {
        let padding = width - text_len;
        let left_pad = padding / 2;
        let right_pad = padding - left_pad;
        format!("{}{}{}", " ".repeat(left_pad), text, " ".repeat(right_pad))
    }
}

/// Pad text to the right with spaces to reach a target width
pub fn pad_right(text: &str, width: usize) -> String {
    if text.len() >= width {
        text.to_string()
    } else {
        format!("{}{}", text, " ".repeat(width - text.len()))
    }
}

/// Pad text to the left with spaces to reach a target width
pub fn pad_left(text: &str, width: usize) -> String {
    if text.len() >= width {
        text.to_string()
    } else {
        format!("{}{}", " ".repeat(width - text.len()), text)
    }
}

/// Format a conversation ID for display (show only first 8 characters)
pub fn format_conversation_id(id: &str) -> String {
    if id.len() > 8 {
        format!("{}...", &id[..8])
    } else {
        id.to_string()
    }
}

/// Format agent type for display
pub fn format_agent_type(agent_type: &str) -> String {
    match agent_type {
        "Coding" => "🔧 Coding",
        "Planning" => "📋 Planning", 
        "Evaluator" => "🔍 Evaluator",
        "Writing" => "📝 Writing",
        _ => agent_type,
    }
    .to_string()
}

/// Format tool name for display
pub fn format_tool_name(tool_name: &str) -> String {
    match tool_name {
        "read_file" => "📄 Read File",
        "write_file" => "💾 Write File",
        "search_code" => "🔍 Search Code",
        "list_files" => "📂 List Files",
        "run_tests" => "🧪 Run Tests",
        _ => tool_name,
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    
    #[test]
    fn test_truncate_text() {
        assert_eq!(truncate_text("hello world", 20), "hello world");
        assert_eq!(truncate_text("hello world", 8), "hello...");
        assert_eq!(truncate_text("hello world", 3), "...");
        assert_eq!(truncate_text("hi", 1), "hi");
    }
    
    #[test]
    fn test_wrap_text() {
        let wrapped = wrap_text("hello world test", 10);
        assert_eq!(wrapped, vec!["hello", "world test"]);
        
        let wrapped = wrap_text("hello", 10);
        assert_eq!(wrapped, vec!["hello"]);
        
        let wrapped = wrap_text("", 10);
        assert_eq!(wrapped, vec![""]);
    }
    
    #[test]
    fn test_format_file_size() {
        assert_eq!(format_file_size(500), "500 B");
        assert_eq!(format_file_size(1024), "1.0 KB");
        assert_eq!(format_file_size(1536), "1.5 KB");
        assert_eq!(format_file_size(1_048_576), "1.0 MB");
    }
    
    #[test]
    fn test_pluralize() {
        assert_eq!(pluralize(1, "item", None), "1 item");
        assert_eq!(pluralize(2, "item", None), "2 items");
        assert_eq!(pluralize(0, "item", None), "0 items");
        assert_eq!(pluralize(2, "child", Some("children")), "2 children");
    }
    
    #[test]
    fn test_center_text() {
        assert_eq!(center_text("hi", 6), " hi   ");
        assert_eq!(center_text("hello", 5), "hello");
        assert_eq!(center_text("hello world", 5), "hello world");
    }
}