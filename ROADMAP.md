# Implementation Phases

- Phase 1 - Core Foundation (Week 1):
    * Implement common crate types
    * Implement storage adapters (Qdrant, PostgreSQL)
    * Basic configuration loading
- Phase 2 - Indexing (Week 2):
    * File watcher with inotify
    * Path classifier chain
    * Tree-sitter chunker
    * Ollama embedder
- Phase 3 - RAG (Week 3):
    * Query enhancer
    * Source router (heuristics)
    * Multi-source retriever
    * FastEmbed reranker
- Phase 4 - History (Week 4):
    * Buffer memory (LRU)
    * PostgreSQL semantic search
    * Progressive summarizer
- Phase 5 - Orchestrator (Weeks 5-6):
    * Agent pool
    * Workflow builder (GraphFlow)
    * Wave executor with locking
    * Checkpoint system
- Phase 6 - Polish (Weeks 7-8):
    * MCP tools
    * API server
    * CLI (interactive + one-shot)
    * NixOS module

# Agent Coding Guidelines/Prompt
Research relevant library and API definitions based on recent version and analyze their usage patterns.
Generate Production ready code, modular, configurable, with documentation with recent best-practices and architectural patterns according to the following information in this document.
The various components should share common logic/models from a library crate, but be segragated into their own crates.
Utilize library native patterns wherever possible, minimize custom code. Think before you code.

# Overview:

1. Persistence
2. Smart Multi-Source RAG
3. Smart History Manager
4. Dynamic Agent-Network with smart orchestration
5. User Interfaces
  a. CLI
  b. API
  c. Neovim

## User Flow
```User Query
  → ACP Server (Axum)
  → GraphFlow Orchestrator
      → Complexity Analysis (LLM)
      → Task Decomposition (if complex)
      → Agent Selection (from pool)
      → Agent.dynamic_context()
          → Smart RAG (query enhancement + multi-source)
              → Combination of Qdrant, history and web search (parallel local + online), based given query
              → FastEmbed rerank
      → Agent.prompt() with context
          → MCP tools (LSP/git/files as needed)
          → Result streaming (WebSocket)
  → User Interface
```

