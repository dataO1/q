use anyhow::Result;
use redis::aio::MultiplexedConnection;
use redis::AsyncCommands;
use tokio::sync::Mutex;
use std::sync::Arc;

pub struct RedisCache {
    client: redis::Client,
    connection: Arc<Mutex<MultiplexedConnection>>,
}

impl RedisCache {
    pub async fn new(redis_url: &str) -> Result<Self> {
        let client = redis::Client::open(redis_url)?;
        let connection = client.get_multiplexed_async_connection().await?;
        Ok(Self {
            client,
            connection: Arc::new(Mutex::new(connection)),
        })
    }

    pub async fn set(&self, key: &str, value: &str) -> Result<()> {
        let mut conn = self.connection.lock().await;
        conn.set(key, value).await?;
        Ok(())
    }

    pub async fn get(&self, key: &str) -> Result<Option<String>> {
        let mut conn = self.connection.lock().await;
        let val: Option<String> = conn.get(key).await?;
        Ok(val)
    }
}
