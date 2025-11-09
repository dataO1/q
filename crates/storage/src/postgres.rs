use anyhow::Result;
use sqlx::{PgPool, postgres::PgPoolOptions, Row};
use uuid::Uuid;
use chrono::{DateTime, Utc};

pub struct PostgresClient {
    pool: PgPool,
}

impl PostgresClient {
    /// Create new PostgreSQL client with connection pool
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await?;

        Ok(Self { pool })
    }

    /// Run database migrations
    pub async fn run_migrations(&self) -> Result<()> {
        // Enable pgvector extension
        sqlx::query("CREATE EXTENSION IF NOT EXISTS vector")
            .execute(&self.pool)
            .await?;

        // Create conversations table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS conversations (
                id TEXT PRIMARY KEY,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                project_root TEXT,
                metadata JSONB DEFAULT '{}'::jsonb
            )
            "#
        )
        .execute(&self.pool)
        .await?;

        // Create messages table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS messages (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
                role TEXT NOT NULL CHECK (role IN ('user', 'assistant', 'system')),
                content TEXT NOT NULL,
                timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                metadata JSONB DEFAULT '{}'::jsonb,
                embedding vector(768)
            )
            "#
        )
        .execute(&self.pool)
        .await?;

        // Create index on messages for semantic search
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS messages_embedding_idx
            ON messages
            USING ivfflat (embedding vector_cosine_ops)
            WITH (lists = 100)
            "#
        )
        .execute(&self.pool)
        .await?;

        // Create summaries table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS summaries (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
                summary_type TEXT NOT NULL CHECK (summary_type IN ('progressive', 'final')),
                content TEXT NOT NULL,
                message_range_start UUID,
                message_range_end UUID,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                metadata JSONB DEFAULT '{}'::jsonb
            )
            "#
        )
        .execute(&self.pool)
        .await?;

        // Create workflow checkpoints table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS workflow_checkpoints (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                workflow_state JSONB NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                metadata JSONB DEFAULT '{}'::jsonb
            )
            "#
        )
        .execute(&self.pool)
        .await?;

        // Create audit logs table for HITL
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS audit_logs (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                task_id UUID NOT NULL,
                approved BOOLEAN NOT NULL,
                user_id TEXT,
                comment TEXT,
                timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                metadata JSONB DEFAULT '{}'::jsonb
            )
            "#
        )
        .execute(&self.pool)
        .await?;

        tracing::info!("Database migrations completed successfully");
        Ok(())
    }

    /// Get connection pool reference
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Create a new conversation
    pub async fn create_conversation(&self, id: &str, project_root: Option<&str>) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO conversations (id, project_root)
            VALUES ($1, $2)
            "#
        )
        .bind(id)
        .bind(project_root)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Store a message in the database
    pub async fn store_message(
        &self,
        conversation_id: &str,
        role: &str,
        content: &str,
        metadata: Option<serde_json::Value>,
    ) -> Result<Uuid> {
        let row = sqlx::query(
            r#"
            INSERT INTO messages (conversation_id, role, content, metadata)
            VALUES ($1, $2, $3, $4)
            RETURNING id
            "#
        )
        .bind(conversation_id)
        .bind(role)
        .bind(content)
        .bind(metadata.unwrap_or(serde_json::json!({})))
        .fetch_one(&self.pool)
        .await?;

        let id: Uuid = row.get("id");
        Ok(id)
    }

    /// Update message embedding for semantic search
    pub async fn update_message_embedding(&self, message_id: Uuid, embedding: &[f32]) -> Result<()> {
        // Convert Vec<f32> to pgvector format
        let embedding_str = format!("[{}]", embedding.iter().map(|f| f.to_string()).collect::<Vec<_>>().join(","));

        sqlx::query(
            r#"
            UPDATE messages
            SET embedding = $1::vector
            WHERE id = $2
            "#
        )
        .bind(embedding_str)
        .bind(message_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Semantic search for similar messages
    pub async fn semantic_search(
        &self,
        conversation_id: &str,
        query_embedding: &[f32],
        limit: i64,
    ) -> Result<Vec<(Uuid, String, f32)>> {
        let embedding_str = format!("[{}]", query_embedding.iter().map(|f| f.to_string()).collect::<Vec<_>>().join(","));

        let rows = sqlx::query(
            r#"
            SELECT id, content, (embedding <=> $1::vector) AS distance
            FROM messages
            WHERE conversation_id = $2 AND embedding IS NOT NULL
            ORDER BY embedding <=> $1::vector
            LIMIT $3
            "#
        )
        .bind(embedding_str)
        .bind(conversation_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let results = rows
            .into_iter()
            .map(|row| {
                let id: Uuid = row.get("id");
                let content: String = row.get("content");
                let distance: f32 = row.get("distance");
                (id, content, distance)
            })
            .collect();

        Ok(results)
    }

    /// Get recent messages from conversation
    pub async fn get_recent_messages(
        &self,
        conversation_id: &str,
        limit: i64,
    ) -> Result<Vec<(Uuid, String, String, DateTime<Utc>)>> {
        let rows = sqlx::query(
            r#"
            SELECT id, role, content, timestamp
            FROM messages
            WHERE conversation_id = $1
            ORDER BY timestamp DESC
            LIMIT $2
            "#
        )
        .bind(conversation_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let messages = rows
            .into_iter()
            .map(|row| {
                let id: Uuid = row.get("id");
                let role: String = row.get("role");
                let content: String = row.get("content");
                let timestamp: DateTime<Utc> = row.get("timestamp");
                (id, role, content, timestamp)
            })
            .collect();

        Ok(messages)
    }

    /// Store a summary
    pub async fn store_summary(
        &self,
        conversation_id: &str,
        summary_type: &str,
        content: &str,
        message_range: Option<(Uuid, Uuid)>,
    ) -> Result<Uuid> {
        let (start, end) = message_range.unzip();

        let row = sqlx::query(
            r#"
            INSERT INTO summaries (conversation_id, summary_type, content, message_range_start, message_range_end)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id
            "#
        )
        .bind(conversation_id)
        .bind(summary_type)
        .bind(content)
        .bind(start)
        .bind(end)
        .fetch_one(&self.pool)
        .await?;

        let id: Uuid = row.get("id");
        Ok(id)
    }

    /// Save workflow checkpoint
    pub async fn save_checkpoint(&self, workflow_state: serde_json::Value) -> Result<Uuid> {
        let row = sqlx::query(
            r#"
            INSERT INTO workflow_checkpoints (workflow_state)
            VALUES ($1)
            RETURNING id
            "#
        )
        .bind(workflow_state)
        .fetch_one(&self.pool)
        .await?;

        let id: Uuid = row.get("id");
        Ok(id)
    }

    /// Load workflow checkpoint
    pub async fn load_checkpoint(&self, id: Uuid) -> Result<serde_json::Value> {
        let row = sqlx::query(
            r#"
            SELECT workflow_state
            FROM workflow_checkpoints
            WHERE id = $1
            "#
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?;

        let state: serde_json::Value = row.get("workflow_state");
        Ok(state)
    }

    /// Record HITL audit log
    pub async fn record_audit(
        &self,
        task_id: Uuid,
        approved: bool,
        user_id: Option<&str>,
        comment: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO audit_logs (task_id, approved, user_id, comment)
            VALUES ($1, $2, $3, $4)
            "#
        )
        .bind(task_id)
        .bind(approved)
        .bind(user_id)
        .bind(comment)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
