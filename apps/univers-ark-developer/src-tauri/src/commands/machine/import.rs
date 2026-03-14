use crate::{infra::shell::shell_command, models::MachineImportCandidate};
use serde::Deserialize;
use std::{
    collections::HashMap,
    path::Path,
};
use tauri::async_runtime;
use univers_infra_ssh::SshConfigResolver;

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
struct TailscaleStatusResponse {
    #[serde(default)]
    peer: HashMap<String, TailscalePeer>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
struct TailscalePeer {
    #[serde(default)]
    host_name: String,
    #[serde(default)]
    dns_name: String,
    #[serde(default)]
    tailscale_ips: Vec<String>,
    #[serde(default)]
    online: bool,
    #[serde(default)]
    active: bool,
    #[serde(default)]
    os: String,
}

fn sanitize_machine_id(value: &str) -> String {
    let mut machine_id = String::new();
    let mut previous_was_separator = false;

    for character in value.chars() {
        let normalized = if character.is_ascii_alphanumeric() {
            previous_was_separator = false;
            Some(character.to_ascii_lowercase())
        } else if !previous_was_separator {
            previous_was_separator = true;
            Some('-')
        } else {
            None
        };

        if let Some(character) = normalized {
            machine_id.push(character);
        }
    }

    machine_id.trim_matches('-').to_string()
}

fn display_label(value: &str) -> String {
    value
        .split(['-', '_', '.', ' '])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            let Some(first) = chars.next() else {
                return String::new();
            };

            let mut label = String::new();
            label.extend(first.to_uppercase());
            label.push_str(chars.as_str());
            label
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn default_import_ssh_user() -> String {
    std::env::var("USER")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| String::from("ubuntu"))
}

fn scan_ssh_config_machine_candidates_inner() -> Result<Vec<MachineImportCandidate>, String> {
    let resolver = match SshConfigResolver::from_default_path() {
        Ok(resolver) => resolver,
        Err(_) => return Ok(Vec::new()),
    };

    let mut candidates = resolver
        .aliases()
        .into_iter()
        .filter_map(|alias| {
            let chain = resolver.resolve(&alias).ok()?;
            let final_hop = chain.hops().last()?;
            let jump_chain = chain
                .hops()
                .iter()
                .take(chain.hops().len().saturating_sub(1))
                .map(|hop| crate::models::ImportedMachineJump {
                    host: hop.host.clone(),
                    port: hop.port,
                    user: hop.user.clone(),
                    identity_files: hop
                        .identity_files()
                        .iter()
                        .map(|path| path_to_string(path.as_path()))
                        .collect(),
                })
                .collect::<Vec<_>>();

            let detail = if jump_chain.is_empty() {
                format!("{}@{}:{}", final_hop.user, final_hop.host, final_hop.port)
            } else {
                let route = jump_chain
                    .iter()
                    .map(|jump| format!("{}@{}", jump.user, jump.host))
                    .collect::<Vec<_>>()
                    .join(" -> ");
                format!(
                    "{}@{}:{} via {}",
                    final_hop.user, final_hop.host, final_hop.port, route
                )
            };

            Some(MachineImportCandidate {
                import_id: format!("ssh-config:{alias}"),
                machine_id: sanitize_machine_id(&alias),
                label: display_label(&alias),
                host: final_hop.host.clone(),
                port: final_hop.port,
                ssh_user: final_hop.user.clone(),
                identity_files: final_hop
                    .identity_files()
                    .iter()
                    .map(|path| path_to_string(path.as_path()))
                    .collect(),
                jump_chain,
                description: format!("Imported from SSH config alias {alias}."),
                detail,
            })
        })
        .filter(|candidate| !candidate.machine_id.is_empty())
        .collect::<Vec<_>>();

    candidates.sort_by(|left, right| left.label.cmp(&right.label));
    Ok(candidates)
}

fn scan_tailscale_machine_candidates_inner() -> Result<Vec<MachineImportCandidate>, String> {
    let output = shell_command("tailscale status --json")
        .output()
        .map_err(|error| format!("Failed to execute tailscale status --json: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            String::from("tailscale status --json failed")
        } else {
            stderr
        });
    }

    let parsed: TailscaleStatusResponse = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("Failed to parse tailscale status output: {error}"))?;

    let mut candidates = parsed
        .peer
        .into_values()
        .filter_map(|peer| {
            let dns_name = peer.dns_name.trim_end_matches('.').to_string();
            let host = if !dns_name.is_empty() {
                dns_name.clone()
            } else {
                peer.tailscale_ips.first().cloned().unwrap_or_default()
            };

            if host.trim().is_empty() {
                return None;
            }

            let label_seed = if !peer.host_name.trim().is_empty() {
                peer.host_name.clone()
            } else if !dns_name.is_empty() {
                dns_name.split('.').next().unwrap_or_default().to_string()
            } else {
                host.clone()
            };

            let status = if peer.online {
                "online"
            } else if peer.active {
                "active"
            } else {
                "offline"
            };

            let detail = [
                Some(host.clone()),
                (!peer.os.trim().is_empty()).then(|| peer.os.clone()),
                Some(status.to_string()),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" · ");

            Some(MachineImportCandidate {
                import_id: format!("tailscale:{host}"),
                machine_id: sanitize_machine_id(&label_seed),
                label: display_label(&label_seed),
                host,
                port: 22,
                ssh_user: default_import_ssh_user(),
                identity_files: vec![],
                jump_chain: vec![],
                description: String::from(
                    "Imported from Tailscale peer discovery. Verify SSH user before connecting.",
                ),
                detail,
            })
        })
        .filter(|candidate| !candidate.machine_id.is_empty())
        .collect::<Vec<_>>();

    candidates.sort_by(|left, right| left.label.cmp(&right.label));
    Ok(candidates)
}

#[tauri::command]
pub(crate) async fn scan_ssh_config_machine_candidates(
) -> Result<Vec<MachineImportCandidate>, String> {
    async_runtime::spawn_blocking(scan_ssh_config_machine_candidates_inner)
        .await
        .map_err(|error| format!("Failed to join SSH config scan task: {error}"))?
}

#[tauri::command]
pub(crate) async fn scan_tailscale_machine_candidates(
) -> Result<Vec<MachineImportCandidate>, String> {
    async_runtime::spawn_blocking(scan_tailscale_machine_candidates_inner)
        .await
        .map_err(|error| format!("Failed to join Tailscale scan task: {error}"))?
}
