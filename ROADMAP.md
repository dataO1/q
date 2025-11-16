# Implementation Phases

- Phase 1 - Core Foundation (Week 1):
    * Implement common crate types
    * Implement storage adapters (Qdrant, PostgreSQL)
    * Basic configuration loading
- Phase 2 - Indexing (Week 2) (using swiftide):
    * File Watcher (inotify-based)
        * Watch configured directories for file changes
        * Detect new, modified, deleted files
        * Respect .gitignore patterns
    * Path Classifier Chain
        * Classify files into tiers: System/Personal/Workspace/Dependencies/Online
        * Detect project roots (git)
        * Language detection
        * File type classification
    * Tree-sitter Chunker
        * Smart code-aware chunking
        * Extract definitions (functions, classes, structs)
        * Maintain context boundaries
        * Metadata extraction
    * Ollama Embedder
        * Generate embeddings using Ollama's nomic-embed-text
        * Batch processing for efficiency
        * Dense vector generation
- Phase 3 - RAG (Week 3):
    1. General RAG Framework and Context Manager
        + Define core interfaces and abstractions for query enhancement, source routing, retrieval, and reranking.
        + Integrate common logic models from the existing library crate.
        + Design async APIs supporting parallel multi-source retrieval.
        + Support batch processing and streaming response patterns.
        + Coordinate context-aware filtering and query augmentation.
    2. Query Enhancer Component
        + Implement context-aware query reformulation.
        + Use history and context signals to enhance user queries.
        + Integrate small LLM or predefined heuristics for source-specific query generation.
    3. Source Router Heuristics
        + Develop heuristic logic to route queries to local workspace, personal files, online docs, and history sources.
        + Use keyword matching, task type, and context signals.
        + Support LLM-based routing fallback for ambiguity.
    4. Multi-Source Retriever
        + Implement parallel retrieval from Qdrant, PostgreSQL history, web scrapers, and local caches.
        + Support layered metadata filtering (project root, language, file type).
        + Optimize batching and streaming of retrieved data.
    5. FastEmbed Reranker
        + Implement embedding-based reranking and deduplication.
        + Use cosine similarity with thresholding.
        + Support dynamic weighting based on recency, dependency chain, and conversation signals.
- Phase 4 - History (Week 4):
    * ???
