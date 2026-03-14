use super::discovery::{
    build_target_from_container, parse_discovered_containers, server_state_for_containers,
};
use super::ssh::ssh_destination;
use super::*;
use crate::models::{
    BrowserServiceType, BrowserSurface, ContainerWorkspace, MachineTransport, ManagedContainer,
    ManagedContainerKind,
};

fn fixture_server() -> RemoteContainerServer {
    RemoteContainerServer {
        id: String::from("mechanism-dev"),
        label: String::from("Mechanism"),
        transport: MachineTransport::Ssh,
        host: String::from("mechanism-dev"),
        port: 22,
        description: String::from("Mechanism development server."),
        manager_type: ContainerManagerType::Lxd,
        discovery_mode: ContainerDiscoveryMode::Auto,
        discovery_command: String::new(),
        ssh_user: String::from("david"),
        container_ssh_user: String::from("ubuntu"),
        identity_files: vec![],
        ssh_credential_id: String::new(),
        jump_chain: vec![],
        known_hosts_path: String::new(),
        strict_host_key_checking: false,
        container_name_suffix: String::from("-dev"),
        include_stopped: false,
        target_label_template: String::new(),
        target_host_template: String::from("{serverHost}"),
        target_description_template: String::new(),
        host_terminal_startup_command: String::new(),
        terminal_command_template: String::new(),
        notes: vec![String::from(
            "SSH target: {sshUser}@{containerIp} via {serverHost}.",
        )],
        workspace: ContainerWorkspace::default(),
        services: vec![],
        surfaces: vec![BrowserSurface {
            id: String::from("development"),
            label: String::from("Development"),
            service_type: BrowserServiceType::Vite,
            background_prerender: true,
            tunnel_command: String::from(
                "ssh {sshOptions} -NT -L {localPort}:127.0.0.1:3432 -J {serverHost} {sshUser}@{containerIp}",
            ),
            local_url: String::from("http://127.0.0.1:{localPort}/"),
            remote_url: String::from("http://127.0.0.1:3432/"),
            vite_hmr_tunnel_command: String::from(
                "ssh {sshOptions} -NT -L {localPort}:127.0.0.1:3433 -J {serverHost} {sshUser}@{containerIp}",
            ),
        }],
        containers: vec![],
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
        id: String::from("workflow-dev"),
        kind: ManagedContainerKind::Managed,
        name: String::from("workflow-dev"),
        source: String::from("lxd"),
        ssh_user: String::from("ubuntu"),
        ssh_user_candidates: vec![String::from("ubuntu"), String::from("root")],
        status: String::from("RUNNING"),
        ipv4: String::from("10.211.82.202"),
        label: None,
        description: None,
        workspace: None,
        services: vec![],
        surfaces: vec![],
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
    let expected_known_hosts_file = format!("{home}/.ssh/univers-ark-developer-known_hosts");
    let expected_terminal_command = format!(
        "ssh -o UserKnownHostsFile={expected_known_hosts_file} -o HostKeyAlias=univers-ark-developer--mechanism-dev--workflow-dev -o StrictHostKeyChecking=no -tt -J david@mechanism-dev -p 22 ubuntu@10.211.82.202 'exec /bin/zsh -l || exec /bin/bash -l || exec /bin/sh -l'"
    );
    assert_eq!(target.terminal_command, expected_terminal_command);
    assert_eq!(
        target.surfaces[0].tunnel_command,
        format!(
            "ssh -o UserKnownHostsFile={expected_known_hosts_file} -o HostKeyAlias=univers-ark-developer--mechanism-dev--workflow-dev -o StrictHostKeyChecking=no -NT -L {{localPort}}:127.0.0.1:3432 -J mechanism-dev ubuntu@10.211.82.202"
        )
    );
}

#[test]
fn uses_tmux_startup_for_ark_workbench_container_profiles() {
    let mut server = fixture_server();
    server.workspace.profile = String::from("ark-workbench");
    let container = DiscoveredContainer {
        id: String::from("workflow-dev"),
        kind: ManagedContainerKind::Managed,
        name: String::from("workflow-dev"),
        source: String::from("lxd"),
        ssh_user: String::from("ubuntu"),
        ssh_user_candidates: vec![String::from("ubuntu"), String::from("root")],
        status: String::from("RUNNING"),
        ipv4: String::from("10.211.82.202"),
        label: None,
        description: None,
        workspace: None,
        services: vec![],
        surfaces: vec![],
    };

    let target = build_target_from_container(&server, &container);

    assert_eq!(
        target.terminal_startup_command,
        "tmux-mobile-view attach || exec /bin/zsh -l || exec /bin/bash -l || exec /bin/sh -l"
    );
    assert!(target.terminal_command.contains("tmux-mobile-view attach"));
}

#[test]
fn scan_prefers_detected_container_user_over_stale_saved_user() {
    let server = fixture_server();
    let discovered = DiscoveredContainer {
        id: String::from("automation-dev"),
        kind: ManagedContainerKind::Managed,
        name: String::from("automation-dev"),
        source: String::from("lxd"),
        ssh_user: String::from("ubuntu"),
        ssh_user_candidates: vec![String::from("ubuntu"), String::from("root")],
        status: String::from("RUNNING"),
        ipv4: String::from("10.211.82.78"),
        label: None,
        description: None,
        workspace: None,
        services: vec![],
        surfaces: vec![],
    };
    let existing = MachineContainerConfig {
        id: String::from("automation-dev"),
        name: String::from("automation-dev"),
        kind: ManagedContainerKind::Managed,
        enabled: true,
        source: String::from("lxd"),
        ssh_user: String::from("david"),
        ssh_user_candidates: vec![String::from("david"), String::from("ubuntu")],
        label: String::new(),
        description: String::new(),
        ipv4: String::from("10.211.82.78"),
        status: String::from("RUNNING"),
        workspace: ContainerWorkspace::default(),
        services: vec![],
        surfaces: vec![],
    };

    let value = super::inventory::merge_discovered_container_with_manual_config(
        &server,
        &discovered,
        Some(&existing),
    );

    assert_eq!(value.ssh_user, "ubuntu");
    assert_eq!(
        value.ssh_user_candidates,
        vec![
            String::from("ubuntu"),
            String::from("root"),
            String::from("david"),
        ]
    );
}

#[test]
#[ignore = "requires a reachable SSH target from the local developer environment"]
fn live_russh_exec_uses_configured_identity_file() {
    let target_id =
        std::env::var("UNIVERS_ARK_SSH_TARGET").unwrap_or_else(|_| String::from("domain-dev::host"));
    let output =
        super::execute_target_command_via_russh(&target_id, "printf univers-infra-ssh-ok")
            .unwrap_or_else(|error| panic!("live russh exec failed for {target_id}: {error}"));

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("univers-infra-ssh-ok"),
        "unexpected stdout for {target_id}: {stdout}"
    );
}

#[test]
fn builds_ready_server_state_from_reachable_containers() {
    let containers = vec![
        ManagedContainer {
            server_id: String::from("mechanism-dev"),
            server_label: String::from("Mechanism"),
            container_id: String::from("automation-dev"),
            kind: ManagedContainerKind::Managed,
            transport: MachineTransport::Ssh,
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
            container_id: String::from("runtime-dev"),
            kind: ManagedContainerKind::Managed,
            transport: MachineTransport::Ssh,
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

#[test]
fn computes_ssh_destination() {
    assert_eq!(ssh_destination("10.1.2.3", "ubuntu"), "ubuntu@10.1.2.3");
}
