The long end‑to‑end times are mostly coming from external LLM/RAG calls and some avoidable overhead in how ReAct and tools are wired, rather than from a tight CPU loop in the ReAct code itself. The ReAct/tool implementation can still be improved: there are patterns that can increase token usage, cause unnecessary LLM calls, and serialize tool execution across tasks.
What the trace actually shows

    The Jaeger span for decompose_query has a duration of about 7.2×1087.2×108 ns (≈0.7 s), with busyns around 2.9×1072.9×107 ns, meaning most time is waiting on I/O (LLM / network), not CPU.

​

The main execute span in agents::base.rs for running the planning workflow has a comparable duration and very low busyns, again indicating the bulk of latency is in send_chat_messages to Ollama rather than the Rust workflow loop.

​

The server logs show per‑query timing in the same ballpark (≈700–800 ms from workflow start to the first wave of coding tasks), while the heavy ONNX Runtime initialisation (embedding model) happens much earlier at orchestrator startup, around 2–3 seconds once, not per query.

    ​

So the ReAct loop is not spinning wildly for hundreds of iterations on this trace, but there are aspects of its design that can make each LLM call heavier and introduce contention when tools are used in parallel.
ReAct loop issues

From execute_step_react in base.rs there are several design choices that can hurt latency under some workloads.

​

    Assistant messages are never added back into the history.
    Each iteration calls send_chat_messages with messages.clone(), but the only new messages added after a response are ChatMessage::user("Tool Result: ...") or ChatMessage::user("Tool Error: ..."). The assistant’s previous response (response.message.content) is never pushed as a ChatMessage::assistant.

​

    This means the model does not see its own prior thoughts/tool calls; it only sees the original system/user context plus an accumulating pile of “Tool Result” user messages. That can lead to more iterations than necessary because the model repeatedly re‑derives the tool call logic instead of building on its last response.

Max iterations are fairly high with no semantic stop condition.
The loop runs for _iteration in 0..max_iter with max_iter = max_iterations.unwrap_or(10).

​

    The only break condition is “no tool calls” in the last response; there is no explicit “task complete” signal in the schema. In bad prompt/model combinations, the model can ping‑pong between tool calls and results up to max_iter, adding extra LLM round‑trips.

Very large tool descriptions are injected into messages.
You already pass tools_info via .tools(tools_info.clone()) on the request, which Ollama uses for function calling. In addition, you serialize the tool list into a big # AVAILABLE TOOLS: user message that includes the full FilesystemTool::description() string (multi‑page documentation with all commands).

​

    That description is huge and becomes part of the prompt every time the ReAct step runs. On a small local model, the extra hundreds of tokens of tool prose can easily add tens or hundreds of milliseconds per call.

Final parsing is brittle, adding retries/extra calls risk.
The loop always tries to serde_json::from_str the final content into Value, falling back to Value::String if that fails.

    ​

        With FormatType::StructuredJson(Box::new(JsonStructure::new::<()>>)), the model may struggle to satisfy both JSON output and tool‑call protocol simultaneously; misalignment here can cause extra thinking tokens or tool calls before it stops.

None of these create an infinite loop in the trace you provided, but together they make each ReAct step heavier and more error‑prone than necessary.
Tool execution and locking

Your tool plumbing introduces a couple of potential bottlenecks when several tasks use tools concurrently.

    Global Mutex<ToolRegistry> held across async I/O.
    In execute_step_react, you do tool_registry.lock().await.execute(...).await.

​

    If ToolRegistry::execute awaits on the tool (which it will for FilesystemTool), the tokio::sync::Mutex guard is held for the entire duration of filesystem I/O. That serializes all tool calls across all agents that share the same registry.

FilesystemTool::call takes &mut self even though it is stateless.
The tool’s call(&mut self, parameters: Value) method forces ToolRegistry to store executors as mutable and, combined with the global mutex, prevents concurrent calls to the same tool.

​

    For a stateless tool that just delegates to tokio::fs, taking &self is enough and would allow per‑call concurrency once the registry is changed accordingly.

Single shared tool instance for all parallel coding tasks.
The workflow executes a wave of 5 coding tasks in parallel, but all of them share the same filesystem instance in the global registry.

    ​

        With the current design, a burst of list / write calls from multiple coding agents will be serialized through that one mutex, adding queueing delay even though the underlying filesystem and Tokio can handle them concurrently.

On the single example query this probably adds modest delays (filesystem ops are fast), but once you have heavier tools or more tasks, it can significantly stretch execution time.
Extra LLM / RAG work around the ReAct step

The orchestrator and RAG stack add additional LLM hops around your main coding agents, which show up clearly in the logs.

    A “moderate” query is decomposed via the planning agent, which itself uses a two‑step workflow: a ReAct Project Structure Analysis step with filesystem access, then a OneShot Task Decomposition Planning step.

​

    That means at least one ReAct step (1–N LLM calls) plus one extra LLM call for planning before any coding agent runs.

Each coding task then goes through RAG retrieval and an intent‑classification LLM call (phi3mini), followed by the actual generation step. The logs show repeated classify_intent_llm and “Querying intent classification model phi3mini” entries for each subtask.

​

    With 5 coding subtasks, that can easily become 5–10 extra LLM calls per query on top of the planning agent and RAG rewriter. On a local CPU‑bound model, that cost dominates any Rust‑side overhead.

ONNX Runtime for embeddings (all-MiniLM-L6-v2) is initialised via EmbeddingClient and SmartMultiSourceRag::new, which allocates tens of MB and takes about 2–3 seconds once.

    ​

        If the orchestrator is accidentally constructed per request instead of once at server startup, you would pay that 2–3 s penalty on every query.

Concrete fixes and diagnostics

Given your suspicion around ReAct/tool usage, these are the most impactful, targeted changes to try:

    Tighten the ReAct protocol.

        Push each assistant response into messages as ChatMessage::assistant so the model sees its own prior reasoning.

        Add an explicit “done” flag or status field in the structured output schema and break the loop when the model sets status = "done" instead of relying solely on “no tool calls”.

        Lower max_iterations for the planning ReAct step (e.g. 2–3) and any simple tools‑only analysis steps; your filesystem‑based project scan rarely needs 10 rounds.

    Reduce prompt bloat from tools.

        Keep .tools(tools_info) on the request, but remove or drastically shorten the human‑readable # AVAILABLE TOOLS message. At minimum, don’t inline the full FilesystemTool::description(); provide a one‑paragraph summary and let the formal tool schema do the rest.

    ​

Make tools concurrent and reduce contention.

    Change ToolExecutor::call(&mut self, ...) to &self for stateless tools like filesystem, and update ToolRegistry so it doesn’t hold a global Mutex across async I/O; use per‑tool Arc<dyn ToolExecutor + Send + Sync> and, if needed, internal Mutex only around truly mutable state.

    ​

Measure per‑iteration and per‑tool timings.

    Add logging inside the ReAct loop with iteration, number of tool_calls, and elapsed time per send_chat_messages plus per execute(...) call. That will tell you immediately if some prompts are hitting max_iter or if tools are backing up behind the global mutex.

    ​

Ensure orchestrator and RAG are singletons.

    Verify that Orchestrator::new is called once on server startup and reused, so the expensive ORT initialisation is not repeated per request.