- Phase 5 - Agent-Network:
    * Week 1: Crate Setup and Core Orchestrator
        + Initialize the agent-network crate with the recommended module structure.
        + Implement configuration loading (config.rs) for agent definitions, HITL modes, retry policies, etc.
        + Implement basic orchestrator core (orchestrator.rs):
            - Dynamic graph generation using petgraph based on task decomposition input.
            - Execute task dependency resolution and topological sorting.
        + Define core types, traits, error handling (error.rs).
    * Week 2: Workflow and Graph Execution Engine
        + Implement workflow module:
            - DAG builder (workflow/builder.rs) creating execution DAGs with dependencies.
            - Executor (workflow/executor.rs) supporting wave-based parallel execution of tasks.
            - Implement failure recovery strategies and retry loop coordination outside DAG.
        + Implement conflict resolution and file lock management (conflict.rs, filelocks.rs).
    * Week 3: Agent Abstractions and Core Agents
        + Define agent base traits/interfaces (agents/base.rs) supporting structured I/O and Rig integration.
        + Implement core domain agents:
            - Coding agent (agents/coding.rs).
            - Planning agent (agents/planning.rs).
            - Writing agent (agents/writing.rs).
        + Prepare evaluator agent for quality assessment and optimization loops (agents/evaluator.rs).
    * Week 4: Tool Integrations and MCP Tools Support
        + Integrate MCP tools adapter layer:
            - LSP integration (tools/lsp.rs).
            - Git operations (tools/git.rs).
            - Filesystem access and Tree-sitter parsing (tools/filesystem.rs, tools/treesitter.rs).
        + Implement coordination module (coordination.rs) for orchestrator-agent-tool workflow synchronization.
        + Integrate smart RAG and history context injections with the workflow.
    * Week 5: Human-in-the-Loop (HITL) System and Queuing
        + Implement HITL subsystem:
            - HITL mode management and assessor logic (hitl/assessor.rs).
            - HITL task queue and audit logging (hitl/queue.rs, hitl/audit.rs).
        + Integrate HITL checkpoints into the orchestrator workflow.
        + Add support for agent escalation to HITL on failure or low confidence.
    * Week 6: Token Budget Management and Streaming
        + Implement token budget management (token_budget.rs) around agent LLM calls using tiktoken-rs.
        + Implement real-time streaming of agent, tool, and orchestrator status updates (status_stream.rs).
        + Set up channel-based streaming with Tokio mpsc channels for UI and CLI integration.
    * Week 7: ACP Communication Protocol and API Layer
        + Implement ACP protocol support (acp.rs) with Axum server for API and WebSocket endpoints.
        + Add agent discovery and real-time status streaming endpoints.
        + Connect CLI and UI client interaction layers to the agent-network via ACP.
    * Week 8: Observability, Tracing, and Tests
        + Add OpenTelemetry Jaeger tracing integration (tracing.rs) across all major components.
        + Render petgraph DAGs as dot files for visualization (optional).
        + Write comprehensive integration and unit tests covering tasks, failure scenarios, HITL, and agent execution.
        + Document APIs, configurations, and agent coding guidelines within the crate.
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
  → petgrap Orchestrator
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
    │  PETGRAPH  ORCHESTRATOR                                  │
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
The system is a multi-agent orchestration framework built in Rust that coordinates specialized LLM agents through a DAG-based workflow engine using petgraph for graph execution and Rig for agent/LLM integration. An orchestrator agent dynamically generates execution plans as directed acyclic graphs, where nodes represent agent tasks and edges define dependencies, enabling wave-based parallel execution with automatic topological sorting for optimal performance. The architecture features three-layer RAG (Swiftide indexing, Qdrant/PostgreSQL storage, FastEmbed reranking), real-time event streaming via tokio::mpsc channels for progress monitoring, tool-level Git commits with LLM-generated semantic messages, and hybrid HITL (human-in-the-loop) with orchestrator-defined checkpoints and agent self-escalation for low-confidence decisions. State management is handled through atomic file-level Git commits (no workflow checkpointing), file locking for concurrent agent coordination, and token budgeting with tiktoken-rs to optimize context usage across the entire multi-agent workflow.
## Features
- [ ] Basic functionality
    - [ ] Ollama integration for all agents, with structured in and outputs,
    - [ ] Agents dont need agent-to-agent communication !!
    - [ ] no need for checkpointing system!
      tools integration, streaming.
    - [ ] Reading configuration options from config.toml (i.e.: agent
      definitions, prompts, model selection and other important critical
      options, while staying lean)
    - [ ] no circuit-breaking mechanism for now