```
┌─────────────────────────────────────────────────────────────────┐
│  USER INTERFACES                                                │
│  ┌──────────┐  ┌──────────┐  ┌─────────────────────────────┐  │
│  │  Shell   │  │ Neovim   │  │  Optional Web UI            │  │
│  │  (stdio) │  │ (LSP/RPC)│  │  (WebSocket/React)          │  │
│  └────┬─────┘  └────┬─────┘  └──────────┬──────────────────┘  │
└───────┼─────────────┼───────────────────┼──────────────────────┘
        └─────────────┴───────────────────┘
                      │
    ┌─────────────────▼────────────────────┐
    │  ACP SERVER (Axum)                   │
    │  - REST API + WebSocket              │
    │  - Agent discovery endpoints         │
    │  - Streaming task execution          │
    │  - HITL approval queue               │
    └─────────────────┬────────────────────┘
                      │
    ┌─────────────────▼────────────────────────────────────────┐
    │  GRAPHFLOW ORCHESTRATOR                                  │
    │  - Dynamic workflow graph construction (runtime DAG)     │
    │  - Task dependency resolution (topological sort)         │
    │  - Wave-based parallel execution (petgraph)              │
    │  - Conditional routing (NextAction::Continue)            │
    │  - Session state management (PostgreSQL)                 │
    └─────────────────┬────────────────────────────────────────┘
                      │
         ┌────────────┴────────────┐
         │                         │
    ┌────▼─────────────┐    ┌─────▼──────────────┐
    │  ORCHESTRATOR    │    │  SPECIALIZED       │
    │  AGENT           │    │  AGENTS (Pool)     │
    │                  │    │                    │
    │  Model: Llama    │    │  - Coding Agent    │
    │  3.3 70B         │    │    (Qwen2.5-Coder  │
    │                  │    │     32B)           │
    │  Responsibilities│    │  - Planning Agent  │
    │  - Task          │    │    (Qwen2.5 14B)   │
    │    decomposition │    │  - Writing Agent   │
    │  - Complexity    │    │    (Qwen2.5 14B)   │
    │    analysis      │    │                    │
    │  - SubAgent      │    │  Features:         │
    │    delegation    │    │  - MCP tools       │
    │  - Conflict      │    │  - Isolated context│
    │    resolution    │    │  - File locking    │
    └────┬─────────────┘    └─────┬──────────────┘
         │                        │
         └────────────┬───────────┘
                      │
         ┌────────────▼────────────────────┐
         │  ALL AGENTS USE:                │
         │  ┌──────────────────────────┐  │
         │  │ Dynamic Context (Rig)    │  │
         │  │ - Smart RAG injection    │  │
         │  │ - Conversation history   │  │
         │  └──────────────────────────┘  │
         │  ┌──────────────────────────┐  │
         │  │ MCP Tools                │  │
         │  │ - LSP queries            │  │
         │  │ - Tree-sitter parsing    │  │
         │  │ - Git operations         │  │
         │  │ - File I/O               │  │
         │  └──────────────────────────┘  │
         └─────────────┬───────────────────┘
                       │
         ┌─────────────▼───────────────────────────────────────┐
         │  SMART MULTI-SOURCE RAG SYSTEM                      │
         │                                                      │
         │  ┌──────────────────────────────────────────────┐  │
         │  │ CONTEXT MANAGER                              │  │
         │  │ - Project scope detection (git root)         │  │
         │  │ - Conversation history filtering (semantic)  │  │
         │  │ - Token budget management (tiktoken)         │  │
         │  └──────────────────────────────────────────────┘  │
         │                                                      │
         │  ┌──────────────────────────────────────────────┐  │
         │  │ QUERY ENHANCEMENT                            │  │
         │  │ - Context-aware expansion (LLM)              │  │
         │  │ - History-based reformulation (DH-RAG)       │  │
         │  │ - Source-specific query generation           │  │
         │  └──────────────────────────────────────────────┘  │
         │                                                      │
         │  ┌──────────────────────────────────────────────┐  │
         │  │ PARALLEL MULTI-SOURCE RETRIEVAL (tokio::join)│  │
         │  │                                               │  │
         │  │  ┌─────────────────┐  ┌──────────────────┐  │  │
         │  │  │ Local Code      │  │ Online Docs      │  │  │
         │  │  │ (Qdrant)        │  │ (Qdrant)         │  │  │
         │  │  │ - SwiftIDE      │  │ - Web scraper    │  │
         │  │  │   indexed       │  │   indexed        │  │  │
         │  │  │ - Definition    │  │ - Latest APIs    │  │  │
         │  │  │   metadata      │  │ - Best practices │  │  │
         │  │  └─────────────────┘  └──────────────────┘  │  │
         │  └──────────────────────────────────────────────┘  │
         │                                                      │
         │  ┌──────────────────────────────────────────────┐  │
         │  │ RERANKING & SYNTHESIS                        │  │
         │  │ - FastEmbed BGE-Reranker-Base                │  │
         │  │ - Deduplication (content hash)               │  │
         │  │ - Scope filtering (project_id)               │  │
         │  │ - Relevance scoring (cosine similarity)      │  │
         │  └──────────────────────────────────────────────┘  │
         └──────────────────┬───────────────────────────────────┘
                            │
              ┌─────────────┴─────────────┐
              │                           │
    ┌─────────▼────────┐        ┌────────▼─────────┐
    │  QDRANT VECTOR   │        │  POSTGRESQL      │
    │  DATABASE        │        │                  │
    │                  │        │  - Conversations │
    │  Collections:    │        │  - Messages      │
    │  - local_code    │        │    (embeddings)  │
    │  - online_docs   │        │  - User prefs    │
    │  - conv_history  │        │  - Summaries     │
    │                  │        │  - Session state │
    │  Features:       │        │                  │
    │  - IVF indexing  │        │  Extensions:     │
    │  - Metadata      │        │  - pgvector      │
    │    filtering     │        │                  │
    │  - Sharding      │        │  Indexes:        │
    │    support       │        │  - Vector HNSW   │
    └──────────────────┘        │  - GIN on JSONB  │
                                └──────────────────┘
              │                           │
    ┌─────────▼────────┐        ┌────────▼─────────┐
    │  INDEXING        │        │  HISTORY         │
    │  PIPELINE        │        │  MANAGER         │
    │  (SwiftIDE)      │        │                  │
    │                  │        │  Features:       │
    │  - File loader   │        │  - Short-term    │
    │    (gitignore)   │        │    memory (LRU)  │
    │  - Tree-sitter   │        │  - Long-term     │
    │    chunking      │        │    semantic      │
    │  - Definition    │        │    search        │
    │    extraction    │        │  - Adaptive      │
    │  - Ollama        │        │    summarization │
    │    embedding     │        │  - Topic         │
    │  - Qdrant        │        │    clustering    │
    │    storage       │        │                  │
    └──────────────────┘        └──────────────────┘

    ┌───────────────────────────────────────────────┐
    │  SUPPORTING SYSTEMS                           │
    │                                               │
    │  ┌──────────────────┐  ┌──────────────────┐ │
    │  │ HITL             │  │ Redis Cache      │ │
    │  │ Orchestrator     │  │ - Session state  │ │
    │  │ - Approval queue │  │ - Query cache    │ │
    │  │ - Risk assessor  │  │ - Hot user data  │ │
    │  │ - Audit log      │  │ - LRU eviction   │ │
    │  └──────────────────┘  └──────────────────┘ │
    │                                               │
    │  ┌──────────────────┐  ┌──────────────────┐ │
    │  │ File Lock        │  │ Shared Context   │ │
    │  │ Manager          │  │ - Interface defs │ │
    │  │ - PathBuf→TaskID │  │ - Type registry  │ │
    │  │ - RwLock guards  │  │ - Constraints    │ │
    │  └──────────────────┘  └──────────────────┘ │
    └───────────────────────────────────────────────┘

    ┌───────────────────────────────────────────────┐
    │  EXTERNAL SERVICES (Local)                    │
    │                                               │
    │  ┌──────────────┐  ┌────────────────────┐   │
    │  │ Ollama       │  │ LSP Servers        │   │
    │  │ - Llama 3.3  │  │ - rust-analyzer    │   │
    │  │ - Qwen2.5    │  │ - typescript-ls    │   │
    │  │ - nomic-embed│  │ - lua-language-srv │   │
    │  └──────────────┘  └────────────────────┘   │
    └───────────────────────────────────────────────┘
```
## Data filtering and scope for agents
Use a 3-Layer hybrid  data filtering approach with pre-filter, post-filter
// Optimal filtering strategy

