//! Build script for generating ACP API client from OpenAPI specification
//!
//! This script fetches the OpenAPI specification from a running ACP server
//! and generates a strongly-typed Rust client using progenitor.

use anyhow::{Context, Result};
use std::env;
use std::path::PathBuf;

const DEFAULT_ACP_URL: &str = "http://localhost:9999";
const OPENAPI_ENDPOINT: &str = "/api-doc/openapi.json";

fn main() -> Result<()> {
    println!("cargo:rerun-if-changed=build.rs");
    
    // Allow override of ACP server URL via environment variable
    let acp_url = env::var("ACP_URL").unwrap_or_else(|_| DEFAULT_ACP_URL.to_string());
    let openapi_url = format!("{}{}", acp_url, OPENAPI_ENDPOINT);
    
    println!("cargo:warning=Fetching OpenAPI spec from: {}", openapi_url);
    
    // Fetch the OpenAPI spec from the running server
    let openapi_json = fetch_openapi_spec(&openapi_url)
        .context("Failed to fetch OpenAPI spec from server. Make sure the ACP server is running.")?;
    
    // Parse JSON into OpenAPI spec
    let openapi_spec: openapiv3::OpenAPI = serde_json::from_value(openapi_json)
        .context("Failed to parse OpenAPI specification")?;
    
    // Generate the client code
    let mut generator = progenitor::Generator::default();
    
    let tokens = generator
        .generate_tokens(&openapi_spec)
        .context("Failed to generate client tokens from OpenAPI spec")?;
    
    let output_file = PathBuf::from(env::var("OUT_DIR")?).join("acp_client.rs");
    
    std::fs::write(&output_file, tokens.to_string())
        .context("Failed to write generated client code")?;
    
    println!("cargo:warning=Generated ACP client at: {}", output_file.display());
    
    Ok(())
}

fn fetch_openapi_spec(url: &str) -> Result<serde_json::Value> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .context("Failed to create HTTP client")?;
    
    let response = client
        .get(url)
        .send()
        .context("Failed to fetch OpenAPI specification")?;
    
    if !response.status().is_success() {
        anyhow::bail!("HTTP error {}: {}", response.status(), 
                      response.text().unwrap_or_default());
    }
    
    let spec: serde_json::Value = response
        .json()
        .context("Failed to parse OpenAPI specification as JSON")?;
    
    Ok(spec)
}

