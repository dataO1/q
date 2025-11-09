use ai_agent_storage::PostgresClient;
use uuid::Uuid;

// Helper to get ISOLATED test database URL
fn get_test_db_url() -> String {
    std::env::var("TEST_DATABASE_URL")
        .expect("TEST_DATABASE_URL must be set (use docker-compose.test.yml)")
}

// Setup: Create test client and run migrations once
async fn setup_test_db() -> PostgresClient {
    let client = PostgresClient::new(&get_test_db_url())
        .await
        .expect("Failed to connect to test database");

    // Run migrations (creates tables if they don't exist)
    client.run_migrations()
        .await
        .expect("Failed to run migrations");

    client
}

#[tokio::test]
#[ignore] // Run only when test services are available
async fn test_postgres_connection() {
    let client = setup_test_db().await;

    // Test passes if we got here and can acquire a connection
    assert!(client.pool().acquire().await.is_ok());
}

#[tokio::test]
#[ignore]
async fn test_create_and_store_conversation() {
    let client = setup_test_db().await;

    // Use unique conversation ID to avoid conflicts
    let conv_id = format!("test-conv-{}", Uuid::new_v4());

    // Create conversation
    client.create_conversation(&conv_id, Some("/test/project"))
        .await
        .expect("Failed to create conversation");

    // Store message
    let message_id = client
        .store_message(&conv_id, "user", "Hello, world!", None)
        .await
        .expect("Failed to store message");

    assert_ne!(message_id, Uuid::nil());

    // Cleanup: delete test conversation
    sqlx::query("DELETE FROM conversations WHERE id = $1")
        .bind(&conv_id)
        .execute(client.pool())
        .await
        .ok();
}

#[tokio::test]
#[ignore]
async fn test_get_recent_messages() {
    let client = setup_test_db().await;

    let conv_id = format!("test-conv-{}", Uuid::new_v4());
    client.create_conversation(&conv_id, None).await.unwrap();

    // Store multiple messages
    for i in 0..5 {
        client
            .store_message(&conv_id, "user", &format!("Message {}", i), None)
            .await
            .unwrap();
    }

    // Retrieve recent messages
    let messages = client
        .get_recent_messages(&conv_id, 3)
        .await
        .expect("Failed to get recent messages");

    assert_eq!(messages.len(), 3);

    // Cleanup
    sqlx::query("DELETE FROM conversations WHERE id = $1")
        .bind(&conv_id)
        .execute(client.pool())
        .await
        .ok();
}

#[tokio::test]
#[ignore]
async fn test_checkpoint_save_and_load() {
    let client = setup_test_db().await;

    let workflow_state = serde_json::json!({
        "task_id": "test-task-123",
        "current_step": 5,
        "completed_tasks": ["task1", "task2"]
    });

    // Save checkpoint
    let checkpoint_id = client
        .save_checkpoint(workflow_state.clone())
        .await
        .expect("Failed to save checkpoint");

    // Load checkpoint
    let loaded_state = client
        .load_checkpoint(checkpoint_id)
        .await
        .expect("Failed to load checkpoint");

    assert_eq!(workflow_state, loaded_state);

    // Cleanup
    sqlx::query("DELETE FROM workflow_checkpoints WHERE id = $1")
        .bind(checkpoint_id)
        .execute(client.pool())
        .await
        .ok();
}

#[tokio::test]
#[ignore]
async fn test_semantic_search() {
    let client = setup_test_db().await;

    let conv_id = format!("test-conv-{}", Uuid::new_v4());
    client.create_conversation(&conv_id, None).await.unwrap();

    // Store a message
    let msg_id = client
        .store_message(&conv_id, "user", "Test message for semantic search", None)
        .await
        .unwrap();

    // Update with embedding (768-dimensional vector, all zeros for test)
    let embedding = vec![0.0f32; 768];
    client
        .update_message_embedding(msg_id, &embedding)
        .await
        .expect("Failed to update embedding");

    // Search for similar messages
    let results = client
        .semantic_search(&conv_id, &embedding, 5)
        .await
        .expect("Failed to perform semantic search");

    assert!(!results.is_empty());
    assert_eq!(results[0].0, msg_id);

    // Cleanup
    sqlx::query("DELETE FROM conversations WHERE id = $1")
        .bind(&conv_id)
        .execute(client.pool())
        .await
        .ok();
}

#[tokio::test]
#[ignore]
async fn test_store_and_retrieve_summary() {
    let client = setup_test_db().await;

    let conv_id = format!("test-conv-{}", Uuid::new_v4());
    client.create_conversation(&conv_id, None).await.unwrap();

    // Store messages
    let msg1_id = client
        .store_message(&conv_id, "user", "First message", None)
        .await
        .unwrap();

    let msg2_id = client
        .store_message(&conv_id, "assistant", "Second message", None)
        .await
        .unwrap();

    // Store summary
    let summary_id = client
        .store_summary(
            &conv_id,
            "progressive",
            "Summary of conversation",
            Some((msg1_id, msg2_id)),
        )
        .await
        .expect("Failed to store summary");

    assert_ne!(summary_id, Uuid::nil());

    // Cleanup
    sqlx::query("DELETE FROM conversations WHERE id = $1")
        .bind(&conv_id)
        .execute(client.pool())
        .await
        .ok();
}

#[tokio::test]
#[ignore]
async fn test_audit_log() {
    let client = setup_test_db().await;

    let task_id = Uuid::new_v4();

    // Record approval
    client
        .record_audit(task_id, true, Some("test_user"), Some("Looks good"))
        .await
        .expect("Failed to record audit");

    // Cleanup
    sqlx::query("DELETE FROM audit_logs WHERE task_id = $1")
        .bind(task_id)
        .execute(client.pool())
        .await
        .ok();
}