1. Layer 1: INDEX-TIME: Separate by workspace/system_files/personal_files
   - workspace_main (your projects)
   - workspace_deps (dependencies)
   - workspace_docs (online docs)
   - personal_files (notes, personal configs, scripts, general documents etc)
   - system_files (man pages, system configs, standart libraies, nixos configs,
     installed binaries)

1. Layer 2: QUERY-TIME PRE-FILTER: Project + language
   - project_root = detected git root
   - language = relevant to task
   - file_type = source/config/docs
   - Keeps ~10-20% of workspace (optimal!) [web:307]
   - query personal_files/system_files if relevant to task (should be handled by
     the smartRAG)

3. Layer 3: SMART RAG POST-FILTER: Context-aware
   - Conversation history boost
   - Dependency graph boost
   - Recency boost
   - Exclude generated/test files

## Deployment
Fully declarative packages and services via flake.nix that can be used and configured in a system flake/configuration.nix. Relevant configuration files from the rust binaries should be optionally linked from there. Native packages/tools/patterns, wherever possible and make sure no run time configuration/dependency installment is necessary.
### Features
- [ ] devShell:
    - [ ] rust development enrionment with up to date toolchain etc.
    - [ ] scripts for debugging
- [ ] Package builds:
    - [ ] indexing binary
    - [ ] agent network binary (DAG + RAG + API)
- [ ] nixosModule (to use in systems configuration.nix):
    - [ ] declarative indexing service (should include qdrant nixos service settings)
    - [ ] declarative agent network service (should include necessary SQL db nixos service
      integration)

# 1. Persistence
## Architecture
a. Qdrant vector DB for semantic search over indexed files.
b. SQL DB for persisting conversation history and user preferences.
c. A rust binary that can run in the background, detecting file changes via inotify
watchers, smartly chunks the file, embedds them, and stores them in Qdrant.
### Features
- [ ] Indexing / Semantic Search
    - [x] Automatic File Watching based on inotify
    - [x] Smart Chunking depending on File Type
    - [ ] Treesitter advanced context retrieval (definitions etc.) for code
    - [ ] Multiple data set support (look at data layering) (i.e.: LlamaIndex has support for several
      online vector data sets)
    - [ ] Store dense and sparse vectors for indexed files
    - [ ] Coarse pre-filtering (layer 1 data filtering) (how to detect if
      personal_file, system_file or workspace_x?)
- [ ] History (PostgreSQL + pgvector)
    - [ ] data layering and isolation/scope (look at data layering)
    - [ ] further dynamic memory layering (Short-term + long-term + summaries)

