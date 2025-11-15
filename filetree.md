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
│   ├── src
│   │   ├── config.rs
│   │   ├── error.rs
│   │   ├── lib.rs
│   │   ├── llm.rs
│   │   └── types.rs
│   └── tests
│       ├── config_test.rs
│       └── types_test.rs
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
│   ├── src
│   │   ├── bin
│   │   │   └── indexer.rs
│   │   ├── chunk_adaptive.rs
│   │   ├── classifier.rs
│   │   ├── lib.rs
│   │   ├── metadata_chunk_transformer.rs
│   │   ├── metadata_transformer.rs
│   │   ├── pipeline.rs
│   │   └── watcher.rs
│   └── tests
│       ├── classifier_test.rs
│       ├── common
│       │   └── mod.rs
│       ├── pipeline_test.rs
│       └── watcher_integration_test.rs
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
│   ├── src
│   │   ├── bin
│   │   │   └── rag_cli.rs
│   │   ├── context_manager.rs
│   │   ├── context_providers.rs
│   │   ├── lib.rs
│   │   ├── query_enhancer.rs
│   │   ├── reranker.rs
│   │   ├── retriever.rs
│   │   └── source_router.rs
│   └── tests
│       ├── query_enhancer_tests.rs
│       └── retriever_tests.rs
└── storage
    ├── Cargo.toml
    ├── src
    │   ├── lib.rs
    │   ├── postgres.rs
    │   ├── qdrant.rs
    │   └── redis.rs
    └── tests
        ├── postgres_test.rs
        ├── qdrant_test.rs
        └── redis_test.rs

32 directories, 94 files
