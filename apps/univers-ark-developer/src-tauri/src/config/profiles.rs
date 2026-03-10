use crate::models::{BrowserSurface, ContainerWorkspace, DeveloperService, DeveloperTarget, sync_target_services};
use serde::Deserialize;
use std::collections::HashMap;

use super::RemoteContainerServer;

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(super) struct ContainerProfileConfig {
    #[serde(default)]
    pub(super) workspace: ContainerWorkspace,
    #[serde(default)]
    pub(super) services: Vec<DeveloperService>,
    #[serde(default)]
    pub(super) surfaces: Vec<BrowserSurface>,
}

pub(super) type ContainerProfiles = HashMap<String, ContainerProfileConfig>;

fn fill_string(target: &mut String, fallback: &str) {
    if target.trim().is_empty() && !fallback.trim().is_empty() {
        *target = fallback.to_string();
    }
}

fn merge_workspace_defaults(
    workspace: &mut ContainerWorkspace,
    defaults: &ContainerWorkspace,
) {
    fill_string(&mut workspace.profile, &defaults.profile);
    fill_string(&mut workspace.default_tool, &defaults.default_tool);
    fill_string(&mut workspace.project_path, &defaults.project_path);
    fill_string(&mut workspace.files_root, &defaults.files_root);
    fill_string(
        &mut workspace.primary_web_service_id,
        &defaults.primary_web_service_id,
    );
    fill_string(
        &mut workspace.tmux_command_service_id,
        &defaults.tmux_command_service_id,
    );
}

pub(super) fn apply_profile_defaults_to_target(
    target: &mut DeveloperTarget,
    profile_defaults: &ContainerProfiles,
) {
    let profile_name = target.workspace.profile.trim().to_string();
    let Some(defaults) = profile_defaults.get(&profile_name) else {
        sync_target_services(target);
        return;
    };

    merge_workspace_defaults(&mut target.workspace, &defaults.workspace);

    if target.services.is_empty() && target.surfaces.is_empty() {
        target.services = defaults.services.clone();
        target.surfaces = defaults.surfaces.clone();
    }

    sync_target_services(target);
}

pub(super) fn apply_profile_defaults_to_remote_server(
    server: &mut RemoteContainerServer,
    profile_defaults: &ContainerProfiles,
) {
    let profile_name = server.workspace.profile.trim().to_string();
    let Some(defaults) = profile_defaults.get(&profile_name) else {
        return;
    };

    merge_workspace_defaults(&mut server.workspace, &defaults.workspace);

    if server.services.is_empty() && server.surfaces.is_empty() {
        server.services = defaults.services.clone();
        server.surfaces = defaults.surfaces.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{BrowserServiceType, CommandService, DeveloperServiceKind};

    fn fixture_profiles() -> ContainerProfiles {
        HashMap::from([(
            String::from("ark-workbench"),
            ContainerProfileConfig {
                workspace: ContainerWorkspace {
                    profile: String::from("ark-workbench"),
                    default_tool: String::from("dashboard"),
                    project_path: String::from("~/repos/hvac-workbench"),
                    files_root: String::from("~/repos/hvac-workbench"),
                    primary_web_service_id: String::from("development"),
                    tmux_command_service_id: String::from("tmux-developer"),
                },
                services: vec![
                    DeveloperService {
                        id: String::from("development"),
                        label: String::from("Development"),
                        kind: DeveloperServiceKind::Web,
                        description: String::from("Primary Vite development surface."),
                        web: Some(BrowserSurface {
                            id: String::from("development"),
                            label: String::from("Development"),
                            service_type: BrowserServiceType::Vite,
                            tunnel_command: String::new(),
                            local_url: String::from("http://127.0.0.1:3432/"),
                            remote_url: String::from("http://127.0.0.1:3432/"),
                            vite_hmr_tunnel_command: String::new(),
                        }),
                        endpoint: None,
                        command: None,
                    },
                    DeveloperService {
                        id: String::from("tmux-developer"),
                        label: String::from("Developer Tmux"),
                        kind: DeveloperServiceKind::Command,
                        description: String::from("Restart the developer tmux server."),
                        web: None,
                        endpoint: None,
                        command: Some(CommandService {
                            restart: String::from("cm dev restart developer"),
                        }),
                    },
                ],
                surfaces: vec![],
            },
        )])
    }

    #[test]
    fn applies_ark_workbench_defaults_to_target() {
        let mut target = DeveloperTarget {
            id: String::from("local"),
            label: String::from("Local"),
            host: String::from("localhost"),
            description: String::new(),
            terminal_command: String::new(),
            notes: vec![],
            workspace: ContainerWorkspace {
                profile: String::from("ark-workbench"),
                ..ContainerWorkspace::default()
            },
            services: vec![],
            surfaces: vec![],
        };

        apply_profile_defaults_to_target(&mut target, &fixture_profiles());

        assert_eq!(target.workspace.default_tool, "dashboard");
        assert_eq!(target.workspace.primary_web_service_id, "development");
        assert_eq!(target.services.len(), 2);
        assert_eq!(target.surfaces.len(), 1);
        assert!(target
            .services
            .iter()
            .any(|service| service.id == "development" && service.web.is_some()));
    }
}
