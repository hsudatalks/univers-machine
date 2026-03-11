mod daemon;
mod status;

use clap::{Parser, Subcommand};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use univers_daemon_core::agent::db::Db;
use univers_daemon_core::agent::event::HookEvent;

#[derive(Parser)]
#[command(
    name = "univers-container-daemon",
    about = "Container daemon: monitor Claude Code sessions, manage tmux services, install software"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the background HTTP daemon
    Daemon {
        /// Port to listen on
        #[arg(short, long, default_value = "3100")]
        port: u16,
    },
    /// Process a hook event from stdin (forwards to daemon, falls back to SQLite)
    Event,
    /// Show active sessions
    Status {
        /// Include ended sessions
        #[arg(long)]
        all: bool,
        /// Output as JSON
        #[arg(long)]
        json: bool,
        /// Watch mode (refresh every 2s)
        #[arg(short, long)]
        watch: bool,
    },
    /// Clean old ended sessions
    Clean {
        /// Hours threshold (default: 24)
        #[arg(long, default_value = "24")]
        hours: u32,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Daemon { port } => {
            tracing_subscriber::registry()
                .with(tracing_subscriber::fmt::layer())
                .with(tracing_subscriber::EnvFilter::new(
                    std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
                ))
                .init();

            if let Err(e) = daemon::run_daemon(port).await {
                eprintln!("Daemon error: {e}");
                std::process::exit(1);
            }
        }
        Commands::Event => {
            if let Err(e) = handle_event_cmd().await {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
        Commands::Status { all, json, watch } => {
            if watch {
                loop {
                    print!("\x1b[2J\x1b[H");
                    if let Err(e) = status::show_status(all, json).await {
                        eprintln!("Error: {e}");
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
            } else if let Err(e) = status::show_status(all, json).await {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
        Commands::Clean { hours } => {
            let result = tokio::task::spawn_blocking(move || {
                let db = Db::open().map_err(|e| anyhow::anyhow!("{e}"))?;
                let n = db.clean_old(hours).map_err(|e| anyhow::anyhow!("{e}"))?;
                Ok::<_, anyhow::Error>(n)
            })
            .await;
            match result {
                Ok(Ok(n)) => println!("Cleaned {n} ended session(s) older than {hours}h."),
                Ok(Err(e)) => {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
        }
    }
}

async fn handle_event_cmd() -> anyhow::Result<()> {
    let buf = tokio::task::spawn_blocking(|| {
        use std::io::Read;
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        Ok::<_, anyhow::Error>(buf)
    })
    .await??;

    let ev: HookEvent = serde_json::from_str(&buf)?;

    // Try to forward to daemon via HTTP
    let client = reqwest::Client::new();
    match client
        .post("http://127.0.0.1:3100/event")
        .json(&ev)
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => return Ok(()),
        _ => {}
    }

    // Fallback: write directly to SQLite
    let cwd = ev.cwd.as_deref().unwrap_or("unknown").to_owned();
    let event_name = ev.event_name().to_owned();
    let status_val = ev.status().to_owned();
    let tool_name = ev.tool_name.clone();
    let tool_input = ev.tool_input_summary();
    let session_id = ev.session_id.clone();

    tokio::task::spawn_blocking(move || {
        let db = Db::open().map_err(|e| anyhow::anyhow!("{e}"))?;
        db.upsert_session(
            &session_id,
            &cwd,
            &status_val,
            &event_name,
            tool_name.as_deref(),
        )
        .map_err(|e| anyhow::anyhow!("{e}"))?;
        db.insert_event(
            &session_id,
            &event_name,
            tool_name.as_deref(),
            tool_input.as_deref(),
        )
        .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok::<_, anyhow::Error>(())
    })
    .await??;

    Ok(())
}
