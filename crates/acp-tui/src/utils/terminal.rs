//! Terminal utilities and helpers

use crate::{Error, Result};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, stdout};

/// Terminal manager for setup and cleanup
pub struct TerminalManager {
    /// The ratatui terminal instance
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
}

impl TerminalManager {
    /// Initialize the terminal for TUI mode
    pub fn new() -> Result<Self> {
        // Enable raw mode for character-by-character input
        enable_raw_mode().map_err(|e| Error::Ui(crate::error::UiError::TerminalInit {
            source: Box::new(e),
        }))?;
        
        let mut stdout = stdout();
        
        // Enter alternate screen and enable mouse capture
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
            .map_err(|e| Error::Ui(crate::error::UiError::TerminalInit {
                source: Box::new(e),
            }))?;
        
        // Create the terminal backend
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)
            .map_err(|e| Error::Ui(crate::error::UiError::TerminalInit {
                source: Box::new(e),
            }))?;
        
        Ok(Self { terminal })
    }
    
    /// Get a mutable reference to the terminal
    pub fn terminal_mut(&mut self) -> &mut Terminal<CrosstermBackend<io::Stdout>> {
        &mut self.terminal
    }
    
    /// Get the terminal size
    pub fn size(&self) -> Result<ratatui::layout::Size> {
        self.terminal.size().map_err(|e| Error::Ui(crate::error::UiError::RenderError {
            component: "terminal".to_string(),
            source: Some(Box::new(e)),
        }))
    }
    
    /// Clear the terminal
    pub fn clear(&mut self) -> Result<()> {
        self.terminal.clear().map_err(|e| Error::Ui(crate::error::UiError::RenderError {
            component: "terminal".to_string(),
            source: Some(Box::new(e)),
        }))
    }
    
    /// Cleanup terminal state (called automatically on drop)
    fn cleanup(&mut self) -> Result<()> {
        disable_raw_mode().map_err(|e| Error::Ui(crate::error::UiError::TerminalInit {
            source: Box::new(e),
        }))?;
        
        execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )
        .map_err(|e| Error::Ui(crate::error::UiError::TerminalInit {
            source: Box::new(e),
        }))?;
        
        self.terminal.show_cursor().map_err(|e| Error::Ui(crate::error::UiError::RenderError {
            component: "cursor".to_string(),
            source: Some(Box::new(e)),
        }))?;
        
        Ok(())
    }
}

impl Drop for TerminalManager {
    fn drop(&mut self) {
        // Best effort cleanup - ignore errors since we can't handle them in Drop
        let _ = self.cleanup();
    }
}

/// Check if the terminal supports colors
pub fn supports_color() -> bool {
    // Simple heuristic - check for TERM environment variable
    if let Ok(term) = std::env::var("TERM") {
        !term.is_empty() && term != "dumb"
    } else {
        false
    }
}

/// Check if the terminal supports Unicode characters
pub fn supports_unicode() -> bool {
    // Check if locale supports UTF-8
    std::env::var("LANG")
        .or_else(|_| std::env::var("LC_ALL"))
        .map(|locale| locale.to_lowercase().contains("utf"))
        .unwrap_or(false)
}

/// Get terminal dimensions
pub fn get_size() -> Result<(u16, u16)> {
    crossterm::terminal::size().map_err(Error::Io)
}

/// Check if terminal is wide enough for the application
pub fn is_wide_enough(min_width: u16) -> Result<bool> {
    let (width, _) = get_size()?;
    Ok(width >= min_width)
}

/// Check if terminal is tall enough for the application
pub fn is_tall_enough(min_height: u16) -> Result<bool> {
    let (_, height) = get_size()?;
    Ok(height >= min_height)
}

