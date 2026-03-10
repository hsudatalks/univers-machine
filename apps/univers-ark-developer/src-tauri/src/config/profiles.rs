use crate::models::{
    BrowserServiceType, BrowserSurface, CommandService, ContainerWorkspace, DeveloperService,
    DeveloperServiceKind, DeveloperTarget, sync_target_services,
};

use super::RemoteContainerServer;

#[derive(Clone)]
struct ContainerProfileDefaults {
    workspace: ContainerWorkspace,
    services: Vec<DeveloperService>,
}

fn ark_workbench_profile_defaults() -> ContainerProfileDefaults {
    ContainerProfileDefaults {
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
                id: String::from("preview"),
                label: String::from("Preview"),
                kind: DeveloperServiceKind::Web,
                description: String::from("Preview surface."),
                web: Some(BrowserSurface {
                    id: String::from("preview"),
                    label: String::from("Preview"),
                    service_type: BrowserServiceType::Http,
                    tunnel_command: String::new(),
                    local_url: String::from("http://127.0.0.1:4173/"),
                    remote_url: String::from("http://127.0.0.1:4173/"),
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
                    restart: String::from(
                        "cd ~/repos/univers-container && ./.claude/skills/container-manage/bin/cm dev restart developer",
                    ),
                }),
            },
        ],
    }
}

fn profile_defaults(profile: &str) -> Option<ContainerProfileDefaults> {
    match profile.trim() {
        "ark-workbench" => Some(ark_workbench_profile_defaults()),
        _ => None,
    }
}

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

pub(super) fn apply_profile_defaults_to_target(target: &mut DeveloperTarget) {
    let profile_name = target.workspace.profile.trim().to_string();
    let Some(defaults) = profile_defaults(&profile_name) else {
        sync_target_services(target);
        return;
    };

    merge_workspace_defaults(&mut target.workspace, &defaults.workspace);

    if target.services.is_empty() && target.surfaces.is_empty() {
        target.services = defaults.services;
    }

    sync_target_services(target);
}

pub(super) fn apply_profile_defaults_to_remote_server(server: &mut RemoteContainerServer) {
    let profile_name = server.workspace.profile.trim().to_string();
    let Some(defaults) = profile_defaults(&profile_name) else {
        return;
    };

    merge_workspace_defaults(&mut server.workspace, &defaults.workspace);

    if server.services.is_empty() && server.surfaces.is_empty() {
        server.services = defaults.services;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        apply_profile_defaults_to_target(&mut target);

        assert_eq!(target.workspace.default_tool, "dashboard");
        assert_eq!(target.workspace.primary_web_service_id, "development");
        assert_eq!(target.services.len(), 3);
        assert_eq!(target.surfaces.len(), 2);
        assert!(target
            .services
            .iter()
            .any(|service| service.id == "development" && service.web.is_some()));
    }
}
