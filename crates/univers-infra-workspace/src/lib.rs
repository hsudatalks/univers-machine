use std::path::PathBuf;

pub fn container_tmux_server_name() -> String {
    configured_tmux_server("UNIVERS_CONTAINER_TMUX_SERVER", "container")
}

pub fn machine_tmux_server_name() -> String {
    configured_tmux_server("UNIVERS_MACHINE_TMUX_SERVER", "machine")
}

pub fn first_existing_directory(
    candidates: impl IntoIterator<Item = PathBuf>,
) -> Option<PathBuf> {
    candidates.into_iter().find(|path| path.is_dir())
}

pub fn machine_tmux_working_directory() -> PathBuf {
    let mut candidates = Vec::new();

    if let Ok(path) = std::env::var("UNIVERS_MACHINE_TMUX_WORKDIR") {
        candidates.push(PathBuf::from(path));
    }
    if let Ok(path) = std::env::current_dir() {
        candidates.push(path);
    }
    if let Some(path) = user_home_dir() {
        candidates.push(path.join("repos"));
        candidates.push(path);
    }
    candidates.push(PathBuf::from("/tmp"));

    first_existing_directory(candidates.into_iter()).unwrap_or_else(|| PathBuf::from("/tmp"))
}

pub fn container_tmux_working_directory() -> PathBuf {
    let mut candidates = Vec::new();

    if let Ok(path) = std::env::var("UNIVERS_CONTAINER_TMUX_WORKDIR") {
        candidates.push(PathBuf::from(path));
    }
    if let Ok(path) = std::env::current_dir() {
        candidates.push(path);
    }
    if let Some(path) = user_home_dir() {
        candidates.push(path);
    }
    candidates.push(PathBuf::from("/tmp"));

    first_existing_directory(candidates.into_iter()).unwrap_or_else(|| PathBuf::from("/tmp"))
}

pub fn discover_servers_config_path() -> Option<PathBuf> {
    let mut candidates = Vec::new();

    if let Ok(path) = std::env::var("UNIVERS_DAEMON_SERVERS_CONFIG") {
        candidates.push(PathBuf::from(path));
    }
    if let Ok(current_dir) = std::env::current_dir() {
        candidates.push(current_dir.join("config/servers.yaml"));
        candidates.push(current_dir.join("config/servers.yaml.example"));
    }
    if let Some(config_home) = user_config_home() {
        candidates.push(config_home.join("univers-machine/servers.yaml"));
        candidates.push(config_home.join("univers-machine/servers.yaml.example"));
    }

    candidates.into_iter().find(|path| path.is_file())
}

pub fn command_exists(name: &str) -> bool {
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
        .any(|path| path.join(name).is_file())
}

fn user_home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn user_config_home() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from) {
        return Some(path);
    }
    user_home_dir().map(|home| home.join(".config"))
}

fn configured_tmux_server(env_key: &str, fallback_suffix: &str) -> String {
    if let Ok(server) = std::env::var(env_key) {
        let trimmed = server.trim();
        if !trimmed.is_empty() {
            return sanitize_tmux_server_segment(trimmed);
        }
    }
    namespaced_tmux_server(fallback_suffix)
}

fn namespaced_tmux_server(suffix: &str) -> String {
    let prefix = std::env::var("UNIVERS_TMUX_SERVER_PREFIX")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| String::from("ark"));
    format!(
        "{}-{}",
        sanitize_tmux_server_segment(&prefix),
        sanitize_tmux_server_segment(suffix)
    )
}

fn sanitize_tmux_server_segment(value: &str) -> String {
    let mut sanitized = String::with_capacity(value.len());
    let mut last_was_dash = false;
    for ch in value.chars() {
        let normalized = if ch.is_ascii_alphanumeric() { ch } else { '-' };
        if normalized == '-' {
            if !last_was_dash {
                sanitized.push('-');
                last_was_dash = true;
            }
        } else {
            sanitized.push(normalized.to_ascii_lowercase());
            last_was_dash = false;
        }
    }

    let sanitized = sanitized.trim_matches('-');
    if sanitized.is_empty() {
        String::from("ark")
    } else {
        sanitized.to_string()
    }
}
