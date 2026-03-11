use chrono::{DateTime, Utc};
use comfy_table::{presets::UTF8_FULL_CONDENSED, Cell, Color, Table};
use univers_daemon_core::agent::event::SessionSnapshot;

const DAEMON_URL: &str = "http://127.0.0.1:3100";

pub async fn show_status(all: bool, json: bool) -> anyhow::Result<()> {
    let sessions = fetch_sessions(all).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&sessions)?);
        return Ok(());
    }

    if sessions.is_empty() {
        println!("No active Claude Code sessions.");
        return Ok(());
    }

    print_table(&sessions);
    Ok(())
}

async fn fetch_sessions(all: bool) -> anyhow::Result<Vec<SessionSnapshot>> {
    let url = if all {
        format!("{DAEMON_URL}/status/all")
    } else {
        format!("{DAEMON_URL}/status")
    };

    // Try daemon HTTP first (legacy endpoints for backward compat)
    match reqwest::get(&url).await {
        Ok(resp) if resp.status().is_success() => {
            let sessions: Vec<SessionSnapshot> = resp.json().await?;
            return Ok(sessions);
        }
        _ => {}
    }

    // Fallback to direct SQLite read
    let include_ended = all;
    let rows = tokio::task::spawn_blocking(move || {
        let db =
            univers_daemon_core::agent::db::Db::open().map_err(|e| anyhow::anyhow!("{e}"))?;
        db.list_sessions(include_ended)
            .map_err(|e| anyhow::anyhow!("{e}"))
    })
    .await??;

    Ok(rows.into_iter().map(SessionSnapshot::from).collect())
}

fn print_table(sessions: &[SessionSnapshot]) {
    let mut table = Table::new();
    table.load_preset(UTF8_FULL_CONDENSED);
    table.set_header(vec![
        "Session",
        "Directory",
        "Status",
        "Last Activity",
        "Tool",
    ]);

    for s in sessions {
        let short_id = if s.session_id.len() > 8 {
            &s.session_id[..8]
        } else {
            &s.session_id
        };
        let dir = shorten_path(&s.cwd);
        let ago = time_ago(&s.updated_at);
        let tool = format_tool(s.last_tool.as_deref());
        let status_cell = colored_status(&s.status);

        table.add_row(vec![
            Cell::new(short_id),
            Cell::new(dir),
            status_cell,
            Cell::new(ago),
            Cell::new(tool),
        ]);
    }

    println!("{table}");
}

fn colored_status(status: &str) -> Cell {
    let color = match status {
        "active" => Color::Green,
        "waiting_input" => Color::Yellow,
        "idle" => Color::DarkGrey,
        "ended" => Color::Red,
        _ => Color::White,
    };
    Cell::new(status).fg(color)
}

fn shorten_path(path: &str) -> String {
    if let Ok(home) = std::env::var("HOME") {
        if let Some(rest) = path.strip_prefix(&home) {
            return format!("~{rest}");
        }
    }
    path.to_string()
}

fn time_ago(rfc3339: &str) -> String {
    let Ok(dt) = rfc3339.parse::<DateTime<Utc>>() else {
        return rfc3339.to_string();
    };
    let diff = Utc::now() - dt;
    let secs = diff.num_seconds();
    if secs < 0 {
        return "just now".to_string();
    }
    if secs < 60 {
        return format!("{secs}s ago");
    }
    let mins = secs / 60;
    if mins < 60 {
        return format!("{mins}m ago");
    }
    let hours = mins / 60;
    if hours < 24 {
        return format!("{hours}h ago");
    }
    let days = hours / 24;
    format!("{days}d ago")
}

fn format_tool(tool: Option<&str>) -> String {
    tool.unwrap_or("--").to_string()
}
