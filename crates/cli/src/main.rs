use ai_agent_cli::{oneshot, interactive};  // Import from library
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "ai")]
#[command(about = "AI Agent CLI")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Execute a one-shot query
    Execute { query: String },
    /// Start interactive mode
    Interactive,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Execute { query }) => {
            oneshot::execute(&query).await?;
        }
        Some(Commands::Interactive) | None => {
            interactive::run().await?;
        }
    }

    Ok(())
}
