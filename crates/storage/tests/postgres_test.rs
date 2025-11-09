use ai_agent_storage::PostgresClient;
use uuid::Uuid;
use sqlx::PgPool;

// Helper to get ISOLATED test database URL
fn get_test_db_url() -> String {
    std::env::var("TEST_DATABASE_URL")
        .expect("TEST_DATABASE_URL must be set (use docker-compose.test.yml)")
}

// Setup: Create isolated test schema
async fn setup_test_db() -> (PostgresClient, String) {
    let client = PostgresClient::new(&get_test_db_url())
        .await
        .expect("Failed to connect to test database");

    // Run migrations in test database
    client.run_migrations().await.expect("Failed to run migrations");

    // Create unique schema for this test run to avoid conflicts
    let schema_name = format!("test_{}", Uuid::new_v4().to_string().replace('-', "_"));

    sqlx::query(&format!("CREATE SCHEMA {}", schema_name))
        .execute(client.pool())
        .await
        .expect("Failed to create test schema");

    // Set search path to use test schema
    sqlx::query(&format!("SET search_path TO {}", schema_name))
        .execute(client.pool())
        .await
        .expect("Failed to set search path");

    (client, schema_name)
}

// Teardown: Clean up test schema
async fn cleanup_test_db(client: &PostgresClient, schema_name: &str) {
    sqlx::query(&format!("DROP SCHEMA IF EXISTS {} CASCADE", schema_name))
        .execute(client.pool())
        .await
        .ok(); // Ignore errors during cleanup
}

#[tokio::test]
#[ignore] // Run only when test services are available
async fn test_postgres_connection() {
    let (client, schema) = setup_test_db().await;

    // Test passes if we got here
    assert!(client.pool().acquire().await.is_ok());

    cleanup_test_db(&client, &schema).await;
}

#[tokio::test]
#[ignore]
async fn test_create_and_store_conversation() {
    let (client, schema) = setup_test_db().await;

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

    cleanup_test_db(&client, &schema).await;
}

#[tokio::test]
#[ignore]
async fn test_get_recent_messages() {
    let (client, schema) = setup_test_db().await;

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

    cleanup_test_db(&client, &schema).await;
}

#[tokio::test]
#[ignore]
async fn test_checkpoint_save_and_load() {
    let (client, schema) = setup_test_db().await;

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

    cleanup_test_db(&client, &schema).await;
}
