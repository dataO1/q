use ai_agent_common::types::*;

#[test]
fn test_task_id_creation() {
    let task_id1 = TaskId::new();
    let task_id2 = TaskId::new();

    assert_ne!(task_id1, task_id2);
    assert_eq!(task_id1, task_id1);
}

#[test]
fn test_conversation_id_creation() {
    let conv_id1 = ConversationId::new();
    let conv_id2 = ConversationId::new();

    assert_ne!(conv_id1, conv_id2);
}

#[test]
fn test_conversation_id_from_string() {
    let id_str = "test-conversation-123".to_string();
    let conv_id = ConversationId::from_string(id_str.clone());

    assert_eq!(conv_id.0, id_str);
}

#[test]
fn test_collection_tier_names() {
    assert_eq!(CollectionTier::System.collection_name(), "system_knowledge");
    assert_eq!(CollectionTier::Personal.collection_name(), "personal_docs");
    assert_eq!(CollectionTier::Workspace.collection_name(), "workspace_dev");
    assert_eq!(CollectionTier::Dependencies.collection_name(), "external_deps");
    assert_eq!(CollectionTier::Online.collection_name(), "online_docs");
}

#[test]
fn test_collection_tier_all() {
    let all_tiers = CollectionTier::all();
    assert_eq!(all_tiers.len(), 5);
    assert!(all_tiers.contains(&CollectionTier::System));
    assert!(all_tiers.contains(&CollectionTier::Workspace));
}

#[test]
fn test_agent_type_name() {
    assert_eq!(AgentType::Orchestrator.name(), "orchestrator");
    assert_eq!(AgentType::Coding.name(), "coding");
    assert_eq!(AgentType::Planning.name(), "planning");
    assert_eq!(AgentType::Writing.name(), "writing");
}

#[test]
fn test_message_creation() {
    let user_msg = Message::new_user("Hello".to_string());
    assert_eq!(user_msg.role, Role::User);
    assert_eq!(user_msg.content, "Hello");

    let assistant_msg = Message::new_assistant("Hi there".to_string());
    assert_eq!(assistant_msg.role, Role::Assistant);
    assert_eq!(assistant_msg.content, "Hi there");
}

#[test]
fn test_status_event_serialization() {
    let event = StatusEvent::TaskStarted {
        task_id: "task123".to_string(),
        description: "Test task".to_string(),
    };

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("TaskStarted"));
    assert!(json.contains("task123"));
}
