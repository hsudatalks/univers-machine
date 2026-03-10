use crate::models::{BrowserSurface, ContainerWorkspace, DeveloperService, DeveloperTarget, sync_target_services};
use serde::Deserialize;
use std::collections::HashMap;

use super::RemoteContainerServer;

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(super) struct ContainerProfileConfig {
    #[serde(default)]
    pub(super) extends: String,
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

fn merge_services(
    base: &[DeveloperService],
    overrides: &[DeveloperService],
) -> Vec<DeveloperService> {
    let mut merged = base.to_vec();

    for override_service in overrides {
        if let Some(existing) = merged
            .iter_mut()
            .find(|service| service.id == override_service.id)
        {
            *existing = override_service.clone();
        } else {
            merged.push(override_service.clone());
        }
    }

    merged
}

fn merge_surfaces(
    base: &[BrowserSurface],
    overrides: &[BrowserSurface],
) -> Vec<BrowserSurface> {
    let mut merged = base.to_vec();

    for override_surface in overrides {
        if let Some(existing) = merged
            .iter_mut()
            .find(|surface| surface.id == override_surface.id)
        {
            *existing = override_surface.clone();
        } else {
            merged.push(override_surface.clone());
        }
    }

    merged
}

fn resolve_profile_defaults_inner(
    profile_name: &str,
    profile_defaults: &ContainerProfiles,
    stack: &mut Vec<String>,
) -> Option<ContainerProfileConfig> {
    let defaults = profile_defaults.get(profile_name)?.clone();

    if defaults.extends.trim().is_empty() {
        return Some(defaults);
    }

    let parent_name = defaults.extends.trim().to_string();
    if stack.iter().any(|entry| entry == &parent_name) {
        return Some(defaults);
    }

    stack.push(parent_name.clone());
    let parent = resolve_profile_defaults_inner(&parent_name, profile_defaults, stack);
    stack.pop();

    let Some(parent) = parent else {
        return Some(defaults);
    };

    let mut merged = defaults.clone();
    merge_workspace_defaults(&mut merged.workspace, &parent.workspace);
    merged.services = merge_services(&parent.services, &defaults.services);
    merged.surfaces = merge_surfaces(&parent.surfaces, &defaults.surfaces);

    Some(merged)
}

fn resolve_profile_defaults(
    profile_name: &str,
    profile_defaults: &ContainerProfiles,
) -> Option<ContainerProfileConfig> {
    let mut stack = vec![profile_name.to_string()];
    let mut resolved = resolve_profile_defaults_inner(profile_name, profile_defaults, &mut stack)?;
    if resolved.workspace.profile.trim().is_empty() {
        resolved.workspace.profile = profile_name.to_string();
    }
    Some(resolved)
}

pub(super) fn apply_profile_defaults_to_target(
    target: &mut DeveloperTarget,
    profile_defaults: &ContainerProfiles,
    default_profile: Option<&str>,
) {
    let profile_name = if target.workspace.profile.trim().is_empty() {
        default_profile.unwrap_or_default().trim().to_string()
    } else {
        target.workspace.profile.trim().to_string()
    };

    let Some(defaults) = resolve_profile_defaults(&profile_name, profile_defaults) else {
        sync_target_services(target);
        return;
    };

    if target.workspace.profile.trim().is_empty() && !profile_name.is_empty() {
        target.workspace.profile = profile_name;
    }

    merge_workspace_defaults(&mut target.workspace, &defaults.workspace);
    target.services = merge_services(&defaults.services, &target.services);
    target.surfaces = merge_surfaces(&defaults.surfaces, &target.surfaces);

    sync_target_services(target);
}

pub(super) fn apply_profile_defaults_to_remote_server(
    server: &mut RemoteContainerServer,
    profile_defaults: &ContainerProfiles,
    default_profile: Option<&str>,
) {
    let profile_name = if server.workspace.profile.trim().is_empty() {
        default_profile.unwrap_or_default().trim().to_string()
    } else {
        server.workspace.profile.trim().to_string()
    };

    let Some(defaults) = resolve_profile_defaults(&profile_name, profile_defaults) else {
        return;
    };

    if server.workspace.profile.trim().is_empty() && !profile_name.is_empty() {
        server.workspace.profile = profile_name;
    }

    merge_workspace_defaults(&mut server.workspace, &defaults.workspace);
    server.services = merge_services(&defaults.services, &server.services);
    server.surfaces = merge_surfaces(&defaults.surfaces, &server.surfaces);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{BrowserServiceType, CommandService, DeveloperServiceKind};

    fn fixture_profiles() -> ContainerProfiles {
        HashMap::from([(
            String::from("ark-workbench"),
            ContainerProfileConfig {
                extends: String::new(),
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

        apply_profile_defaults_to_target(&mut target, &fixture_profiles(), None);

        assert_eq!(target.workspace.default_tool, "dashboard");
        assert_eq!(target.workspace.primary_web_service_id, "development");
        assert_eq!(target.services.len(), 2);
        assert_eq!(target.surfaces.len(), 1);
        assert!(target
            .services
            .iter()
            .any(|service| service.id == "development" && service.web.is_some()));
    }

    #[test]
    fn applies_default_profile_when_target_profile_is_empty() {
        let mut target = DeveloperTarget {
            id: String::from("local"),
            label: String::from("Local"),
            host: String::from("localhost"),
            description: String::new(),
            terminal_command: String::new(),
            notes: vec![],
            workspace: ContainerWorkspace::default(),
            services: vec![],
            surfaces: vec![],
        };

        apply_profile_defaults_to_target(&mut target, &fixture_profiles(), Some("ark-workbench"));

        assert_eq!(target.workspace.profile, "ark-workbench");
        assert_eq!(target.workspace.default_tool, "dashboard");
        assert_eq!(target.services.len(), 2);
    }

    #[test]
    fn merges_inherited_profile_services() {
        let mut profiles = fixture_profiles();
        profiles.insert(
            String::from("derived"),
            ContainerProfileConfig {
                extends: String::from("ark-workbench"),
                workspace: ContainerWorkspace {
                    profile: String::from("derived"),
                    files_root: String::from("~/repos/custom"),
                    ..ContainerWorkspace::default()
                },
                services: vec![DeveloperService {
                    id: String::from("api"),
                    label: String::from("API"),
                    kind: DeveloperServiceKind::Endpoint,
                    description: String::new(),
                    web: None,
                    endpoint: Some(crate::models::EndpointService {
                        probe_type: crate::models::EndpointProbeType::Http,
                        host: String::from("127.0.0.1"),
                        port: 3000,
                        path: String::from("/health"),
                        url: String::new(),
                    }),
                    command: None,
                }],
                surfaces: vec![],
            },
        );

        let mut target = DeveloperTarget {
            id: String::from("local"),
            label: String::from("Local"),
            host: String::from("localhost"),
            description: String::new(),
            terminal_command: String::new(),
            notes: vec![],
            workspace: ContainerWorkspace {
                profile: String::from("derived"),
                ..ContainerWorkspace::default()
            },
            services: vec![],
            surfaces: vec![],
        };

        apply_profile_defaults_to_target(&mut target, &profiles, None);

        assert_eq!(target.workspace.files_root, "~/repos/custom");
        assert_eq!(target.workspace.default_tool, "dashboard");
        assert!(target.services.iter().any(|service| service.id == "development"));
        assert!(target.services.iter().any(|service| service.id == "api"));
    }
}
