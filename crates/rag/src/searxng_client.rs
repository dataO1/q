//! SearXNG API Client for Web Search
//!
//! This module provides a robust HTTP client for interacting with SearXNG instances,
//! enabling privacy-focused web search with JSON API support.

use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, info, instrument, warn};

/// SearXNG search result returned from the API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// Target URL of the search result
    pub url: String,

    /// Page title
    pub title: String,

    /// Content snippet/description
    #[serde(default)]
    pub content: String,

    /// Search engine that provided this result
    #[serde(default)]
    pub engine: String,

    /// Result score/ranking (if available)
    #[serde(default)]
    pub score: f32,
}

/// SearXNG API response structure
#[derive(Debug, Deserialize)]
struct SearXNGResponse {
    /// Search query
    query: String,

    /// Number of results
    number_of_results: usize,

    /// List of search results
    results: Vec<SearXNGResult>,

    /// Suggestions (if any)
    #[serde(default)]
    suggestions: Vec<String>,
}

/// Internal representation of a SearXNG result
#[derive(Debug, Deserialize)]
struct SearXNGResult {
    url: String,
    title: String,

    #[serde(default)]
    content: String,

    #[serde(default)]
    engine: String,

    #[serde(default)]
    score: f32,
}

/// SearXNG HTTP client with connection pooling and error handling
#[derive(Debug, Clone)]
pub struct SearXNGClient {
    /// Base URL of the SearXNG instance (e.g., "http://localhost:8888")
    endpoint: String,

    /// HTTP client with connection pooling
    client: Client,

    /// Request timeout duration
    timeout: Duration,

    /// Maximum results to return per query
    max_results: usize,

    /// Preferred search engines (e.g., ["google", "duckduckgo"])
    engines: Vec<String>,
}

impl SearXNGClient {
    /// Create a new SearXNG client
    ///
    /// # Arguments
    ///
    /// * `endpoint` - Base URL of SearXNG instance (e.g., "http://localhost:8888")
    /// * `timeout_secs` - Request timeout in seconds
    /// * `max_results` - Maximum number of results to return
    /// * `engines` - Preferred search engines to use
    ///
    /// # Returns
    ///
    /// Configured `SearXNGClient` instance
    ///
    /// # Errors
    ///
    /// Returns error if HTTP client initialization fails
    #[instrument(skip_all, fields(endpoint = %endpoint))]
    pub fn new(
        endpoint: String,
        timeout_secs: u64,
        max_results: usize,
        engines: Vec<String>,
    ) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .pool_max_idle_per_host(10)
            .pool_idle_timeout(Duration::from_secs(90))
            .build()
            .context("Failed to build HTTP client")?;

        let instance = Self {
            endpoint,
            client,
            timeout: Duration::from_secs(timeout_secs),
            max_results,
            engines,
        };

        info!(
            "Initialized SearXNG client: endpoint={}, timeout={}s, max_results={}",
            instance.endpoint, timeout_secs, max_results
        );

        Ok(instance)
    }

    /// Perform a web search query
    ///
    /// # Arguments
    ///
    /// * `query` - Search query string
    ///
    /// # Returns
    ///
    /// Vector of `SearchResult` containing URLs, titles, and content snippets
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - SearXNG is unreachable
    /// - Request times out
    /// - Response parsing fails
    #[instrument(skip(self), fields(query_len = query.len(), endpoint = %self.endpoint))]
    pub async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        let url = format!("{}/search", self.endpoint);

        // Build query parameters
        let mut params = vec![
            ("q", query.to_string()),
            ("format", "json".to_string()),
        ];

        // Add engine preferences if specified
        if !self.engines.is_empty() {
            let engines_str = self.engines.join(",");
            params.push(("engines", engines_str));
        }

        debug!("Sending SearXNG request: query='{}', params={:?}", query, params);

        // Execute request with retry logic
        let response = self
            .client
            .get(&url)
            .query(&params)
            .send()
            .await
            .context("Failed to send request to SearXNG")?;

        // Check status
        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!(
                "SearXNG returned error status {}: {}",
                status,
                error_text
            );
        }

        // Parse JSON response
        let search_response: SearXNGResponse = response
            .json()
            .await
            .context("Failed to parse SearXNG JSON response")?;

        info!(
            "SearXNG search completed: query='{}', found {} results",
            query, search_response.number_of_results
        );

        // Convert to our SearchResult format and limit results
        let results: Vec<SearchResult> = search_response
            .results
            .into_iter()
            .take(self.max_results)
            .map(|r| SearchResult {
                url: r.url,
                title: r.title,
                content: r.content,
                engine: r.engine,
                score: r.score,
            })
            .collect();

        debug!("Returning {} search results", results.len());

        Ok(results)
    }

    /// Perform a health check on the SearXNG instance
    ///
    /// Validates that SearXNG is reachable and responding correctly.
    /// Recommended to call during startup to fail fast if misconfigured.
    ///
    /// # Returns
    ///
    /// `Ok(())` if SearXNG is healthy, error otherwise
    #[instrument(skip(self), fields(endpoint = %self.endpoint))]
    pub async fn health_check(&self) -> Result<()> {
        let url = format!("{}/", self.endpoint);

        debug!("Performing SearXNG health check at {}", url);

        let response = self
            .client
            .get(&url)
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .context("SearXNG health check failed: instance unreachable")?;

        if !response.status().is_success() {
            anyhow::bail!(
                "SearXNG health check failed: status {}",
                response.status()
            );
        }

        info!("SearXNG health check passed");
        Ok(())
    }

    /// Extract and deduplicate URLs from search results
    ///
    /// # Arguments
    ///
    /// * `results` - Search results to process
    ///
    /// # Returns
    ///
    /// Vector of unique URLs
    pub fn extract_urls(results: &[SearchResult]) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        results
            .iter()
            .filter_map(|r| {
                if seen.insert(r.url.clone()) {
                    Some(r.url.clone())
                } else {
                    None
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_creation() {
        let client = SearXNGClient::new(
            "http://localhost:8888".to_string(),
            10,
            5,
            vec!["duckduckgo".to_string()],
        );
        assert!(client.is_ok());
    }

    #[test]
    fn test_url_extraction() {
        let results = vec![
            SearchResult {
                url: "https://example.com".to_string(),
                title: "Example".to_string(),
                content: "Test".to_string(),
                engine: "google".to_string(),
                score: 1.0,
            },
            SearchResult {
                url: "https://example.com".to_string(), // Duplicate
                title: "Example 2".to_string(),
                content: "Test".to_string(),
                engine: "bing".to_string(),
                score: 0.9,
            },
            SearchResult {
                url: "https://test.com".to_string(),
                title: "Test".to_string(),
                content: "Content".to_string(),
                engine: "google".to_string(),
                score: 0.8,
            },
        ];

        let urls = SearXNGClient::extract_urls(&results);
        assert_eq!(urls.len(), 2);
        assert!(urls.contains(&"https://example.com".to_string()));
        assert!(urls.contains(&"https://test.com".to_string()));
    }
}
