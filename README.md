# AI Agent Network - Multi-Agent Orchestration Framework

A production-ready multi-agent orchestration framework built in Rust with comprehensive RAG (Retrieval-Augmented Generation) capabilities and ACP (Agent Communication Protocol) API.

## 🚀 Quick Start

### Start the ACP Server

```bash
# Start with default config (config.dev.toml)
cargo run -p ai-agent-network -- server

# Or specify custom config and port
cargo run -p ai-agent-network -- --config config.toml server --port 8080
```

The server provides:
- **REST API** at `http://localhost:9999` (default port)
- **Interactive Documentation** at `http://localhost:9999/docs` (Swagger UI)
- **OpenAPI Specification** at `http://localhost:9999/api-doc/openapi.json`

### Execute Single Query

```bash
cargo run -p ai-agent-network -- execute "Analyze the codebase structure"
```

### Validate Configuration

```bash
cargo run -p ai-agent-network -- validate-config
```

## 📋 Prerequisites

### Required Services (Docker)

Start the infrastructure services:

```bash
docker-compose up -d
```

This starts:
- **PostgreSQL** (localhost:15432) - Data persistence
- **Redis** (localhost:16379) - Caching & session management  
- **Qdrant** (localhost:16334) - Vector database for RAG
- **SearXNG** (localhost:8888) - Web search proxy
- **Jaeger** (localhost:14268) - Distributed tracing

### Required Models (Ollama)

Install and start required LLM models:

```bash
# Install models (adjust based on config.dev.toml)
ollama pull qwen3:8b           # Main agent model
ollama pull all-minilm:l6-v2   # Embedding model
ollama pull qwen3:4b           # Query enhancement
ollama pull phi3:mini          # Classification

# Start Ollama server
ollama serve  # Default: http://localhost:11434
```

## 🏗️ Architecture

```
┌─────────────────────┐
│   ACP REST API      │ ← Client Integration Layer
├─────────────────────┤
│  Execution Manager  │ ← Conversation & Stream Management
├─────────────────────┤
│   Orchestrator      │ ← Multi-Agent Workflow Coordination
├─────────────────────┤
│   Agent Network     │ ← Specialized AI Agents
│  Coding│Planning│   │   (Coding, Planning, Evaluator)
│       Evaluator     │
├─────────────────────┤
│   RAG System       │ ← Context-Aware Information Retrieval
│ Workspace│Personal│ │   (Local + Web Sources)
│       Web Sources   │
├─────────────────────┤
│   Storage Layer     │ ← Persistence & Caching
│ PostgreSQL│Redis│   │   (PostgreSQL, Redis, Qdrant)
│      Qdrant         │
└─────────────────────┘
```

## 🔧 Configuration

Main configuration file: `config.dev.toml` (development) or `config.toml` (production)

### Key Sections

- **`[agent_network.acp]`** - API server settings (host, port)
- **`[agent_network.agents]`** - Agent definitions with models and tools
- **`[storage]`** - Database connection strings
- **`[rag]`** - RAG system configuration and models
- **`[indexing]`** - Content indexing paths and filters

### Agent Configuration

The framework supports specialized agent types:

- **Coding**: Code analysis, implementation, debugging (`qwen3:8b`)
- **Planning**: Task decomposition and workflow planning (`qwen3:8b`)  
- **Evaluator**: Code review and quality assessment (`qwen3:8b`)

## 🧪 Testing

### Quick Test Suite

```bash
sh scripts/run_tests.sh
```

### RAG Integration Tests (65 comprehensive tests)

```bash
# All RAG integration tests
cargo test -p ai-agent-rag --test source_router_tests --test searxng_integration_tests --test retriever_priority_tests --test query_enhancer_tests --test error_edge_case_tests -- --test-threads=1

# Individual test categories
cargo test -p ai-agent-rag --test source_router_tests      # Query routing (12 tests)
cargo test -p ai-agent-rag --test searxng_integration_tests # Web search (15 tests)
cargo test -p ai-agent-rag --test retriever_priority_tests  # Multi-source retrieval (9 tests)
cargo test -p ai-agent-rag --test query_enhancer_tests      # Query enhancement (8 tests) 
cargo test -p ai-agent-rag --test error_edge_case_tests     # Error handling (13 tests)
```

**Test Requirements:**
- Docker services running (`docker ps` should show all containers healthy)
- LLM-dependent tests marked `#[ignore]` (run separately when Ollama available)
- Tests auto-create missing Qdrant collections on first run

## 📚 API Usage

### Core Endpoints

- **POST** `/execute` - Start asynchronous multi-agent workflow
- **GET** `/capabilities` - Discover available agents and features  
- **GET** `/health` - Check API server status
- **WebSocket** `/stream/{conversation_id}` - Real-time execution updates

### Example API Call

```bash
curl -X POST http://localhost:9999/execute \
  -H "Content-Type: application/json" \
  -d '{
    "query": "Analyze the authentication module and suggest improvements",
    "project_scope": {
      "root": "/path/to/project",
      "languages": ["rust", "typescript"],
      "frameworks": ["axum", "react"],
      "key_files": [
        {"path": "src/auth.rs", "purpose": "Authentication logic"}
      ]
    }
  }'
```

## 🔍 Troubleshooting

**Common Issues:**
- **Config not found**: Run from project root directory
- **Service connectivity**: Verify `docker ps -a` shows healthy containers
- **Model not found**: Ensure all required Ollama models are pulled
- **LLM timeouts**: Check Ollama service is running on correct port

**Debug Output:**
```bash
RUST_LOG=debug cargo run -p ai-agent-network -- server
```

## 🚦 Service Endpoints

**Development Services:**
- ACP API: `http://localhost:9999`
- Qdrant: `http://localhost:16334` 
- PostgreSQL: `postgresql://test_user:test_pass@localhost:15432/ai_agent_test`
- Redis: `redis://localhost:16379`
- SearXNG: `http://localhost:8888`
- Ollama: `http://localhost:11434`
- Jaeger: `http://localhost:14268`

**Documentation:**
- Swagger UI: `http://localhost:9999/docs`
- OpenAPI Spec: `http://localhost:9999/api-doc/openapi.json`