//! Application view logic (View in Elm architecture)

use ratatui::{Frame, layout::{Constraint, Direction, Layout, Rect}};
use tuirealm::Application;

use crate::{
    application::AppModel,
    message::{ComponentId, LayoutMode},
};

/// Render the complete application UI
pub fn render(
    model: &AppModel,
    app: &mut Application<ComponentId, crate::message::UserEvent, crate::message::APIEvent>,
    frame: &mut Frame,
) {
    let area = frame.area();

    match model.layout_mode {
        LayoutMode::Normal => render_normal_layout(model, app, frame, area),
        LayoutMode::HitlReview => render_hitl_layout(model, app, frame, area),
    }
    if model.show_help {
        app.view(&ComponentId::Help, frame, area);
    }
    // Render HITL review modal if active
    if model.current_hitl_request.is_some() {
        app.view(&ComponentId::HitlReview, frame, area);
    }
}

/// Render normal layout: Timeline + QueryInput + StatusLine
fn render_normal_layout(
    model: &AppModel,
    app: &mut Application<ComponentId, crate::message::UserEvent, crate::message::APIEvent>,
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
    model: &AppModel,
    app: &mut Application<ComponentId, crate::message::UserEvent, crate::message::APIEvent>,
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
