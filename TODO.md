- really integrate the RAG and history context!!
- decide whether to use workflowsteps or single step agents.
- fine tune which models to use for which tasks.

### #######################################################
### Dynamic Context Expansion During Agent Execution
### #######################################################

**Problem:** Agents currently receive RAG context **once** at task start and cannot request additional context during execution when they encounter ambiguous or insufficient information.

**Why We Need This:**
1. **Adaptive Intelligence**: Agents should be able to gather more context when they realize their initial context is insufficient
2. **State-of-the-Art Alignment**: Claude CLI, Perplexity, and other modern agent systems allow dynamic knowledge retrieval during execution
3. **Better Task Completion**: Agents can iteratively refine their understanding rather than proceeding with incomplete information
4. **Local Inference Optimization**: Only pulls additional context when needed, reducing unnecessary token usage

**Current Limitation:**
```rust
// In executor.rs:483-514 - context retrieved ONCE at task start
if let Some(provider) = context_provider.as_ref() {
    let context = provider.retrieve_context(task.description.clone(), ...).await;
    agent_context = agent_context.with_rag_context(context); // FIXED for entire task
}
// Agent executes with this fixed context - cannot get more
```

**Implementation Plan:**

1. **Add RAG Tool to ToolSet** (`tools/mod.rs`):
   ```rust
   pub enum DynamicTool {
       // ... existing tools
       RagQuery(Arc<RagQueryTool>),
   }

   pub struct RagQueryTool {
       context_provider: Arc<ContextProvider>,
       project_scope: ProjectScope,
       conversation_id: ConversationId,
   }
   ```

2. **RagQueryTool Implementation**:
   - Accept query + optional scope filters as parameters
   - Call the existing `ContextProvider::retrieve_context()` method
   - Return formatted context similar to current `FormattedRagContext`
   - Support different query types (code search, documentation, web search)

3. **Agent Integration**:
   - Add `"rag_query"` to required_tools for agents that need dynamic context
   - Agents can call during ReAct loops: `rag_query("authentication middleware patterns")`
   - Tool returns additional context that agents can use for better decision-making

4. **Example Usage Flow**:
   ```
   Task: "Add authentication to user service"
   → Initial RAG: Basic user model structure
   → Agent: "Need more context about auth patterns"
   → Agent calls: rag_query("authentication middleware nodejs express")
   → Tool returns: Auth middleware examples + JWT patterns
   → Agent: "Need database schema details"
   → Agent calls: rag_query("user table schema database")
   → Tool returns: Database structure + relations
   → Agent generates complete, accurate implementation
   ```

5. **Implementation Files to Modify**:
   - `crates/agent-network/src/tools/rag.rs` (new file)
   - `crates/agent-network/src/tools/mod.rs` (add RagQueryTool)
   - Update agent `define_workflow_steps()` to include `"rag_query"` in required_tools
   - Add proper tool instructions in `ToolSet::get_tool_type_instructions()`

**Benefits:**
- Agents become truly adaptive and can handle complex, ambiguous tasks
- Reduces initial context bloat by loading additional context only when needed
- Matches patterns used by state-of-the-art agent systems
- Maintains existing upfront context loading for planning while adding dynamic expansion capability

### #######################################################
### Query Decomposition (sub-question generation)
### #######################################################

❌ Current State: Your System

Based on your logs, your system does:

text
User Query: "Search most effective binary search algorithms and implement..."
           ↓
   [Source Router] → Classifies entire query as "Online" or "Workspace"
           ↓
   [Query Enhancement] → Rewrites ENTIRE query for that tier
           ↓
   [Retrieval] → Sends same enhanced query to all sources

Problem: The entire complex query goes to every source - there's no decomposition or intent-aware routing per sub-query.
✅ State of the Art: Query Decomposition RAG

Modern RAG systems use query decomposition (also called sub-question generation):

​

text
User Query: "Search most effective binary search algorithms and implement
             a binary search function in python with a main method to execute from the cli"
           ↓
   [Intent Analyzer] → Detects multi-intent query
           ↓
   [Query Decomposer] → Breaks into atomic sub-queries:
           ├─ "most effective binary search algorithms comparison" → Web
           ├─ "binary search implementation examples python" → Code search
           └─ "python CLI main method pattern" → Docs/Workspace
           ↓
   [Per-Query Routing] → Each sub-query routed to best source
           ↓
   [Parallel Retrieval] → All queries execute concurrently
           ↓
   [Result Merging + Reranking] → Combine and score by relevance

🎯 State-of-the-Art Approaches
1. Plan*RAG (Best for Complex Queries)
​

Creates a DAG (Directed Acyclic Graph) of sub-queries with dependencies:

text
"Who wrote the coming-of-age novel published by Viking Press in 1951?"

DAG Structure:
┌─────────────────────────────────────┐
│ "coming-of-age novel Viking Press"  │  → Web Search
└──────────────┬──────────────────────┘
               ↓ (entity: novel name)
┌─────────────────────────────────────┐
│ "author of [The Catcher in the Rye]"│  → Knowledge Graph
└─────────────────────────────────────┘

Key insight: Sub-queries can depend on each other's results.
2. MultiHop-RAG
​

Standard approach for multi-hop questions:

    Decompose: LLM breaks query into independent sub-questions

    Retrieve: Get passages for each sub-question

    Merge: Combine and deduplicate results

    Rerank: Score by relevance to original complex query

Your example decomposition:

python
# Original query
"Search most effective binary search algorithms and implement a binary
search function in python with a main method to execute from the cli"

