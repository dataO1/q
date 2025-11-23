2. Target Hierarchical Structure (Tree ASCII)

This is what a useful trace for an Agent Network should look like. It focuses on logical flow (Task -> Plan -> Execute -> Tool) rather than function call stacks.

text
[Trace: execute_query] (Total: 125s)
├── [Span: orchestrator_decompose] (Duration: 5s)
│   ├── [Span: llm_planning_call] (Model: "llama3.1", Tokens: 450/120)
│   └── [Span: parse_plan] (Tasks: 2)
│
├── [Span: execute_workflow_dag] (Tasks: 2, Strategy: "Parallel")
│   ├── [Span: task_execution] (TaskID: "task-1", Agent: "CodingAgent")
│   │   ├── [Span: rag_context_retrieval] (Duration: 30s) <--- HOTSPOT
│   │   │   ├── [Span: query_embedding] (Provider: "ort", Duration: 1.5s)
│   │   │   └── [Span: vector_search] (Hits: 15, Score: 0.85)
│   │   │
│   │   ├── [Span: react_loop] (MaxIter: 10)
│   │   │   ├── [Span: iteration_1]
│   │   │   │   ├── [Span: llm_inference] (Prompt: 2048t, Gen: 150t, Stop: "tool_call")
│   │   │   │   └── [Span: tool_execute] (Tool: "filesystem", Op: "read", Path: "/src/main.rs")
│   │   │   │
│   │   │   ├── [Span: iteration_2]
│   │   │   │   ├── [Span: llm_inference] (Prompt: 2300t, Gen: 50t, Stop: "stop")
│   │   │   │   └── [Span: tool_execute] (Tool: "filesystem", Op: "write", Path: "/src/search.py")
│   │   │   │
│   │   │   └── [Span: validate_result] (Status: "Success")
│   │
│   └── [Span: task_execution] (TaskID: "task-2", Agent: "ReviewAgent")
│       └── ... (similar structure)
│
└── [Span: synthesize_results] (Duration: 0.5s)

3. Attributes You MUST Log Per Span Type

To achieve the tree above, clean up your tracing instrumentation. Remove the default trace::init() verbose flags if possible, or explicitly filter attributes.
A. For LLM Calls (llm_inference / chat_completion)

    llm.provider: "ollama", "openai"

    llm.model: "llama3", "phi3"

    llm.token_count.prompt: 1024

    llm.token_count.completion: 150

    llm.latency_per_token: 45ms

    CRITICAL: llm.cached: true/false (if you add caching later)

B. For ReAct Loop (react_loop, iteration_N)

    agent.name: "CodingAgent"

    agent.iteration: 1, 2, 3...

    agent.state: "thinking", "tool_use", "done"

    agent.stop_reason: "tool_call", "max_iterations", "stop_sequence"

C. For Tools (tool_execute)

    tool.name: "filesystem"

    tool.action: "write_file" (parse this from args if possible)

    tool.target: "/path/to/file.rs" (crucial for debugging file contention)

    tool.status: "ok", "error"

D. For RAG (rag_retrieval)

    rag.query: "binary search implementation"

    rag.sources_found: 15

    rag.embedding_time_ms: 1500

Summary of Efficiency Fixes

    Disable Code Metadata: Stop automatically injecting file/line/module. It bloats the trace by 90% without helping performance analysis.

    Add "Business Logic" Tags: Manually add tags for tokens, iterations, and filenames.

    Flatten Tool Spans: Instead of tool_execute -> mutex_lock -> function_call -> internal_logic, just have one tool_execute span with attributes describing the action.

This structure will allow you to instantly see: "Ah, Task 1 failed because Iteration 4 called filesystem on a locked file, and the LLM took 40s to generate the call.
