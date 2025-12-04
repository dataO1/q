//! Application view logic (View in Elm architecture)

use ratatui::{Frame, layout::{Constraint, Direction, Layout, Rect}};
use tuirealm::Application;

use crate::{
    application::AppModel,
    message::{ComponentId },
};


/// Render normal layout: Timeline + QueryInput + StatusLine
pub fn render(
    model: &AppModel,
    app: &mut Application<ComponentId, crate::message::UserEvent, crate::message::APIEvent>,
    frame: &mut Frame,
) {
    let area = frame.area();
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

    if model.show_hitl_popup {
        // Render over the entire screen (full overlay)
        app.view(&ComponentId::HitlReview, frame, area);
    }

    // Render help overlay last (highest z-index)
    if model.show_help {
        app.view(&ComponentId::Help, frame, area);
    }
}
