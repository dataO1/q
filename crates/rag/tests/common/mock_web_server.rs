//! Mock web server for testing web crawler functionality

use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path, any};
use anyhow::Result;

/// Mock web server for testing web crawling
pub struct MockWebServer {
    server: MockServer,
}

impl MockWebServer {
    /// Start a new mock web server
    pub async fn start() -> Result<Self> {
        let server = MockServer::start().await;
        Ok(Self { server })
    }

    /// Get the base URL of the mock server
    pub fn base_url(&self) -> String {
        self.server.uri()
    }

    /// Get URL for a specific path
    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url(), path)
    }

    /// Setup mock for Rust documentation page
    pub async fn setup_rust_docs(&self) {
        let rust_doc_content = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <title>Rust Documentation - Async Functions</title>
        </head>
        <body>
            <h1>Async Functions in Rust</h1>
            <p>Async functions in Rust allow you to write asynchronous code that doesn't block the thread.</p>
            
            <h2>Basic Syntax</h2>
            <pre><code>
async fn example() -> Result<String, Error> {
    let data = fetch_data().await?;
    Ok(format!("Processed: {}", data))
}
            </code></pre>
            
            <h2>Error Handling</h2>
            <p>Use the <code>?</code> operator for error propagation in async functions.</p>
            <code>let result = risky_operation().await?;</code>
            
            <h3>Best Practices</h3>
            <ul>
                <li>Always handle errors appropriately</li>
                <li>Use proper error types</li>
                <li>Consider timeout handling</li>
            </ul>
        </body>
        </html>
        "#;

        Mock::given(method("GET"))
            .and(path("/rust/async"))
            .respond_with(ResponseTemplate::new(200).set_body_string(rust_doc_content))
            .mount(&self.server)
            .await;
    }

    /// Setup mock for Python documentation page  
    pub async fn setup_python_docs(&self) {
        let python_doc_content = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <title>Python Documentation - Async/Await</title>
        </head>
        <body>
            <h1>Python Asyncio</h1>
            <p>Python's asyncio library provides support for asynchronous programming.</p>
            
            <h2>Basic Example</h2>
            <pre><code>
import asyncio

async def main():
    print('Hello')
    await asyncio.sleep(1)
    print('World')

asyncio.run(main())
            </code></pre>
            
            <h2>Error Handling</h2>
            <p>Use try/except blocks with async functions:</p>
            <pre><code>
async def handle_errors():
    try:
        result = await risky_function()
        return result
    except Exception as e:
        print(f"Error: {e}")
        return None
            </code></pre>
        </body>
        </html>
        "#;

        Mock::given(method("GET"))
            .and(path("/python/asyncio"))
            .respond_with(ResponseTemplate::new(200).set_body_string(python_doc_content))
            .mount(&self.server)
            .await;
    }

    /// Setup mock for general programming concepts page
    pub async fn setup_programming_concepts(&self) {
        let concepts_content = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <title>Programming Concepts - Error Handling</title>
        </head>
        <body>
            <h1>Error Handling in Programming</h1>
            <p>Error handling is a critical aspect of robust software development.</p>
            
            <h2>Common Patterns</h2>
            <h3>Result Types</h3>
            <p>Many languages use Result or Option types for safe error handling.</p>
            
            <h3>Exception Handling</h3>
            <p>Try/catch blocks are common in many programming languages.</p>
            
            <h3>Error Propagation</h3>
            <p>Proper error propagation ensures errors are handled at the right level.</p>
            
            <h2>Best Practices</h2>
            <ul>
                <li>Fail fast and provide clear error messages</li>
                <li>Use appropriate error types for different scenarios</li>
                <li>Log errors appropriately for debugging</li>
                <li>Consider recovery strategies where appropriate</li>
            </ul>
            
            <p>Remember: good error handling makes your code more maintainable and reliable.</p>
        </body>
        </html>
        "#;

        Mock::given(method("GET"))
            .and(path("/concepts/error-handling"))
            .respond_with(ResponseTemplate::new(200).set_body_string(concepts_content))
            .mount(&self.server)
            .await;
    }

    /// Setup mock for a page that returns JSON
    pub async fn setup_api_docs(&self) {
        let api_response = r#"
        {
            "title": "REST API Documentation",
            "description": "Comprehensive guide to our REST API",
            "endpoints": [
                {
                    "path": "/api/users",
                    "method": "GET", 
                    "description": "Retrieve list of users"
                },
                {
                    "path": "/api/users/{id}",
                    "method": "GET",
                    "description": "Retrieve specific user by ID"
                }
            ],
            "authentication": {
                "type": "Bearer Token",
                "description": "Include Authorization header with Bearer token"
            }
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/api/docs"))
            .respond_with(ResponseTemplate::new(200)
                .set_body_string(api_response)
                .insert_header("content-type", "application/json"))
            .mount(&self.server)
            .await;
    }

    /// Setup mock for a page that returns 404
    pub async fn setup_not_found(&self) {
        Mock::given(method("GET"))
            .and(path("/not-found"))
            .respond_with(ResponseTemplate::new(404).set_body_string("Page not found"))
            .mount(&self.server)
            .await;
    }

    /// Setup mock for a page that returns 500 error
    pub async fn setup_server_error(&self) {
        Mock::given(method("GET"))
            .and(path("/server-error"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal server error"))
            .mount(&self.server)
            .await;
    }

    /// Setup mock for a slow response (for timeout testing)
    pub async fn setup_slow_response(&self) {
        Mock::given(method("GET"))
            .and(path("/slow"))
            .respond_with(ResponseTemplate::new(200)
                .set_body_string("This response is slow")
                .set_delay(std::time::Duration::from_secs(15))) // Longer than typical timeout
            .mount(&self.server)
            .await;
    }

    /// Setup all common mocks for comprehensive testing
    pub async fn setup_all_mocks(&self) {
        self.setup_rust_docs().await;
        self.setup_python_docs().await;
        self.setup_programming_concepts().await;
        self.setup_api_docs().await;
        self.setup_not_found().await;
        self.setup_server_error().await;
        self.setup_slow_response().await;
    }
}

/// Helper function to create a mock server with all standard mocks
pub async fn create_test_server() -> Result<MockWebServer> {
    let server = MockWebServer::start().await?;
    server.setup_all_mocks().await;
    Ok(server)
}