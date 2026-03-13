use super::ports::allocate_surface_port;
use crate::{
    constants::SURFACE_HOST,
    machine::{read_targets_file, resolve_raw_target},
    models::{
        BrowserSurface, DeveloperService, DeveloperTarget, MachineTransport, TargetsFile,
        TunnelState,
    },
    services::catalog::web_service,
};
use url::Url;

pub(crate) fn replace_known_tunnel_placeholders(
    tunnel_command: &str,
    remote_url: &str,
    local_port: u16,
) -> String {
    let mut resolved = tunnel_command.replace("{localPort}", &local_port.to_string());

    if let Ok(remote_url) = Url::parse(remote_url) {
        if let Some(host) = remote_url.host_str() {
            resolved = resolved.replace("{remoteHost}", host);
            resolved = resolved.replace("{previewRemoteHost}", host);
        }

        if let Some(port) = remote_url.port_or_known_default() {
            resolved = resolved.replace("{remotePort}", &port.to_string());
            resolved = resolved.replace("{previewRemotePort}", &port.to_string());
        }
    }

    resolved
}

fn rewrite_forward_spec_local_port(forward_spec: &str, local_port: u16) -> String {
    match forward_spec.split_once(':') {
        Some((_, rest)) => format!("{local_port}:{rest}"),
        None => forward_spec.to_string(),
    }
}

fn rewrite_tunnel_forward_port(tunnel_command: &str, local_port: u16) -> String {
    let mut tokens = tunnel_command
        .split_whitespace()
        .map(str::to_string)
        .collect::<Vec<_>>();

    for index in 0..tokens.len() {
        if tokens[index] == "-L" {
            if let Some(forward_spec) = tokens.get_mut(index + 1) {
                *forward_spec = rewrite_forward_spec_local_port(forward_spec, local_port);
                return tokens.join(" ");
            }
        }

        if let Some(forward_spec) = tokens[index].strip_prefix("-L") {
            tokens[index] = format!(
                "-L{}",
                rewrite_forward_spec_local_port(forward_spec, local_port)
            );
            return tokens.join(" ");
        }
    }

    tunnel_command.to_string()
}

fn resolve_runtime_tunnel_command(
    tunnel_command: &str,
    remote_url: &str,
    local_port: u16,
) -> String {
    let placeholder_resolved =
        replace_known_tunnel_placeholders(tunnel_command, remote_url, local_port);

    if placeholder_resolved != tunnel_command {
        return placeholder_resolved;
    }

    rewrite_tunnel_forward_port(&placeholder_resolved, local_port)
}

pub(crate) fn resolve_runtime_vite_hmr_tunnel_command(
    tunnel_command: &str,
    local_port: u16,
) -> String {
    let placeholder_resolved = tunnel_command.replace("{localPort}", &local_port.to_string());

    if placeholder_resolved != tunnel_command {
        return placeholder_resolved;
    }

    rewrite_tunnel_forward_port(&placeholder_resolved, local_port)
}

fn resolve_runtime_local_url(local_url: &str, remote_url: &str, local_port: u16) -> String {
    let template = if local_url.trim().is_empty() {
        remote_url
    } else {
        local_url
    }
    .replace("{localPort}", &local_port.to_string());

    if let Ok(mut url) = Url::parse(&template) {
        let _ = url.set_host(Some(SURFACE_HOST));
        let _ = url.set_port(Some(local_port));
        return url.to_string();
    }

    if let Ok(mut remote_url) = Url::parse(remote_url) {
        let _ = remote_url.set_host(Some(SURFACE_HOST));
        let _ = remote_url.set_port(Some(local_port));
        return remote_url.to_string();
    }

    format!("http://{SURFACE_HOST}:{local_port}/")
}

fn hydrate_surface(
    target: &DeveloperTarget,
    surface: &BrowserSurface,
    tunnel_state: &TunnelState,
) -> Result<BrowserSurface, String> {
    if !should_manage_surface_tunnel(target, surface) {
        return Ok(surface.clone());
    }

    let local_port = allocate_surface_port(tunnel_state, &target.id, &surface.id)?;
    let mut runtime_surface = surface.clone();

    if !runtime_surface.tunnel_command.trim().is_empty() {
        runtime_surface.tunnel_command = resolve_runtime_tunnel_command(
            &runtime_surface.tunnel_command,
            &runtime_surface.remote_url,
            local_port,
        );
    }
    runtime_surface.local_url = resolve_runtime_local_url(
        &runtime_surface.local_url,
        &runtime_surface.remote_url,
        local_port,
    );

    Ok(runtime_surface)
}

