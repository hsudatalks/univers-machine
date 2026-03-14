mod application;
mod container;
mod daemon;
mod dashboard;
mod self_daemon;
mod status;

use application::daemon_service::DaemonServiceApplicationService;
use clap::{Parser, Subcommand};
use self_daemon::{
    DaemonServiceLogs, DaemonServiceMutationResult, DaemonServiceStatus, DaemonServiceUnitFile,
    InstallDaemonServiceRequest, UpdateDaemonServiceRequest,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use univers_daemon_shared::agent::event::HookEvent;
use univers_daemon_shared::agent::repository::SessionRepository;
use univers_daemon_shared::application::installer::InstallerApplicationService;
use univers_daemon_shared::installer::InstallerRegistry;
use univers_infra_sqlite::SqliteSessionRepository;

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
    /// Show recent service logs
    Logs {
        /// Number of log lines to fetch
        #[arg(long, default_value_t = 100)]
        lines: usize,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Show the installed unit file
    Cat {
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
    /// Update the installed service configuration
    Update {
        /// Override the daemon binary path
        #[arg(long)]
        binary_path: Option<String>,
        /// Override the working directory
        #[arg(long)]
        working_directory: Option<String>,
        /// Override the daemon port
        #[arg(long)]
        port: Option<u16>,
        /// Override RUST_LOG for the service
        #[arg(long)]
        log_level: Option<String>,
        /// Enable the service after updating
        #[arg(long, conflicts_with = "disable")]
        enable: bool,
        /// Disable the service after updating
        #[arg(long, conflicts_with = "enable")]
        disable: bool,
        /// Restart or start the service after updating
        #[arg(long, conflicts_with = "no_restart")]
        restart: bool,
        /// Update the unit file without restarting the service
        #[arg(long, conflicts_with = "restart")]
        no_restart: bool,
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

fn handle_info_cmd(json: bool) -> anyhow::Result<()> {
    let service = DaemonServiceApplicationService::new();
    let info = service.info();
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
    let service = DaemonServiceApplicationService::new();
    match command {
        ServiceCommands::Status { json } => {
            let status = service.service_status();
            if json {
                println!("{}", serde_json::to_string_pretty(&status)?);
            } else {
                print_service_status(&status);
            }
        }
        ServiceCommands::Logs { lines, json } => {
            let logs = service.service_logs(lines).await?;
            print_service_logs(&logs, json)?;
        }
        ServiceCommands::Cat { json } => {
            let unit = service.service_unit_file().await?;
            print_service_unit_file(&unit, json)?;
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
            let result = service
                .install_service(InstallDaemonServiceRequest {
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
        ServiceCommands::Update {
            binary_path,
            working_directory,
            port,
            log_level,
            enable,
            disable,
            restart,
            no_restart,
            json,
        } => {
            let result = service
                .update_service(UpdateDaemonServiceRequest {
                    binary_path,
                    working_directory,
                    port,
                    log_level,
                    enable: select_optional_bool(enable, disable),
                    restart: select_optional_bool(restart, no_restart),
                })
                .await?;
            print_service_mutation_result(&result, json)?;
        }
        ServiceCommands::Start { json } => {
            let result = service.start_service().await?;
            print_service_mutation_result(&result, json)?;
        }
        ServiceCommands::Stop { json } => {
            let result = service.stop_service().await?;
            print_service_mutation_result(&result, json)?;
        }
        ServiceCommands::Restart { json } => {
            let result = service.restart_service().await?;
            print_service_mutation_result(&result, json)?;
        }
        ServiceCommands::Uninstall { json } => {
            let result = service.uninstall_service().await?;
            print_service_mutation_result(&result, json)?;
        }
    }

    Ok(())
}

fn select_optional_bool(enable: bool, disable: bool) -> Option<bool> {
    if enable {
        Some(true)
    } else if disable {
        Some(false)
    } else {
        None
    }
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

fn print_service_logs(logs: &DaemonServiceLogs, json: bool) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(logs)?);
        return Ok(());
    }

    println!("unit_name: {}", logs.unit_name);
    println!("logs_available: {}", logs.logs_available);
    println!("lines: {}", logs.lines);

    if logs.entries.is_empty() {
        println!("entries: <empty>");
        return Ok(());
    }

    println!("entries:");
    for entry in &logs.entries {
        println!("{entry}");
    }

    Ok(())
}

fn print_service_unit_file(unit: &DaemonServiceUnitFile, json: bool) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(unit)?);
        return Ok(());
    }

    println!("unit_name: {}", unit.unit_name);
    println!("unit_path: {}", unit.unit_path);
    println!("installed: {}", unit.installed);
    match &unit.content {
        Some(content) => {
            println!("content:");
            print!("{content}");
            if !content.ends_with('\n') {
                println!();
            }
        }
        None => println!("content: <missing>"),
    }

    Ok(())
}
