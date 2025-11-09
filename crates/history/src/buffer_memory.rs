use ai_agent_common::*;
use lru::LruCache;
use std::num::NonZeroUsize;

pub struct BufferMemory {
    cache: LruCache<ConversationId, Vec<Message>>,
    max_messages: usize,
}

impl BufferMemory {
    pub fn new(capacity: usize) -> Self {
        todo!("Initialize LRU buffer")
    }

    pub fn add_message(&mut self, conversation_id: ConversationId, message: Message) {
        todo!("Add to buffer, evict if needed")
    }

    pub fn get_recent(&self, conversation_id: &ConversationId) -> Option<&Vec<Message>> {
        todo!("Get last N messages")
    }
}
