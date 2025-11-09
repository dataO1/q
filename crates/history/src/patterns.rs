use ai_agent_common::*;

pub struct ConversationPatternDetector;

impl ConversationPatternDetector {
    pub fn detect_correction(&self, message: &str) -> bool {
        todo!("Detect 'actually', 'no', etc.")
    }

    pub fn detect_cancellation(&self, message: &str) -> bool {
        todo!("Detect 'never mind', 'stop', etc.")
    }

    pub fn detect_context_switch(&self, current: &Message, previous: &[Message]) -> bool {
        todo!("Detect topic changes")
    }
}
