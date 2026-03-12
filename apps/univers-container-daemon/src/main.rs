mod container;
mod daemon;
mod dashboard;
mod self_daemon;
mod status;

use clap::{Parser, Subcommand};
use self_daemon::{
    collect_daemon_info, collect_service_status, install_service, restart_service, start_service,
    stop_service, uninstall_service, DaemonServiceMutationResult, DaemonServiceStatus,
    InstallDaemonServiceRequest,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use univers_daemon_core::agent::db::Db;
use univers_daemon_core::agent::event::HookEvent;

const DEFAULT_DAEMON_PORT: u16 = 3100;

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
        #[arg(short, long, default_value_t = DEFAULT_DAEMON_PORT)]
        port: u16,
    },
    /// Process a hook event from stdin (forwards to daemon, falls back to SQLite)
    Event {
        /// Daemon port (for HTTP forward)
        #[arg(long, env = "UNIVERS_CONTAINER_DAEMON_PORT", default_value_t = DEFAULT_DAEMON_PORT)]
        port: u16,
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
        #[arg(long, env = "UNIVERS_CONTAINER_DAEMON_PORT", default_value_t = DEFAULT_DAEMON_PORT)]
        port: u16,
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
    /// Show daemon runtime and service metadata
    Info {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Manage the daemon user service
    Service {
        #[command(subcommand)]
        command: ServiceCommands,
    },
}

#[derive(Subcommand)]
enum ServiceCommands {
    /// Show current service status
    Status {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Install a user-level systemd service
    Install {
        /// Override the daemon binary path
        #[arg(long)]
        binary_path: Option<String>,
        /// Override the working directory
        #[arg(long)]
        working_directory: Option<String>,
        /// Daemon port for the installed service
        #[arg(long, default_value_t = DEFAULT_DAEMON_PORT)]
        port: u16,
        /// RUST_LOG value for the service
        #[arg(long)]
        log_level: Option<String>,
        /// Do not enable the service after installing it
        #[arg(long)]
        no_enable: bool,
        /// Do not start the service after installing it
        #[arg(long)]
        no_start: bool,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Start the installed service
    Start {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Stop the installed service
    Stop {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Restart the installed service
    Restart {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Remove the installed service
    Uninstall {
        /// Output as JSON
        #[arg(long)]
        json: bool,
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
        Commands::Event { port } => {
            if let Err(e) = handle_event_cmd(port).await {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
        Commands::Status {
            all,
            json,
            watch,
            port,
        } => {
            if watch {
                loop {
                    print!("\x1b[2J\x1b[H");
                    if let Err(e) = status::show_status(all, json, port).await {
                        eprintln!("Error: {e}");
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
            } else if let Err(e) = status::show_status(all, json, port).await {
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
        Commands::Install { name, run } => {
            if let Err(e) = handle_install_cmd(&name, run).await {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
        Commands::Info { json } => {
            if let Err(e) = handle_info_cmd(json) {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
        Commands::Service { command } => {
            if let Err(e) = handle_service_cmd(command).await {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
    }
}

async fn handle_event_cmd(port: u16) -> anyhow::Result<()> {
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
        .post(format!("http://127.0.0.1:{port}/api/agents/event"))
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

async fn handle_install_cmd(name: &str, run: bool) -> anyhow::Result<()> {
    let registry = univers_daemon_core::installer::InstallerRegistry::with_defaults();

    if run {
        println!("Installing {name}...");
        let result = registry.install(name).await?;
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
        let status = registry.check_status(name).await?;
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

fn handle_info_cmd(json: bool) -> anyhow::Result<()> {
    let info = collect_daemon_info();
    if json {
        println!("{}", serde_json::to_string_pretty(&info)?);
    } else {
        println!("name: {}", info.name);
        println!("version: {}", info.version);
        println!("pid: {}", info.pid);
        println!("executable: {}", info.executable_path);
        println!("started_at: {}", info.started_at);
        println!(
            "listen_port: {}",
            info.listen_port
                .map(|value| value.to_string())
                .unwrap_or_else(|| "unknown".into())
        );
        print_service_status(&info.service);
    }
    Ok(())
}

async fn handle_service_cmd(command: ServiceCommands) -> anyhow::Result<()> {
    match command {
        ServiceCommands::Status { json } => {
            let status = collect_service_status();
            if json {
                println!("{}", serde_json::to_string_pretty(&status)?);
            } else {
                print_service_status(&status);
            }
        }
        ServiceCommands::Install {
            binary_path,
            working_directory,
            port,
            log_level,
            no_enable,
            no_start,
            json,
        } => {
            let result = install_service(InstallDaemonServiceRequest {
                binary_path,
                working_directory,
                port: Some(port),
                log_level,
                enable: Some(!no_enable),
                start: Some(!no_start),
            })
            .await?;
            print_service_mutation_result(&result, json)?;
        }
        ServiceCommands::Start { json } => {
            let result = start_service().await?;
            print_service_mutation_result(&result, json)?;
        }
        ServiceCommands::Stop { json } => {
            let result = stop_service().await?;
            print_service_mutation_result(&result, json)?;
        }
        ServiceCommands::Restart { json } => {
            let result = restart_service().await?;
            print_service_mutation_result(&result, json)?;
        }
        ServiceCommands::Uninstall { json } => {
            let result = uninstall_service().await?;
            print_service_mutation_result(&result, json)?;
        }
    }

    Ok(())
}

fn print_service_mutation_result(
    result: &DaemonServiceMutationResult,
    json: bool,
) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(result)?);
    } else {
        println!("{}", result.message);
        print_service_status(&result.service);
    }
    Ok(())
}

fn print_service_status(status: &DaemonServiceStatus) {
    println!("service_manager: {}", status.manager);
    println!("manager_available: {}", status.manager_available);
    println!("user_session_available: {}", status.user_session_available);
    println!("unit_name: {}", status.unit_name);
    println!("unit_path: {}", status.unit_path);
    println!("installed: {}", status.installed);
    println!("enabled: {}", status.enabled);
    println!("active: {}", status.active);
    if let Some(error) = &status.last_error {
        println!("last_error: {error}");
    }
}