- [ ] Dynamic DAC graph generation with dependencies and tools (via petgraph).
  For now graphs should be fixed after initial planning, no mid-graph-execution
  replanning.
    - [ ] Orchestrator node has a list of tools and agents. On query analyses and decomposes it, generates a detailed dynamic petgraph compatible execution plan using rig native patterns, fitting for the task:
        - [ ] computes full graph execution with failure aware parallel waves with toposort:
        ```Example:
            // Wave 0: [analyze, security_scan] <- parallel
            // Wave 1: [implement]               <- waits for both
            // Wave 2: [test]                    <- waits for implement
        ```
        - [ ] including HITL stops
            - [ ] varios HITLmodes based on risk assesment
            ```
                pub enum HITLMode {
                    Blocking,     // Pause workflow until human approves
                    Async,        // Continue, but flag for post-hoc review
                    SampleBased,  // Only review X% of tasks
            }
            ```
            - [ ] Orchestrator defines HITL checkpoints for high-risk tasks (baked into graph)
            - [ ] Agents can escalate dynamically when confidence is low (runtime decision) (this should be supported by the general architecture, but not yet implemented)
        - [ ] failure fallback edges (if agent signals failure  the graph should handle this gracefully, ie the orchestrator should be aware of this and the graph template should support it). there should be various ErrorRecoveryStrategy per task, decided by the orchestrator on graph generation:
            ```
            pub enum ErrorRecoveryStrategy {
                /// Retry same agent (transient failures)
                Retry { max_attempts: usize, backoff: Duration },

                /// Switch to backup agent (agent-specific failures)
                SwitchAgent { backup: AgentId },

                /// Skip task and continue (non-critical)
                Skip,

                /// Request human intervention (critical failures)
                EscalateToHuman,

                /// Abort entire workflow
                Abort,
            }
            ## Error Recovery
            - Transient errors (network timeout) → Retry 3x with exponential backoff
            - Agent failures (model crash) → Switch to backup agent (if available)
            - User errors (invalid input) → Skip task, log warning
            - Critical errors (disk full) → Escalate to human, abort workflow
            ```
        - [ ] evaluator optimizer loops, based on the agents risk level.
            - [ ] orchestrator evaluates risk level per agent and bakes in an evaluator
              agent in the graph depending on the evaluated riks-level and its assigned quality strategy. this evaluator node should read the output, be aware of the context (which agent produced the output, what it should review and how harsh, who will read the review):
                ```
                enum QualityStrategy {
                    Always,               // Every task gets evaluator
                    OnlyForCritical,      // Only if task is high-risk
                    AfterNIterations(usize), // After N refinement attempts
                    Never,                // Skip evaluation
                }
                ```
            - [ ] The petgraph DAG enforces correct execution order without cycles, while retries happen as orchestrator-driven looped executions. This keeps the graph clean and your workflow semantics simple, leveraging orchestration logic for retries without violating DAG acyclicity.
            - [ ] Orchestrator manages retry loop: Instead of introducing cycles in the graph, the orchestrator detects low-quality evaluation results and triggers re-execution of the original agent node(s) explicitly outside the graph traversal. This might mean re-running node(s) or a wave from scratch.
            - [ ] State and control are outside the DAG: The orchestrator maintains metadata about retry counts, evaluation scores, and decides when to stop retrying or escalate.
        - [ ] Complexety Analysis and task decomposition (subtask splitting). Should
          be possible with the architecture, but not yet implemented.
        - [ ] Per-agent, per-execution token budget optimization via tiktoken-rs before and after llm calls according to
          best practices, using:
            - [ ] context pruning
            - [ ] prompt caching
            - [ ] model selection
- [ ] tools wrapper, which orchestrator initializes agents with:
    - [ ] streaming based
    - [ ] conflict detection and resolution (multiple agents working on same files)
        - [ ] has reference to a File locks manager component which keeps a list of open files for all tools (so that parallel agents have to wait for a lock for writing the same file)
        - [ ] post-execution verification(optional for now)
    - [ ] failure detection (example: file lock times out at 30s, agent cant
      read file it needs, agent fails, graph acts according to its failure
      recovery plan)
- [ ] per-agent pre-fetched (on graph execution, not agent execution) combined context:
    - [ ] smartRAG integration
    - [ ] History Manager integration
- [ ] Tools integration (via MCP)
    - [ ] Treesitter
    - [ ] Filesystem for writing files, listing files etc.
    - [ ] Tightly integrated with git (respect gitignore)
        - [ ] git commit on side-effect(i.e file write) with smart commit
          message (generated by the agent).
    - [ ] LSP integration
    - [ ] keep open for more tools
- [ ] Tracing/monitoring
    - [ ] with open telemetry + jaeger
    - [ ] petgraph output as rendered dot file (if possible)
- [ ] ACP integration
    - [ ] status update streams from agents via orchestrator as a centralised gateway (users never interact with agents directly):
        - [ ] Three layer streaming (Layer 1 agents produce events, Layer 2 Tools produce file events, Orchestrator collects and streams)
            - [ ] Agents emit status updates via channels (tokio::mpsc)
            - [ ] Orchestrator collects and streams to user
            - [ ] Both layers participate but orchestrator coordinates
            - [ ] Each agents output should have its own agent id for channel,
              such that the UI later can dynamically show parallel execution updates for running agents:
              ```Example:
                Coding Agent: Loading files (<cropped file tools output>)
                Review Agent: Reviewing previous implementation...
              ```
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

