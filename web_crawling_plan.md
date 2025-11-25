For a local-first web retrieval source in your Rust RAG system, the modern approach combines intelligent crawling, semantic caching, content-aware chunking, and deduplication.
Crawling Libraries

spider-rs is your best Rust-native option. It's a high-performance, concurrent crawler written in Rust with:

    Async streaming support (integrates well with your existing streaming architecture)

    Headless Chrome support for JavaScript-heavy sites

    Built-in request caching (cache_chrome_hybrid feature)

    Subscribable events for real-time processing

    Markdown output optimized for LLMs

Alternative: qrawl for simpler composable crawling operations.

If you prefer API-based services that integrate well with Rust, Spider Cloud provides a REST API with Rust performance guarantees and returns LLM-ready markdown.

Caching Strategy (Critical for Local Use)

State-of-the-art RAG systems use approximate caching with LSH to avoid repeated retrieval. The Proximity system demonstrates 72-75% latency reduction:

Query-level cache: Use lsh-rs crate for locality-sensitive hashing to match similar queries. Instead of exact query matching, cache hits occur for semantically similar queries using cosine similarity or L2 distance.

Content-level cache:

    Store crawled HTML/markdown with TTL-based invalidation

Track content hashes (SHA-256) for deduplication

Use in-memory FAISS for embedding cache

For local deployment, FAISS (IndexFlatL2) provides lightweight vector storage without external dependencies.
Content Processing Pipeline

Markdown conversion: Modern crawlers output LLM-ready markdown that preserves semantic structure (headings, lists, code blocks) while removing boilerplate. Spider-rs supports this natively.

Chunking strategy: Use recursive character splitting with semantic boundaries:

    Split at headers, paragraphs, sentence boundaries (in that order)

    512-1024 token chunks with 10-20% overlap for context continuity

    Preserve metadata (URL, title, crawl timestamp, content hash)

Deduplication: Implement MinHash or SimHash before indexing:

    Content-level: Hash chunks to detect duplicates across sources

    URL-level: Normalize and hash URLs to prevent re-crawling

    The datasketch library pattern (though Python) can be replicated in Rust

Architecture Recommendation

text
Query → LSH Cache Check → [Cache Hit: Return cached chunks]
                       → [Cache Miss: Crawl + Process]
                            ↓
                       spider-rs crawl
                            ↓
                       Markdown extraction
                            ↓
                       Content hash check (dedup)
                            ↓
                       Recursive chunking
                            ↓
                       Local embedding (Ollama)
                            ↓
                       FAISS index + Qdrant store
                            ↓
                       Update LSH cache

Local Storage Considerations

    FAISS in-memory index for hot query cache (limited by RAM)

    Content hash map (BTreeMap or RocksDB) for deduplication

    TTL metadata in Qdrant for cache invalidation (7-30 days for web content)

Incremental crawling: Store last-crawled timestamps and content hashes to detect updates

This approach keeps your system local-first while leveraging proven techniques from production RAG systems that achieve 50-95% latency improvements through intelligent caching

    Raw Content Cache (The "Web Source"):
    Use a simple Key-Value store (like Sled or Redb in Rust) to store URL -> Markdown.

        Check: Has this URL been crawled in the last 24h?

        Yes: Load Markdown from Sled.

        No: Crawl -> Process -> Save to Sled -> Return.

    Semantic Cache (The "Smart" Layer):
    Create a specific collection in Qdrant named cache_queries.

        Store: vector: embedding(user_query), payload: { answer: "...", source_urls: [...] }

        Before searching web: Check this collection. If distance < 0.1 (highly similar), return cached payload.

    Summary

    Vector DB (Main): Only for your Local Code and Saved Knowledge.

    Vector DB (Cache Collection): For Semantic Caching of questions you've already answered.

    Key-Value Store: For raw web pages and search results.
