// In components/realm/help.rs

use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    style::{Color, Style},
    text::{Line, Span},
};
use tuirealm::{
    command::{Cmd, CmdResult},
    event::{Key, KeyEvent as TuiKeyEvent},
    Component, Event, MockComponent, State, StateValue, AttrValue, Attribute,
};
use crate::message::{AppMsg, NoUserEvent, ComponentMsg};

/// Help overlay component using TUIRealm architecture
pub struct HelpRealmComponent {
    /// Whether the help overlay is visible
    visible: bool,
    /// Whether this component is focused
    focused: bool,
}

impl HelpRealmComponent {
    /// Create new help component
    pub fn new() -> Self {
        Self {
            visible: false,
            focused: false,
        }
    }

    /// Show the help overlay
    pub fn show(&mut self) {
        self.visible = true;
        self.focused = true;
    }

    /// Hide the help overlay
    pub fn hide(&mut self) {
        self.visible = false;
        self.focused = false;
    }

    /// Is the help overlay visible?
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Get the popup area for centering the help overlay
    fn get_popup_area(&self, area: Rect) -> Rect {
        let popup_width = area.width.min(60);
        let popup_height = area.height.min(20);
        Rect {
            x: (area.width.saturating_sub(popup_width)) / 2,
            y: (area.height.saturating_sub(popup_height)) / 2,
            width: popup_width,
            height: popup_height,
        }
    }
}

impl Component<ComponentMsg, AppMsg> for HelpRealmComponent {
    fn on(&mut self, ev: Event<AppMsg>) -> Option<ComponentMsg> {
        // Only respond when visible
        if !self.visible {
            return None;
        }

        match ev {
            Event::Keyboard(_) => {
                // Any key closes help
                Some(ComponentMsg::HelpToggle)
            }
            _ => None,
        }
    }
}

impl MockComponent for HelpRealmComponent {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        if !self.visible {
            return;
        }

        let popup_area = self.get_popup_area(area);

        // Clear background
        frame.render_widget(Clear, popup_area);

        let help_text = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(" ACP TUI Help ", Style::default().fg(Color::Green))
            ]),
            Line::from(""),
            Line::from(" ╔═══ Navigation ═══╗"),
            Line::from(" ║ Tab: Focus next ║"),
            Line::from(" ║ ↑/↓: Scroll ║"),
            Line::from(" ║ PgUp/PgDn: Fast ║"),
            Line::from(" ║ Enter: Submit ║"),
            Line::from(" ╠═══ Actions ════╗"),
            Line::from(" ║ c: Clear timeline ║"),
            Line::from(" ║ ?: Toggle help ║"),
            Line::from(" ║ q: Quit app ║"),
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

    fn query(&self, attr: Attribute) -> Option<AttrValue> {
        match attr {
            Attribute::Focus => Some(AttrValue::Flag(self.focused)),
            Attribute::Display => Some(AttrValue::Flag(self.visible)),
            _ => None,
        }
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        match attr {
            Attribute::Focus => {
                if let AttrValue::Flag(focused) = value {
                    self.focused = focused;
                }
            }
            Attribute::Display => {
                if let AttrValue::Flag(visible) = value {
                    if visible {
                        self.show();
                    } else {
                        self.hide();
                    }
                }
            }
            _ => {}
        }
    }

    fn state(&self) -> State {
        if self.visible {
            State::One(StateValue::Bool(true))
        } else {
            State::None
        }
    }

    fn perform(&mut self, cmd: Cmd) -> CmdResult {
        match cmd {
            Cmd::Cancel => {
                self.hide();
                CmdResult::Changed(State::None)
            }
            _ => CmdResult::None,
        }
    }
}