# Decomposed sub-queries
[
    "What are the most effective binary search algorithms?",
    "How to implement binary search in Python?",
    "How to create a Python CLI with main method?",
    "Python argparse command-line arguments example"
]

3. RQ-RAG (Reasoning-Enhanced)
​

Adds an intent classifier to detect which queries need decomposition:

python
if is_multi_intent(query):
    sub_queries = decompose(query)
    # Route each independently
else:
    # Single intent - use as-is

🔍 How Your Example Should Be Handled
Your Query:

text
"Search most effective binary search algorithms and implement a binary search
function in python with a main method to execute from the cli"

Ideal Decomposition:
Sub-Query	Intent	Best Source	Reasoning
"most effective binary search algorithm comparison benchmark"	Informational	Web (SearXNG)	Needs recent comparisons, benchmarks
"binary search implementation python code example"	Code Example	Web + Qdrant	Needs working code, could be in local files
"python main method CLI argparse pattern"	How-to	Qdrant (local docs)	Common pattern, likely in your codebase
"python __name__ == '__main__' best practices"	Reference	Web (StackOverflow)	Needs community consensus
Why This Matters:

    Without decomposition: Sends entire messy query to all sources → poor retrieval

    With decomposition: Each sub-query optimized for its source → excellent results


### #######################################################
### 🔬 Specialized Retrieval Agents: Complete Deep Dive
### #######################################################

Specialized retrieval is a state-of-the-art approach where different retrieval methods are used for different query types, dramatically improving RAG system quality.
Why It Exists

The fundamental problem: Different queries require fundamentally different retrieval strategies:

​

    "What is photosynthesis?" → Needs semantic similarity (dense vectors)

    "Find error code E4782" → Needs exact keyword match (BM25)

    "Who did John work with at company X?" → Needs relationship traversal (knowledge graph)

    "Average Q4 revenue" → Needs structured data (SQL)

No single retrieval method handles all cases well.

​
The Three Core Retrieval Types
Type	Best For	Strengths	Weaknesses	Speed
Dense/Vector	Semantic queries, concepts	Handles synonyms, semantic similarity	Misses exact keywords	Medium
BM25/Sparse	Technical terms, IDs, codes	Fast, precise, transparent	No semantic understanding	Fast (10-100x)
Graph/KG	Relationships, multi-hop	Explainable paths, complex reasoning	Expensive to build, slower	Slow
Performance Improvements (Proven)

From ORAN Telecom benchmark (600 questions):

​
Metric	VectorRAG	GraphRAG	HybridRAG	Winner
Faithfulness	0.55	0.59	0.59	Graph/Hybrid
Factual Correctness	0.48	0.50	0.58	Hybrid (+8%)
Context Relevance	0.10	0.11	0.04	GraphRAG
Answer Relevance	0.73	0.74	0.72	GraphRAG

From production Reddit benchmarks:

​

    Dense only: Recall@10 = 0.75

    BM25 only: Recall@10 = 0.70

    Hybrid (Dense+BM25): Recall@10 = 0.85 (+10-15% improvement!)

    + Reranking: Recall@10 = 0.88 (+3-5% more)

Query complexity matters:

​

    Easy questions: Hybrid = 0.65, Vector = 0.61 (small gap)

    Hard questions: Hybrid = 0.52, Vector = 0.38 (+37% improvement!)

How It Works: Hybrid Architecture

text
User Query → Query Understanding Agent
              │
              ├─→ Dense Retriever (semantic)     → 50 results
              ├─→ BM25 Retriever (keyword)       → 30 results
              └─→ Graph Retriever (relationships) → 15 results
              │
              ↓
         Reciprocal Rank Fusion (RRF)
              │
              ↓
         Cross-Encoder Reranking (optional)
              │
              ↓
         Top-K Results → LLM Generation

Fusion Strategy (RRF):

​

text
Score(doc) = Σ weight_i / (60 + rank_i)

Simple, no calibration needed, proven effective.

​
How Agents Should Use This

Agent decision process:

    Analyze task → Identify information needs

    Choose retriever(s):

        Conceptual understanding → semantic

        Exact terms/IDs → keyword

        Dependencies/relationships → graph

        Complex → hybrid (default)

    Retrieve iteratively: Broad → Specific → Relationships

    Synthesize results to complete task

Tool interface:

rust
search_context(
    query: "error handling patterns microservices",
    retriever_type: "hybrid",  // "semantic" | "keyword" | "graph" | "hybrid"
    scope: "code",
    max_results: 5
)

State-of-the-Art Implementations

Production systems:

​

    LlamaIndex: QueryFusionRetriever with RRF

    LangChain: EnsembleRetriever + CrossEncoderReranker

    Weaviate: Native hybrid search (alpha parameter for BM25/vector balance)

    HybridRAG (arXiv 2408.04948): Vector + Graph fusion for finance domain

Real deployments:

    Microsoft Azure AI Search: Agentic retrieval with multi-query intelligent retrieval

​

IBM Granite: Multi-agent RAG with specialized retrievers

​

Anthropic: Multi-agent research system with iterative JIT retrieval

    ​

For Your Rust System

Phase 1 (High ROI, 2-3 days):

    Add BM25 using tantivy (Rust-native)

    Implement RRF fusion (simple algorithm)

    Expose via search_context tool

    Expected: +10-15% improvement

Phase 2 (Higher ROI, 2-4 weeks):

    Build knowledge graph from code relationships

    Add graph retriever (Neo4j/Memgraph)

    Implement adaptive routing

    Expected: +7-8% additional improvement

