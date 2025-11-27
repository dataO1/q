🔍 Complete Trace Analysis: WebCrawlerRetriever Silent Failure
❌ Critical Finding: WebCrawlerRetriever Returns Immediately

The trace confirms WebCrawlerRetriever IS being called but returns in ~40-100 microseconds without performing any web search [attached:file:96].
Evidence from Trace

json
{
  "operationName": "retrieval_source",
  "priority": "3",
  "sourcetype": "unknown",  // ⚠️ Should be "web_crawler"
  "querycount": "9",
  "duration": 97,  // ⚠️ 97 microseconds!
  "thread.id": 3
}

This appears 4 times across different retrieval attempts [attached:file:96].
🎯 Root Cause Confirmed
Problem 1: Queries Are NOT CollectionTier::Online

From the logs [attached:file:95]:

text
"Detected web intent via heuristics for query: ..."

But then queries are being enhanced with tier tags:

    tier: Online - 5 queries

    tier: Workspace - 5 queries

    tier: Personal - 5 queries

Your WebCrawlerRetriever.retrieve() ONLY processes Online tier [attached:file:92]:

rust
for (tier, query) in queries {
    if tier != CollectionTier::Online {
        debug!("Skipping non-online tier: {:?}", tier);
        continue;  // ⚠️ EARLY RETURN
    }
    // ... web search code
}

However, looking at MultiSourceRetriever in retriever.rs [attached:file:94], I see the problem:

rust
let fetch_futures = queries.into_iter().map(|(tier, q_list)| {
    let tier = tier.clone();
    let q_list = q_list.clone();
    // ...
    let results = s_self
        .retrieve(
            q_list.into_iter()
                .map(|q| (tier.clone(), q))  // ⚠️ ALL queries get SAME tier!
                .collect(),
            // ...
        )
        .await?;
});

This means if the HashMap<CollectionTier, Vec<String>> has:

    Workspace => ["query1", "query2", "query3"]

    Personal => ["query4", "query5"]

    Online => ["query6", "query7"]

Then WebCrawlerRetriever receives:

    FIRST call: [(Workspace, "query1"), (Workspace, "query2"), (Workspace, "query3")] → All skipped!

    SECOND call: [(Personal, "query4"), (Personal, "query5")] → All skipped!

    THIRD call: [(Online, "query6"), (Online, "query7")] → These should work!

🐛 Why Are Online Queries Also Failing?

Even though the router detects "web intent" and generates Online tier queries, the trace shows NO SearXNG calls. This means one of:

    No queries reach the Online tier (unlikely given the log)

    self.searxng_client is None (health check failed silently)

    Empty query list after tier filtering
