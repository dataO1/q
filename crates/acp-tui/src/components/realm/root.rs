//! Root component - handles global app events without rendering

use ratatui::layout::Rect;
use tuirealm::{
    command::{Cmd, CmdResult}, event::{Key, KeyEvent, KeyModifiers}, props::{Alignment, TextSpan},Attribute, Component, Event, Frame, MockComponent, State, StateValue, Sub, SubClause, SubEventClause
};

use crate::message::{UserEvent, APIEvent};

#[derive(Debug, Default)]
pub struct RootRealmComponent {
    // No state needed, just a message router
}

impl RootRealmComponent {
    pub fn new() -> Self {
        Self::default()
    }
}

impl MockComponent for RootRealmComponent {
    fn view(&mut self, _frame: &mut Frame, _area: Rect) {
        // Render nothing - this component is invisible
    }

    fn query(&self, attr: tuirealm::Attribute) -> Option<tuirealm::AttrValue> {
        None
    }

    fn attr(&mut self, attr: tuirealm::Attribute, value: tuirealm::AttrValue) -> () {
        ()
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        CmdResult::None
    }
}

impl Component<UserEvent, APIEvent> for RootRealmComponent {
    fn on(&mut self, ev: Event<APIEvent>) -> Option<UserEvent> {
        match ev {
            Event::User(api_event) => {
                match api_event{
                    APIEvent::WebSocketConnected(subsriber_id) =>{
                        Some(UserEvent::WebSocketConnected(subsriber_id))
                    },
                    _ =>{None}
                }
            }
            // Let other events pass through to focused components
            _ => None,
        }
    }
}
