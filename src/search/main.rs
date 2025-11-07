use anyhow::{Context, Result};
use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame, Terminal,
};
use semantic_search::Config;
use std::path::PathBuf;
use tokio::sync::mpsc;

#[derive(Parser, Debug)]
#[command(name = "q")]
#[command(about = "Semantic code search TUI")]
struct Args {
    /// Search query
    query: Option<String>,

    /// Config file path
    #[arg(short, long, default_value = "config.toml")]
    config: PathBuf,

    /// Number of results
    #[arg(short = 'n', long, default_value = "10")]
    limit: usize,
}

struct App {
    query: String,
    results: Vec<SearchResult>,
    status: AppStatus,
    selected: usize,
}

enum AppStatus {
    Input,
    Searching,
    Results,
    Error(String),
}

#[derive(Clone)]
struct SearchResult {
    path: String,
    score: f32,
    snippet: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let config = if args.config.exists() {
        Config::from_file(&args.config)?
    } else {
        Config::default_config()
    };

    if let Some(query) = args.query {
        // CLI mode: just print results
        let results = search(&query, &config, args.limit).await?;
        for (i, result) in results.iter().enumerate() {
            println!("{}. {} (score: {:.4})", i + 1, result.path, result.score);
            println!("   {}", result.snippet);
            println!();
        }
        return Ok(());
    }

    // TUI mode
    run_tui(config, args.limit).await
}

async fn run_tui(config: Config, limit: usize) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App {
        query: String::new(),
        results: Vec::new(),
        status: AppStatus::Input,
        selected: 0,
    };

    let (tx, mut rx) = mpsc::unbounded_channel();

    let result = loop {
        terminal.draw(|f| ui(f, &app))?;

        // Non-blocking event check
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break Ok(()),
                    KeyCode::Char(c) => {
                        if matches!(app.status, AppStatus::Input) {
                            app.query.push(c);
                        }
                    }
                    KeyCode::Backspace => {
                        if matches!(app.status, AppStatus::Input) {
                            app.query.pop();
                        }
                    }
                    KeyCode::Enter => {
                        if matches!(app.status, AppStatus::Input) && !app.query.is_empty() {
                            app.status = AppStatus::Searching;
                            let query = app.query.clone();
                            let config_clone = config.clone();
                            let tx_clone = tx.clone();

                            tokio::spawn(async move {
                                match search(&query, &config_clone, limit).await {
                                    Ok(results) => {
                                        let _ = tx_clone.send(SearchResponse::Results(results));
                                    }
                                    Err(e) => {
                                        let _ = tx_clone.send(SearchResponse::Error(e.to_string()));
                                    }
                                }
                            });
                        }
                    }
                    KeyCode::Down => {
                        if matches!(app.status, AppStatus::Results) && app.selected < app.results.len().saturating_sub(1) {
                            app.selected += 1;
                        }
                    }
                    KeyCode::Up => {
                        if matches!(app.status, AppStatus::Results) && app.selected > 0 {
                            app.selected -= 1;
                        }
                    }
                    _ => {}
                }
            }
        }

        // Check for search results
        if let Ok(response) = rx.try_recv() {
            match response {
                SearchResponse::Results(results) => {
                    app.results = results;
                    app.status = AppStatus::Results;
                    app.selected = 0;
                }
                SearchResponse::Error(e) => {
                    app.status = AppStatus::Error(e);
                }
            }
        }
    };

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

enum SearchResponse {
    Results(Vec<SearchResult>),
    Error(String),
}

fn ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(f.area());

    // Input box
    let input = Paragraph::new(app.query.as_str())
        .style(Style::default().fg(Color::Yellow))
        .block(Block::default().borders(Borders::ALL).title("Search Query"));
    f.render_widget(input, chunks[0]);

    // Results or status
    match &app.status {
        AppStatus::Input => {
            let help = Paragraph::new("Type your search query and press Enter")
                .style(Style::default().fg(Color::Gray))
                .block(Block::default().borders(Borders::ALL).title("Results"));
            f.render_widget(help, chunks[1]);
        }
        AppStatus::Searching => {
            let searching = Paragraph::new("🔍 Searching...")
                .style(Style::default().fg(Color::Cyan))
                .block(Block::default().borders(Borders::ALL).title("Results"));
            f.render_widget(searching, chunks[1]);
        }
        AppStatus::Results => {
            let items: Vec<ListItem> = app
                .results
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    let style = if i == app.selected {
                        Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };

                    let content = vec![
                        Line::from(vec![
                            Span::styled(format!("{} ", r.path), Style::default().fg(Color::Green)),
                            Span::styled(format!("(score: {:.4})", r.score), Style::default().fg(Color::Yellow)),
                        ]),
                        Line::from(Span::styled(format!("  {}", r.snippet), Style::default().fg(Color::Gray))),
                    ];
                    ListItem::new(content).style(style)
                })
                .collect();

            let results_list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title(format!("Results ({})", app.results.len())));
            f.render_widget(results_list, chunks[1]);
        }
        AppStatus::Error(e) => {
            let error = Paragraph::new(format!("Error: {}", e))
                .style(Style::default().fg(Color::Red))
                .block(Block::default().borders(Borders::ALL).title("Error"));
            f.render_widget(error, chunks[1]);
        }
    }

    // Status bar
    let status = Paragraph::new("q/Esc: Quit | ↑↓: Navigate | Enter: Search")
        .style(Style::default().fg(Color::Gray))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(status, chunks[2]);
}

async fn search(query: &str, config: &Config, limit: usize) -> Result<Vec<SearchResult>> {
    use qdrant_client::qdrant::{SearchPointsBuilder};
    use swiftide::integrations::ollama::Ollama;

    // Connect to Qdrant
    let qdrant = qdrant_client::Qdrant::from_url(&config.qdrant.url)
        .build()
        .context("Failed to connect to Qdrant")?;

    // Embed query using Ollama
    let ollama = Ollama::builder()
        .default_embed_model(&config.ollama.embedding_model)
        .build()?;

    // Get embedding for query
    let embedding = ollama.embed(query).await
        .context("Failed to embed query")?;

    // Search in Qdrant
    let search_result = qdrant
        .search_points(
            SearchPointsBuilder::new(&config.qdrant.collection_name, embedding, limit as u64)
                .with_payload(true)
        )
        .await
        .context("Failed to search")?;

    // Convert results
    let results = search_result
        .result
        .into_iter()
        .map(|point| {
            let payload = point.payload;
            let path = payload
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let snippet = payload
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .chars()
                .take(100)
                .collect::<String>();

            SearchResult {
                path,
                score: point.score,
                snippet,
            }
        })
        .collect();

    Ok(results)
}
