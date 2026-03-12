mod candidates;
mod host_probe;

use crate::machine::{DiscoveredContainer, RemoteContainerServer};
use crate::models::ManagedContainerKind;

use self::{
    candidates::{managed_container_ssh_user_candidates, preferred_available_container_ssh_users},
    host_probe::discover_container_ssh_users_via_host,
};

pub(super) fn resolve_container_ssh_user(
    server: &RemoteContainerServer,
    container: &DiscoveredContainer,
) -> String {
    if matches!(container.kind, ManagedContainerKind::Host) {
        return server.ssh_user.clone();
    }

    if !container.ssh_user.trim().is_empty() {
        return container.ssh_user.clone();
    }

    super::super::ssh::managed_container_ssh_user(server).to_string()
}

pub(super) fn enrich_discovered_container_ssh_users(
    server: &RemoteContainerServer,
    containers: Vec<DiscoveredContainer>,
) -> Vec<DiscoveredContainer> {
    std::thread::scope(|scope| {
        let handles = containers
            .into_iter()
            .map(|container| {
                scope.spawn(move || {
                    if matches!(container.kind, ManagedContainerKind::Host) {
                        return DiscoveredContainer {
                            ssh_user: server.ssh_user.clone(),
                            ssh_user_candidates: vec![server.ssh_user.clone()],
                            ..container
                        };
                    }

                    let discovered_users =
                        discover_container_ssh_users_via_host(server, &container);
                    let ssh_user_candidates = preferred_available_container_ssh_users(
                        server,
                        &container,
                        discovered_users,
                    );
                    let ssh_user = ssh_user_candidates.first().cloned().unwrap_or_else(|| {
                        managed_container_ssh_user_candidates(server)[0].clone()
                    });

                    DiscoveredContainer {
                        ssh_user,
                        ssh_user_candidates,
                        ..container
                    }
                })
            })
            .collect::<Vec<_>>();

        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machine::{
        ContainerDiscoveryMode, ContainerManagerType, DiscoveredContainer, RemoteContainerServer,
    };
    use crate::models::{ContainerWorkspace, MachineTransport};

    fn fixture_server() -> RemoteContainerServer {
        RemoteContainerServer {
            id: String::from("qa-dev"),
            label: String::from("QA"),
            transport: MachineTransport::Ssh,
            host: String::from("qa-dev"),
            port: 22,
            description: String::new(),
            manager_type: ContainerManagerType::Lxd,
            discovery_mode: ContainerDiscoveryMode::Auto,
            discovery_command: String::new(),
            ssh_user: String::from("david"),
            container_ssh_user: String::from("david"),
            identity_files: vec![],
            jump_chain: vec![],
            known_hosts_path: String::new(),
            strict_host_key_checking: false,
            container_name_suffix: String::new(),
            include_stopped: false,
            target_label_template: String::new(),
            target_host_template: String::new(),
            target_description_template: String::new(),
            host_terminal_startup_command: String::new(),
            terminal_command_template: String::new(),
            notes: vec![],
            workspace: ContainerWorkspace::default(),
            services: vec![],
            surfaces: vec![],
            containers: vec![],
        }
    }

    #[test]
    fn prefers_available_container_users_over_stale_machine_default() {
        let server = fixture_server();
        let container = DiscoveredContainer {
            id: String::from("maintenance-dev"),
            kind: ManagedContainerKind::Managed,
            name: String::from("maintenance-dev"),
            source: String::from("lxd"),
            ssh_user: String::new(),
            ssh_user_candidates: vec![],
            status: String::from("RUNNING"),
            ipv4: String::from("10.0.0.2"),
            label: None,
            description: None,
            workspace: None,
            services: vec![],
            surfaces: vec![],
        };

        let users = preferred_available_container_ssh_users(
            &server,
            &container,
            vec![String::from("root"), String::from("ubuntu")],
        );

        assert_eq!(users, vec![String::from("ubuntu"), String::from("root")]);
    }

    #[test]
    fn parses_direct_container_user_query_output() {
        let users = host_probe::parse_discovered_container_users("ubuntu\nroot\nubuntu\n");
        assert_eq!(users, vec![String::from("ubuntu"), String::from("root")]);
    }
}
