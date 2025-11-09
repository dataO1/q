use rustyline::Editor;
use ai_agent_common::*;

pub async fn run() -> anyhow::Result<()> {
    let mut rl = Editor::<()>::new()?;

    println!("AI Agent System - Interactive Mode");
    println!("Type your query or @<Tab> for context providers");

    loop {
        let readline = rl.readline("ai> ");
        match readline {
            Ok(line) => {
                if line.trim() == "exit" {
                    break;
                }

                // Process query
                process_query(&line).await?;
            }
            Err(_) => break,
        }
    }

    Ok(())
}

async fn process_query(query: &str) -> anyhow::Result<()> {
    todo!("Send query to orchestrator via API")
}
