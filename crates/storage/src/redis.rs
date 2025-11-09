use anyhow::{Context, Result};
use redis::aio::MultiplexedConnection;
use redis::AsyncCommands;
use tokio::sync::Mutex;
use std::sync::Arc;

pub struct RedisCache {
    client: redis::Client,
    connection: Arc<Mutex<MultiplexedConnection>>,
}

impl RedisCache {
    /// Create new Redis cache client
    pub async fn new(redis_url: &str) -> Result<Self> {
        let client = redis::Client::open(redis_url)
            .context("Failed to create Redis client")?;

        let connection = client
            .get_multiplexed_async_connection()
            .await
            .context("Failed to connect to Redis")?;

        tracing::info!("Connected to Redis at {}", redis_url);

        Ok(Self {
            client,
            connection: Arc::new(Mutex::new(connection)),
        })
    }

    /// Set a key-value pair
    pub async fn set(&self, key: &str, value: &str) -> Result<()> {
        let mut conn = self.connection.lock().await;
        conn.set::<_, _, ()>(key, value)
            .await
            .context("Failed to set Redis key")?;
        Ok(())
    }

    /// Set a key-value pair with expiration (in seconds)
    pub async fn set_ex(&self, key: &str, value: &str, expiration_secs: u64) -> Result<()> {
        let mut conn = self.connection.lock().await;
        conn.set_ex::<_, _, ()>(key, value, expiration_secs)
            .await
            .context("Failed to set Redis key with expiration")?;
        Ok(())
    }

    /// Get a value by key
    pub async fn get(&self, key: &str) -> Result<Option<String>> {
        let mut conn = self.connection.lock().await;
        let val: Option<String> = conn.get(key)
            .await
            .context("Failed to get Redis key")?;
        Ok(val)
    }

    /// Delete a key
    pub async fn delete(&self, key: &str) -> Result<()> {
        let mut conn = self.connection.lock().await;
        conn.del::<_, ()>(key)
            .await
            .context("Failed to delete Redis key")?;
        Ok(())
    }

    /// Check if a key exists
    pub async fn exists(&self, key: &str) -> Result<bool> {
        let mut conn = self.connection.lock().await;
        let exists: bool = conn.exists(key)
            .await
            .context("Failed to check Redis key existence")?;
        Ok(exists)
    }

    /// Set expiration on existing key
    pub async fn expire(&self, key: &str, seconds: u64) -> Result<()> {
        let mut conn = self.connection.lock().await;
        conn.expire::<_, ()>(key, seconds as i64)
            .await
            .context("Failed to set expiration on Redis key")?;
        Ok(())
    }

    /// Get multiple keys at once
    pub async fn mget(&self, keys: &[&str]) -> Result<Vec<Option<String>>> {
        let mut conn = self.connection.lock().await;
        let values: Vec<Option<String>> = conn.get(keys)
            .await
            .context("Failed to get multiple Redis keys")?;
        Ok(values)
    }

    /// Set multiple key-value pairs at once
    pub async fn mset(&self, pairs: &[(&str, &str)]) -> Result<()> {
        let mut conn = self.connection.lock().await;
        conn.set_multiple::<_, _, ()>(pairs)
            .await
            .context("Failed to set multiple Redis keys")?;
        Ok(())
    }

    /// Increment a counter
    pub async fn incr(&self, key: &str) -> Result<i64> {
        let mut conn = self.connection.lock().await;
        let new_value: i64 = conn.incr(key, 1)
            .await
            .context("Failed to increment Redis counter")?;
        Ok(new_value)
    }

    /// Decrement a counter
    pub async fn decr(&self, key: &str) -> Result<i64> {
        let mut conn = self.connection.lock().await;
        let new_value: i64 = conn.decr(key, 1)
            .await
            .context("Failed to decrement Redis counter")?;
        Ok(new_value)
    }

    /// Store a JSON value
    pub async fn set_json<T: serde::Serialize>(&self, key: &str, value: &T) -> Result<()> {
        let json_str = serde_json::to_string(value)
            .context("Failed to serialize value to JSON")?;
        self.set(key, &json_str).await
    }

    /// Store a JSON value with expiration
    pub async fn set_json_ex<T: serde::Serialize>(
        &self,
        key: &str,
        value: &T,
        expiration_secs: u64,
    ) -> Result<()> {
        let json_str = serde_json::to_string(value)
            .context("Failed to serialize value to JSON")?;
        self.set_ex(key, &json_str, expiration_secs).await
    }

