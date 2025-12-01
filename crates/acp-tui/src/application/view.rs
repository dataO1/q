//! Application view logic (View in Elm architecture)

use ratatui::{Frame, layout::{Constraint, Direction, Layout, Rect}};
use tuirealm::Application;

use crate::{
    application::AppModel,
    message::{ComponentId, LayoutMode},
};

/// Render the complete application UI
pub fn render_app(
    model: &AppModel,
    app: &mut Application<ComponentId, crate::message::AppMsg, crate::message::NoUserEvent>,
    frame: &mut Frame,
) {
    let area = frame.area();
    
    match model.layout_mode {
        LayoutMode::Normal => render_normal_layout(app, frame, area),
        LayoutMode::HitlReview => render_hitl_layout(app, frame, area),
    }
    
    // Render overlay components
    if model.show_help {
        render_help_overlay(frame, area);
    }
    
    // Render HITL review modal if active
    if model.current_hitl_request.is_some() {
        app.view(&ComponentId::HitlReview, frame, area);
    }
}

/// Render normal layout: Timeline + QueryInput + StatusLine
fn render_normal_layout(
    app: &mut Application<ComponentId, crate::message::AppMsg, crate::message::NoUserEvent>,
    frame: &mut Frame,
    area: Rect,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),         // Timeline gets remaining space
            Constraint::Length(5),      // Query input
            Constraint::Length(1),      // Status line
        ])
        .split(area);
    
    app.view(&ComponentId::Timeline, frame, chunks[0]);
    app.view(&ComponentId::QueryInput, frame, chunks[1]);
    app.view(&ComponentId::StatusLine, frame, chunks[2]);
}

/// Render HITL review layout: HitlQueue + Timeline (smaller) + StatusLine
fn render_hitl_layout(
    app: &mut Application<ComponentId, crate::message::AppMsg, crate::message::NoUserEvent>,
    frame: &mut Frame,
    area: Rect,
) {
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(40),     // HITL Queue sidebar
            Constraint::Min(20),        // Main area
        ])
        .split(area);
    
    let main_area_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),         // Timeline
            Constraint::Length(1),      // Status line
        ])
        .split(main_chunks[1]);
    
    app.view(&ComponentId::HitlQueue, frame, main_chunks[0]);
    app.view(&ComponentId::Timeline, frame, main_area_chunks[0]);
    app.view(&ComponentId::StatusLine, frame, main_area_chunks[1]);
}

/// Render help overlay
fn render_help_overlay(frame: &mut Frame, area: Rect) {
    use ratatui::{
        widgets::{Block, Borders, Clear, Paragraph, Wrap},
        style::{Color, Style},
        text::{Line, Span},
    };
    
    // Calculate popup area
    let popup_width = area.width.min(60);
    let popup_height = area.height.min(20);
    let popup_area = Rect {
        x: (area.width.saturating_sub(popup_width)) / 2,
        y: (area.height.saturating_sub(popup_height)) / 2,
        width: popup_width,
        height: popup_height,
    };
    
    // Clear background
    frame.render_widget(Clear, popup_area);
    
    let help_text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(" ACP TUI Help ", Style::default().fg(Color::Green))
        ]),
        Line::from(""),
        Line::from(" ╔═══ Navigation ═══╗"),
        Line::from(" ║ Tab: Focus next   ║"),
        Line::from(" ║ ↑/↓: Scroll       ║"),
        Line::from(" ║ PgUp/PgDn: Fast   ║"),
        Line::from(" ║ Enter: Submit     ║"),
        Line::from(" ╠═══ Actions ════╗"),
        Line::from(" ║ c: Clear timeline ║"),
        Line::from(" ║ ?: Toggle help    ║"),
        Line::from(" ║ q: Quit app       ║"),
        Line::from(" ║ Ctrl+C: Force quit║"),
        Line::from(" ╚═══════════════════╝"),
        Line::from(""),
        Line::from(" Press any key to close"),
    ];
    
    let help_widget = Paragraph::new(help_text)
        .block(Block::default()
            .title(" Help ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow)))
        .wrap(Wrap { trim: true });
    
    frame.render_widget(help_widget, popup_area);
}