# 2. Smart Multi-Source RAG
## Overview
A smart RAG system, that can be used by the agent network for retrieving
context-aware, relevant, up-to-date information.
## Features
- [ ] RAG-aware source routing
    - [ ] parallel retrieval
    - [ ] batch streaming to agents
    - [ ] Hybrid Source Selection
        - [ ] Fast Heuristics for 90% of queries (keyword matching, task type, context signals,
          file location, boost nearby files etc)
        - [ ] LLM Router 10% of queries (for ambiguous queries)
    - [ ] Source specific query generation
        - [ ] for each source enhance query with local code, personal docs,
          system docs, online with small llm
    - [ ] weighted reranking (FastEmbed)
        - [ ] post-filter (layer 3) (i.e boost recently edited, discussed files,
          files in dependecy chain, exclude tests, generated code etc.)
        - [ ] learned weightings (adjusting while usage)
        - [ ] diversity detection
- [ ] Qdrant search
    - [ ] choose search strategy
        - [ ] key-word focus (via sparse embedding)
        - [ ] semantic focus (via dense embedding)
        - [ ] hybrid/automatic (via qdrant/swiftide hybrid search)
    - [ ] Query-time data filtering (data filtering layer 2)
        - [ ] Qdrant-native Metadata filter on project_root, language etc.
- [ ] Integrate Context Providers (like with Continue.dev @ notations)

