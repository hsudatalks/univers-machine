use crate::models::{BrowserSurface, DeveloperService, DeveloperServiceKind, DeveloperTarget};

pub(crate) fn web_service(surface: &BrowserSurface) -> DeveloperService {
    DeveloperService {
        id: surface.id.clone(),
        label: surface.label.clone(),
        kind: DeveloperServiceKind::Web,
        description: String::new(),
        web: Some(surface.clone()),
        endpoint: None,
        command: None,
    }
}

pub(crate) fn tmux_command_service(target: &DeveloperTarget) -> Option<&DeveloperService> {
    let preferred_id = target.workspace.tmux_command_service_id.trim();

    if !preferred_id.is_empty() {
        if let Some(service) = target.services.iter().find(|service| {
            matches!(service.kind, DeveloperServiceKind::Command)
                && service.id == preferred_id
                && service
                    .command
                    .as_ref()
                    .map(|command| !command.restart.trim().is_empty())
                    .unwrap_or(false)
        }) {
            return Some(service);
        }
    }

    target
        .services
        .iter()
        .find(|service| {
            matches!(service.kind, DeveloperServiceKind::Command)
                && service.id == "tmux-developer"
                && service
                    .command
                    .as_ref()
                    .map(|command| !command.restart.trim().is_empty())
                    .unwrap_or(false)
        })
        .or_else(|| {
            target.services.iter().find(|service| {
                matches!(service.kind, DeveloperServiceKind::Command)
                    && service
                        .command
                        .as_ref()
                        .map(|command| !command.restart.trim().is_empty())
                        .unwrap_or(false)
            })
        })
}

pub(crate) fn command_service<'a>(
    target: &'a DeveloperTarget,
    service_id: &str,
) -> Option<&'a DeveloperService> {
    target.services.iter().find(|service| {
        matches!(service.kind, DeveloperServiceKind::Command)
            && service.id == service_id
            && service
                .command
                .as_ref()
                .map(|command| !command.restart.trim().is_empty())
                .unwrap_or(false)
    })
}
