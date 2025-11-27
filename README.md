# AI Agent Network - Multi-Agent Orchestration Framework

A production-ready multi-agent orchestration framework built in Rust with comprehensive RAG (Retrieval-Augmented Generation) capabilities.

## Testing

### Quick Start - Run All Tests

```bash
sh scripts/run_tests.sh
```

### Integration Test Setup

The RAG system includes comprehensive integration tests that validate real-world functionality with external services. These tests require Docker infrastructure to be running.

#### Prerequisites

1. **Docker Services**: Start the test infrastructure
   ```bash
   docker-compose up -d
   ```

2. **Configuration**: Ensure `config.dev.toml` exists in the project root with proper service endpoints

3. **Test Data**: Tests automatically create and index test data from configured directories

#### Running RAG Integration Tests

The RAG integration test suite includes 65 comprehensive tests across 5 major functional areas:

##### Main Test Entry Points

```bash
# Run all RAG integration tests
cargo test -p ai-agent-rag --test source_router_tests --test searxng_integration_tests --test retriever_priority_tests --test query_enhancer_tests --test error_edge_case_tests -- --test-threads=1

# Run specific test categories
cargo test -p ai-agent-rag --test source_router_tests      # Query routing and classification
cargo test -p ai-agent-rag --test searxng_integration_tests # Web search functionality  
cargo test -p ai-agent-rag --test retriever_priority_tests  # Multi-source retrieval orchestration
cargo test -p ai-agent-rag --test query_enhancer_tests      # Query enhancement capabilities
cargo test -p ai-agent-rag --test error_edge_case_tests     # Error handling and resilience
```

##### Test Categories

1. **Source Router Tests** (12 tests)
   - LLM-based query classification into Workspace/Personal/Online tiers
   - Heuristic routing with fallback mechanisms
   - Configuration validation and edge cases
   - **Entry Point**: `cargo test -p ai-agent-rag --test source_router_tests`

2. **SearXNG Integration Tests** (15 tests)
   - Web search integration via SearXNG
   - Search functionality, caching, error handling
   - Concurrent request processing
   - **Entry Point**: `cargo test -p ai-agent-rag --test searxng_integration_tests`

3. **Retriever Priority Tests** (9 tests)
   - Priority-based multi-source retrieval (local sources priority 1, web sources priority 3)
   - Stream ordering and concurrent processing within priority groups
   - Qdrant vector database integration
   - **Entry Point**: `cargo test -p ai-agent-rag --test retriever_priority_tests`

4. **Query Enhancer Tests** (8 tests)
   - Query enhancement functionality with LLM models
   - Redis caching for performance optimization
   - Heuristic processing and edge cases
   - **Entry Point**: `cargo test -p ai-agent-rag --test query_enhancer_tests`

5. **Error Edge Case Tests** (13 tests)
   - Error handling for service failures (Redis, Qdrant, Ollama)
   - Security testing for injection attacks and malformed inputs
   - Resource exhaustion and timeout scenarios
   - **Entry Point**: `cargo test -p ai-agent-rag --test error_edge_case_tests`

##### Test Data Management

Tests automatically handle data setup:
- **Collection Detection**: Tests check if required Qdrant collections exist
- **Automatic Indexing**: Missing collections trigger indexing of test data from `config.dev.toml` configured directories:
  - Workspace data: `./test-workspace` (local code/documentation)
  - Personal data: `./test-notes` (user documents)
- **Data Persistence**: Test data is preserved between runs for performance (no cleanup)

##### Test Execution Notes

- **LLM Dependencies**: Tests requiring Ollama connectivity are marked `#[ignore]` and can be run separately when LLM services are available
- **Service Dependencies**: All tests assume Docker services are running (postgres, redis, qdrant, searxng, jaeger)
- **Configuration**: Tests use flexible config loading that works from any execution directory
- **Parallel Execution**: Use `--test-threads=1` to avoid resource conflicts

##### Infrastructure Verification

Verify Docker services are running:
```bash
docker ps -a
# Should show: postgres-test, redis-test, qdrant-test, searxng-test, jaeger (all healthy/running)
```

Service endpoints from `config.dev.toml`:
- Qdrant: `http://localhost:16334`
- Redis: `redis://localhost:16379`  
- PostgreSQL: `postgresql://test_user:test_pass@localhost:15432/ai_agent_test`
- SearXNG: `http://localhost:8888`
- Jaeger: `http://localhost:14268`

#### Troubleshooting

**Common Issues:**
- **Config not found**: Ensure you're running from the project root directory
- **Collection errors**: Tests will automatically create missing collections on first run  
- **Service connectivity**: Verify Docker services are healthy with `docker ps -a`
- **LLM timeouts**: LLM-dependent tests are marked `#[ignore]` - run them separately when Ollama is available

**Debug Output:**
```bash
# Run with detailed output
RUST_LOG=debug cargo test -p ai-agent-rag --test <test_name> -- --nocapture
```
