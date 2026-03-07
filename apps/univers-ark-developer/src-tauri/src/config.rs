use crate::models::{
    BrowserSurface, DeveloperTarget, ManagedContainer, ManagedServer, TargetsFile,
};
use crate::shell;
use csv::ReaderBuilder;
use serde::Deserialize;
use std::{
    fs,
    path::PathBuf,
    process::Output,
    sync::{Mutex, OnceLock},
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawTargetsFile {
    selected_target_id: Option<String>,
    #[serde(default)]
    targets: Vec<DeveloperTarget>,
    #[serde(default)]
    remote_servers: Vec<RemoteContainerServer>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteContainerServer {
    id: String,
    label: String,
    host: String,
    description: String,
    #[serde(default)]
    discovery_command: String,
    ssh_user: String,
    #[serde(default = "default_remote_server_ssh_options")]
    ssh_options: String,
    #[serde(default = "default_container_name_suffix")]
    container_name_suffix: String,
    #[serde(default)]
    include_stopped: bool,
    #[serde(default)]
    target_label_template: String,
    #[serde(default)]
    target_host_template: String,
    #[serde(default)]
    target_description_template: String,
    #[serde(default)]
    terminal_command_template: String,
    #[serde(default)]
    notes: Vec<String>,
    surfaces: Vec<BrowserSurface>,
}

#[derive(Debug, Clone)]
struct DiscoveredContainer {
    name: String,
    status: String,
    ipv4: String,
}

struct RemoteContainerContext<'a> {
    container_ip: &'a str,
    container_label: &'a str,
    container_name: &'a str,
    server: &'a RemoteContainerServer,
}

#[derive(Clone)]
struct ResolvedInventory {
    targets_file: TargetsFile,
    servers: Vec<ManagedServer>,
}

#[derive(Clone)]
struct CachedResolvedInventory {
    inventory: ResolvedInventory,
}

struct DiscoveredServerInventory {
    server: ManagedServer,
    available_targets: Vec<DeveloperTarget>,
}

fn default_remote_server_ssh_options() -> String {
    String::from("-o StrictHostKeyChecking=accept-new")
}

fn default_container_name_suffix() -> String {
    String::from("-dev")
}

pub(crate) fn app_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

pub(crate) fn targets_file_path() -> PathBuf {
    app_root().join("developer-targets.json")
}

fn targets_cache() -> &'static Mutex<Option<CachedResolvedInventory>> {
    static TARGETS_CACHE: OnceLock<Mutex<Option<CachedResolvedInventory>>> = OnceLock::new();

    TARGETS_CACHE.get_or_init(|| Mutex::new(None))
}

fn read_raw_targets_file() -> Result<RawTargetsFile, String> {
    let config_path = targets_file_path();
    let content = fs::read_to_string(&config_path)
        .map_err(|error| format!("Failed to read {}: {}", config_path.display(), error))?;

    serde_json::from_str::<RawTargetsFile>(&content)
        .map_err(|error| format!("Failed to parse {}: {}", config_path.display(), error))
}

fn default_discovery_command(server: &RemoteContainerServer) -> String {
    format!("ssh {} 'lxc list --format csv -c ns4'", server.host)
}

fn trim_quotes(value: &str) -> &str {
    value.trim().trim_matches('"').trim_matches('\'')
}

fn managed_known_hosts_file() -> String {
    let home = if cfg!(windows) {
        std::env::var("USERPROFILE").ok()
    } else {
        std::env::var("HOME").ok()
    };

    match home {
        Some(home) if !home.trim().is_empty() => {
            let normalized = if cfg!(windows) {
                home.replace('\\', "/")
            } else {
                home
            };
            format!("{}/.ssh/univers-ark-developer-known_hosts", normalized)
        }
        _ => String::from("~/.ssh/univers-ark-developer-known_hosts"),
    }
}

