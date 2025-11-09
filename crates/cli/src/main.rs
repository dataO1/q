use clap::Parser;
use ai_agent_common::*;

mod interactive;
mod oneshot;
mod completions;
mod display;

#[derive(Parser)]
#[command(name = "ai")]
#[command(about = "AI Agent System CLI")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Query (one-shot mode)
    query: Option<String>,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Generate shell completions
    Completions {
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Completions { shell }) => {
            completions::generate(shell);
        }
        None => {
            if let Some(query) = cli.query {
                // One-shot mode
                oneshot::execute(&query).await?;
            } else {
                // Interactive mode
                interactive::run().await?;
            }
        }
    }

    Ok(())
}