# 3. Smart History Manager
A smart history manager for agents to use for retrieving past conversation information.
Use the rig conversation module and use [this blog](https://dev.to/joshmo_dev/creating-ai-memories-using-rig-mongodb-2pg7) as a reference point but apply for PostgreSQL.

## Features
- [ ] Hierarchical Memory
    - [ ] Short-term (Immediate):
        - [ ] Last 5-10 turns
        - [ ] Full fidelity
        - [ ] Always included
        - [ ] Metric: 100% accuracy on recent exchanges
    - [ ] Long-term (Selective):
        - [ ] Semantic search in all history
        - [ ] Compressed/summarized
        - [ ] Retrieved on relevance
        - [ ] Metric: Top-3 recall for related topics
    - [ ] Working Memory (Dynamic):
        - [ ] Current task context
        - [ ] Active file references
        - [ ] Recent code changes
        - [ ] Metric: file reference accuracy >90%
- [ ] Context Window Management
    - [ ] Sliding window
    - [ ] Semantic chunking (group related exchanges)
    - [ ] progressive summarization (by a small llm/embedding defined in a config)
    - [ ] topic-based pruning
- [ ] History aware retrieval
- [ ] Conversation Patterns
    - [ ] Correction Patterns
    - [ ] Cancellation
    - [ ] Context Switching
    - [ ] Resume Pattern
- [ ] Metadata Tracking
    - [ ] per message (timestamp, role, topic tags, file references, code
  snippets, success rating per user feedback)
    - [ ] per conversation (project id, primary topic, active files, task type,
      last activity)

## Layers
```
Layer 1: Buffer Memory (LRU)
- Last 10 messages in-memory
- 0ms retrieval
- Token count tracking

Layer 2: Semantic Memory (PostgreSQL+pgvector)
- All messages indexed
- 10-20ms semantic search
- Topic clustering

Layer 3: Compressed Summaries (PostgreSQL)
- Every 50 messages → summary
- 5ms lookup
- Topic extraction
```

# 4. Dynamic Agent-Network with smart orchestration
## Overview
A Rust binary for orchestrating smart dynamiclly generated agent networks based on rig, GraphFlow.
## Features
- [ ] Basic functionality
    - [ ] Ollama integration
    - [ ] Reading configuration options from config.toml (i.e.: agent
      definitions, prompts, model selection and other important critical
      options, while staying lean)
- [ ] Dynamic DAC graph generation (petgraph + GraphFlow)
    - [ ] Complexety Analysis and task decomposition (subtask splitting)
        - [ ] wave execution
        - [ ] conflict detection and resolution (multiple agents working on same files)
            - [ ] Locking (Arch<RwLock<HashMap>>)
        - [ ] post-execution verification
    - [ ] HITL (with tokio::sync channels?)
    - [ ] LangGraph's Checkpointing System for state snapshots, HITL approval, crash
  recovery, with persistance
- [ ] per-agent combined context generation
    - [ ] smartRAG integration
    - [ ] History Manager integration
- [ ] Tools integration (via MCP)
    - [ ] Treesitter
    - [ ] Filesystem for writing files, listing files etc.
    - [ ] Tightly integrated with git (respect gitignore)
        - [ ] git commit on side-effect(i.e file write) with smart message.
    - [ ] LSP integration
    - [ ] keep open for more tools
- [ ] ACP integration
    - [ ] status update streams from agents via orchestrator as a centralised gateway (users never interact with agents directly)
    - [ ] common interactions as requests/capabilities (query, code-completion)


# 5. User Interfaces
## CLI
### Overview
Run queries to the agent network via cli.
### Features
- [ ] Basics
    - [ ] Modern rust cli with relevant parameters with two modes:
        - [ ] Interactive mode (when called with no parameters) with rustyline, whih supports Context Provider
          integration (@notation) with autocompletion.
        - [ ] One-shot mode with parameters
    - [ ] While running it should show relevant status updates, like running
      agents, their context size, retrieved sources (via streaming)
## API
### Overview
Provide an ACP API for UIs to connect to.
### Features
- [ ] ACP communication protocol (Axum + Websocket)
    - [ ] Streaming for status updates and HITL
    - [ ] Query endpoints for agent-network functionality
## Neovim


# File Structure

crates
├── api
│   ├── Cargo.toml
│   └── src
│       ├── lib.rs
│       ├── middleware
│       │   ├── logging.rs
│       │   └── mod.rs
│       ├── routes
│       │   ├── agents.rs
│       │   ├── execute.rs
│       │   ├── mod.rs
│       │   └── stream.rs
│       └── server.rs
├── cli
│   ├── Cargo.toml
│   └── src
│       ├── api_client.rs
│       ├── completions.rs
│       ├── display.rs
│       ├── interactive.rs
│       ├── lib.rs
│       ├── main.rs
│       └── oneshot.rs
├── common
│   ├── Cargo.toml
│   └── src
│       ├── config.rs
│       ├── error.rs
│       ├── lib.rs
│       └── types.rs
├── history
│   ├── Cargo.toml
│   └── src
│       ├── buffer_memory.rs
│       ├── lib.rs
│       ├── manager.rs
│       ├── metadata.rs
│       ├── patterns.rs
│       ├── semantic_memory.rs
│       └── summarizer.rs
├── indexing
│   ├── Cargo.toml
│   └── src
│       ├── chunker.rs
│       ├── classifier.rs
│       ├── embedder.rs
│       ├── lib.rs
│       ├── storage.rs
│       └── watcher.rs
├── mcp-tools
│   ├── Cargo.toml
│   └── src
│       ├── filesystem.rs
│       ├── git.rs
│       ├── lib.rs
│       ├── lsp.rs
│       ├── treesitter.rs
│       └── web.rs
├── orchestrator
│   ├── Cargo.toml
│   └── src
│       ├── agents
│       │   ├── coding.rs
│       │   ├── mod.rs
│       │   ├── orchestrator.rs
│       │   ├── planning.rs
│       │   ├── pool.rs
│       │   └── writing.rs
│       ├── coordination
│       │   ├── conflict.rs
│       │   ├── file_locks.rs
│       │   ├── mod.rs
│       │   └── shared_context.rs
│       ├── hitl
│       │   ├── assessor.rs
│       │   ├── audit.rs
│       │   ├── mod.rs
│       │   └── queue.rs
│       ├── lib.rs
│       ├── main.rs
│       └── workflow
│           ├── analyzer.rs
│           ├── builder.rs
│           ├── checkpoint.rs
│           ├── executor.rs
│           └── mod.rs
├── rag
│   ├── Cargo.toml
│   └── src
│       ├── context_manager.rs
│       ├── context_providers.rs
│       ├── lib.rs
│       ├── query_enhancer.rs
│       ├── reranker.rs
│       ├── retriever.rs
│       └── source_router.rs
└── storage
    ├── Cargo.toml
    └── src
        ├── lib.rs
        ├── postgres.rs
        ├── qdrant.rs
        └── redis.rs

25 directories, 79 files


# Changelog

Summary of Phase 1 Implementation

What we've completed:

✅ Common Crate:

    Complete type system with all domain types

    Comprehensive error handling with conversions

    Full configuration loading with validation

✅ Storage Layer:

    PostgreSQL: Full implementation with migrations, conversation storage, semantic search, checkpoints, audit logs

    Qdrant: Vector database with metadata filtering, batch operations, search

    Redis: Caching layer with JSON support, lists, sets, TTL, and cache patterns

Phase 1 is now complete and production-ready! All storage adapters are fully implemented with:

    Proper error handling

    Logging/tracing

    Connection pooling

    Async/await throughout

    Type safety

    Documentation