fn extract_ipv4(raw_ipv4: &str) -> Option<String> {
    let mut interface_matches = Vec::new();

    for raw_entry in raw_ipv4.split(['\n', ';']) {
        let entry = trim_quotes(raw_entry);
        if entry.is_empty() {
            continue;
        }

        let Some(ipv4) = entry
            .split(|character: char| {
                character.is_whitespace() || matches!(character, ',' | ';' | '(' | ')')
            })
            .map(str::trim)
            .find(|token| !token.is_empty() && token.parse::<std::net::Ipv4Addr>().is_ok())
        else {
            continue;
        };

        let interface = entry
            .split_once('(')
            .and_then(|(_, tail)| tail.split_once(')'))
            .map(|(name, _)| name.trim())
            .unwrap_or_default()
            .to_string();

        interface_matches.push((ipv4.to_string(), interface));
    }

    interface_matches
        .iter()
        .find(|(_, interface)| interface.eq_ignore_ascii_case("eth0"))
        .map(|(ipv4, _)| ipv4.clone())
        .or_else(|| {
            interface_matches
                .iter()
                .find(|(_, interface)| {
                    !interface.is_empty()
                        && !interface.eq_ignore_ascii_case("docker0")
                        && !interface.starts_with("br-")
                        && !interface.eq_ignore_ascii_case("lxdbr0")
                        && !interface.eq_ignore_ascii_case("lo")
                })
                .map(|(ipv4, _)| ipv4.clone())
        })
        .or_else(|| interface_matches.first().map(|(ipv4, _)| ipv4.clone()))
}

fn title_case_word(word: &str) -> String {
    let mut characters = word.chars();
    let Some(first) = characters.next() else {
        return String::new();
    };

    let mut title_cased = String::new();
    title_cased.extend(first.to_uppercase());
    title_cased.push_str(characters.as_str());
    title_cased
}

fn default_container_label(name: &str, suffix: &str) -> String {
    let trimmed = if !suffix.is_empty() && name.ends_with(suffix) {
        &name[..name.len() - suffix.len()]
    } else {
        name
    };

    trimmed
        .split(['-', '_', ' '])
        .filter(|part| !part.is_empty())
        .map(title_case_word)
        .collect::<Vec<_>>()
        .join(" ")
}

fn container_host_key_alias(server: &RemoteContainerServer, container_name: &str) -> String {
    format!("univers-ark-developer--{}--{}", server.id, container_name)
}

fn ssh_options_for_context(server: &RemoteContainerServer, container_name: &str) -> String {
    let base_options = server.ssh_options.trim();
    let host_key_alias = container_host_key_alias(server, container_name);
    let known_hosts_file = managed_known_hosts_file();
    let managed_known_hosts_option = format!("-o UserKnownHostsFile={}", known_hosts_file);

    if base_options.is_empty() {
        format!(
            "{} -o HostKeyAlias={}",
            managed_known_hosts_option, host_key_alias
        )
    } else {
        format!(
            "{} {} -o HostKeyAlias={}",
            base_options, managed_known_hosts_option, host_key_alias
        )
    }
}

fn replace_remote_placeholders(template: &str, context: &RemoteContainerContext<'_>) -> String {
    template
        .replace("{serverId}", &context.server.id)
        .replace("{serverLabel}", &context.server.label)
        .replace("{serverHost}", &context.server.host)
        .replace("{serverDescription}", &context.server.description)
        .replace("{containerIp}", context.container_ip)
        .replace("{containerLabel}", context.container_label)
        .replace("{containerName}", context.container_name)
        .replace(
            "{containerHostKeyAlias}",
            &container_host_key_alias(context.server, context.container_name),
        )
        .replace(
            "{sshOptions}",
            &ssh_options_for_context(context.server, context.container_name),
        )
        .replace("{sshUser}", &context.server.ssh_user)
}

