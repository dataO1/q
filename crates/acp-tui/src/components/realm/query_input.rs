//! QueryInput TUIRealm component
//!
//! Multi-line text input component using tui-textarea with TUIRealm integration.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders},
    Frame,
};
use tuirealm::{
    command::{Cmd, CmdResult},
    event::{Key, KeyEvent as TuiKeyEvent, KeyModifiers as TuiKeyModifiers},
    Component, Event, MockComponent, State, StateValue, AttrValue, Attribute,
};
use tui_textarea::TextArea;

use crate::message::{APIEvent, NoUserEvent, UserEvent};

/// QueryInput component using TUIRealm architecture
pub struct QueryInputRealmComponent {
    /// Multi-line text area
    textarea: TextArea<'static>,
    /// Whether this component is focused
    focused: bool,
    /// Whether to show placeholder text
    show_placeholder: bool,
}

impl QueryInputRealmComponent {
    /// Create new query input component
    pub fn new() -> Self {
        let mut textarea = TextArea::default();
        textarea.set_placeholder_text("Enter query (Enter to submit, Shift+Enter for new line)");
        textarea.set_block(
            Block::default()
                .title("Query Input")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::White))
        );

        Self {
            textarea,
            focused: false,
            show_placeholder: true,
        }
    }

    /// Get the current query text
    pub fn get_query(&self) -> String {
        self.textarea.lines().join("\n")
    }

    /// Clear the input
    pub fn clear(&mut self) {
        self.textarea = TextArea::default();
        self.textarea.set_placeholder_text("Enter query (Enter to submit, Shift+Enter for new line)");
        self.update_border_style();
        self.show_placeholder = true;
    }

    /// Set query text (for restoring after connection)
    pub fn set_query(&mut self, query: &str) {
        self.textarea = TextArea::from([query]);
        self.update_border_style();
        self.show_placeholder = query.is_empty();
    }

    /// Update border style based on focus
    fn update_border_style(&mut self) {
        let border_style = if self.focused {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::White)
        };

        self.textarea.set_block(
            Block::default()
                .title("Query Input")
                .borders(Borders::ALL)
                .border_style(border_style)
        );
    }

    /// Get dynamic height based on content
    pub fn get_height(&self) -> u16 {
        let input_lines = self.textarea.lines().len();
        (input_lines + 2).max(5).min(15) as u16 // min 5, max 15 lines
    }
}

impl Component<UserEvent, APIEvent> for QueryInputRealmComponent {
    fn on(&mut self, ev: Event<APIEvent>) -> Option<UserEvent> {
        match ev {
            Event::Keyboard(key_event) => {
                if self.focused {
                    match key_event {
                        // Submit on Ctrl+Enter, regular Enter just adds newline
                        TuiKeyEvent { code: Key::Enter, modifiers } => {
                            if modifiers.is_empty() {
                                let query = self.get_query();
                                if !query.trim().is_empty() {
                                    Some(UserEvent::QuerySubmitted(self.get_query()))
                                } else {
                                    None
                                }
                            }
                            else if modifiers.intersects(TuiKeyModifiers::ALT){
                                let crossterm_event = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
                                self.textarea.input(crossterm_event);
                                self.show_placeholder = self.textarea.lines().iter().all(|line| line.is_empty());
                                None
                            }else{None}
                        },
                        // Tab navigation
                        TuiKeyEvent { code: Key::Tab, modifiers } if modifiers.intersects(TuiKeyModifiers::SHIFT) => {
                            Some(UserEvent::FocusPrevious)
                        },
                        TuiKeyEvent { code: Key::Tab, .. } => {
                            Some(UserEvent::FocusNext)
                        },

                        // All other input
                        key => {
                            // Convert TuiKeyEvent to crossterm KeyEvent
                            let crossterm_modifiers = KeyModifiers::NONE; // For now, just use NONE
                            let crossterm_event = match key.code {
                                Key::Char(c) => KeyEvent::new(KeyCode::Char(c), crossterm_modifiers),
                                Key::Backspace => KeyEvent::new(KeyCode::Backspace, crossterm_modifiers),
                                Key::Delete => KeyEvent::new(KeyCode::Delete, crossterm_modifiers),
                                Key::Left => KeyEvent::new(KeyCode::Left, crossterm_modifiers),
                                Key::Right => KeyEvent::new(KeyCode::Right, crossterm_modifiers),
                                Key::Up => KeyEvent::new(KeyCode::Up, crossterm_modifiers),
                                Key::Down => KeyEvent::new(KeyCode::Down, crossterm_modifiers),
                                Key::Home => KeyEvent::new(KeyCode::Home, crossterm_modifiers),
                                Key::End => KeyEvent::new(KeyCode::End, crossterm_modifiers),
                                _ => return None,
                            };

                            self.textarea.input(crossterm_event);
                            self.show_placeholder = self.textarea.lines().iter().all(|line| line.is_empty());
                            None
                        }
                    }
                } else {
                    None
                }
            },
            Event::User(_) => None, // AppMsg events don't affect QueryInput directly
            _ => None,
        }
    }
}

impl MockComponent for QueryInputRealmComponent {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        self.textarea.set_cursor_line_style(Style::default());
        frame.render_widget(&self.textarea, area);
    }

    fn query(&self, attr: Attribute) -> Option<AttrValue> {
        match attr {
            Attribute::Focus => Some(AttrValue::Flag(self.focused)),
            Attribute::Text => Some(AttrValue::String(self.get_query())),
            _ => None,
        }
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        match attr {
            Attribute::Focus => {
                if let AttrValue::Flag(focused) = value {
                    self.focused = focused;
                    self.update_border_style();
                }
            }
            Attribute::Text => {
                if let AttrValue::String(text) = value {
                    self.set_query(&text);
                }
            }
            _ => {}
        }
    }

    fn state(&self) -> State {
        State::One(StateValue::String(self.get_query()))
    }

    fn perform(&mut self, cmd: Cmd) -> CmdResult {
        match cmd {
            Cmd::Submit => {
                let query = self.get_query();
                if !query.trim().is_empty() {
                    CmdResult::Submit(State::One(StateValue::String(query)))
                } else {
                    CmdResult::None
                }
            },
            Cmd::Cancel => {
                self.clear();
                CmdResult::Changed(State::None)
            },
            _ => CmdResult::None,
        }
    }
}
