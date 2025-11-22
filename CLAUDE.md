# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Development Commands

### Testing
- Run all tests: `sh scripts/run_tests.sh`
  - This script starts Docker infrastructure (PostgreSQL, Redis, Qdrant) and runs comprehensive tests across all crates
  - Individual crate tests: `cargo test -p <crate-name>`
  - Integration tests with external deps use `--ignored` flag and require test infrastructure

### Building
- Build all crates: `cargo build`
- Build specific crate: `cargo build -p <crate-name>` 
- Release build: `cargo build --release`

### Running
- Agent-Network server: `cargo run -p ai-agent-network -- server`
- Execute single query: `cargo run -p ai-agent-network -- execute "your query"`
- CLI interface: `cargo run -p ai-agent-cli`
- Configuration file: `config.dev.toml` (development) or `config.toml` (production)

## Architecture Overview

This is a multi-agent orchestration framework built in Rust with a workspace structure containing these key crates:

### Core Crates
- **agent-network**: Main orchestrator with DAG-based workflow execution, HITL integration, and MCP tool support
- **common**: Shared configuration, LLM integration, and common types
- **rag**: Retrieval-Augmented Generation with context management and query enhancement  
- **storage**: Database abstractions for PostgreSQL, Redis, and Qdrant vector DB
- **indexing**: File watching, chunking, and content classification using Swiftide
- **history**: Memory management with buffer and semantic memory systems
- **api**: Axum-based REST API server with WebSocket streaming
- **cli**: Interactive and one-shot CLI interface

### Key Architecture Patterns
- **Workflow Orchestration**: Uses petgraph for DAG-based task execution with wave-based parallel processing
- **Agent Types**: Specialized agents (coding, planning, writing, evaluator) with configurable LLM backends
- **Human-in-the-Loop (HITL)**: Audit queue system for human oversight and intervention
- **Conflict Resolution**: File locking and coordination mechanisms for concurrent operations
- **Context Management**: RAG-based context injection with history and shared context systems
- **Token Budget Management**: Intelligent token allocation across agent interactions
- **Real-time Streaming**: Status updates via WebSocket for live execution monitoring

### External Dependencies
- **LLM Backend**: Ollama integration for local LLM hosting
- **Vector Database**: Qdrant for embeddings and similarity search
- **Relational Database**: PostgreSQL for structured data and metadata
- **Cache Layer**: Redis for session management and caching
- **Indexing Pipeline**: Swiftide with tree-sitter for code analysis
- **Observability**: OpenTelemetry integration for distributed tracing

### Configuration
- Main config in `config.toml` or `config.dev.toml`
- Test environment uses `.env.test` for database connections
- Supports environment variable overrides (RUST_LOG, etc.)
- Agent-specific models and prompts configurable per use case

### Testing Strategy
- Unit tests for individual crate functionality
- Integration tests requiring external services marked with `--ignored`
- Docker Compose setup for test infrastructure in `scripts/run_tests.sh`
- Separate test databases to avoid data pollution

## Development Notes

- The project uses workspace dependencies with wildcard versions for consistency
- OpenTelemetry instrumentation is built-in for monitoring distributed agent execution  
- File operations use sophisticated locking to prevent conflicts during concurrent agent work
- RAG system supports multiple context providers with intelligent routing and reranking