fn render_template(
    template: &str,
    context: &RemoteContainerContext<'_>,
    fallback: impl FnOnce() -> String,
) -> String {
    if template.trim().is_empty() {
        return fallback();
    }

    replace_remote_placeholders(template, context)
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn render_surface(
    surface: &BrowserSurface,
    context: &RemoteContainerContext<'_>,
) -> BrowserSurface {
    BrowserSurface {
        id: surface.id.clone(),
        label: replace_remote_placeholders(&surface.label, context),
        tunnel_command: replace_remote_placeholders(&surface.tunnel_command, context),
        local_url: replace_remote_placeholders(&surface.local_url, context),
        remote_url: replace_remote_placeholders(&surface.remote_url, context),
        vite_hmr_tunnel_command: replace_remote_placeholders(
            &surface.vite_hmr_tunnel_command,
            context,
        ),
    }
}

fn ssh_destination(server: &RemoteContainerServer, container_ip: &str) -> String {
    format!("{}@{}", server.ssh_user, container_ip)
}

fn default_container_terminal_remote_command() -> String {
    String::from(
        "tmux-mobile-view attach || exec /bin/zsh -l || exec /bin/bash -l || exec /bin/sh -l",
    )
}

fn build_ssh_command(
    server: &RemoteContainerServer,
    container_ip: &str,
    container_name: &str,
    extra_options: &[&str],
    remote_command: Option<&str>,
) -> String {
    let ssh_options = ssh_options_for_context(server, container_name);
    let mut command = String::from("ssh");

    if !ssh_options.is_empty() {
        command.push(' ');
        command.push_str(&ssh_options);
    }

    for extra_option in extra_options {
        command.push(' ');
        command.push_str(extra_option);
    }

    command.push_str(" -J ");
    command.push_str(&server.host);
    command.push(' ');
    command.push_str(&ssh_destination(server, container_ip));

    if let Some(remote_command) = remote_command {
        command.push(' ');
        command.push_str(remote_command);
    }

    command
}

fn terminal_command_for_server(
    server: &RemoteContainerServer,
    context: &RemoteContainerContext<'_>,
) -> String {
    render_template(&server.terminal_command_template, context, || {
        let remote_command = default_container_terminal_remote_command();

        build_ssh_command(
            server,
            context.container_ip,
            context.container_name,
            &["-tt"],
            Some(&shell_single_quote(&remote_command)),
        )
    })
}

fn ssh_probe_command_for_server(
    server: &RemoteContainerServer,
    container_ip: &str,
    container_name: &str,
) -> String {
    build_ssh_command(
        server,
        container_ip,
        container_name,
        &[
            "-o BatchMode=yes",
            "-o ConnectTimeout=4",
            "-o ConnectionAttempts=1",
        ],
        Some("true"),
    )
}

fn probe_container_ssh(
    server: &RemoteContainerServer,
    container_ip: &str,
    container_name: &str,
) -> (bool, String, String) {
    let command = ssh_probe_command_for_server(server, container_ip, container_name);
    let output = shell::shell_command(&command).output();

    match output {
        Ok(output) if output.status.success() => (
            true,
            String::from("ready"),
            format!("SSH ready via {}.", server.host),
        ),
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let detail = if !stderr.is_empty() {
                stderr
            } else if !stdout.is_empty() {
                stdout
            } else {
                format!("Probe command exited with {}", output.status)
            };

            (false, String::from("error"), detail)
        }
        Err(error) => (
            false,
            String::from("error"),
            format!("Failed to run SSH probe: {}", error),
        ),
    }
}

fn local_public_key() -> Option<String> {
    let home = if cfg!(windows) {
        std::env::var("USERPROFILE").ok()
    } else {
        std::env::var("HOME").ok()
    };

    let home = home?;
    let ssh_dir = PathBuf::from(&home).join(".ssh");

    for key_name in &["id_ed25519.pub", "id_rsa.pub"] {
        let path = ssh_dir.join(key_name);
        if let Ok(content) = fs::read_to_string(&path) {
            let trimmed = content.trim().to_string();
            if !trimmed.is_empty() {
                return Some(trimmed);
            }
        }
    }

    None
}

fn deploy_key_command(
    server: &RemoteContainerServer,
    container_name: &str,
    public_key: &str,
) -> String {
    let user_home = format!("/home/{}", server.ssh_user);
    let inject_script = format!(
        "mkdir -p {home}/.ssh && chmod 700 {home}/.ssh && echo {key} >> {home}/.ssh/authorized_keys && chmod 600 {home}/.ssh/authorized_keys",
        home = user_home,
        key = shell_single_quote(public_key),
    );

    let is_orbstack = server
        .discovery_command
        .to_ascii_lowercase()
        .contains("orb");

    if is_orbstack {
        format!(
            "ssh {} \"orb run -m {} -u {} bash -c {}\"",
            server.host,
            container_name,
            server.ssh_user,
            shell_single_quote(&inject_script),
        )
    } else {
        format!(
            "ssh {} \"lxc exec {} -- bash -c {}\"",
            server.host,
            container_name,
            shell_single_quote(&inject_script),
        )
    }
}