fn hydrate_target(
    target: &DeveloperTarget,
    tunnel_state: &TunnelState,
) -> Result<DeveloperTarget, String> {
    let services = if target.services.is_empty() {
        target
            .surfaces
            .iter()
            .map(|surface| {
                hydrate_surface(target, surface, tunnel_state)
                    .map(|hydrated_surface| web_service(&hydrated_surface))
            })
            .collect::<Result<Vec<_>, _>>()?
    } else {
        target
            .services
            .iter()
            .map(|service| hydrate_service(target, service, tunnel_state))
            .collect::<Result<Vec<_>, _>>()?
    };
    let surfaces = services
        .iter()
        .filter_map(|service| service.web.clone())
        .collect();

    Ok(DeveloperTarget {
        services,
        surfaces,
        ..target.clone()
    })
}

fn hydrate_service(
    target: &DeveloperTarget,
    service: &DeveloperService,
    tunnel_state: &TunnelState,
) -> Result<DeveloperService, String> {
    let mut hydrated_service = service.clone();

    if let Some(browser_surface) = &service.web {
        hydrated_service.web = Some(hydrate_surface(target, browser_surface, tunnel_state)?);
    }

    Ok(hydrated_service)
}

fn should_manage_surface_tunnel(target: &DeveloperTarget, surface: &BrowserSurface) -> bool {
    !surface.tunnel_command.trim().is_empty() || matches!(target.transport, MachineTransport::Ssh)
}

pub(crate) fn read_runtime_targets_file(tunnel_state: &TunnelState) -> Result<TargetsFile, String> {
    let targets_file = read_targets_file()?;
    let targets = targets_file
        .targets
        .into_iter()
        .map(|target| hydrate_target(&target, tunnel_state))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(TargetsFile {
        selected_target_id: targets_file.selected_target_id,
        default_profile: targets_file.default_profile,
        targets,
    })
}

fn resolve_runtime_target(
    target_id: &str,
    tunnel_state: &TunnelState,
) -> Result<DeveloperTarget, String> {
    let target = resolve_raw_target(target_id)?;
    hydrate_target(&target, tunnel_state)
}

pub(crate) fn resolve_runtime_web_surface(
    target_id: &str,
    service_id: &str,
    tunnel_state: &TunnelState,
) -> Result<BrowserSurface, String> {
    let target = resolve_runtime_target(target_id, tunnel_state)?;

    target
        .services
        .into_iter()
        .find(|service| service.id == service_id)
        .and_then(|service| service.web)
        .ok_or_else(|| {
            format!(
                "Unknown web service {service_id} for target {target_id}"
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{BrowserServiceType, ContainerWorkspace};
    use url::Url;

    fn fixture_target(transport: MachineTransport) -> DeveloperTarget {
        DeveloperTarget {
            id: String::from("mechanism-dev::workflow-dev"),
            machine_id: String::from("mechanism-dev"),
            container_id: String::from("workflow-dev"),
            transport,
            container_kind: crate::models::ManagedContainerKind::Managed,
            label: String::from("Workflow"),
            host: String::from("mechanism-dev"),
            description: String::new(),
            terminal_command: String::new(),
            terminal_startup_command: String::new(),
            notes: vec![],
            workspace: ContainerWorkspace::default(),
            services: vec![web_service(&BrowserSurface {
                id: String::from("development"),
                label: String::from("Development"),
                service_type: BrowserServiceType::Vite,
                background_prerender: true,
                tunnel_command: String::new(),
                local_url: String::from("http://127.0.0.1:3432/"),
                remote_url: String::from("http://127.0.0.1:3432/"),
                vite_hmr_tunnel_command: String::new(),
            })],
            surfaces: vec![],
        }
    }

    #[test]
    fn hydrates_ssh_services_without_explicit_tunnel_commands() {
        let tunnel_state = TunnelState::default();
        let hydrated = hydrate_target(&fixture_target(MachineTransport::Ssh), &tunnel_state)
            .expect("target should hydrate");
        let surface = hydrated.services[0].web.as_ref().expect("web surface");
        let local_url = Url::parse(&surface.local_url).expect("valid local url");

        assert_eq!(local_url.host_str(), Some(SURFACE_HOST));
        assert_ne!(local_url.port_or_known_default(), Some(3432));
        assert!(surface.tunnel_command.is_empty());
    }

    #[test]
    fn keeps_legacy_local_services_direct_without_tunnel_commands() {
        let tunnel_state = TunnelState::default();
        let hydrated = hydrate_target(&fixture_target(MachineTransport::Local), &tunnel_state)
            .expect("target should hydrate");
        let surface = hydrated.services[0].web.as_ref().expect("web surface");

        assert_eq!(surface.local_url, "http://127.0.0.1:3432/");
        assert!(surface.tunnel_command.is_empty());
    }
}
