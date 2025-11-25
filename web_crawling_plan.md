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

### Current feedback from gemini

🐞 Diagnosis: Web Source Retrieval Failure

The RAG system fails to return web results because the WebCrawlerRetriever is simulating search by constructing URL strings (like https://google.com/search?q=...) and then crawling the raw HTML of those search result pages using spider-rs. It is not parsing the search results to find actual links to crawl.

When spider-rs crawls a modern search engine results page (SERP), it gets a JavaScript-heavy, anti-bot-protected page or raw HTML full of unhelpful scripts, not the actual content you want.

Here is the breakdown of the failure chain:
1. ❌ Faulty "Search" Implementation

In web_crawler.rs, the generate_search_urls function merely appends the query to a URL:

rust
search_urls.push(format!("https://stackoverflow.com/search?q={}", urlencoding::encode(query)));

Then, retrieve calls crawl_url(search_url). This crawls the search results list page, not the answers.
2. ❌ Incorrect Crawling of SERPs

The spider website configuration in crawl_url is set to with_limit(1).

rust
website.with_limit(1); // Only crawl the single URL

This means it downloads the single page https://stackoverflow.com/search?q=..., extracts its text (which is likely "Search results for X..."), chunks it, and returns it. It does not follow the links to the actual answers.
3. ⚠️ Missing Search API

You are missing a real Search API (like Google Custom Search, Bing API, or DuckDuckGo HTML scraping) to turn a query into a list of target_urls.
🛠️ Fix Plan

To fix this locally without paid APIs, we need to scrape the search engine results to get actual target URLs, and then crawl those URLs.
Step 1: Fix generate_search_urls to actually resolve links

The current implementation just guesses URLs. You need a function that actually performs a search. Since you want a local approach, we can scrape DuckDuckGo's HTML version (easier to parse) or use a library that does this.
Step 2: Update retrieve logic

Instead of crawling the search URL directly, the flow should be:

    fetch_search_results(query) -> Vec<Url>

    crawl_parallel(urls) -> Vec<Content>

For SERP solution we should use SearXNG. Setup a service in the docker-compose,
add it to the configs and implement for the primitive search.

Step 3: Verify Agent Consumption

The ContextBuilder correctly formats RAG fragments. If we fix the retrieval, the agent will see the content.
📝 Actionable Code Changes

I will create a patch to replace the naive generate_search_urls with a basic SERP scraper and update the retrieve flow.