/// Get recommended layout dimensions based on terminal size
pub fn get_layout_dimensions(terminal_width: u16) -> (u16, u16) {
    // Calculate DAG width (right panel) - aim for 25-30% of terminal width
    let dag_width = std::cmp::max(25, (terminal_width as f32 * 0.28) as u16);
    let dag_width = std::cmp::min(dag_width, 40); // Cap at 40 characters
    
    // History view gets the rest (minus borders)
    let history_width = terminal_width.saturating_sub(dag_width + 3); // 3 for borders
    
    (history_width, dag_width)
}

/// Unicode box drawing characters for DAG visualization
pub mod box_chars {
    /// Horizontal line
    pub const HORIZONTAL: &str = "─";
    
    /// Vertical line
    pub const VERTICAL: &str = "│";
    
    /// Top-left corner
    pub const TOP_LEFT: &str = "┌";
    
    /// Top-right corner
    pub const TOP_RIGHT: &str = "┐";
    
    /// Bottom-left corner
    pub const BOTTOM_LEFT: &str = "└";
    
    /// Bottom-right corner
    pub const BOTTOM_RIGHT: &str = "┘";
    
    /// Cross intersection
    pub const CROSS: &str = "┼";
    
    /// T-intersection (down)
    pub const T_DOWN: &str = "┬";
    
    /// T-intersection (up)
    pub const T_UP: &str = "┴";
    
    /// T-intersection (right)
    pub const T_RIGHT: &str = "├";
    
    /// T-intersection (left)
    pub const T_LEFT: &str = "┤";
    
    /// Heavy horizontal line
    pub const HEAVY_HORIZONTAL: &str = "━";
    
    /// Heavy vertical line
    pub const HEAVY_VERTICAL: &str = "┃";
    
    /// Double horizontal line
    pub const DOUBLE_HORIZONTAL: &str = "═";
    
    /// Double vertical line
    pub const DOUBLE_VERTICAL: &str = "║";
}

/// Symbols for DAG nodes
pub mod node_symbols {
    /// Active node (filled diamond)
    pub const ACTIVE: &str = "◆";
    
    /// Pending node (hexagon)
    pub const PENDING: &str = "⬢";
    
    /// Completed node (empty diamond)
    pub const COMPLETED: &str = "◇";
    
    /// Failed node (X)
    pub const FAILED: &str = "✗";
    
    /// Planning node (square)
    pub const PLANNING: &str = "◉";
    
    /// Agent node (circle)
    pub const AGENT: &str = "●";
    
    /// Tool node (gear)
    pub const TOOL: &str = "⚙";
    
    /// Evaluation node (magnifying glass)
    pub const EVALUATION: &str = "🔍";
}

/// Spinner characters for animations
pub mod spinner_chars {
    /// Standard spinner frames
    pub const SPINNER_DOTS: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    
    /// Clock spinner frames
    pub const SPINNER_CLOCK: &[&str] = &["🕐", "🕑", "🕒", "🕓", "🕔", "🕕", "🕖", "🕗", "🕘", "🕙", "🕚", "🕛"];
    
    /// Simple spinner frames
    pub const SPINNER_SIMPLE: &[&str] = &["|", "/", "-", "\\"];
    
    /// Braille spinner frames
    pub const SPINNER_BRAILLE: &[&str] = &["⣾", "⣽", "⣻", "⢿", "⡿", "⣟", "⣯", "⣷"];
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_get_layout_dimensions() {
        // Test with a typical terminal width
        let (history_width, dag_width) = get_layout_dimensions(80);
        assert!(dag_width >= 25);
        assert!(dag_width <= 40);
        assert!(history_width > 0);
        assert_eq!(history_width + dag_width + 3, 80);
        
        // Test with a narrow terminal
        let (history_width, dag_width) = get_layout_dimensions(40);
        assert!(dag_width >= 25);
        assert!(history_width > 0);
        
        // Test with a wide terminal
        let (history_width, dag_width) = get_layout_dimensions(120);
        assert_eq!(dag_width, 40); // Should be capped at 40
        assert!(history_width > 40);
    }
}