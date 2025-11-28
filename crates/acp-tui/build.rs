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
    
    // For now, use the fallback spec due to OpenAPI 3.1.0 compatibility issues
    println!("cargo:warning=Using fallback OpenAPI specification for compatibility");
    let openapi_json = get_fallback_spec();
    
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

fn get_fallback_spec() -> serde_json::Value {
    // Embedded minimal OpenAPI spec as fallback
    serde_json::json!({
        "openapi": "3.0.3",
        "info": {
            "title": "Agent Communication Protocol (ACP) API",
            "version": "1.0.0",
            "description": "REST API for multi-agent orchestration"
        },
        "servers": [
            {
                "url": "http://localhost:9999",
                "description": "Local development server"
            }
        ],
        "paths": {
            "/execute": {
                "post": {
                    "operationId": "executeQuery",
                    "tags": ["execution"],
                    "summary": "Execute a query",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "$ref": "#/components/schemas/ExecuteRequest"
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Execution started",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "$ref": "#/components/schemas/ExecuteResponse"
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "/capabilities": {
                "get": {
                    "operationId": "getCapabilities",
                    "tags": ["discovery"],
                    "summary": "List agent capabilities",
                    "responses": {
                        "200": {
                            "description": "Available capabilities",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "$ref": "#/components/schemas/CapabilitiesResponse"
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "/health": {
                "get": {
                    "operationId": "healthCheck",
                    "tags": ["health"],
                    "summary": "Health check",
                    "responses": {
                        "200": {
                            "description": "Server health",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "$ref": "#/components/schemas/HealthResponse"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        },
        "components": {
            "schemas": {
                "ExecuteRequest": {
                    "type": "object",
                    "required": ["query"],
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "The user query to execute"
                        },
                        "project_scope": {
                            "type": "object",
                            "description": "Project context information"
                        }
                    }
                },
                "ExecuteResponse": {
                    "type": "object",
                    "required": ["conversation_id"],
                    "properties": {
                        "conversation_id": {
                            "type": "string",
                            "description": "Unique identifier for tracking execution"
                        },
                        "message": {
                            "type": "string",
                            "description": "Response message"
                        }
                    }
                },
                "CapabilitiesResponse": {
                    "type": "object",
                    "required": ["agents", "features", "version"],
                    "properties": {
                        "agents": {
                            "type": "array",
                            "items": {
                                "$ref": "#/components/schemas/AgentCapability"
                            }
                        },
                        "features": {
                            "type": "array",
                            "items": {
                                "type": "string"
                            }
                        },
                        "version": {
                            "type": "string"
                        }
                    }
                },
                "AgentCapability": {
                    "type": "object",
                    "required": ["agent_type", "description", "tools"],
                    "properties": {
                        "agent_type": {
                            "type": "string"
                        },
                        "description": {
                            "type": "string"
                        },
                        "tools": {
                            "type": "array",
                            "items": {
                                "type": "string"
                            }
                        }
                    }
                },
                "HealthResponse": {
                    "type": "object",
                    "required": ["status"],
                    "properties": {
                        "status": {
                            "type": "string"
                        },
                        "message": {
                            "type": "string"
                        },
                        "timestamp": {
                            "type": "string",
                            "format": "date-time"
                        }
                    }
                }
            }
        }
    })
}