    /// Get and deserialize a JSON value
    pub async fn get_json<T: serde::de::DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        match self.get(key).await? {
            Some(json_str) => {
                let value = serde_json::from_str(&json_str)
                    .context("Failed to deserialize JSON from Redis")?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    /// Add item to a list (LPUSH)
    pub async fn list_push(&self, key: &str, value: &str) -> Result<()> {
        let mut conn = self.connection.lock().await;
        conn.lpush::<_, _, ()>(key, value)
            .await
            .context("Failed to push to Redis list")?;
        Ok(())
    }

    /// Get list range
    pub async fn list_range(&self, key: &str, start: isize, stop: isize) -> Result<Vec<String>> {
        let mut conn = self.connection.lock().await;
        let values: Vec<String> = conn.lrange(key, start, stop)
            .await
            .context("Failed to get Redis list range")?;
        Ok(values)
    }

    /// Trim list to specified range
    pub async fn list_trim(&self, key: &str, start: isize, stop: isize) -> Result<()> {
        let mut conn = self.connection.lock().await;
        conn.ltrim::<_, ()>(key, start, stop)
            .await
            .context("Failed to trim Redis list")?;
        Ok(())
    }

    /// Add to a set
    pub async fn set_add(&self, key: &str, member: &str) -> Result<()> {
        let mut conn = self.connection.lock().await;
        conn.sadd::<_, _, ()>(key, member)
            .await
            .context("Failed to add to Redis set")?;
        Ok(())
    }

    /// Get all set members
    pub async fn set_members(&self, key: &str) -> Result<Vec<String>> {
        let mut conn = self.connection.lock().await;
        let members: Vec<String> = conn.smembers(key)
            .await
            .context("Failed to get Redis set members")?;
        Ok(members)
    }

    /// Check if member exists in set
    pub async fn set_is_member(&self, key: &str, member: &str) -> Result<bool> {
        let mut conn = self.connection.lock().await;
        let is_member: bool = conn.sismember(key, member)
            .await
            .context("Failed to check Redis set membership")?;
        Ok(is_member)
    }

    /// Get all keys matching pattern
    pub async fn keys(&self, pattern: &str) -> Result<Vec<String>> {
        let mut conn = self.connection.lock().await;
        let keys: Vec<String> = conn.keys(pattern)
            .await
            .context("Failed to get Redis keys")?;
        Ok(keys)
    }

    /// Flush all keys (use with caution!)
    pub async fn flush_all(&self) -> Result<()> {
        let mut conn = self.connection.lock().await;
        redis::cmd("FLUSHALL")
            .query_async::<String>(&mut *conn)  // Fixed: single generic, correct type
            .await
            .context("Failed to flush Redis database")?;
        tracing::warn!("Flushed all Redis keys");
        Ok(())
    }

    /// Ping Redis to check connection
    pub async fn ping(&self) -> Result<()> {
        let mut conn = self.connection.lock().await;
        redis::cmd("PING")
            .query_async::<String>(&mut *conn)  // Fixed: single generic parameter
            .await
            .context("Failed to ping Redis")?;
        Ok(())
    }
}

/// Helper functions for common cache patterns
impl RedisCache {
    /// Cache a computation result with TTL
    pub async fn cache_or_compute<F, T, Fut>(
        &self,
        key: &str,
        ttl_secs: u64,
        compute_fn: F,
    ) -> Result<T>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
        T: serde::Serialize + serde::de::DeserializeOwned,
    {
        // Try to get from cache first
        if let Some(cached) = self.get_json::<T>(key).await? {
            tracing::debug!("Cache hit for key: {}", key);
            return Ok(cached);
        }

        // Cache miss, compute the value
        tracing::debug!("Cache miss for key: {}, computing...", key);
        let value = compute_fn().await?;

        // Store in cache
        self.set_json_ex(key, &value, ttl_secs).await?;

        Ok(value)
    }

    /// Invalidate cache by pattern
    pub async fn invalidate_pattern(&self, pattern: &str) -> Result<usize> {
        let keys = self.keys(pattern).await?;
        let count = keys.len();

        for key in keys {
            self.delete(&key).await?;
        }

        tracing::debug!("Invalidated {} keys matching pattern: {}", count, pattern);
        Ok(count)
    }
}
