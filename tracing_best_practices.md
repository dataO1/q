
Best Practice Rules

    Use info_span! (or #[instrument]) for:

        Any async operation that takes > 10ms (LLM calls, DB queries, Tool execution).

        Distinct logical steps (e.g., "iteration_1", "iteration_2").

        Why: This breaks your giant trace into small bars that upload to the server incrementally.

    Use info! / debug! for:

        High-cardinality details inside those spans (e.g., "user input was empty", "cache hit").

        Things that don't have a duration.

    Avoid debug! for Metrics:

        Don't log debug!("tokens used: 50").

        Record it on the span: Span::current().record("tokens", 50).

Using info_span! effectively forces a "flush" of information to the tracing collector every time a sub-task finishes. This solves the feeling of "nothing is happening" because you see the trace tree growing leaf-by-leaf.
