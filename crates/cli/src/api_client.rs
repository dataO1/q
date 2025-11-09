use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Client for communicating with the ACP API server
pub struct ApiClient {
    base_url: String,
    client: reqwest::Client,
}

#[derive(Serialize)]
struct ExecuteRequest {
    query: String,
    conversation_id: Option<String>,
}

#[derive(Deserialize)]
struct ExecuteResponse {
    task_id: String,
    stream_url: String,
}

impl ApiClient {
    pub async fn new(base_url: &str) -> Result<Self> {
        Ok(Self {
            base_url: base_url.to_string(),
            client: reqwest::Client::new(),
        })
    }

    pub async fn execute_query(&self, query: &str) -> Result<String> {
        let url = format!("{}/execute", self.base_url);

        let request = ExecuteRequest {
            query: query.to_string(),
            conversation_id: None,
        };

        let response = self.client
            .post(&url)
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("API request failed: {}", response.status());
        }

        let body = response.text().await?;
        Ok(body)
    }

    pub async fn stream_status(&self, task_id: &str) -> Result<()> {
        // TODO: Implement SSE streaming
        todo!("Implement status streaming")
    }
}
