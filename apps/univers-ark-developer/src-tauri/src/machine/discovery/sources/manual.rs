use crate::{machine::MachineContainerConfig, models::ManagedContainerKind};

use super::super::super::{DiscoveredContainer, RemoteContainerServer};

fn machine_container_to_discovered(container: &MachineContainerConfig) -> DiscoveredContainer {
    let has_workspace_override = !container.workspace.profile.trim().is_empty()
        || !container.workspace.default_tool.trim().is_empty()
        || !container.workspace.project_path.trim().is_empty()
        || !container.workspace.files_root.trim().is_empty()
        || !container.workspace.primary_web_service_id.trim().is_empty()
        || !container
            .workspace
            .tmux_command_service_id
            .trim()
            .is_empty();

    DiscoveredContainer {
        id: if container.id.trim().is_empty() {
            container.name.clone()
        } else {
            container.id.clone()
        },
        kind: container.kind,
        name: container.name.clone(),
        source: if container.source.trim().is_empty() {
            if matches!(container.kind, ManagedContainerKind::Host) {
                String::from("host")
            } else {
                String::from("manual")
            }
        } else {
            container.source.clone()
        },
        ssh_user: container.ssh_user.clone(),
        ssh_user_candidates: container.ssh_user_candidates.clone(),
        status: container.status.clone(),
        ipv4: container.ipv4.clone(),
        label: (!container.label.trim().is_empty()).then(|| container.label.clone()),
        description: (!container.description.trim().is_empty())
            .then(|| container.description.clone()),
        workspace: has_workspace_override.then(|| container.workspace.clone()),
        services: container.services.clone(),
        surfaces: container.surfaces.clone(),
    }
}

pub(super) fn discover_manual_containers(
    server: &RemoteContainerServer,
) -> Vec<DiscoveredContainer> {
    server
        .containers
        .iter()
        .filter(|container| matches!(container.kind, ManagedContainerKind::Managed))
        .filter(|container| container.enabled)
        .filter(|container| !container.name.trim().is_empty() && !container.ipv4.trim().is_empty())
        .map(machine_container_to_discovered)
        .collect()
}

pub(crate) fn cached_server_containers(server: &RemoteContainerServer) -> Vec<DiscoveredContainer> {
    discover_manual_containers(server)
}