fn auto_deploy_public_key(
    server: &RemoteContainerServer,
    container_name: &str,
) -> Option<String> {
    let public_key = local_public_key()?;
    let command = deploy_key_command(server, container_name, &public_key);
    let output = shell::shell_command(&command).output().ok()?;

    if output.status.success() {
        Some(format!(
            "Automatically deployed local SSH public key to {} via {}.",
            container_name, server.host
        ))
    } else {
        None
    }
}

fn discover_server_containers_output(server: &RemoteContainerServer) -> Result<String, String> {
    let command = if server.discovery_command.trim().is_empty() {
        default_discovery_command(server)
    } else {
        server.discovery_command.clone()
    };

    let output = shell::shell_command(&command)
        .output()
        .map_err(|error| {
            format!(
                "Failed to discover containers on {} with `{}`: {}",
                server.host, command, error
            )
        })?;

    if !output.status.success() {
        return Err(format!(
            "Failed to discover containers on {} with `{}`: {}",
            server.host,
            command,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn parse_discovered_containers(
    server: &RemoteContainerServer,
    discovery_output: &str,
) -> Result<Vec<DiscoveredContainer>, String> {
    let mut containers = Vec::new();
    let mut reader = ReaderBuilder::new()
        .has_headers(false)
        .from_reader(discovery_output.as_bytes());

    for row in reader.records() {
        let row = row.map_err(|error| {
            format!(
                "Failed to parse discovery output for {} as CSV: {}",
                server.host, error
            )
        })?;

        let name = trim_quotes(row.get(0).unwrap_or_default());
        let status = trim_quotes(row.get(1).unwrap_or_default());
        let raw_ipv4 = trim_quotes(row.get(2).unwrap_or_default());

        if name.is_empty() || status.is_empty() {
            continue;
        }

        if !server.include_stopped && !status.eq_ignore_ascii_case("running") {
            continue;
        }

        if !server.container_name_suffix.is_empty()
            && !name.ends_with(&server.container_name_suffix)
        {
            continue;
        }

        let Some(ipv4) = extract_ipv4(raw_ipv4) else {
            continue;
        };

        containers.push(DiscoveredContainer {
            name: name.to_string(),
            status: status.to_string(),
            ipv4,
        });
    }

    Ok(containers)
}

fn discover_server_containers(
    server: &RemoteContainerServer,
) -> Result<Vec<DiscoveredContainer>, String> {
    let output = discover_server_containers_output(server)?;
    parse_discovered_containers(server, &output)
}

fn build_target_from_container(
    server: &RemoteContainerServer,
    container: &DiscoveredContainer,
) -> DeveloperTarget {
    let container_label = default_container_label(&container.name, &server.container_name_suffix);
    let context = RemoteContainerContext {
        container_ip: &container.ipv4,
        container_label: &container_label,
        container_name: &container.name,
        server,
    };

    let label = render_template(&server.target_label_template, &context, || {
        container_label.clone()
    });
    let host = render_template(&server.target_host_template, &context, || {
        server.host.clone()
    });
    let description = render_template(&server.target_description_template, &context, || {
        format!(
            "{} development container on {} ({})",
            container_label, server.label, container.status
        )
    });
    let terminal_command = terminal_command_for_server(server, &context);
    let notes = server
        .notes
        .iter()
        .map(|note| replace_remote_placeholders(note, &context))
        .collect::<Vec<_>>();
    let surfaces = server
        .surfaces
        .iter()
        .map(|surface| render_surface(surface, &context))
        .collect::<Vec<_>>();

    DeveloperTarget {
        id: format!("{}::{}", server.id, container.name),
        label,
        host,
        description,
        terminal_command,
        notes,
        surfaces,
    }
}

fn build_managed_container(
    server: &RemoteContainerServer,
    container: &DiscoveredContainer,
) -> (ManagedContainer, Option<DeveloperTarget>) {
    let target = build_target_from_container(server, container);
    let ssh_command = target.terminal_command.clone();
    let ssh_dest = ssh_destination(server, &container.ipv4);
    let (mut ssh_reachable, mut ssh_state, mut ssh_message) =
        probe_container_ssh(server, &container.ipv4, &container.name);

    if !ssh_reachable && ssh_message.contains("Permission denied") {
        if let Some(deploy_message) =
            auto_deploy_public_key(server, &container.name)
        {
            let (retry_reachable, retry_state, retry_message) =
                probe_container_ssh(server, &container.ipv4, &container.name);

            if retry_reachable {
                ssh_reachable = true;
                ssh_state = retry_state;
                ssh_message = format!(
                    "{} {}",
                    retry_message, deploy_message
                );
            } else {
                ssh_message = format!(
                    "Key deployed but SSH still failed: {}",
                    retry_message
                );
            }
        }
    }

    (
        ManagedContainer {
            server_id: server.id.clone(),
            server_label: server.label.clone(),
            target_id: target.id.clone(),
            name: container.name.clone(),
            label: target.label.clone(),
            status: container.status.clone(),
            ipv4: container.ipv4.clone(),
            ssh_user: server.ssh_user.clone(),
            ssh_destination: ssh_dest,
            ssh_command,
            ssh_state,
            ssh_message,
            ssh_reachable,
        },
        ssh_reachable.then_some(target),
    )
}

fn server_state_for_containers(containers: &[ManagedContainer]) -> (String, String) {
    if containers.is_empty() {
        return (
            String::from("empty"),
            String::from("No matching development containers were detected."),
        );
    }

    let reachable = containers
        .iter()
        .filter(|container| container.ssh_reachable)
        .count();

    if reachable == containers.len() {
        return (
            String::from("ready"),
            format!("{} development container(s) are SSH reachable.", reachable),
        );
    }

    if reachable > 0 {
        return (
            String::from("degraded"),
            format!(
                "{} of {} development container(s) are SSH reachable.",
                reachable,
                containers.len()
            ),
        );
    }

    (
        String::from("error"),
        format!(
            "Detected {} development container(s), but none are SSH reachable.",
            containers.len()
        ),
    )
}

fn discover_remote_server_inventory(server: &RemoteContainerServer) -> DiscoveredServerInventory {
    match discover_server_containers(server) {
        Ok(containers) => {
            let mut managed_containers = Vec::new();
            let mut available_targets = Vec::new();

            for container in containers {
                let (managed_container, available_target) =
                    build_managed_container(server, &container);
                managed_containers.push(managed_container);

                if let Some(available_target) = available_target {
                    available_targets.push(available_target);
                }
            }

            let (state, message) = server_state_for_containers(&managed_containers);

            DiscoveredServerInventory {
                server: ManagedServer {
                    id: server.id.clone(),
                    label: server.label.clone(),
                    host: server.host.clone(),
                    description: server.description.clone(),
                    state,
                    message,
                    containers: managed_containers,
                },
                available_targets,
            }
        }
        Err(error) => DiscoveredServerInventory {
            server: ManagedServer {
                id: server.id.clone(),
                label: server.label.clone(),
                host: server.host.clone(),
                description: server.description.clone(),
                state: String::from("error"),
                message: error,
                containers: Vec::new(),
            },
            available_targets: Vec::new(),
        },
    }
}

fn load_inventory(force_refresh: bool) -> Result<ResolvedInventory, String> {
    if !force_refresh {
        if let Ok(cache) = targets_cache().lock() {
            if let Some(cached) = cache.as_ref() {
                return Ok(cached.inventory.clone());
            }
        }
    }

    let raw_targets_file = read_raw_targets_file()?;
    let mut targets = raw_targets_file.targets;
    let mut servers = Vec::new();

    for server in &raw_targets_file.remote_servers {
        let discovered_inventory = discover_remote_server_inventory(server);
        targets.extend(discovered_inventory.available_targets);
        servers.push(discovered_inventory.server);
    }

    let inventory = ResolvedInventory {
        targets_file: TargetsFile {
            selected_target_id: raw_targets_file.selected_target_id,
            targets,
        },
        servers,
    };

    if let Ok(mut cache) = targets_cache().lock() {
        *cache = Some(CachedResolvedInventory {
            inventory: inventory.clone(),
        });
    }

    Ok(inventory)
}

pub(crate) fn read_server_inventory(force_refresh: bool) -> Result<Vec<ManagedServer>, String> {
    load_inventory(force_refresh).map(|inventory| inventory.servers)
}

pub(crate) fn read_targets_file() -> Result<TargetsFile, String> {
    load_inventory(false).map(|inventory| inventory.targets_file)
}

pub(crate) fn resolve_raw_target(target_id: &str) -> Result<DeveloperTarget, String> {
    let targets_file = read_targets_file()?;

    targets_file
        .targets
        .into_iter()
        .find(|target| target.id == target_id)
        .ok_or_else(|| format!("Unknown target: {}", target_id))
}

pub(crate) fn run_target_shell_command(
    target_id: &str,
    remote_command: &str,
) -> Result<Output, String> {
    let inventory = load_inventory(false)?;

    if let Some(container) = inventory
        .servers
        .iter()
        .flat_map(|server| server.containers.iter())
        .find(|container| container.target_id == target_id)
    {
        let raw_targets_file = read_raw_targets_file()?;
        let server = raw_targets_file
            .remote_servers
            .iter()
            .find(|server| server.id == container.server_id)
            .ok_or_else(|| format!("Unknown remote server for {}", target_id))?;
        let quoted_remote_command = shell_single_quote(remote_command);
        let ssh_command = build_ssh_command(
            server,
            &container.ipv4,
            &container.name,
            &[],
            Some(&quoted_remote_command),
        );

        return shell::shell_command(&ssh_command)
            .output()
            .map_err(|error| {
                format!(
                    "Failed to execute remote shell command for {}: {}",
                    target_id, error
                )
            });
    }

    shell::shell_command(remote_command)
        .output()
        .map_err(|error| {
            format!(
                "Failed to execute local shell command for {}: {}",
                target_id, error
            )
        })
}

pub(crate) fn read_bootstrap_data(
    force_refresh: bool,
) -> Result<(TargetsFile, Vec<ManagedServer>), String> {
    let inventory = load_inventory(force_refresh)?;
    Ok((inventory.targets_file, inventory.servers))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_server() -> RemoteContainerServer {
        RemoteContainerServer {
            id: String::from("mechanism-dev"),
            label: String::from("Mechanism"),
            host: String::from("mechanism-dev"),
            description: String::from("Mechanism development server."),
            discovery_command: String::new(),
            ssh_user: String::from("ubuntu"),
            ssh_options: String::from("-o StrictHostKeyChecking=accept-new"),
            container_name_suffix: String::from("-dev"),
            include_stopped: false,
            target_label_template: String::new(),
            target_host_template: String::from("{serverHost}"),
            target_description_template: String::new(),
            terminal_command_template: String::new(),
            notes: vec![String::from(
                "SSH target: {sshUser}@{containerIp} via {serverHost}.",
            )],
            surfaces: vec![BrowserSurface {
                id: String::from("development"),
                label: String::from("Development"),
                tunnel_command: String::from(
                    "ssh {sshOptions} -NT -L {localPort}:127.0.0.1:3432 -J {serverHost} {sshUser}@{containerIp}",
                ),
                local_url: String::from("http://127.0.0.1:{localPort}/"),
                remote_url: String::from("http://127.0.0.1:3432/"),
                vite_hmr_tunnel_command: String::from(
                    "ssh {sshOptions} -NT -L {localPort}:127.0.0.1:3433 -J {serverHost} {sshUser}@{containerIp}",
                ),
            }],
        }
    }

    #[test]
    fn parses_running_dev_containers_from_lxd_csv() {
        let server = fixture_server();
        let discovery_output = "\
automation-dev,RUNNING,10.211.82.78 (eth0)\n\
runtime-dev,RUNNING,10.211.82.38 (eth0)\n\
tooling,STOPPED,\n\
workflow-dev,RUNNING,10.211.82.202 (eth0)\n";

        let containers = parse_discovered_containers(&server, discovery_output).unwrap();

        assert_eq!(containers.len(), 3);
        assert_eq!(containers[0].name, "automation-dev");
        assert_eq!(containers[0].ipv4, "10.211.82.78");
        assert_eq!(containers[1].name, "runtime-dev");
        assert_eq!(containers[2].name, "workflow-dev");
    }

    #[test]
    fn prefers_eth0_address_from_multiline_csv_field() {
        let server = fixture_server();
        let discovery_output = "\
env-dev,RUNNING,\"172.17.0.1 (docker0)\n\
10.197.97.142 (eth0)\"\n";

        let containers = parse_discovered_containers(&server, discovery_output).unwrap();

        assert_eq!(containers.len(), 1);
        assert_eq!(containers[0].name, "env-dev");
        assert_eq!(containers[0].ipv4, "10.197.97.142");
    }

    #[test]
    fn renders_terminal_and_tunnel_commands_for_discovered_container() {
        let server = fixture_server();
        let container = DiscoveredContainer {
            name: String::from("workflow-dev"),
            status: String::from("RUNNING"),
            ipv4: String::from("10.211.82.202"),
        };

        let target = build_target_from_container(&server, &container);

        assert_eq!(target.id, "mechanism-dev::workflow-dev");
        assert_eq!(target.label, "Workflow");
        assert_eq!(target.host, "mechanism-dev");
        let home = if cfg!(windows) {
            std::env::var("USERPROFILE").unwrap().replace('\\', "/")
        } else {
            std::env::var("HOME").unwrap()
        };
        let expected_known_hosts_file = format!(
            "{}/.ssh/univers-ark-developer-known_hosts",
            home
        );
        let expected_terminal_command = format!(
            "ssh -o StrictHostKeyChecking=accept-new -o UserKnownHostsFile={kh} -o HostKeyAlias=univers-ark-developer--mechanism-dev--workflow-dev -tt -J mechanism-dev ubuntu@10.211.82.202 'tmux-mobile-view attach || exec /bin/zsh -l || exec /bin/bash -l || exec /bin/sh -l'",
            kh = expected_known_hosts_file
        );
        assert_eq!(target.terminal_command, expected_terminal_command);
        assert_eq!(
            target.surfaces[0].tunnel_command,
            format!(
                "ssh -o StrictHostKeyChecking=accept-new -o UserKnownHostsFile={} -o HostKeyAlias=univers-ark-developer--mechanism-dev--workflow-dev -NT -L {{localPort}}:127.0.0.1:3432 -J mechanism-dev ubuntu@10.211.82.202",
                expected_known_hosts_file
            )
        );
    }

    #[test]
    fn builds_ready_server_state_from_reachable_containers() {
        let containers = vec![
            ManagedContainer {
                server_id: String::from("mechanism-dev"),
                server_label: String::from("Mechanism"),
                target_id: String::from("mechanism-dev::automation-dev"),
                name: String::from("automation-dev"),
                label: String::from("Automation"),
                status: String::from("RUNNING"),
                ipv4: String::from("10.211.82.78"),
                ssh_user: String::from("ubuntu"),
                ssh_destination: String::from("ubuntu@10.211.82.78"),
                ssh_command: String::from("ssh -J mechanism-dev ubuntu@10.211.82.78"),
                ssh_state: String::from("ready"),
                ssh_message: String::from("SSH ready via mechanism-dev."),
                ssh_reachable: true,
            },
            ManagedContainer {
                server_id: String::from("mechanism-dev"),
                server_label: String::from("Mechanism"),
                target_id: String::from("mechanism-dev::runtime-dev"),
                name: String::from("runtime-dev"),
                label: String::from("Runtime"),
                status: String::from("RUNNING"),
                ipv4: String::from("10.211.82.38"),
                ssh_user: String::from("ubuntu"),
                ssh_destination: String::from("ubuntu@10.211.82.38"),
                ssh_command: String::from("ssh -J mechanism-dev ubuntu@10.211.82.38"),
                ssh_state: String::from("ready"),
                ssh_message: String::from("SSH ready via mechanism-dev."),
                ssh_reachable: true,
            },
        ];

        let (state, message) = server_state_for_containers(&containers);

        assert_eq!(state, "ready");
        assert!(message.contains("2 development container(s)"));
    }
}
