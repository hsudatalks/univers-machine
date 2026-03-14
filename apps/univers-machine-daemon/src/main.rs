mod application;
mod daemon;
mod machine;
mod status;

use clap::{Parser, Subcommand};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use univers_daemon_shared::agent::event::HookEvent;
use univers_daemon_shared::agent::repository::SessionRepository;
use univers_daemon_shared::application::installer::InstallerApplicationService;
use univers_daemon_shared::installer::InstallerRegistry;
use univers_infra_sqlite::SqliteSessionRepository;

const DEFAULT_DAEMON_PORT: u16 = 3200;

#[derive(Parser)]
#[command(
    name = "univers-machine-daemon",
    about = "Machine daemon: system management, agent monitoring, tmux services, software installation"
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
        #[arg(short, long, default_value_t = DEFAULT_DAEMON_PORT)]
        port: u16,
        /// Bearer token for API authentication (optional, recommended)
        #[arg(long, env = "DAEMON_AUTH_TOKEN")]
        auth_token: Option<String>,
    },
    /// Process a hook event from stdin (forwards to daemon, falls back to SQLite)
    Event {
        /// Daemon port (for HTTP forward)
        #[arg(long, env = "UNIVERS_MACHINE_DAEMON_PORT", default_value_t = DEFAULT_DAEMON_PORT)]
        port: u16,
        /// Bearer token for API authentication
        #[arg(long, env = "DAEMON_AUTH_TOKEN")]
        auth_token: Option<String>,
    },
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
        /// Daemon port (for HTTP fetch)
        #[arg(long, env = "UNIVERS_MACHINE_DAEMON_PORT", default_value_t = DEFAULT_DAEMON_PORT)]
        port: u16,
        /// Bearer token for API authentication
        #[arg(long, env = "DAEMON_AUTH_TOKEN")]
        auth_token: Option<String>,
    },
    /// Clean old ended sessions
    Clean {
        /// Hours threshold (default: 24)
        #[arg(long, default_value = "24")]
        hours: u32,
    },
    /// Check installer status
    Install {
        /// Software name to check/install
        name: String,
        /// Actually perform install (default: check only)
        #[arg(long)]
        run: bool,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Daemon { port, auth_token } => {
            tracing_subscriber::registry()
                .with(tracing_subscriber::fmt::layer())
                .with(tracing_subscriber::EnvFilter::new(
                    std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
                ))
                .init();

            if let Err(e) = daemon::run_daemon(port, auth_token).await {
                eprintln!("Daemon error: {e}");
                std::process::exit(1);
            }
        }
        Commands::Event { port, auth_token } => {
            if let Err(e) = handle_event_cmd(port, auth_token.as_deref()).await {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
        Commands::Status {
            all,
            json,
            watch,
            port,
            auth_token,
        } => {
            if watch {
                loop {
                    print!("\x1b[2J\x1b[H");
                    if let Err(e) =
                        status::show_status(all, json, port, auth_token.as_deref()).await
                    {
                        eprintln!("Error: {e}");
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
            } else if let Err(e) = status::show_status(all, json, port, auth_token.as_deref()).await
            {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
        Commands::Clean { hours } => {
            let repository = SqliteSessionRepository::new();
            match tokio::task::spawn_blocking(move || repository.clean_old(hours)).await {
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
        Commands::Install { name, run } => {
            if let Err(e) = handle_install_cmd(&name, run).await {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
    }
}

async fn handle_event_cmd(port: u16, auth_token: Option<&str>) -> anyhow::Result<()> {
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
    let mut request = client
        .post(format!("http://127.0.0.1:{port}/api/agents/event"))
        .json(&ev);
    if let Some(token) = auth_token {
        request = request.bearer_auth(token);
    }
    match request.send().await {
        Ok(resp) if resp.status().is_success() => return Ok(()),
        _ => {}
    }

    // Fallback: write directly to SQLite
    let repository = SqliteSessionRepository::new();
    tokio::task::spawn_blocking(move || repository.persist_event(&ev)).await??;

    Ok(())
}

async fn handle_install_cmd(name: &str, run: bool) -> anyhow::Result<()> {
    let registry = std::sync::Arc::new(InstallerRegistry::with_defaults());
    let service = InstallerApplicationService::new(registry);

    if run {
        println!("Installing {name}...");
        let result = service.install(name).await?;
        if result.success {
            println!(
                "Success: {}{}",
                result.message,
                result
                    .version
                    .map(|v| format!(" (version: {v})"))
                    .unwrap_or_default()
            );
        } else {
            eprintln!("Failed: {}", result.message);
            std::process::exit(1);
        }
    } else {
        let status = service.installer_status(name).await?;
        if status.installed {
            println!(
                "{name}: installed{}",
                status
                    .version
                    .map(|v| format!(" (version: {v})"))
                    .unwrap_or_default()
            );
        } else {
            println!("{name}: not installed");
        }
    }

    Ok(())
}
